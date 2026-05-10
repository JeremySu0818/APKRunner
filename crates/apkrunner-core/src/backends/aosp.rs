use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{SecondsFormat, Utc};
use uuid::Uuid;

use crate::android_host::{
    ArtifactDownloader, CommandOutput, HostCommandRunner, ManagedChild, NoopArtifactDownloader,
    SystemHostCommandRunner,
};
use crate::apk::LoadedApk;
use crate::error::{ApkRunnerError, ApkRunnerResult};
use crate::event::{LogLevel, RuntimeEvent};
use crate::frame::{parse_png_dimensions, FrameFormat};
use crate::input::{escape_adb_input_text, InputEvent};
use crate::runner::{AppConfiguration, AppInstance};
use crate::runtime::RuntimeBackend;
use crate::runtime_bundle::{
    ensure_success, env_refs, ManagedRuntimeManager, ProvisioningState, RuntimeBundleConfiguration,
};

pub struct AospRuntimeBackend {
    bundle: ManagedRuntimeManager,
    command_runner: Box<dyn HostCommandRunner>,
    downloader: Box<dyn ArtifactDownloader>,
    running_instances: HashSet<Uuid>,
    events: VecDeque<RuntimeEvent>,
    emulator_child: Option<Box<dyn ManagedChild>>,
    owned_emulator: bool,
}

impl AospRuntimeBackend {
    pub fn new(
        sandbox_root: impl Into<PathBuf>,
        config: RuntimeBundleConfiguration,
    ) -> ApkRunnerResult<Self> {
        Ok(Self::with_host(
            ManagedRuntimeManager::new(sandbox_root, config),
            Box::new(SystemHostCommandRunner),
            Box::new(NoopArtifactDownloader),
        ))
    }

    pub fn with_host(
        bundle: ManagedRuntimeManager,
        command_runner: Box<dyn HostCommandRunner>,
        downloader: Box<dyn ArtifactDownloader>,
    ) -> Self {
        Self {
            bundle,
            command_runner,
            downloader,
            running_instances: HashSet::new(),
            events: VecDeque::new(),
            emulator_child: None,
            owned_emulator: false,
        }
    }

    fn ensure_ready(&mut self) -> ApkRunnerResult<()> {
        match self
            .bundle
            .ensure_provisioned(&mut *self.command_runner, &mut *self.downloader)?
        {
            ProvisioningState::Ready => Ok(()),
            ProvisioningState::NeedsCommandLineToolsDownload { sdk_root } => {
                Err(ApkRunnerError::RuntimeBackendError(format!(
                    "managed Android command-line tools are not installed under {}; package them with APKRunner or complete the official download/licensing step before starting the AOSP backend",
                    sdk_root.display()
                )))
            }
            other => Err(ApkRunnerError::RuntimeBackendError(format!(
                "managed Android runtime provisioning did not complete: {other:?}"
            ))),
        }
    }

    fn launch_emulator(&mut self) -> ApkRunnerResult<()> {
        if !self.bundle.config().launch_emulator || self.emulator_child.is_some() {
            return Ok(());
        }
        let resolved = self.bundle.resolved();
        let tools = self.bundle.tool_paths();
        let mut args = vec![
            "-avd".to_string(),
            resolved.avd_name,
            "-no-snapshot-save".to_string(),
        ];
        args.extend(self.bundle.config().emulator_extra_args.clone());
        let env = self.bundle.command_env();
        let env_refs = env_refs(&env);
        let child = self
            .command_runner
            .spawn(&tools.emulator, &args, &env_refs)?;
        self.emulator_child = Some(child);
        self.owned_emulator = true;
        Ok(())
    }

    fn wait_for_boot(&mut self) -> ApkRunnerResult<()> {
        self.run_adb(
            &["wait-for-device"],
            self.bundle.config().boot_timeout(),
            "adb wait-for-device",
        )?;

        let started = Instant::now();
        loop {
            let output = self.run_adb_output(
                &["shell", "getprop", "sys.boot_completed"],
                self.short_poll_timeout(),
            )?;
            if String::from_utf8_lossy(&output.stdout).trim() == "1" {
                return Ok(());
            }
            if started.elapsed() >= self.bundle.config().boot_timeout() {
                return Err(ApkRunnerError::RuntimeBackendError(
                    "timed out waiting for managed Android emulator boot".to_string(),
                ));
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    fn clear_logs(&mut self) -> ApkRunnerResult<()> {
        self.run_adb(&["logcat", "-c"], self.command_timeout(), "adb logcat -c")
    }

    fn install_apk(&mut self, apk_path: &Path) -> ApkRunnerResult<()> {
        self.run_adb(
            &["install", "-r", "-t", &apk_path.to_string_lossy()],
            self.command_timeout(),
            "adb install",
        )
    }

    fn launch_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()> {
        if let Some(activity) = &instance.launcher_activity {
            let component = format!("{}/{}", instance.package_name, activity);
            self.run_adb(
                &["shell", "am", "start", "-n", &component],
                self.command_timeout(),
                "adb shell am start",
            )
        } else {
            self.run_adb(
                &[
                    "shell",
                    "monkey",
                    "-p",
                    &instance.package_name,
                    "-c",
                    "android.intent.category.LAUNCHER",
                    "1",
                ],
                self.command_timeout(),
                "adb shell monkey",
            )
        }
    }

    fn collect_bounded_logs(&mut self) {
        match self.run_adb_output(
            &["logcat", "-d", "-v", "brief", "-t", "200"],
            self.command_timeout(),
        ) {
            Ok(output) => {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    if let Some(event) = parse_logcat_brief_line(line) {
                        self.events.push_back(event);
                    }
                }
            }
            Err(error) => self.events.push_back(RuntimeEvent::log(
                LogLevel::Warn,
                "APKRunner",
                format!("failed to collect logcat: {error}"),
            )),
        }
    }

    fn capture_first_frame(&mut self) -> ApkRunnerResult<()> {
        if !self.bundle.config().capture_frame_on_start {
            return Ok(());
        }
        let output =
            self.run_adb_output(&["exec-out", "screencap", "-p"], self.command_timeout())?;
        if !output.is_success() {
            return ensure_success_for_adb(&output, "adb exec-out screencap -p");
        }
        let surface_size = parse_png_dimensions(&output.stdout).ok_or_else(|| {
            ApkRunnerError::RuntimeBackendError(
                "managed adb screencap did not return a valid PNG".to_string(),
            )
        })?;
        self.events.push_back(RuntimeEvent::FrameReady {
            surface_size,
            frame_format: FrameFormat::Png,
            payload_base64: STANDARD.encode(output.stdout),
            metadata: "Managed Android Emulator adb exec-out screencap -p".to_string(),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        });
        Ok(())
    }

    fn force_stop(&mut self, package_name: &str) -> ApkRunnerResult<()> {
        self.run_adb(
            &["shell", "am", "force-stop", package_name],
            self.command_timeout(),
            "adb shell am force-stop",
        )
    }

    fn run_adb(&mut self, args: &[&str], timeout: Duration, context: &str) -> ApkRunnerResult<()> {
        let output = self.run_adb_output(args, timeout)?;
        if output.is_success() {
            return Ok(());
        }
        ensure_success_for_adb(&output, context)
    }

    fn run_adb_output(
        &mut self,
        args: &[&str],
        timeout: Duration,
    ) -> ApkRunnerResult<CommandOutput> {
        let tools = self.bundle.tool_paths();
        let args = args
            .iter()
            .map(|arg| (*arg).to_string())
            .collect::<Vec<_>>();
        let env = self.bundle.command_env();
        let env_refs = env_refs(&env);
        self.command_runner
            .run(&tools.adb, &args, &env_refs, timeout)
    }

    fn short_poll_timeout(&self) -> Duration {
        self.command_timeout().min(Duration::from_secs(5))
    }

    fn command_timeout(&self) -> Duration {
        self.bundle.config().command_timeout()
    }

    fn push_app_started(&mut self, instance: &AppInstance) {
        self.events.push_back(RuntimeEvent::AppStarted {
            package_name: instance.package_name.clone(),
            instance_id: instance.id.to_string(),
        });
    }

    fn push_app_stopped(&mut self, instance: &AppInstance) {
        self.events.push_back(RuntimeEvent::AppStopped {
            package_name: instance.package_name.clone(),
            instance_id: instance.id.to_string(),
        });
    }
}

impl RuntimeBackend for AospRuntimeBackend {
    fn name(&self) -> &'static str {
        "AospRuntimeBackend"
    }

    fn create_app_instance(
        &mut self,
        loaded_apk: &LoadedApk,
        config: &AppConfiguration,
    ) -> ApkRunnerResult<AppInstance> {
        let package_name = config
            .package_name
            .clone()
            .unwrap_or_else(|| loaded_apk.summary.package_name.clone());
        Ok(AppInstance {
            id: Uuid::new_v4(),
            loaded_apk_id: loaded_apk.id,
            package_name,
            launcher_activity: loaded_apk.summary.launcher_activity.clone(),
            apk_path: loaded_apk.host_path.clone(),
        })
    }

    fn start_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()> {
        if self.running_instances.contains(&instance.id) {
            return Err(ApkRunnerError::AppAlreadyRunning(instance.id));
        }
        self.ensure_ready()?;
        self.launch_emulator()?;
        self.wait_for_boot()?;
        self.clear_logs()?;
        self.install_apk(&instance.apk_path)?;
        self.launch_app(instance)?;
        self.running_instances.insert(instance.id);
        self.push_app_started(instance);
        self.collect_bounded_logs();
        self.capture_first_frame()?;
        Ok(())
    }

    fn stop_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()> {
        if !self.running_instances.contains(&instance.id) {
            return Err(ApkRunnerError::AppNotRunning(instance.id));
        }
        self.force_stop(&instance.package_name)?;
        self.running_instances.remove(&instance.id);
        self.collect_bounded_logs();
        self.push_app_stopped(instance);
        if self.bundle.config().terminate_emulator_on_stop && self.owned_emulator {
            if let Some(child) = &mut self.emulator_child {
                child.kill()?;
            }
            self.emulator_child = None;
            self.owned_emulator = false;
        }
        Ok(())
    }

    fn dispatch_input(&mut self, instance: &AppInstance, input: InputEvent) -> ApkRunnerResult<()> {
        if !self.running_instances.contains(&instance.id) {
            return Err(ApkRunnerError::AppNotRunning(instance.id));
        }
        match input {
            InputEvent::Tap { x, y } => self.run_adb(
                &["shell", "input", "tap", &x.to_string(), &y.to_string()],
                self.command_timeout(),
                "adb shell input tap",
            ),
            InputEvent::Key { key_code } => self.run_adb(
                &["shell", "input", "keyevent", &key_code.to_string()],
                self.command_timeout(),
                "adb shell input keyevent",
            ),
            InputEvent::Text { text } => {
                let text = escape_adb_input_text(&text)?;
                self.run_adb(
                    &["shell", "input", "text", &text],
                    self.command_timeout(),
                    "adb shell input text",
                )
            }
        }
    }

    fn poll_runtime_events(&mut self) -> Vec<RuntimeEvent> {
        self.events.drain(..).collect()
    }
}

pub fn parse_logcat_brief_line(line: &str) -> Option<RuntimeEvent> {
    let mut chars = line.chars();
    let priority = chars.next()?;
    if chars.next()? != '/' {
        return None;
    }
    let rest = chars.as_str();
    let (tag, message) = if let Some((tag_with_pid, message)) = rest.split_once("):") {
        let tag = tag_with_pid
            .split_once('(')
            .map(|(tag, _)| tag)
            .unwrap_or(tag_with_pid)
            .trim();
        (tag, message.trim())
    } else {
        let (tag, message) = rest.split_once(':')?;
        (tag.trim(), message.trim())
    };
    Some(RuntimeEvent::log(
        match priority {
            'V' | 'D' => LogLevel::Debug,
            'I' => LogLevel::Info,
            'W' => LogLevel::Warn,
            'E' | 'F' => LogLevel::Error,
            _ => return None,
        },
        tag,
        message,
    ))
}

fn ensure_success_for_adb(output: &CommandOutput, context: &str) -> ApkRunnerResult<()> {
    ensure_success(Path::new("managed-adb"), &[], output, context)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;

    use tempfile::tempdir;

    use super::*;
    use crate::android_host::ArtifactDownloader;
    use crate::runtime_bundle::{ManagedToolPaths, RuntimeBundleConfiguration};

    #[derive(Debug, Clone)]
    struct RecordedCommand {
        program: PathBuf,
        args: Vec<String>,
        env: Vec<(String, String)>,
    }

    #[derive(Default)]
    struct SharedState {
        commands: Vec<RecordedCommand>,
        spawns: Vec<RecordedCommand>,
    }

    #[derive(Clone)]
    struct MockRunner {
        state: Rc<RefCell<SharedState>>,
    }

    impl MockRunner {
        fn new() -> (Self, Rc<RefCell<SharedState>>) {
            let state = Rc::new(RefCell::new(SharedState::default()));
            (
                Self {
                    state: Rc::clone(&state),
                },
                state,
            )
        }
    }

    impl HostCommandRunner for MockRunner {
        fn run(
            &mut self,
            program: &Path,
            args: &[String],
            env: &[(&str, &str)],
            _timeout: Duration,
        ) -> ApkRunnerResult<CommandOutput> {
            self.state.borrow_mut().commands.push(RecordedCommand {
                program: program.to_path_buf(),
                args: args.to_vec(),
                env: env
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            });
            if args == ["shell", "getprop", "sys.boot_completed"] {
                return Ok(CommandOutput::success(b"1\n".to_vec()));
            }
            if args == ["logcat", "-d", "-v", "brief", "-t", "200"] {
                return Ok(CommandOutput::success(
                    b"I/ActivityManager( 123): Started proc\nW/APKRunner: note\n".to_vec(),
                ));
            }
            if args == ["exec-out", "screencap", "-p"] {
                return Ok(CommandOutput::success(test_png(320, 180)));
            }
            Ok(CommandOutput::success(Vec::new()))
        }

        fn spawn(
            &mut self,
            program: &Path,
            args: &[String],
            env: &[(&str, &str)],
        ) -> ApkRunnerResult<Box<dyn ManagedChild>> {
            self.state.borrow_mut().spawns.push(RecordedCommand {
                program: program.to_path_buf(),
                args: args.to_vec(),
                env: env
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            });
            Ok(Box::new(MockChild))
        }
    }

    unsafe impl Send for MockRunner {}

    struct MockChild;

    impl ManagedChild for MockChild {
        fn kill(&mut self) -> ApkRunnerResult<()> {
            Ok(())
        }

        fn try_wait(&mut self) -> ApkRunnerResult<Option<i32>> {
            Ok(None)
        }
    }

    struct MockDownloader;

    impl ArtifactDownloader for MockDownloader {
        fn download(&mut self, _url: &str, _destination: &Path) -> ApkRunnerResult<()> {
            Ok(())
        }
    }

    fn test_png(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&width.to_be_bytes());
        png.extend_from_slice(&height.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        png.extend_from_slice(&[0, 0, 0, 0]);
        png
    }

    fn ready_backend(
        launcher_activity: Option<String>,
    ) -> (
        AospRuntimeBackend,
        AppInstance,
        ManagedToolPaths,
        Rc<RefCell<SharedState>>,
    ) {
        let temp = tempdir().expect("tempdir");
        let sandbox = temp.path().to_path_buf();
        let manager = ManagedRuntimeManager::new(
            &sandbox,
            RuntimeBundleConfiguration {
                boot_timeout_ms: 1000,
                command_timeout_ms: 1000,
                ..RuntimeBundleConfiguration::default()
            },
        );
        let tools = manager.tool_paths();
        for path in [
            &tools.sdkmanager,
            &tools.avdmanager,
            &tools.adb,
            &tools.emulator,
        ] {
            fs::create_dir_all(path.parent().expect("tool parent")).expect("tool dir");
            fs::write(path, "").expect("tool file");
        }
        fs::create_dir_all(manager.resolved().avd_path()).expect("avd");
        manager.write_manifest().expect("manifest");
        let apk_path = sandbox.join("app.apk");
        fs::write(&apk_path, "fake").expect("apk");
        let (runner, state) = MockRunner::new();
        let backend =
            AospRuntimeBackend::with_host(manager, Box::new(runner), Box::new(MockDownloader));
        let instance = AppInstance {
            id: Uuid::new_v4(),
            loaded_apk_id: Uuid::new_v4(),
            package_name: "com.example".to_string(),
            launcher_activity,
            apk_path,
        };
        std::mem::forget(temp);
        (backend, instance, tools, state)
    }

    #[test]
    fn logcat_brief_parser_maps_priority_tag_and_message() {
        let event = parse_logcat_brief_line("W/MyTag(  42): hello").expect("log line");
        match event {
            RuntimeEvent::Log {
                level,
                tag,
                message,
                ..
            } => {
                assert_eq!(level, LogLevel::Warn);
                assert_eq!(tag, "MyTag");
                assert_eq!(message, "hello");
            }
            _ => panic!("expected log event"),
        }
    }

    #[test]
    fn start_app_uses_managed_emulator_and_adb_paths() {
        let (mut backend, instance, tools, state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        let state = state.borrow();
        assert_eq!(state.spawns[0].program, tools.emulator);
        assert!(state.spawns[0].args.starts_with(&[
            "-avd".to_string(),
            "APKRunner_Default_API_35_google_atd_x86_64".to_string(),
            "-no-snapshot-save".to_string(),
        ]));
        assert!(state.spawns[0].args.contains(&"-no-window".to_string()));
        assert!(state.spawns[0].args.contains(&"-no-audio".to_string()));
        assert!(state.spawns[0]
            .env
            .iter()
            .any(|(key, value)| key == "ANDROID_SDK_ROOT" && !value.is_empty()));
        assert!(state
            .commands
            .iter()
            .all(|command| command.program == tools.adb));
        assert!(state
            .commands
            .iter()
            .all(|command| command.program.file_name().unwrap() != "adb-from-path"));
    }

    #[test]
    fn start_app_installs_apk_and_launches_activity() {
        let (mut backend, instance, _tools, state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        let state = state.borrow();
        assert!(state.commands.iter().any(|command| {
            command.args
                == vec![
                    "install".to_string(),
                    "-r".to_string(),
                    "-t".to_string(),
                    instance.apk_path.to_string_lossy().into_owned(),
                ]
        }));
        assert!(state.commands.iter().any(|command| {
            command.args
                == vec![
                    "shell".to_string(),
                    "am".to_string(),
                    "start".to_string(),
                    "-n".to_string(),
                    "com.example/com.example.MainActivity".to_string(),
                ]
        }));
    }

    #[test]
    fn start_app_falls_back_to_monkey_without_launcher_activity() {
        let (mut backend, instance, _tools, state) = ready_backend(None);
        backend.start_app(&instance).expect("start app");
        let state = state.borrow();
        assert!(state.commands.iter().any(|command| {
            command.args
                == vec![
                    "shell".to_string(),
                    "monkey".to_string(),
                    "-p".to_string(),
                    "com.example".to_string(),
                    "-c".to_string(),
                    "android.intent.category.LAUNCHER".to_string(),
                    "1".to_string(),
                ]
        }));
    }

    #[test]
    fn start_app_emits_frame_ready_png() {
        let (mut backend, instance, _tools, _state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        assert!(backend.poll_runtime_events().iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::FrameReady {
                    frame_format: FrameFormat::Png,
                    surface_size,
                    ..
                } if surface_size.width == 320 && surface_size.height == 180
            )
        }));
    }

    #[test]
    fn dispatch_input_builds_tap_key_and_text_commands() {
        let (mut backend, instance, _tools, state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        backend
            .dispatch_input(&instance, InputEvent::Tap { x: 7, y: 9 })
            .expect("tap");
        backend
            .dispatch_input(&instance, InputEvent::Key { key_code: 66 })
            .expect("key");
        backend
            .dispatch_input(
                &instance,
                InputEvent::Text {
                    text: "hello world".to_string(),
                },
            )
            .expect("text");
        let state = state.borrow();
        assert!(state
            .commands
            .iter()
            .any(|command| { command.args == ["shell", "input", "tap", "7", "9"] }));
        assert!(state
            .commands
            .iter()
            .any(|command| { command.args == ["shell", "input", "keyevent", "66"] }));
        assert!(state
            .commands
            .iter()
            .any(|command| { command.args == ["shell", "input", "text", "hello%sworld"] }));
    }

    #[test]
    fn dispatch_input_rejects_unsafe_text() {
        let (mut backend, instance, _tools, _state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        assert!(backend
            .dispatch_input(
                &instance,
                InputEvent::Text {
                    text: "hello;world".to_string(),
                },
            )
            .is_err());
    }

    #[test]
    fn stop_app_uses_managed_adb_force_stop() {
        let (mut backend, instance, _tools, state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        backend.stop_app(&instance).expect("stop app");
        let state = state.borrow();
        assert!(state
            .commands
            .iter()
            .any(|command| { command.args == ["shell", "am", "force-stop", "com.example"] }));
    }

    #[test]
    fn lifecycle_rejects_duplicate_start() {
        let (mut backend, instance, _tools, _state) =
            ready_backend(Some("com.example.MainActivity".to_string()));
        backend.start_app(&instance).expect("start app");
        let error = backend.start_app(&instance).expect_err("duplicate start");
        assert!(matches!(error, ApkRunnerError::AppAlreadyRunning(_)));
    }
}

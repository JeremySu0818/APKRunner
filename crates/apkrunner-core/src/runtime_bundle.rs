use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::android_host::{ArtifactDownloader, HostCommandRunner};
use crate::error::{ApkRunnerError, ApkRunnerResult};

pub const DEFAULT_ANDROID_API_LEVEL: u32 = 35;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidSystemImageChannel {
    Aosp,
    AospAtd,
    GoogleApis,
    GoogleAtd,
    GoogleApisPlaystore,
}

impl AndroidSystemImageChannel {
    pub fn package_segment(self) -> &'static str {
        match self {
            Self::Aosp => "default",
            Self::AospAtd => "aosp_atd",
            Self::GoogleApis => "google_apis",
            Self::GoogleAtd => "google_atd",
            Self::GoogleApisPlaystore => "google_apis_playstore",
        }
    }

    pub fn manifest_value(self) -> &'static str {
        match self {
            Self::Aosp => "aosp",
            Self::AospAtd => "aospAtd",
            Self::GoogleApis => "googleApis",
            Self::GoogleAtd => "googleAtd",
            Self::GoogleApisPlaystore => "googleApisPlaystore",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidSystemImageAbi {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "arm64-v8a")]
    Arm64V8a,
}

impl AndroidSystemImageAbi {
    pub fn package_segment(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Arm64V8a => "arm64-v8a",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeBundleConfiguration {
    pub bundle_root: Option<PathBuf>,
    pub sdk_root: Option<PathBuf>,
    pub avd_home: Option<PathBuf>,
    pub avd_name: Option<String>,
    pub api_level: u32,
    pub system_image_channel: AndroidSystemImageChannel,
    pub abi: AndroidSystemImageAbi,
    pub launch_emulator: bool,
    pub boot_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub capture_frame_on_start: bool,
    pub terminate_emulator_on_stop: bool,
    pub allow_system_tool_overrides: bool,
    pub debug_adb_path: Option<PathBuf>,
    pub debug_emulator_path: Option<PathBuf>,
    pub debug_sdkmanager_path: Option<PathBuf>,
    pub debug_avdmanager_path: Option<PathBuf>,
    pub prepackaged_cmdline_tools_root: Option<PathBuf>,
    pub emulator_extra_args: Vec<String>,
}

impl Default for RuntimeBundleConfiguration {
    fn default() -> Self {
        Self {
            bundle_root: None,
            sdk_root: None,
            avd_home: None,
            avd_name: None,
            api_level: DEFAULT_ANDROID_API_LEVEL,
            system_image_channel: AndroidSystemImageChannel::GoogleAtd,
            abi: default_host_abi(),
            launch_emulator: true,
            boot_timeout_ms: 120_000,
            command_timeout_ms: 30_000,
            capture_frame_on_start: true,
            terminate_emulator_on_stop: false,
            allow_system_tool_overrides: false,
            debug_adb_path: None,
            debug_emulator_path: None,
            debug_sdkmanager_path: None,
            debug_avdmanager_path: None,
            prepackaged_cmdline_tools_root: None,
            emulator_extra_args: vec![
                "-no-window".to_string(),
                "-no-audio".to_string(),
                "-no-boot-anim".to_string(),
                "-gpu".to_string(),
                "swiftshader_indirect".to_string(),
            ],
        }
    }
}

impl RuntimeBundleConfiguration {
    pub fn with_environment_overrides(mut self) -> Self {
        if let Some(path) = env_path("APKRUNNER_RUNTIME_BUNDLE_ROOT") {
            self.bundle_root = Some(path);
        }
        if let Some(path) = env_path("APKRUNNER_ANDROID_SDK_ROOT") {
            self.sdk_root = Some(path);
        }
        if let Some(path) = env_path("APKRUNNER_ANDROID_AVD_HOME") {
            self.avd_home = Some(path);
        }
        if let Ok(name) = env::var("APKRUNNER_ANDROID_AVD_NAME") {
            if !name.trim().is_empty() {
                self.avd_name = Some(name);
            }
        }
        if let Ok(api_level) = env::var("APKRUNNER_ANDROID_API_LEVEL") {
            if let Ok(api_level) = api_level.parse() {
                self.api_level = api_level;
            }
        }
        if let Ok(abi) = env::var("APKRUNNER_ANDROID_ABI") {
            if let Some(abi) = parse_abi(&abi) {
                self.abi = abi;
            }
        }
        if let Ok(channel) = env::var("APKRUNNER_ANDROID_SYSTEM_IMAGE_CHANNEL") {
            if let Some(channel) = parse_channel(&channel) {
                self.system_image_channel = channel;
            }
        }
        if env_bool("APKRUNNER_ALLOW_SYSTEM_ANDROID_TOOLS") {
            self.allow_system_tool_overrides = true;
        }
        if let Some(path) = env_path("APKRUNNER_ADB_PATH") {
            self.debug_adb_path = Some(path);
        }
        if let Some(path) = env_path("APKRUNNER_EMULATOR_PATH") {
            self.debug_emulator_path = Some(path);
        }
        if let Some(path) = env_path("APKRUNNER_SDKMANAGER_PATH") {
            self.debug_sdkmanager_path = Some(path);
        }
        if let Some(path) = env_path("APKRUNNER_AVDMANAGER_PATH") {
            self.debug_avdmanager_path = Some(path);
        }
        if let Some(path) = env_path("APKRUNNER_PREPACKAGED_CMDLINE_TOOLS_ROOT") {
            self.prepackaged_cmdline_tools_root = Some(path);
        }
        self
    }

    pub fn command_timeout(&self) -> Duration {
        Duration::from_millis(self.command_timeout_ms)
    }

    pub fn boot_timeout(&self) -> Duration {
        Duration::from_millis(self.boot_timeout_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRuntimeBundle {
    pub bundle_root: PathBuf,
    pub sdk_root: PathBuf,
    pub avd_home: PathBuf,
    pub avd_name: String,
    pub emulator_home: PathBuf,
}

impl ResolvedRuntimeBundle {
    pub fn avd_path(&self) -> PathBuf {
        self.avd_home.join(format!("{}.avd", self.avd_name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Unix,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedToolPaths {
    pub sdkmanager: PathBuf,
    pub avdmanager: PathBuf,
    pub adb: PathBuf,
    pub emulator: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReadiness {
    pub sdk_root_exists: bool,
    pub sdkmanager_path: PathBuf,
    pub sdkmanager_exists: bool,
    pub avdmanager_path: PathBuf,
    pub avdmanager_exists: bool,
    pub adb_path: PathBuf,
    pub adb_exists: bool,
    pub emulator_path: PathBuf,
    pub emulator_exists: bool,
    pub avd_path: PathBuf,
    pub avd_exists: bool,
    pub manifest_path: PathBuf,
    pub manifest_exists: bool,
}

impl RuntimeReadiness {
    pub fn ready(&self) -> bool {
        self.sdk_root_exists
            && self.sdkmanager_exists
            && self.avdmanager_exists
            && self.adb_exists
            && self.emulator_exists
            && self.avd_exists
            && self.manifest_exists
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningPlan {
    pub state: ProvisioningState,
    pub required_packages: Vec<String>,
    pub resolved: ResolvedRuntimeBundle,
    pub tools: ManagedToolPaths,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisioningState {
    Ready,
    NeedsCommandLineToolsDownload { sdk_root: PathBuf },
    NeedsSdkPackagesInstall { missing: Vec<String> },
    NeedsAvdCreate { avd_name: String, avd_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBundleManifest {
    pub schema_version: u32,
    pub api_level: u32,
    pub abi: String,
    pub system_image_channel: String,
    pub sdk_root: PathBuf,
    pub avd_home: PathBuf,
    pub avd_name: String,
    pub packages: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ManagedRuntimeManager {
    sandbox_root: PathBuf,
    config: RuntimeBundleConfiguration,
}

impl ManagedRuntimeManager {
    pub fn new(sandbox_root: impl Into<PathBuf>, config: RuntimeBundleConfiguration) -> Self {
        Self {
            sandbox_root: sandbox_root.into(),
            config,
        }
    }

    pub fn config(&self) -> &RuntimeBundleConfiguration {
        &self.config
    }

    pub fn resolved(&self) -> ResolvedRuntimeBundle {
        let bundle_root = self
            .config
            .bundle_root
            .clone()
            .unwrap_or_else(|| self.sandbox_root.join("android-runtime"));
        let sdk_root = self
            .config
            .sdk_root
            .clone()
            .unwrap_or_else(|| bundle_root.join("sdk"));
        let avd_home = self
            .config
            .avd_home
            .clone()
            .unwrap_or_else(|| bundle_root.join("avd"));
        let avd_name = self.config.avd_name.clone().unwrap_or_else(|| {
            format!(
                "APKRunner_Default_API_{}_{}_{}",
                self.config.api_level,
                self.config.system_image_channel.package_segment(),
                self.config.abi.package_segment()
            )
        });
        ResolvedRuntimeBundle {
            emulator_home: bundle_root.join("emulator-home"),
            bundle_root,
            sdk_root,
            avd_home,
            avd_name,
        }
    }

    pub fn required_packages(&self) -> Vec<String> {
        vec![
            "cmdline-tools;latest".to_string(),
            "platform-tools".to_string(),
            "emulator".to_string(),
            format!("platforms;android-{}", self.config.api_level),
            self.system_image_package(),
        ]
    }

    pub fn system_image_package(&self) -> String {
        format!(
            "system-images;android-{};{};{}",
            self.config.api_level,
            self.config.system_image_channel.package_segment(),
            self.config.abi.package_segment()
        )
    }

    pub fn managed_tool_paths(&self) -> ManagedToolPaths {
        self.managed_tool_paths_for_os(current_host_os())
    }

    pub fn managed_tool_paths_for_os(&self, os: HostOs) -> ManagedToolPaths {
        let resolved = self.resolved();
        ManagedToolPaths {
            sdkmanager: resolved
                .sdk_root
                .join("cmdline-tools")
                .join("latest")
                .join("bin")
                .join(sdk_tool_name("sdkmanager", os)),
            avdmanager: resolved
                .sdk_root
                .join("cmdline-tools")
                .join("latest")
                .join("bin")
                .join(sdk_tool_name("avdmanager", os)),
            adb: resolved
                .sdk_root
                .join("platform-tools")
                .join(executable_name("adb", os)),
            emulator: resolved
                .sdk_root
                .join("emulator")
                .join(executable_name("emulator", os)),
        }
    }

    pub fn tool_paths(&self) -> ManagedToolPaths {
        let mut tools = self.managed_tool_paths();
        if self.config.allow_system_tool_overrides {
            if let Some(path) = &self.config.debug_sdkmanager_path {
                tools.sdkmanager = path.clone();
            }
            if let Some(path) = &self.config.debug_avdmanager_path {
                tools.avdmanager = path.clone();
            }
            if let Some(path) = &self.config.debug_adb_path {
                tools.adb = path.clone();
            }
            if let Some(path) = &self.config.debug_emulator_path {
                tools.emulator = path.clone();
            }
        }
        tools
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.resolved().bundle_root.join("bundle-manifest.json")
    }

    pub fn readiness(&self) -> RuntimeReadiness {
        let resolved = self.resolved();
        let tools = self.tool_paths();
        let manifest_path = self.manifest_path();
        let avd_path = resolved.avd_path();
        RuntimeReadiness {
            sdk_root_exists: resolved.sdk_root.is_dir(),
            sdkmanager_exists: tools.sdkmanager.is_file(),
            sdkmanager_path: tools.sdkmanager,
            avdmanager_exists: tools.avdmanager.is_file(),
            avdmanager_path: tools.avdmanager,
            adb_exists: tools.adb.is_file(),
            adb_path: tools.adb,
            emulator_exists: tools.emulator.is_file(),
            emulator_path: tools.emulator,
            avd_exists: avd_path.is_dir(),
            avd_path,
            manifest_exists: manifest_path.is_file(),
            manifest_path,
        }
    }

    pub fn provisioning_plan(&self) -> ProvisioningPlan {
        let resolved = self.resolved();
        let tools = self.tool_paths();
        let readiness = self.readiness();
        let required_packages = self.required_packages();
        let state = if !readiness.sdkmanager_exists || !readiness.avdmanager_exists {
            ProvisioningState::NeedsCommandLineToolsDownload {
                sdk_root: resolved.sdk_root.clone(),
            }
        } else if !readiness.adb_exists || !readiness.emulator_exists || !readiness.manifest_exists
        {
            ProvisioningState::NeedsSdkPackagesInstall {
                missing: self.missing_runtime_components(&readiness),
            }
        } else if !readiness.avd_exists {
            ProvisioningState::NeedsAvdCreate {
                avd_name: resolved.avd_name.clone(),
                avd_path: resolved.avd_path(),
            }
        } else {
            ProvisioningState::Ready
        };
        ProvisioningPlan {
            state,
            required_packages,
            resolved,
            tools,
        }
    }

    pub fn ensure_provisioned(
        &self,
        runner: &mut dyn HostCommandRunner,
        downloader: &mut dyn ArtifactDownloader,
    ) -> ApkRunnerResult<ProvisioningState> {
        let resolved = self.resolved();
        fs::create_dir_all(&resolved.bundle_root).map_err(|source| {
            ApkRunnerError::HostIoFailure {
                path: resolved.bundle_root.clone(),
                reason: source.to_string(),
            }
        })?;
        fs::create_dir_all(&resolved.sdk_root).map_err(|source| ApkRunnerError::HostIoFailure {
            path: resolved.sdk_root.clone(),
            reason: source.to_string(),
        })?;
        fs::create_dir_all(&resolved.avd_home).map_err(|source| ApkRunnerError::HostIoFailure {
            path: resolved.avd_home.clone(),
            reason: source.to_string(),
        })?;
        fs::create_dir_all(&resolved.emulator_home).map_err(|source| {
            ApkRunnerError::HostIoFailure {
                path: resolved.emulator_home.clone(),
                reason: source.to_string(),
            }
        })?;

        let mut plan = self.provisioning_plan();
        if matches!(
            plan.state,
            ProvisioningState::NeedsCommandLineToolsDownload { .. }
        ) {
            if let Some(source) = &self.config.prepackaged_cmdline_tools_root {
                self.install_prepackaged_cmdline_tools(source)?;
                plan = self.provisioning_plan();
            } else {
                let _ = downloader;
                return Ok(plan.state);
            }
        }

        if matches!(
            plan.state,
            ProvisioningState::NeedsSdkPackagesInstall { .. }
        ) {
            self.run_sdkmanager(runner)?;
            self.write_manifest()?;
            plan = self.provisioning_plan();
        }

        if matches!(plan.state, ProvisioningState::NeedsAvdCreate { .. }) {
            self.run_avdmanager(runner)?;
            self.write_manifest()?;
            plan = self.provisioning_plan();
        }

        Ok(plan.state)
    }

    pub fn run_sdkmanager(&self, runner: &mut dyn HostCommandRunner) -> ApkRunnerResult<()> {
        let resolved = self.resolved();
        let tools = self.tool_paths();
        let mut args = vec![format!(
            "--sdk_root={}",
            resolved.sdk_root.to_string_lossy()
        )];
        args.extend(self.sdkmanager_install_packages());
        let env = self.command_env();
        let env_refs = env_refs(&env);
        let output = runner.run(
            &tools.sdkmanager,
            &args,
            &env_refs,
            self.config.command_timeout(),
        )?;
        ensure_success(&tools.sdkmanager, &args, &output, "sdkmanager install")
    }

    pub fn run_avdmanager(&self, runner: &mut dyn HostCommandRunner) -> ApkRunnerResult<()> {
        let resolved = self.resolved();
        let tools = self.tool_paths();
        let args = vec![
            "create".to_string(),
            "avd".to_string(),
            "--force".to_string(),
            "--name".to_string(),
            resolved.avd_name.clone(),
            "--package".to_string(),
            self.system_image_package(),
            "--device".to_string(),
            "pixel".to_string(),
            "--path".to_string(),
            resolved.avd_path().to_string_lossy().into_owned(),
        ];
        let env = self.command_env();
        let env_refs = env_refs(&env);
        let output = runner.run(
            &tools.avdmanager,
            &args,
            &env_refs,
            self.config.command_timeout(),
        )?;
        ensure_success(&tools.avdmanager, &args, &output, "avdmanager create avd")
    }

    fn sdkmanager_install_packages(&self) -> Vec<String> {
        self.required_packages()
            .into_iter()
            .filter(|package| package != "cmdline-tools;latest")
            .collect()
    }

    pub fn command_env(&self) -> Vec<(String, String)> {
        let resolved = self.resolved();
        vec![
            (
                "ANDROID_SDK_ROOT".to_string(),
                resolved.sdk_root.to_string_lossy().into_owned(),
            ),
            (
                "ANDROID_AVD_HOME".to_string(),
                resolved.avd_home.to_string_lossy().into_owned(),
            ),
            (
                "ANDROID_EMULATOR_HOME".to_string(),
                resolved.emulator_home.to_string_lossy().into_owned(),
            ),
        ]
    }

    pub fn write_manifest(&self) -> ApkRunnerResult<()> {
        let resolved = self.resolved();
        let manifest = RuntimeBundleManifest {
            schema_version: 1,
            api_level: self.config.api_level,
            abi: self.config.abi.package_segment().to_string(),
            system_image_channel: self
                .config
                .system_image_channel
                .manifest_value()
                .to_string(),
            sdk_root: resolved.sdk_root.clone(),
            avd_home: resolved.avd_home.clone(),
            avd_name: resolved.avd_name.clone(),
            packages: self.required_packages(),
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        };
        fs::create_dir_all(&resolved.bundle_root).map_err(|source| {
            ApkRunnerError::HostIoFailure {
                path: resolved.bundle_root.clone(),
                reason: source.to_string(),
            }
        })?;
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
        fs::write(self.manifest_path(), bytes).map_err(|source| ApkRunnerError::HostIoFailure {
            path: self.manifest_path(),
            reason: source.to_string(),
        })
    }

    fn install_prepackaged_cmdline_tools(&self, source: &Path) -> ApkRunnerResult<()> {
        let destination = self
            .resolved()
            .sdk_root
            .join("cmdline-tools")
            .join("latest");
        if destination.exists() {
            return Ok(());
        }
        copy_dir_recursive(source, &destination)
    }

    fn missing_runtime_components(&self, readiness: &RuntimeReadiness) -> Vec<String> {
        let mut missing = Vec::new();
        if !readiness.adb_exists {
            missing.push("platform-tools".to_string());
        }
        if !readiness.emulator_exists {
            missing.push("emulator".to_string());
        }
        if !readiness.manifest_exists {
            missing.push("bundle-manifest.json".to_string());
        }
        missing
    }
}

pub fn ensure_success(
    program: &Path,
    args: &[String],
    output: &crate::android_host::CommandOutput,
    context: &str,
) -> ApkRunnerResult<()> {
    if output.is_success() {
        return Ok(());
    }
    Err(ApkRunnerError::RuntimeBackendError(format!(
        "{context} failed via {} {} with status {:?}: {}",
        program.display(),
        args.join(" "),
        output.status_code,
        String::from_utf8_lossy(&output.stderr)
    )))
}

pub fn env_refs(env: &[(String, String)]) -> Vec<(&str, &str)> {
    env.iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

pub fn default_host_abi() -> AndroidSystemImageAbi {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        AndroidSystemImageAbi::Arm64V8a
    } else {
        AndroidSystemImageAbi::X86_64
    }
}

fn current_host_os() -> HostOs {
    if cfg!(target_os = "windows") {
        HostOs::Windows
    } else {
        HostOs::Unix
    }
}

fn sdk_tool_name(base: &str, os: HostOs) -> String {
    match os {
        HostOs::Unix => base.to_string(),
        HostOs::Windows => format!("{base}.bat"),
    }
}

fn executable_name(base: &str, os: HostOs) -> String {
    match os {
        HostOs::Unix => base.to_string(),
        HostOs::Windows => format!("{base}.exe"),
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn env_bool(name: &str) -> bool {
    env::var(name).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn parse_abi(value: &str) -> Option<AndroidSystemImageAbi> {
    match value {
        "x86_64" | "X86_64" => Some(AndroidSystemImageAbi::X86_64),
        "arm64-v8a" | "Arm64V8a" | "arm64" => Some(AndroidSystemImageAbi::Arm64V8a),
        _ => None,
    }
}

fn parse_channel(value: &str) -> Option<AndroidSystemImageChannel> {
    match value {
        "aosp" | "Aosp" | "default" => Some(AndroidSystemImageChannel::Aosp),
        "aospAtd" | "aosp_atd" | "AospAtd" => Some(AndroidSystemImageChannel::AospAtd),
        "googleApis" | "google_apis" | "GoogleApis" => Some(AndroidSystemImageChannel::GoogleApis),
        "googleAtd" | "google_atd" | "GoogleAtd" => Some(AndroidSystemImageChannel::GoogleAtd),
        "googleApisPlaystore" | "google_apis_playstore" | "GoogleApisPlaystore" => {
            Some(AndroidSystemImageChannel::GoogleApisPlaystore)
        }
        _ => None,
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> ApkRunnerResult<()> {
    if !source.is_dir() {
        return Err(ApkRunnerError::HostIoFailure {
            path: source.to_path_buf(),
            reason: "prepackaged cmdline-tools source is not a directory".to_string(),
        });
    }
    fs::create_dir_all(destination).map_err(|error| ApkRunnerError::HostIoFailure {
        path: destination.to_path_buf(),
        reason: error.to_string(),
    })?;
    for entry in fs::read_dir(source).map_err(|error| ApkRunnerError::HostIoFailure {
        path: source.to_path_buf(),
        reason: error.to_string(),
    })? {
        let entry = entry.map_err(|error| ApkRunnerError::HostIoFailure {
            path: source.to_path_buf(),
            reason: error.to_string(),
        })?;
        let file_type = entry
            .file_type()
            .map_err(|error| ApkRunnerError::HostIoFailure {
                path: entry.path(),
                reason: error.to_string(),
            })?;
        let child_destination = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &child_destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &child_destination).map_err(|error| {
                ApkRunnerError::HostIoFailure {
                    path: child_destination,
                    reason: error.to_string(),
                }
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::*;
    use crate::android_host::{CommandOutput, ManagedChild};

    #[derive(Default)]
    struct RecordingRunner {
        calls: Vec<RecordedCall>,
    }

    #[derive(Debug, Clone)]
    struct RecordedCall {
        program: PathBuf,
        args: Vec<String>,
        env: Vec<(String, String)>,
    }

    impl HostCommandRunner for RecordingRunner {
        fn run(
            &mut self,
            program: &Path,
            args: &[String],
            env: &[(&str, &str)],
            _timeout: Duration,
        ) -> ApkRunnerResult<CommandOutput> {
            self.calls.push(RecordedCall {
                program: program.to_path_buf(),
                args: args.to_vec(),
                env: env
                    .iter()
                    .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                    .collect(),
            });
            Ok(CommandOutput::success(Vec::new()))
        }

        fn spawn(
            &mut self,
            _program: &Path,
            _args: &[String],
            _env: &[(&str, &str)],
        ) -> ApkRunnerResult<Box<dyn ManagedChild>> {
            unreachable!("provisioning tests do not spawn children")
        }
    }

    #[test]
    fn defaults_resolve_to_apkrunner_owned_paths() {
        let temp = tempdir().expect("tempdir");
        let manager =
            ManagedRuntimeManager::new(temp.path(), RuntimeBundleConfiguration::default());
        let resolved = manager.resolved();
        assert_eq!(resolved.bundle_root, temp.path().join("android-runtime"));
        assert_eq!(resolved.sdk_root, resolved.bundle_root.join("sdk"));
        assert_eq!(resolved.avd_home, resolved.bundle_root.join("avd"));
        assert_eq!(
            resolved.avd_name,
            format!(
                "APKRunner_Default_API_{}_{}_{}",
                DEFAULT_ANDROID_API_LEVEL,
                RuntimeBundleConfiguration::default()
                    .system_image_channel
                    .package_segment(),
                default_host_abi().package_segment()
            )
        );
    }

    #[test]
    fn system_tool_overrides_are_ignored_unless_enabled() {
        let temp = tempdir().expect("tempdir");
        let config = RuntimeBundleConfiguration {
            debug_adb_path: Some(PathBuf::from("/usr/bin/adb")),
            ..RuntimeBundleConfiguration::default()
        };
        let manager = ManagedRuntimeManager::new(temp.path(), config.clone());
        assert_ne!(manager.tool_paths().adb, PathBuf::from("/usr/bin/adb"));

        let manager = ManagedRuntimeManager::new(
            temp.path(),
            RuntimeBundleConfiguration {
                allow_system_tool_overrides: true,
                ..config
            },
        );
        assert_eq!(manager.tool_paths().adb, PathBuf::from("/usr/bin/adb"));
    }

    #[test]
    fn managed_tool_paths_resolve_for_unix_and_windows() {
        let temp = tempdir().expect("tempdir");
        let manager =
            ManagedRuntimeManager::new(temp.path(), RuntimeBundleConfiguration::default());
        let unix = manager.managed_tool_paths_for_os(HostOs::Unix);
        assert!(unix
            .sdkmanager
            .ends_with("cmdline-tools/latest/bin/sdkmanager"));
        assert!(unix
            .avdmanager
            .ends_with("cmdline-tools/latest/bin/avdmanager"));
        assert!(unix.adb.ends_with("platform-tools/adb"));
        assert!(unix.emulator.ends_with("emulator/emulator"));

        let windows = manager.managed_tool_paths_for_os(HostOs::Windows);
        assert!(windows
            .sdkmanager
            .ends_with("cmdline-tools/latest/bin/sdkmanager.bat"));
        assert!(windows
            .avdmanager
            .ends_with("cmdline-tools/latest/bin/avdmanager.bat"));
        assert!(windows.adb.ends_with("platform-tools/adb.exe"));
        assert!(windows.emulator.ends_with("emulator/emulator.exe"));
    }

    #[test]
    fn required_sdk_packages_include_default_runtime_components() {
        let temp = tempdir().expect("tempdir");
        let manager = ManagedRuntimeManager::new(
            temp.path(),
            RuntimeBundleConfiguration {
                api_level: 35,
                system_image_channel: AndroidSystemImageChannel::GoogleApis,
                abi: AndroidSystemImageAbi::X86_64,
                ..RuntimeBundleConfiguration::default()
            },
        );
        assert_eq!(
            manager.required_packages(),
            vec![
                "cmdline-tools;latest",
                "platform-tools",
                "emulator",
                "platforms;android-35",
                "system-images;android-35;google_apis;x86_64"
            ]
        );
    }

    #[test]
    fn provisioning_plan_needs_cmdline_tools_when_absent() {
        let temp = tempdir().expect("tempdir");
        let manager =
            ManagedRuntimeManager::new(temp.path(), RuntimeBundleConfiguration::default());
        let plan = manager.provisioning_plan();
        assert!(matches!(
            plan.state,
            ProvisioningState::NeedsCommandLineToolsDownload { .. }
        ));
    }

    #[test]
    fn sdkmanager_command_uses_managed_sdk_root() {
        let temp = tempdir().expect("tempdir");
        let manager =
            ManagedRuntimeManager::new(temp.path(), RuntimeBundleConfiguration::default());
        let mut runner = RecordingRunner::default();
        manager
            .run_sdkmanager(&mut runner)
            .expect("sdkmanager command should be recorded");
        let call = runner.calls.first().expect("sdkmanager call");
        assert_eq!(call.program, manager.tool_paths().sdkmanager);
        assert_eq!(
            call.args[0],
            format!(
                "--sdk_root={}",
                manager.resolved().sdk_root.to_string_lossy()
            )
        );
    }

    #[test]
    fn avdmanager_command_uses_managed_avd_home_env() {
        let temp = tempdir().expect("tempdir");
        let manager =
            ManagedRuntimeManager::new(temp.path(), RuntimeBundleConfiguration::default());
        let mut runner = RecordingRunner::default();
        manager
            .run_avdmanager(&mut runner)
            .expect("avdmanager command should be recorded");
        let call = runner.calls.first().expect("avdmanager call");
        assert_eq!(call.program, manager.tool_paths().avdmanager);
        assert!(call.args.contains(&"--path".to_string()));
        assert!(call.env.iter().any(|(key, value)| {
            key == "ANDROID_AVD_HOME" && value == &manager.resolved().avd_home.to_string_lossy()
        }));
    }

    #[test]
    fn partial_runtime_bundle_config_deserializes_with_defaults() {
        let config: RuntimeBundleConfiguration = serde_json::from_value(serde_json::json!({
            "bundleRoot": "/tmp/apkrunner-runtime",
            "launchEmulator": true
        }))
        .expect("partial runtime bundle should deserialize");
        assert_eq!(
            config.bundle_root,
            Some(PathBuf::from("/tmp/apkrunner-runtime"))
        );
        assert_eq!(config.api_level, DEFAULT_ANDROID_API_LEVEL);
        assert_eq!(
            config.system_image_channel,
            AndroidSystemImageChannel::GoogleAtd
        );
        assert!(config.capture_frame_on_start);
    }
}

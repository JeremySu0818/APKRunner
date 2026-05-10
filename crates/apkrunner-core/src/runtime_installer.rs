use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::error::{ApkRunnerError, ApkRunnerResult};
use crate::runtime_bundle::{
    AndroidSystemImageAbi, AndroidSystemImageChannel, ManagedRuntimeManager,
    RuntimeBundleConfiguration, RuntimeBundleManifest,
};

const COMMAND_LINE_TOOLS_VERSION: &str = "14742923";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60 * 30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeInstallState {
    NotInstalled,
    Installed,
    Installing,
    Deleting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBundleStatus {
    pub state: RuntimeInstallState,
    pub bundle_root: PathBuf,
    pub sdk_root: PathBuf,
    pub avd_home: PathBuf,
    pub avd_name: String,
    pub manifest_path: PathBuf,
    pub installed: bool,
    pub phase: String,
    pub message: String,
    pub progress: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInstallProgress {
    pub state: RuntimeInstallState,
    pub phase: String,
    pub message: String,
    pub progress: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct RuntimeInstallRequest {
    pub bundle_root: PathBuf,
    pub api_level: u32,
    pub channel: AndroidSystemImageChannel,
    pub abi: AndroidSystemImageAbi,
}

impl RuntimeInstallRequest {
    pub fn for_bundle_root(bundle_root: impl Into<PathBuf>) -> Self {
        let config = RuntimeBundleConfiguration::default();
        Self {
            bundle_root: bundle_root.into(),
            api_level: config.api_level,
            channel: config.system_image_channel,
            abi: config.abi,
        }
    }

    fn manager(&self) -> ManagedRuntimeManager {
        ManagedRuntimeManager::new(
            self.bundle_root.parent().unwrap_or(&self.bundle_root),
            RuntimeBundleConfiguration {
                bundle_root: Some(self.bundle_root.clone()),
                api_level: self.api_level,
                system_image_channel: self.channel,
                abi: self.abi,
                ..RuntimeBundleConfiguration::default()
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct CommandLineToolsDownload {
    platform: &'static str,
    sha256: Option<&'static str>,
}

impl CommandLineToolsDownload {
    fn for_host() -> ApkRunnerResult<Self> {
        if cfg!(target_os = "windows") {
            Ok(Self {
                platform: "win",
                sha256: None,
            })
        } else if cfg!(target_os = "macos") {
            Ok(Self {
                platform: "mac",
                sha256: None,
            })
        } else if cfg!(target_os = "linux") {
            Ok(Self {
                platform: "linux",
                sha256: Some("04453066b540409d975c676d781da1477479dde3761310f1a7eb92a1dfb15af7"),
            })
        } else {
            Err(ApkRunnerError::RuntimeBackendError(
                "managed Android runtime installer supports Windows, macOS, and Linux".to_string(),
            ))
        }
    }

    fn file_name(self) -> String {
        format!(
            "commandlinetools-{}-{}_latest.zip",
            self.platform, COMMAND_LINE_TOOLS_VERSION
        )
    }

    fn url(self) -> String {
        format!(
            "https://dl.google.com/android/repository/{}",
            self.file_name()
        )
    }
}

pub fn runtime_bundle_status(bundle_root: impl Into<PathBuf>) -> RuntimeBundleStatus {
    let request = RuntimeInstallRequest::for_bundle_root(bundle_root);
    let manager = request.manager();
    let resolved = manager.resolved();
    let readiness = manager.readiness();
    let ready = readiness.ready();
    RuntimeBundleStatus {
        state: if ready {
            RuntimeInstallState::Installed
        } else {
            RuntimeInstallState::NotInstalled
        },
        bundle_root: resolved.bundle_root,
        sdk_root: resolved.sdk_root,
        avd_home: resolved.avd_home,
        avd_name: resolved.avd_name,
        manifest_path: readiness.manifest_path,
        installed: ready,
        phase: if ready {
            "ready".to_string()
        } else {
            "missing".to_string()
        },
        message: if ready {
            "Managed Android runtime is installed.".to_string()
        } else {
            "Managed Android runtime is not installed.".to_string()
        },
        progress: None,
        error: None,
    }
}

pub fn delete_runtime_bundle(
    bundle_root: impl AsRef<Path>,
    mut progress: impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<RuntimeBundleStatus> {
    let bundle_root = bundle_root.as_ref();
    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Deleting,
        phase: "delete".to_string(),
        message: "Deleting managed Android runtime bundle.".to_string(),
        progress: None,
    });
    if bundle_root.exists() {
        fs::remove_dir_all(bundle_root).map_err(|source| ApkRunnerError::HostIoFailure {
            path: bundle_root.to_path_buf(),
            reason: source.to_string(),
        })?;
    }
    Ok(runtime_bundle_status(bundle_root.to_path_buf()))
}

pub fn install_runtime_bundle(
    request: RuntimeInstallRequest,
    mut progress: impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<RuntimeBundleStatus> {
    let manager = request.manager();
    let resolved = manager.resolved();
    let tools = manager.tool_paths();
    fs::create_dir_all(&resolved.bundle_root).map_err(io_error(&resolved.bundle_root))?;
    fs::create_dir_all(&resolved.sdk_root).map_err(io_error(&resolved.sdk_root))?;
    fs::create_dir_all(&resolved.avd_home).map_err(io_error(&resolved.avd_home))?;
    fs::create_dir_all(&resolved.emulator_home).map_err(io_error(&resolved.emulator_home))?;

    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "prepare".to_string(),
        message: "Preparing managed Android runtime directories.".to_string(),
        progress: Some(0.02),
    });

    if !tools.sdkmanager.is_file() || !tools.avdmanager.is_file() {
        install_command_line_tools(&resolved.bundle_root, &resolved.sdk_root, &mut progress)?;
    }

    accept_licenses(&tools.sdkmanager, &manager, &mut progress)?;
    install_sdk_packages(&tools.sdkmanager, &manager, &mut progress)?;
    create_avd(&tools.avdmanager, &manager, &mut progress)?;
    write_manifest(&request, &manager)?;

    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installed,
        phase: "complete".to_string(),
        message: "Managed Android runtime is ready.".to_string(),
        progress: Some(1.0),
    });

    Ok(runtime_bundle_status(resolved.bundle_root))
}

fn install_command_line_tools(
    bundle_root: &Path,
    sdk_root: &Path,
    progress: &mut impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<()> {
    let download = CommandLineToolsDownload::for_host()?;
    let downloads = bundle_root.join("downloads");
    let archive_path = downloads.join(download.file_name());
    fs::create_dir_all(&downloads).map_err(io_error(&downloads))?;
    download_file(&download.url(), &archive_path, download.sha256, progress)?;

    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "extract-command-line-tools".to_string(),
        message: "Extracting Android command-line tools.".to_string(),
        progress: Some(0.22),
    });
    let destination = sdk_root.join("cmdline-tools").join("latest");
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(io_error(&destination))?;
    }
    extract_cmdline_tools(&archive_path, &destination)?;
    Ok(())
}

fn download_file(
    url: &str,
    destination: &Path,
    expected_sha256: Option<&str>,
    progress: &mut impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<()> {
    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "download-command-line-tools".to_string(),
        message: "Downloading official Android command-line tools.".to_string(),
        progress: Some(0.05),
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(60 * 20))
        .build()
        .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
    let total = response.content_length();
    let temp_path = destination.with_extension("download");
    let mut file = File::create(&temp_path).map_err(io_error(&temp_path))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 1024 * 64];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(io_error(&temp_path))?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        let fraction = total.map(|total| (downloaded as f64 / total as f64).clamp(0.0, 1.0));
        progress(RuntimeInstallProgress {
            state: RuntimeInstallState::Installing,
            phase: "download-command-line-tools".to_string(),
            message: format!(
                "Downloaded {} MB of command-line tools.",
                downloaded / 1_000_000
            ),
            progress: fraction.map(|fraction| 0.05 + fraction * 0.15),
        });
    }
    let actual = hex_lower(&hasher.finalize());
    if let Some(expected_sha256) = expected_sha256 {
        if actual != expected_sha256 {
            return Err(ApkRunnerError::RuntimeBackendError(format!(
                "command-line tools checksum mismatch: expected {expected_sha256}, actual {actual}"
            )));
        }
    }
    fs::rename(&temp_path, destination).map_err(io_error(destination))?;
    Ok(())
}

fn extract_cmdline_tools(archive_path: &Path, destination: &Path) -> ApkRunnerResult<()> {
    let file = File::open(archive_path).map_err(io_error(archive_path))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
    fs::create_dir_all(destination).map_err(io_error(destination))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
        let Some(enclosed) = entry.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let relative = enclosed.strip_prefix("cmdline-tools").map_err(|_| {
            ApkRunnerError::RuntimeBackendError(
                "invalid command-line tools archive layout".to_string(),
            )
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output_path = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(io_error(&output_path))?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).map_err(io_error(parent))?;
            }
            let mut output = File::create(&output_path).map_err(io_error(&output_path))?;
            io::copy(&mut entry, &mut output).map_err(io_error(&output_path))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    fs::set_permissions(&output_path, fs::Permissions::from_mode(mode))
                        .map_err(io_error(&output_path))?;
                }
            }
        }
    }
    Ok(())
}

fn accept_licenses(
    sdkmanager: &Path,
    manager: &ManagedRuntimeManager,
    progress: &mut impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<()> {
    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "accept-licenses".to_string(),
        message: "Accepting Android SDK package licenses for this managed bundle.".to_string(),
        progress: Some(0.26),
    });
    run_managed_command_with_input(
        sdkmanager,
        &["--licenses".to_string()],
        &manager.command_env(),
        Some("y\n".repeat(32).as_bytes()),
        COMMAND_TIMEOUT,
        None,
    )?;
    Ok(())
}

fn install_sdk_packages(
    sdkmanager: &Path,
    manager: &ManagedRuntimeManager,
    progress: &mut impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<()> {
    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "install-sdk-packages".to_string(),
        message: "Installing Android Emulator, Platform Tools, API platform, and ATD image."
            .to_string(),
        progress: Some(0.34),
    });
    let resolved = manager.resolved();
    let mut args = vec![format!(
        "--sdk_root={}",
        resolved.sdk_root.to_string_lossy()
    )];
    args.extend(
        manager
            .required_packages()
            .into_iter()
            .filter(|package| package != "cmdline-tools;latest"),
    );
    let mut tick = |elapsed: Duration| {
        let fraction = (elapsed.as_secs_f64() / 600.0).clamp(0.0, 1.0);
        progress(RuntimeInstallProgress {
            state: RuntimeInstallState::Installing,
            phase: "install-sdk-packages".to_string(),
            message: format!(
                "Installing Android SDK packages. Elapsed: {}s.",
                elapsed.as_secs()
            ),
            progress: Some(0.34 + fraction * 0.40),
        });
    };
    run_managed_command_with_input(
        sdkmanager,
        &args,
        &manager.command_env(),
        Some("y\n".repeat(64).as_bytes()),
        COMMAND_TIMEOUT,
        Some(&mut tick),
    )?;
    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "install-sdk-packages".to_string(),
        message: "Android SDK packages installed.".to_string(),
        progress: Some(0.78),
    });
    Ok(())
}

fn create_avd(
    avdmanager: &Path,
    manager: &ManagedRuntimeManager,
    progress: &mut impl FnMut(RuntimeInstallProgress),
) -> ApkRunnerResult<()> {
    let resolved = manager.resolved();
    if resolved.avd_path().is_dir() && manager.manifest_path().is_file() {
        return Ok(());
    }
    progress(RuntimeInstallProgress {
        state: RuntimeInstallState::Installing,
        phase: "create-avd".to_string(),
        message: "Creating APKRunner-managed Android Virtual Device.".to_string(),
        progress: Some(0.84),
    });
    let args = vec![
        "create".to_string(),
        "avd".to_string(),
        "--force".to_string(),
        "--name".to_string(),
        resolved.avd_name,
        "--package".to_string(),
        manager.system_image_package(),
        "--device".to_string(),
        "pixel".to_string(),
        "--path".to_string(),
        manager.resolved().avd_path().to_string_lossy().into_owned(),
    ];
    run_managed_command_with_input(
        avdmanager,
        &args,
        &manager.command_env(),
        Some(b"no\n"),
        COMMAND_TIMEOUT,
        None,
    )?;
    Ok(())
}

fn write_manifest(
    request: &RuntimeInstallRequest,
    manager: &ManagedRuntimeManager,
) -> ApkRunnerResult<()> {
    let resolved = manager.resolved();
    let manifest = RuntimeBundleManifest {
        schema_version: 1,
        api_level: request.api_level,
        abi: request.abi.package_segment().to_string(),
        system_image_channel: request.channel.manifest_value().to_string(),
        sdk_root: resolved.sdk_root,
        avd_home: resolved.avd_home,
        avd_name: resolved.avd_name,
        packages: manager.required_packages(),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    };
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?;
    fs::write(manager.manifest_path(), bytes).map_err(io_error(&manager.manifest_path()))
}

fn run_managed_command_with_input(
    program: &Path,
    args: &[String],
    env: &[(String, String)],
    input: Option<&[u8]>,
    timeout: Duration,
    mut on_wait: Option<&mut dyn FnMut(Duration)>,
) -> ApkRunnerResult<()> {
    let mut command = command_for_program(program, args);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(io_error(program))?;
    let stdout_reader = child.stdout.take().map(read_stream_in_background);
    let stderr_reader = child.stderr.take().map(read_stream_in_background);
    if let Some(input) = input {
        let mut stdin = child.stdin.take().ok_or_else(|| {
            ApkRunnerError::RuntimeBackendError("failed to open child stdin".to_string())
        })?;
        stdin.write_all(input).map_err(io_error(program))?;
    }
    let started = Instant::now();
    let mut last_wait_update = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?
            .is_some()
        {
            let status = child.wait().map_err(io_error(program))?;
            let stdout = join_reader(stdout_reader);
            let stderr = join_reader(stderr_reader);
            if status.success() {
                return Ok(());
            }
            return Err(ApkRunnerError::RuntimeBackendError(format!(
                "{} failed with status {:?}: {}",
                program.display(),
                status.code(),
                command_tail(&stdout, &stderr)
            )));
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = join_reader(stdout_reader);
            let stderr = join_reader(stderr_reader);
            return Err(ApkRunnerError::RuntimeBackendError(format!(
                "{} timed out after {}s. Last output: {}",
                program.display(),
                timeout.as_secs(),
                command_tail(&stdout, &stderr)
            )));
        }
        if last_wait_update.elapsed() >= Duration::from_secs(1) {
            if let Some(on_wait) = on_wait.as_mut() {
                on_wait(started.elapsed());
            }
            last_wait_update = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn read_stream_in_background<R>(mut stream: R) -> JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        bytes
    })
}

fn join_reader(reader: Option<JoinHandle<Vec<u8>>>) -> Vec<u8> {
    reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default()
}

fn command_tail(stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = Vec::with_capacity(stdout.len() + stderr.len() + 1);
    combined.extend_from_slice(stdout);
    combined.push(b'\n');
    combined.extend_from_slice(stderr);
    let text = String::from_utf8_lossy(&combined);
    let tail = text
        .lines()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    if tail.trim().is_empty() {
        "<no command output>".to_string()
    } else {
        tail
    }
}

fn command_for_program(program: &Path, args: &[String]) -> Command {
    #[cfg(target_os = "windows")]
    {
        if program
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("bat"))
        {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(program).args(args);
            return command;
        }
    }
    let mut command = Command::new(program);
    command.args(args);
    command
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ApkRunnerError + '_ {
    |source| ApkRunnerError::HostIoFailure {
        path: path.to_path_buf(),
        reason: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_tools_urls_are_official_google_downloads() {
        let spec = CommandLineToolsDownload::for_host().expect("host should be supported");
        assert!(spec
            .url()
            .starts_with("https://dl.google.com/android/repository/"));
        assert!(spec.sha256.is_none_or(|checksum| checksum.len() == 64));
    }
}

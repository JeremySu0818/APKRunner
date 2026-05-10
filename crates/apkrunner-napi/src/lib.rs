use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::thread;

use apkrunner_core::{
    delete_runtime_bundle, install_runtime_bundle, runtime_bundle_status, ApkRunnerError,
    AppConfiguration, InputEvent, Runner, RunnerConfiguration, RuntimeBundleStatus,
    RuntimeInstallProgress, RuntimeInstallRequest, RuntimeInstallState,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use parking_lot::Mutex;
use serde_json::{json, Value};
use uuid::Uuid;

static RUNNERS: LazyLock<Mutex<HashMap<Uuid, Runner>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static RUNTIME_OPERATIONS: LazyLock<Mutex<HashMap<Uuid, RuntimeOperationStatus>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeOperationStatus {
    operation_id: String,
    state: RuntimeInstallState,
    phase: String,
    message: String,
    progress: Option<f64>,
    error: Option<String>,
    bundle_status: Option<RuntimeBundleStatus>,
}

fn to_napi_error(error: impl std::fmt::Display) -> napi::Error {
    napi::Error::from_reason(error.to_string())
}

fn parse_uuid(value: &str, label: &str) -> Result<Uuid> {
    Uuid::parse_str(value)
        .map_err(|source| to_napi_error(format!("invalid {label} UUID: {source}")))
}

fn with_runner<T>(
    runner_id: &str,
    callback: impl FnOnce(&mut Runner) -> std::result::Result<T, ApkRunnerError>,
) -> Result<T> {
    let id = parse_uuid(runner_id, "runner")?;
    let mut registry = RUNNERS.lock();
    let runner = registry
        .get_mut(&id)
        .ok_or_else(|| to_napi_error(format!("runner {runner_id} was not found")))?;
    callback(runner).map_err(to_napi_error)
}

fn parse_bundle_root(value: Value) -> Result<PathBuf> {
    value
        .get("bundleRoot")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| to_napi_error("bundleRoot is required"))
}

fn set_operation_progress(operation_id: Uuid, progress: RuntimeInstallProgress) {
    let mut operations = RUNTIME_OPERATIONS.lock();
    if let Some(status) = operations.get_mut(&operation_id) {
        status.state = progress.state;
        status.phase = progress.phase;
        status.message = progress.message;
        status.progress = progress.progress;
    }
}

fn finish_operation(
    operation_id: Uuid,
    state: RuntimeInstallState,
    phase: &str,
    message: String,
    error: Option<String>,
    bundle_status: Option<RuntimeBundleStatus>,
) {
    let mut operations = RUNTIME_OPERATIONS.lock();
    if let Some(status) = operations.get_mut(&operation_id) {
        status.state = state;
        status.phase = phase.to_string();
        status.message = message;
        status.error = error;
        status.bundle_status = bundle_status;
        if status.state == RuntimeInstallState::Installed {
            status.progress = Some(1.0);
        }
    }
}

fn operation_status_value(operation_id: Uuid) -> Result<Value> {
    let operations = RUNTIME_OPERATIONS.lock();
    let status = operations
        .get(&operation_id)
        .ok_or_else(|| to_napi_error(format!("runtime operation {operation_id} was not found")))?;
    serde_json::to_value(status).map_err(to_napi_error)
}

#[napi(js_name = "createRunner")]
pub fn create_runner(config: Value) -> Result<Value> {
    let config: RunnerConfiguration = serde_json::from_value(config).map_err(to_napi_error)?;
    let runner = Runner::new(config).map_err(to_napi_error)?;
    let runner_id = runner.id();
    let backend_name = runner.backend_name().to_string();
    RUNNERS.lock().insert(runner_id, runner);
    Ok(json!({
        "runnerId": runner_id.to_string(),
        "backendName": backend_name
    }))
}

#[napi(js_name = "getRuntimeBundleStatus")]
pub fn get_runtime_bundle_status(config: Value) -> Result<Value> {
    let bundle_root = parse_bundle_root(config)?;
    serde_json::to_value(runtime_bundle_status(bundle_root)).map_err(to_napi_error)
}

#[napi(js_name = "startRuntimeDownload")]
pub fn start_runtime_download(config: Value) -> Result<Value> {
    let bundle_root = parse_bundle_root(config)?;
    let operation_id = Uuid::new_v4();
    RUNTIME_OPERATIONS.lock().insert(
        operation_id,
        RuntimeOperationStatus {
            operation_id: operation_id.to_string(),
            state: RuntimeInstallState::Installing,
            phase: "queued".to_string(),
            message: "Runtime download queued.".to_string(),
            progress: Some(0.0),
            error: None,
            bundle_status: None,
        },
    );

    thread::spawn(move || {
        let result = install_runtime_bundle(
            RuntimeInstallRequest::for_bundle_root(bundle_root),
            |progress| {
                set_operation_progress(operation_id, progress);
            },
        );
        match result {
            Ok(status) => finish_operation(
                operation_id,
                RuntimeInstallState::Installed,
                "complete",
                "Managed Android runtime is ready.".to_string(),
                None,
                Some(status),
            ),
            Err(error) => finish_operation(
                operation_id,
                RuntimeInstallState::Error,
                "error",
                error.to_string(),
                Some(error.to_string()),
                None,
            ),
        }
    });

    operation_status_value(operation_id)
}

#[napi(js_name = "startRuntimeDelete")]
pub fn start_runtime_delete(config: Value) -> Result<Value> {
    let bundle_root = parse_bundle_root(config)?;
    let operation_id = Uuid::new_v4();
    RUNTIME_OPERATIONS.lock().insert(
        operation_id,
        RuntimeOperationStatus {
            operation_id: operation_id.to_string(),
            state: RuntimeInstallState::Deleting,
            phase: "queued".to_string(),
            message: "Runtime delete queued.".to_string(),
            progress: None,
            error: None,
            bundle_status: None,
        },
    );

    thread::spawn(move || {
        let result = delete_runtime_bundle(&bundle_root, |progress| {
            set_operation_progress(operation_id, progress);
        });
        match result {
            Ok(status) => finish_operation(
                operation_id,
                RuntimeInstallState::NotInstalled,
                "complete",
                "Managed Android runtime deleted.".to_string(),
                None,
                Some(status),
            ),
            Err(error) => finish_operation(
                operation_id,
                RuntimeInstallState::Error,
                "error",
                error.to_string(),
                Some(error.to_string()),
                None,
            ),
        }
    });

    operation_status_value(operation_id)
}

#[napi(js_name = "getRuntimeOperationStatus")]
pub fn get_runtime_operation_status(operation_id: String) -> Result<Value> {
    operation_status_value(parse_uuid(&operation_id, "runtime operation")?)
}

#[napi(js_name = "loadApk")]
pub fn load_apk(runner_id: String, apk_path: String) -> Result<Value> {
    with_runner(&runner_id, |runner| {
        runner.load_apk_from_host_path(apk_path)?;
        runner.current_apk_summary().cloned().ok_or_else(|| {
            ApkRunnerError::General("APK summary unavailable after load".to_string())
        })
    })
    .and_then(|summary| serde_json::to_value(summary).map_err(to_napi_error))
}

#[napi(js_name = "createAppInstance")]
pub fn create_app_instance(runner_id: String, config: Value) -> Result<Value> {
    let config: AppConfiguration = serde_json::from_value(config).map_err(to_napi_error)?;
    let instance_id = with_runner(&runner_id, |runner| runner.create_app_instance(config))?;
    Ok(json!({
        "instanceId": instance_id.to_string()
    }))
}

#[napi(js_name = "startApp")]
pub fn start_app(runner_id: String, instance_id: String) -> Result<Value> {
    let instance_id = parse_uuid(&instance_id, "app instance")?;
    with_runner(&runner_id, |runner| {
        runner.start_app(instance_id)?;
        Ok(runner.status())
    })
    .and_then(|status| serde_json::to_value(status).map_err(to_napi_error))
}

#[napi(js_name = "stopApp")]
pub fn stop_app(runner_id: String, instance_id: String) -> Result<Value> {
    let instance_id = parse_uuid(&instance_id, "app instance")?;
    with_runner(&runner_id, |runner| {
        runner.stop_app(instance_id)?;
        Ok(runner.status())
    })
    .and_then(|status| serde_json::to_value(status).map_err(to_napi_error))
}

#[napi(js_name = "dispatchInput")]
pub fn dispatch_input(runner_id: String, instance_id: String, input: Value) -> Result<Value> {
    let instance_id = parse_uuid(&instance_id, "app instance")?;
    let input: InputEvent = serde_json::from_value(input).map_err(to_napi_error)?;
    with_runner(&runner_id, |runner| {
        runner.dispatch_input(instance_id, input)?;
        Ok(runner.status())
    })
    .and_then(|status| serde_json::to_value(status).map_err(to_napi_error))
}

#[napi(js_name = "pollEvents")]
pub fn poll_events(runner_id: String) -> Result<Value> {
    with_runner(&runner_id, |runner| Ok(runner.poll_runtime_events()))
        .and_then(|events| serde_json::to_value(events).map_err(to_napi_error))
}

use std::collections::HashMap;
use std::sync::LazyLock;

use apkrunner_core::{AppConfiguration, ApkRunnerError, Runner, RunnerConfiguration};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use parking_lot::Mutex;
use serde_json::{json, Value};
use uuid::Uuid;

static RUNNERS: LazyLock<Mutex<HashMap<Uuid, Runner>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

#[napi(js_name = "loadApk")]
pub fn load_apk(runner_id: String, apk_path: String) -> Result<Value> {
    with_runner(&runner_id, |runner| {
        runner.load_apk_from_host_path(apk_path)?;
        runner
            .current_apk_summary()
            .cloned()
            .ok_or_else(|| ApkRunnerError::General("APK summary unavailable after load".to_string()))
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

#[napi(js_name = "pollEvents")]
pub fn poll_events(runner_id: String) -> Result<Value> {
    with_runner(&runner_id, |runner| Ok(runner.poll_runtime_events()))
        .and_then(|events| serde_json::to_value(events).map_err(to_napi_error))
}

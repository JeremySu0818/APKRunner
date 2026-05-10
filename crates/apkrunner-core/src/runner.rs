use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::apk::{load_apk, ApkSummary, LoadedApk};
use crate::error::{ApkRunnerError, ApkRunnerResult};
use crate::event::RuntimeEvent;
use crate::input::InputEvent;
use crate::permissions::PermissionState;
use crate::runtime::{backend_for, BackendKind, RuntimeBackend};
use crate::runtime_bundle::RuntimeBundleConfiguration;

pub type RunnerHandle = Uuid;
pub type LoadedApkHandle = Uuid;
pub type AppInstanceHandle = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerConfiguration {
    pub backend_kind: BackendKind,
    pub sandbox_root: PathBuf,
    #[serde(default)]
    pub runtime_bundle: RuntimeBundleConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfiguration {
    pub package_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerStatus {
    pub runner_id: String,
    pub backend_name: String,
    pub loaded_apk_id: Option<String>,
    pub current_app_instance_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppInstance {
    pub id: Uuid,
    pub loaded_apk_id: Uuid,
    pub package_name: String,
    pub launcher_activity: Option<String>,
    pub apk_path: PathBuf,
}

pub struct Runner {
    id: Uuid,
    config: RunnerConfiguration,
    backend: Box<dyn RuntimeBackend>,
    loaded_apk: Option<LoadedApk>,
    instances: HashMap<Uuid, AppInstance>,
    current_instance: Option<Uuid>,
    events: VecDeque<RuntimeEvent>,
}

impl Runner {
    pub fn new(config: RunnerConfiguration) -> ApkRunnerResult<Self> {
        let mut config = config;
        config.runtime_bundle = config.runtime_bundle.clone().with_environment_overrides();
        let backend = backend_for(
            config.backend_kind,
            config.sandbox_root.clone(),
            config.runtime_bundle.clone(),
        )?;
        Ok(Self {
            id: Uuid::new_v4(),
            config,
            backend,
            loaded_apk: None,
            instances: HashMap::new(),
            current_instance: None,
            events: VecDeque::new(),
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn config(&self) -> &RunnerConfiguration {
        &self.config
    }

    pub fn load_apk_from_host_path(
        &mut self,
        path: impl Into<PathBuf>,
    ) -> ApkRunnerResult<LoadedApkHandle> {
        let loaded = load_apk(path.into())?;
        let id = loaded.id;
        for permission in &loaded.summary.requested_permissions {
            if permission.dangerous || permission.state == PermissionState::Unsupported {
                self.events.push_back(RuntimeEvent::PermissionRequest {
                    permission: permission.clone(),
                });
            }
        }
        self.events.push_back(RuntimeEvent::ApkLoaded {
            summary: loaded.summary.clone(),
        });
        self.loaded_apk = Some(loaded);
        Ok(id)
    }

    pub fn current_apk_summary(&self) -> Option<&ApkSummary> {
        self.loaded_apk.as_ref().map(|apk| &apk.summary)
    }

    pub fn create_app_instance(
        &mut self,
        config: AppConfiguration,
    ) -> ApkRunnerResult<AppInstanceHandle> {
        let loaded_apk = self.loaded_apk.as_ref().ok_or_else(|| {
            ApkRunnerError::General("cannot create app instance before loading an APK".to_string())
        })?;
        let instance = self.backend.create_app_instance(loaded_apk, &config)?;
        let id = instance.id;
        self.instances.insert(id, instance);
        self.current_instance = Some(id);
        Ok(id)
    }

    pub fn start_app(&mut self, handle: AppInstanceHandle) -> ApkRunnerResult<()> {
        let instance = self.instances.get(&handle).cloned().ok_or_else(|| {
            ApkRunnerError::RuntimeBackendError(format!("unknown app instance {handle}"))
        })?;
        self.backend.start_app(&instance)
    }

    pub fn stop_app(&mut self, handle: AppInstanceHandle) -> ApkRunnerResult<()> {
        let instance = self.instances.get(&handle).cloned().ok_or_else(|| {
            ApkRunnerError::RuntimeBackendError(format!("unknown app instance {handle}"))
        })?;
        self.backend.stop_app(&instance)
    }

    pub fn dispatch_input(
        &mut self,
        handle: AppInstanceHandle,
        input: InputEvent,
    ) -> ApkRunnerResult<()> {
        let instance = self.instances.get(&handle).cloned().ok_or_else(|| {
            ApkRunnerError::RuntimeBackendError(format!("unknown app instance {handle}"))
        })?;
        self.backend.dispatch_input(&instance, input)
    }

    pub fn poll_runtime_events(&mut self) -> Vec<RuntimeEvent> {
        let mut events = self.events.drain(..).collect::<Vec<_>>();
        events.extend(self.backend.poll_runtime_events());
        events
    }

    pub fn status(&self) -> RunnerStatus {
        RunnerStatus {
            runner_id: self.id.to_string(),
            backend_name: self.backend.name().to_string(),
            loaded_apk_id: self.loaded_apk.as_ref().map(|apk| apk.id.to_string()),
            current_app_instance_id: self.current_instance.map(|id| id.to_string()),
        }
    }
}

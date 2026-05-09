use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::apk::{LoadedApk, UnsupportedFeature, UnsupportedFeatureSeverity, UnsupportedFeatureSource};
use crate::error::{ApkRunnerError, ApkRunnerResult};
use crate::event::{LogLevel, RuntimeEvent};
use crate::frame::PlaceholderFrameRenderer;
use crate::runner::{AppConfiguration, AppInstance};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendKind {
    Skeleton,
    DexVm,
    Aosp,
    Vm,
    ArmTranslation,
}

pub trait RuntimeBackend: Send {
    fn name(&self) -> &'static str;
    fn create_app_instance(
        &mut self,
        loaded_apk: &LoadedApk,
        config: &AppConfiguration,
    ) -> ApkRunnerResult<AppInstance>;
    fn start_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()>;
    fn stop_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()>;
    fn poll_runtime_events(&mut self) -> Vec<RuntimeEvent>;
}

#[derive(Debug, Default)]
pub struct SkeletonRuntimeBackend {
    running_instances: HashSet<Uuid>,
    events: VecDeque<RuntimeEvent>,
}

pub struct DexVmRuntimeBackend;
pub struct AospRuntimeBackend;
pub struct VmRuntimeBackend;
pub struct ArmTranslationRuntimeBackend;

impl SkeletonRuntimeBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn push_log(&mut self, message: impl Into<String>) {
        self.events
            .push_back(RuntimeEvent::log(LogLevel::Info, "APKRunner", message));
    }
}

impl RuntimeBackend for SkeletonRuntimeBackend {
    fn name(&self) -> &'static str {
        "SkeletonRuntimeBackend"
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
        })
    }

    fn start_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()> {
        if self.running_instances.contains(&instance.id) {
            return Err(ApkRunnerError::AppAlreadyRunning(instance.id));
        }
        self.running_instances.insert(instance.id);
        self.push_log(format!("APKRunner: Starting {}", instance.package_name));
        self.push_log(format!(
            "APKRunner: Launcher Activity = {}",
            instance
                .launcher_activity
                .as_deref()
                .unwrap_or("<none>")
        ));
        self.push_log("APKRunner: Using SkeletonRuntimeBackend");
        self.push_log("APKRunner: Runtime execution is not implemented yet");
        self.push_log(
            "APKRunner: Future backends: DexVmRuntimeBackend, AospRuntimeBackend, VmRuntimeBackend",
        );
        self.events.push_back(RuntimeEvent::UnsupportedFeature {
            feature: UnsupportedFeature {
                feature: "Runtime execution".to_string(),
                detail: "SkeletonRuntimeBackend does not execute APK bytecode.".to_string(),
                severity: UnsupportedFeatureSeverity::Warning,
                source: UnsupportedFeatureSource::Runtime,
            },
        });
        self.events.push_back(RuntimeEvent::AppStarted {
            package_name: instance.package_name.clone(),
            instance_id: instance.id.to_string(),
        });
        let frame = PlaceholderFrameRenderer::render(320, 180);
        self.events.push_back(RuntimeEvent::FrameReady {
            surface_size: frame.surface_size,
            frame_format: frame.frame_format,
            payload_base64: frame.payload_base64,
            metadata: frame.metadata,
            timestamp: frame.timestamp,
        });
        Ok(())
    }

    fn stop_app(&mut self, instance: &AppInstance) -> ApkRunnerResult<()> {
        if !self.running_instances.remove(&instance.id) {
            return Err(ApkRunnerError::AppNotRunning(instance.id));
        }
        self.push_log(format!("APKRunner: Stopped {}", instance.package_name));
        self.events.push_back(RuntimeEvent::AppStopped {
            package_name: instance.package_name.clone(),
            instance_id: instance.id.to_string(),
        });
        Ok(())
    }

    fn poll_runtime_events(&mut self) -> Vec<RuntimeEvent> {
        self.events.drain(..).collect()
    }
}

pub fn backend_for(kind: BackendKind) -> ApkRunnerResult<Box<dyn RuntimeBackend>> {
    match kind {
        BackendKind::Skeleton => Ok(Box::new(SkeletonRuntimeBackend::new())),
        BackendKind::DexVm => Err(ApkRunnerError::BackendNotAvailable(
            "DexVmRuntimeBackend is reserved for future research.".to_string(),
        )),
        BackendKind::Aosp => Err(ApkRunnerError::BackendNotAvailable(
            "AospRuntimeBackend is reserved for future research.".to_string(),
        )),
        BackendKind::Vm => Err(ApkRunnerError::BackendNotAvailable(
            "VmRuntimeBackend is reserved for future research.".to_string(),
        )),
        BackendKind::ArmTranslation => Err(ApkRunnerError::BackendNotAvailable(
            "ArmTranslationRuntimeBackend is reserved for future research.".to_string(),
        )),
    }
}

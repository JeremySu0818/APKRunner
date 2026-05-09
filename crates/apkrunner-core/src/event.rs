use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::apk::{ApkSummary, UnsupportedFeature};
use crate::frame::{FrameFormat, SurfaceSize};
use crate::permissions::PermissionRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    #[serde(rename = "DEBUG")]
    Debug,
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARN")]
    Warn,
    #[serde(rename = "ERROR")]
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeEvent {
    Log {
        level: LogLevel,
        tag: String,
        message: String,
        timestamp: String,
    },
    ApkLoaded {
        summary: ApkSummary,
    },
    PermissionRequest {
        permission: PermissionRecord,
    },
    AppStarted {
        #[serde(rename = "packageName")]
        package_name: String,
        #[serde(rename = "instanceId")]
        instance_id: String,
    },
    AppStopped {
        #[serde(rename = "packageName")]
        package_name: String,
        #[serde(rename = "instanceId")]
        instance_id: String,
    },
    UnsupportedFeature {
        feature: UnsupportedFeature,
    },
    FrameReady {
        #[serde(rename = "surfaceSize")]
        surface_size: SurfaceSize,
        #[serde(rename = "frameFormat")]
        frame_format: FrameFormat,
        #[serde(rename = "payloadBase64")]
        payload_base64: String,
        metadata: String,
        timestamp: String,
    },
}

impl RuntimeEvent {
    pub fn log(level: LogLevel, tag: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Log {
            level,
            tag: tag.into(),
            message: message.into(),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_event_serializes_to_json() {
        let event = RuntimeEvent::log(LogLevel::Info, "APKRunner", "hello");
        let json = serde_json::to_string(&event).expect("event should serialize");
        assert!(json.contains("\"type\":\"Log\""));
        assert!(json.contains("\"level\":\"INFO\""));
    }

    #[test]
    fn log_event_round_trips_through_json() {
        let event = RuntimeEvent::log(LogLevel::Warn, "APKRunner", "round trip");
        let json = serde_json::to_string(&event).expect("event should serialize");
        let decoded: RuntimeEvent = serde_json::from_str(&json).expect("event should deserialize");
        assert_eq!(event, decoded);
    }
}

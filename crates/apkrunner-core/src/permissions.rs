use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionState {
    Granted,
    Denied,
    AskEveryTime,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRecord {
    pub name: String,
    pub state: PermissionState,
    pub dangerous: bool,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
struct KnownPermission {
    name: &'static str,
    dangerous: bool,
    description: &'static str,
}

const KNOWN_PERMISSIONS: &[KnownPermission] = &[
    KnownPermission {
        name: "android.permission.INTERNET",
        dangerous: false,
        description: "Allows network socket access.",
    },
    KnownPermission {
        name: "android.permission.ACCESS_NETWORK_STATE",
        dangerous: false,
        description: "Allows reading network connectivity state.",
    },
    KnownPermission {
        name: "android.permission.CAMERA",
        dangerous: true,
        description: "Allows access to camera hardware.",
    },
    KnownPermission {
        name: "android.permission.RECORD_AUDIO",
        dangerous: true,
        description: "Allows microphone recording.",
    },
    KnownPermission {
        name: "android.permission.ACCESS_FINE_LOCATION",
        dangerous: true,
        description: "Allows precise device location access.",
    },
    KnownPermission {
        name: "android.permission.ACCESS_COARSE_LOCATION",
        dangerous: true,
        description: "Allows approximate device location access.",
    },
    KnownPermission {
        name: "android.permission.READ_EXTERNAL_STORAGE",
        dangerous: true,
        description: "Allows reading shared external storage.",
    },
    KnownPermission {
        name: "android.permission.WRITE_EXTERNAL_STORAGE",
        dangerous: true,
        description: "Allows writing shared external storage on legacy Android versions.",
    },
    KnownPermission {
        name: "android.permission.POST_NOTIFICATIONS",
        dangerous: true,
        description: "Allows posting notifications.",
    },
    KnownPermission {
        name: "android.permission.VIBRATE",
        dangerous: false,
        description: "Allows controlling the vibration motor.",
    },
    KnownPermission {
        name: "android.permission.WAKE_LOCK",
        dangerous: false,
        description: "Allows keeping the processor awake or screen on.",
    },
];

pub struct PermissionManager;

impl PermissionManager {
    pub fn build(requested_permissions: &[String]) -> Vec<PermissionRecord> {
        requested_permissions
            .iter()
            .map(|permission| Self::record_for(permission))
            .collect()
    }

    pub fn record_for(permission: &str) -> PermissionRecord {
        if let Some(known) = KNOWN_PERMISSIONS
            .iter()
            .find(|known| known.name == permission)
        {
            PermissionRecord {
                name: permission.to_string(),
                state: if known.dangerous {
                    PermissionState::AskEveryTime
                } else {
                    PermissionState::Granted
                },
                dangerous: known.dangerous,
                description: known.description.to_string(),
            }
        } else {
            PermissionRecord {
                name: permission.to_string(),
                state: PermissionState::Unsupported,
                dangerous: false,
                description: "Unknown Android permission is not modeled by APKRunner.".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_permission_classified_correctly() {
        let record = PermissionManager::record_for("android.permission.CAMERA");
        assert!(record.dangerous);
        assert_eq!(record.state, PermissionState::AskEveryTime);
    }

    #[test]
    fn unknown_permission_marked_unsupported() {
        let record = PermissionManager::record_for("com.example.UNKNOWN_PERMISSION");
        assert_eq!(record.state, PermissionState::Unsupported);
        assert!(!record.dangerous);
    }
}

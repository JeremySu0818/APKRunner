use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ApkRunnerError, ApkRunnerResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualEntry {
    pub virtual_path: String,
    pub is_directory: bool,
    pub byte_len: u64,
}

#[derive(Debug, Clone)]
pub struct VirtualFileSystem {
    sandbox_root: PathBuf,
    package_name: String,
}

impl VirtualFileSystem {
    pub fn new(sandbox_root: impl Into<PathBuf>, package_name: impl Into<String>) -> Self {
        Self {
            sandbox_root: sandbox_root.into(),
            package_name: package_name.into(),
        }
    }

    pub fn initialize_standard_dirs(&self) -> ApkRunnerResult<()> {
        for path in [
            format!("/data/data/{}/files", self.package_name),
            format!("/data/data/{}/cache", self.package_name),
            format!("/data/data/{}/shared_prefs", self.package_name),
            "/sdcard/Download".to_string(),
            "/sdcard/DCIM".to_string(),
            "/sdcard/Pictures".to_string(),
        ] {
            let host_path = self.resolve_virtual_path(&path)?;
            fs::create_dir_all(&host_path).map_err(|source| ApkRunnerError::HostIoFailure {
                path: host_path,
                reason: source.to_string(),
            })?;
        }
        Ok(())
    }

    pub fn read(&self, virtual_path: &str) -> ApkRunnerResult<Vec<u8>> {
        let host_path = self.resolve_virtual_path(virtual_path)?;
        fs::read(&host_path).map_err(|source| ApkRunnerError::HostIoFailure {
            path: host_path,
            reason: source.to_string(),
        })
    }

    pub fn write(&self, virtual_path: &str, data: &[u8]) -> ApkRunnerResult<()> {
        let host_path = self.resolve_virtual_path(virtual_path)?;
        if let Some(parent) = host_path.parent() {
            fs::create_dir_all(parent).map_err(|source| ApkRunnerError::HostIoFailure {
                path: parent.to_path_buf(),
                reason: source.to_string(),
            })?;
        }
        fs::write(&host_path, data).map_err(|source| ApkRunnerError::HostIoFailure {
            path: host_path,
            reason: source.to_string(),
        })
    }

    pub fn list(&self, virtual_path: &str) -> ApkRunnerResult<Vec<VirtualEntry>> {
        let host_path = self.resolve_virtual_path(virtual_path)?;
        let entries = fs::read_dir(&host_path).map_err(|source| ApkRunnerError::HostIoFailure {
            path: host_path.clone(),
            reason: source.to_string(),
        })?;
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| ApkRunnerError::HostIoFailure {
                path: host_path.clone(),
                reason: source.to_string(),
            })?;
            let metadata = entry
                .metadata()
                .map_err(|source| ApkRunnerError::HostIoFailure {
                    path: entry.path(),
                    reason: source.to_string(),
                })?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            result.push(VirtualEntry {
                virtual_path: format!("{}/{}", virtual_path.trim_end_matches('/'), file_name),
                is_directory: metadata.is_dir(),
                byte_len: metadata.len(),
            });
        }
        result.sort_by(|left, right| left.virtual_path.cmp(&right.virtual_path));
        Ok(result)
    }

    pub fn resolve_virtual_path(&self, virtual_path: &str) -> ApkRunnerResult<PathBuf> {
        if !virtual_path.starts_with('/') {
            return Err(ApkRunnerError::VirtualPathEscapesSandbox(
                virtual_path.to_string(),
            ));
        }

        let mut relative = PathBuf::new();
        for component in Path::new(virtual_path).components() {
            match component {
                Component::RootDir => {}
                Component::Normal(part) => relative.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(ApkRunnerError::VirtualPathEscapesSandbox(
                        virtual_path.to_string(),
                    ));
                }
            }
        }

        let host_path = self.sandbox_root.join(relative);
        if !host_path.starts_with(&self.sandbox_root) {
            return Err(ApkRunnerError::VirtualPathEscapesSandbox(
                virtual_path.to_string(),
            ));
        }
        Ok(host_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_path_maps_inside_sandbox() {
        let temp = tempfile::tempdir().expect("tempdir");
        let vfs = VirtualFileSystem::new(temp.path(), "com.example");
        let host = vfs
            .resolve_virtual_path("/data/data/com.example/files/config.json")
            .expect("path should resolve");
        assert!(host.starts_with(temp.path()));
        assert!(host.ends_with("config.json"));
    }

    #[test]
    fn path_escape_returns_structured_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let vfs = VirtualFileSystem::new(temp.path(), "com.example");
        let error = vfs
            .resolve_virtual_path("/data/data/com.example/../other")
            .expect_err("path traversal should fail");
        assert!(matches!(error, ApkRunnerError::VirtualPathEscapesSandbox(_)));
    }
}

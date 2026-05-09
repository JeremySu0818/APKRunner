use std::path::PathBuf;

use thiserror::Error;
use uuid::Uuid;

pub type ApkRunnerResult<T> = Result<T, ApkRunnerError>;

#[derive(Debug, Error)]
pub enum ApkRunnerError {
    #[error("APKRunner error: {0}")]
    General(String),
    #[error("APK loading error: {0}")]
    ApkLoadingError(String),
    #[error("APK is not a ZIP: {path}: {reason}")]
    ApkNotZip { path: PathBuf, reason: String },
    #[error("AndroidManifest.xml is missing")]
    ManifestMissing,
    #[error("manifest parse failure: {0}")]
    ManifestParseFailure(String),
    #[error("DEX file is missing")]
    DexMissing,
    #[error("invalid AXML magic: {0:#06x}")]
    InvalidAxmlMagic(u16),
    #[error("unexpected EOF in AXML")]
    AxmlUnexpectedEof,
    #[error("unsupported AXML chunk type: {0:#06x}")]
    UnsupportedAxmlChunkType(u16),
    #[error("AXML parsing error: {0}")]
    AxmlParsingError(String),
    #[error("invalid DEX magic")]
    InvalidDexMagic,
    #[error("invalid DEX checksum: expected {expected:#010x}, actual {actual:#010x}")]
    InvalidDexChecksum { expected: u32, actual: u32 },
    #[error("unexpected EOF in DEX")]
    DexUnexpectedEof,
    #[error("DEX parsing error: {0}")]
    DexParsingError(String),
    #[error("backend not available: {0}")]
    BackendNotAvailable(String),
    #[error("app is already running: {0}")]
    AppAlreadyRunning(Uuid),
    #[error("app is not running: {0}")]
    AppNotRunning(Uuid),
    #[error("runtime backend error: {0}")]
    RuntimeBackendError(String),
    #[error("virtual path escapes sandbox: {0}")]
    VirtualPathEscapesSandbox(String),
    #[error("host I/O failure at {path}: {reason}")]
    HostIoFailure { path: PathBuf, reason: String },
    #[error("permission error: {0}")]
    PermissionError(String),
    #[error("unknown permission: {0}")]
    UnknownPermission(String),
}

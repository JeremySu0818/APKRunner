use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zip::ZipArchive;

use crate::axml::{parse_axml, AxmlDocument, AxmlElement};
use crate::dex::{parse_dex_file, DexFile};
use crate::error::{ApkRunnerError, ApkRunnerResult};
use crate::permissions::{PermissionManager, PermissionRecord, PermissionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityLevel {
    Green,
    Yellow,
    Red,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnsupportedFeatureSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnsupportedFeatureSource {
    #[serde(rename = "APK")]
    Apk,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedFeature {
    pub feature: String,
    pub detail: String,
    pub severity: UnsupportedFeatureSeverity,
    pub source: UnsupportedFeatureSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeLibrarySummary {
    pub path: String,
    pub abi: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexSummary {
    pub files: Vec<String>,
    pub multidex: bool,
    pub primary_class_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkSummary {
    pub file_name: String,
    pub package_name: String,
    pub version_name: Option<String>,
    pub version_code: Option<u64>,
    pub min_sdk: Option<u32>,
    pub target_sdk: Option<u32>,
    pub launcher_activity: Option<String>,
    pub requested_permissions: Vec<PermissionRecord>,
    pub dex_files: Vec<String>,
    pub dex_class_count: u32,
    pub multidex: bool,
    pub has_resources_arsc: bool,
    pub has_native_libraries: bool,
    pub native_abis: Vec<String>,
    pub native_libraries: Vec<NativeLibrarySummary>,
    pub assets: Vec<String>,
    pub unsupported_features: Vec<UnsupportedFeature>,
    pub compatibility_level: CompatibilityLevel,
}

#[derive(Debug, Clone)]
pub struct LoadedApk {
    pub id: Uuid,
    pub host_path: PathBuf,
    pub manifest: AxmlDocument,
    pub primary_dex: DexFile,
    pub summary: ApkSummary,
}

#[derive(Debug, Clone)]
struct ManifestMetadata {
    package_name: String,
    version_name: Option<String>,
    version_code: Option<u64>,
    min_sdk: Option<u32>,
    target_sdk: Option<u32>,
    launcher_activity: Option<String>,
    requested_permissions: Vec<String>,
}

pub fn load_apk(path: impl AsRef<Path>) -> ApkRunnerResult<LoadedApk> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|source| ApkRunnerError::HostIoFailure {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;
    let mut archive = ZipArchive::new(file).map_err(|source| ApkRunnerError::ApkNotZip {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;

    let entry_names = zip_entry_names(&mut archive)?;
    let manifest_bytes = read_zip_file(&mut archive, "AndroidManifest.xml")
        .map_err(|_| ApkRunnerError::ManifestMissing)?;
    let manifest = parse_axml(&manifest_bytes).map_err(|source| match source {
        ApkRunnerError::InvalidAxmlMagic(_)
        | ApkRunnerError::AxmlUnexpectedEof
        | ApkRunnerError::UnsupportedAxmlChunkType(_) => source,
        other => ApkRunnerError::ManifestParseFailure(other.to_string()),
    })?;
    let metadata = extract_manifest_metadata(&manifest);

    let dex_files = collect_dex_files(&entry_names);
    if dex_files.is_empty() {
        return Err(ApkRunnerError::DexMissing);
    }
    let primary_dex_bytes = read_zip_file(&mut archive, "classes.dex")?;
    let primary_dex = parse_dex_file("classes.dex", &primary_dex_bytes)?;
    let native_libraries = collect_native_libraries(&entry_names);
    let native_abis = native_libraries
        .iter()
        .map(|library| library.abi.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let assets = entry_names
        .iter()
        .filter_map(|entry| entry.strip_prefix("assets/").map(ToString::to_string))
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    let has_resources_arsc = entry_names.iter().any(|entry| entry == "resources.arsc");
    let requested_permissions = PermissionManager::build(&metadata.requested_permissions);
    let unsupported_features = compatibility_features(
        &native_libraries,
        dex_files.len() > 1,
        &primary_dex,
        &requested_permissions,
    );
    let compatibility_level = compatibility_level(&unsupported_features);

    let summary = ApkSummary {
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown.apk".to_string()),
        package_name: metadata.package_name,
        version_name: metadata.version_name,
        version_code: metadata.version_code,
        min_sdk: metadata.min_sdk,
        target_sdk: metadata.target_sdk,
        launcher_activity: metadata.launcher_activity,
        requested_permissions,
        dex_files,
        dex_class_count: primary_dex.header.class_defs_size,
        multidex: entry_names
            .iter()
            .filter(|entry| entry.ends_with(".dex") && entry.starts_with("classes"))
            .count()
            > 1,
        has_resources_arsc,
        has_native_libraries: !native_libraries.is_empty(),
        native_abis,
        native_libraries,
        assets,
        unsupported_features,
        compatibility_level,
    };

    Ok(LoadedApk {
        id: Uuid::new_v4(),
        host_path: path.to_path_buf(),
        manifest,
        primary_dex,
        summary,
    })
}

fn zip_entry_names<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> ApkRunnerResult<Vec<String>> {
    let mut names = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|source| {
            ApkRunnerError::ApkLoadingError(format!("failed to read ZIP entry: {source}"))
        })?;
        names.push(file.name().to_string());
    }
    Ok(names)
}

fn read_zip_file<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> ApkRunnerResult<Vec<u8>> {
    let mut file = archive.by_name(name).map_err(|source| {
        ApkRunnerError::ApkLoadingError(format!("failed to read {name}: {source}"))
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| ApkRunnerError::ApkLoadingError(source.to_string()))?;
    Ok(bytes)
}

fn collect_dex_files(entry_names: &[String]) -> Vec<String> {
    let mut dex_files = entry_names
        .iter()
        .filter(|entry| entry.ends_with(".dex") && entry.starts_with("classes"))
        .cloned()
        .collect::<Vec<_>>();
    dex_files.sort();
    dex_files
}

fn collect_native_libraries(entry_names: &[String]) -> Vec<NativeLibrarySummary> {
    let mut libraries = entry_names
        .iter()
        .filter_map(|entry| {
            let rest = entry.strip_prefix("lib/")?;
            let (abi, name) = rest.split_once('/')?;
            if name.ends_with(".so") {
                Some(NativeLibrarySummary {
                    path: entry.clone(),
                    abi: abi.to_string(),
                    name: name.to_string(),
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    libraries.sort_by(|left, right| left.path.cmp(&right.path));
    libraries
}

fn extract_manifest_metadata(document: &AxmlDocument) -> ManifestMetadata {
    let root = &document.root;
    let package_name = root
        .attribute("package")
        .unwrap_or("unknown.package")
        .to_string();
    let version_name = root.attribute("versionName").map(ToString::to_string);
    let version_code = root
        .attribute("versionCode")
        .and_then(|value| value.parse().ok());
    let uses_sdk = root.children_named("uses-sdk").next();
    let min_sdk = uses_sdk
        .and_then(|element| element.attribute("minSdkVersion"))
        .and_then(|value| value.parse().ok());
    let target_sdk = uses_sdk
        .and_then(|element| element.attribute("targetSdkVersion"))
        .and_then(|value| value.parse().ok());
    let requested_permissions = root
        .children_named("uses-permission")
        .filter_map(|element| element.attribute("name").map(ToString::to_string))
        .collect::<Vec<_>>();
    let launcher_activity = find_launcher_activity(root, &package_name);

    ManifestMetadata {
        package_name,
        version_name,
        version_code,
        min_sdk,
        target_sdk,
        launcher_activity,
        requested_permissions,
    }
}

fn find_launcher_activity(root: &AxmlElement, package_name: &str) -> Option<String> {
    let application = root.children_named("application").next()?;
    for activity in application.children_named("activity") {
        let has_launcher_filter = activity.children_named("intent-filter").any(|filter| {
            let has_main = filter
                .children_named("action")
                .any(|action| action.attribute("name") == Some("android.intent.action.MAIN"));
            let has_launcher = filter.children_named("category").any(|category| {
                category.attribute("name") == Some("android.intent.category.LAUNCHER")
            });
            has_main && has_launcher
        });
        if has_launcher_filter {
            return activity
                .attribute("name")
                .map(|name| expand_activity_name(package_name, name));
        }
    }
    None
}

fn expand_activity_name(package_name: &str, name: &str) -> String {
    if name.starts_with('.') {
        format!("{package_name}{name}")
    } else if name.contains('.') {
        name.to_string()
    } else {
        format!("{package_name}.{name}")
    }
}

fn compatibility_features(
    native_libraries: &[NativeLibrarySummary],
    multidex: bool,
    primary_dex: &DexFile,
    permissions: &[PermissionRecord],
) -> Vec<UnsupportedFeature> {
    let mut features = Vec::new();
    let library_names = native_libraries
        .iter()
        .map(|library| library.name.as_str())
        .collect::<Vec<_>>();

    if library_names.contains(&"libflutter.so") {
        features.push(unsupported(
            "Flutter runtime",
            "libflutter.so was detected; Flutter engine execution is not implemented.",
            UnsupportedFeatureSeverity::Error,
        ));
    }
    if library_names.contains(&"libreactnativejni.so") {
        features.push(unsupported(
            "React Native runtime",
            "libreactnativejni.so was detected; React Native JNI execution is not implemented.",
            UnsupportedFeatureSeverity::Error,
        ));
    }
    if library_names.contains(&"libunity.so") || dex_contains_unity(primary_dex) {
        features.push(unsupported(
            "Unity runtime",
            "Unity native library or player class was detected; Unity runtime execution is not implemented.",
            UnsupportedFeatureSeverity::Error,
        ));
    }
    if !native_libraries.is_empty() {
        features.push(unsupported(
            "Native libraries",
            "JNI/native .so execution is not implemented by the skeleton runtime.",
            UnsupportedFeatureSeverity::Warning,
        ));
    }
    if multidex {
        features.push(unsupported(
            "Multidex",
            "Multiple DEX files were detected; only the primary DEX is summarized in v1.",
            UnsupportedFeatureSeverity::Warning,
        ));
    }
    if dex_contains_google_play_services(primary_dex) {
        features.push(unsupported(
            "Google Play Services",
            "Google Play Services dependency was detected; Play Services APIs are not implemented.",
            UnsupportedFeatureSeverity::Warning,
        ));
    }
    for permission in permissions {
        if permission.state == PermissionState::Unsupported {
            features.push(unsupported(
                "Unknown permission",
                format!("{} is not modeled by APKRunner.", permission.name),
                UnsupportedFeatureSeverity::Warning,
            ));
        }
    }
    features
}

fn unsupported(
    feature: impl Into<String>,
    detail: impl Into<String>,
    severity: UnsupportedFeatureSeverity,
) -> UnsupportedFeature {
    UnsupportedFeature {
        feature: feature.into(),
        detail: detail.into(),
        severity,
        source: UnsupportedFeatureSource::Apk,
    }
}

fn dex_contains_unity(dex: &DexFile) -> bool {
    dex.strings
        .iter()
        .any(|value| value.contains("com/unity3d/player/UnityPlayer"))
        || dex
            .classes
            .iter()
            .any(|class| class.class_name == "com.unity3d.player.UnityPlayer")
}

fn dex_contains_google_play_services(dex: &DexFile) -> bool {
    dex.strings.iter().any(|value| {
        value.contains("com/google/android/gms") || value.contains("Lcom/google/android/gms")
    })
}

fn compatibility_level(features: &[UnsupportedFeature]) -> CompatibilityLevel {
    if features
        .iter()
        .any(|feature| feature.severity == UnsupportedFeatureSeverity::Error)
    {
        CompatibilityLevel::Red
    } else if features
        .iter()
        .any(|feature| feature.severity == UnsupportedFeatureSeverity::Warning)
    {
        CompatibilityLevel::Yellow
    } else {
        CompatibilityLevel::Green
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use byteorder::{LittleEndian, WriteBytesExt};
    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::dex::minimal_valid_dex;

    const RES_STRING_POOL_TYPE: u16 = 0x0001;
    const RES_XML_TYPE: u16 = 0x0003;
    const RES_XML_START_ELEMENT_TYPE: u16 = 0x0102;
    const RES_XML_END_ELEMENT_TYPE: u16 = 0x0103;
    const UTF8_FLAG: u32 = 1 << 8;
    const NO_INDEX: u32 = u32::MAX;

    fn header(out: &mut Vec<u8>, chunk_type: u16, header_size: u16, size: u32) {
        out.write_u16::<LittleEndian>(chunk_type).expect("type");
        out.write_u16::<LittleEndian>(header_size)
            .expect("header size");
        out.write_u32::<LittleEndian>(size).expect("size");
    }

    fn string_pool(strings: &[&str]) -> Vec<u8> {
        let encoded = strings
            .iter()
            .map(|value| {
                let mut bytes = vec![value.chars().count() as u8, value.len() as u8];
                bytes.extend_from_slice(value.as_bytes());
                bytes.push(0);
                bytes
            })
            .collect::<Vec<_>>();
        let strings_start = 28 + strings.len() as u32 * 4;
        let size = strings_start + encoded.iter().map(|bytes| bytes.len() as u32).sum::<u32>();
        let mut out = Vec::new();
        header(&mut out, RES_STRING_POOL_TYPE, 28, size);
        out.write_u32::<LittleEndian>(strings.len() as u32)
            .expect("count");
        out.write_u32::<LittleEndian>(0).expect("styles");
        out.write_u32::<LittleEndian>(UTF8_FLAG).expect("flags");
        out.write_u32::<LittleEndian>(strings_start).expect("start");
        out.write_u32::<LittleEndian>(0).expect("styles start");
        let mut offset = 0u32;
        for value in &encoded {
            out.write_u32::<LittleEndian>(offset).expect("offset");
            offset += value.len() as u32;
        }
        for value in encoded {
            out.extend(value);
        }
        out
    }

    fn start(name: u32, attrs: &[(u32, u32)]) -> Vec<u8> {
        let size = 36 + attrs.len() as u32 * 20;
        let mut out = Vec::new();
        header(&mut out, RES_XML_START_ELEMENT_TYPE, 16, size);
        out.write_u32::<LittleEndian>(1).expect("line");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("comment");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("ns");
        out.write_u32::<LittleEndian>(name).expect("name");
        out.write_u16::<LittleEndian>(20).expect("attr start");
        out.write_u16::<LittleEndian>(20).expect("attr size");
        out.write_u16::<LittleEndian>(attrs.len() as u16)
            .expect("attr count");
        out.write_u16::<LittleEndian>(0).expect("id");
        out.write_u16::<LittleEndian>(0).expect("class");
        out.write_u16::<LittleEndian>(0).expect("style");
        for (name_index, value_index) in attrs {
            out.write_u32::<LittleEndian>(NO_INDEX).expect("attr ns");
            out.write_u32::<LittleEndian>(*name_index)
                .expect("attr name");
            out.write_u32::<LittleEndian>(*value_index).expect("raw");
            out.write_u16::<LittleEndian>(8).expect("typed size");
            out.push(0);
            out.push(0x03);
            out.write_u32::<LittleEndian>(*value_index).expect("data");
        }
        out
    }

    fn end(name: u32) -> Vec<u8> {
        let mut out = Vec::new();
        header(&mut out, RES_XML_END_ELEMENT_TYPE, 16, 24);
        out.write_u32::<LittleEndian>(1).expect("line");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("comment");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("ns");
        out.write_u32::<LittleEndian>(name).expect("name");
        out
    }

    fn manifest() -> Vec<u8> {
        let pool = string_pool(&["manifest", "package", "com.example"]);
        let start = start(0, &[(1, 2)]);
        let end = end(0);
        let size = 8 + pool.len() + start.len() + end.len();
        let mut out = Vec::new();
        header(&mut out, RES_XML_TYPE, 8, size as u32);
        out.extend(pool);
        out.extend(start);
        out.extend(end);
        out
    }

    fn apk_bytes(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (name, bytes) in entries {
            writer.start_file(*name, options).expect("start zip file");
            writer.write_all(bytes).expect("write zip entry");
        }
        writer.finish().expect("finish zip").into_inner()
    }

    #[test]
    fn invalid_zip_returns_structured_error() {
        let temp = tempfile::NamedTempFile::new().expect("temp apk");
        std::fs::write(temp.path(), b"not a zip").expect("write invalid zip");
        let error = load_apk(temp.path()).expect_err("invalid zip should fail");
        assert!(matches!(error, ApkRunnerError::ApkNotZip { .. }));
    }

    #[test]
    fn flutter_compatibility_detection_returns_red() {
        let bytes = apk_bytes(&[
            ("AndroidManifest.xml", manifest()),
            ("classes.dex", minimal_valid_dex()),
            ("lib/arm64-v8a/libflutter.so", vec![1, 2, 3]),
        ]);
        let temp = tempfile::NamedTempFile::new().expect("temp apk");
        std::fs::write(temp.path(), bytes).expect("write apk");
        let apk = load_apk(temp.path()).expect("apk should load");
        assert_eq!(apk.summary.compatibility_level, CompatibilityLevel::Red);
        assert!(apk
            .summary
            .unsupported_features
            .iter()
            .any(|feature| feature.feature == "Flutter runtime"));
    }
}

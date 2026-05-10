use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};

use crate::error::{ApkRunnerError, ApkRunnerResult};

const DEX_HEADER_SIZE: usize = 112;
const ENDIAN_CONSTANT: u32 = 0x1234_5678;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexFile {
    pub path: String,
    pub header: DexHeader,
    pub classes: Vec<DexClassSummary>,
    pub strings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexHeader {
    pub magic: String,
    pub checksum: u32,
    pub signature_sha1: String,
    pub file_size: u32,
    pub header_size: u32,
    pub endian_tag: u32,
    pub string_ids_size: u32,
    pub type_ids_size: u32,
    pub method_ids_size: u32,
    pub class_defs_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexClassSummary {
    pub class_name: String,
    pub method_count: u32,
}

pub fn parse_dex_file(path: impl Into<String>, bytes: &[u8]) -> ApkRunnerResult<DexFile> {
    if bytes.len() < DEX_HEADER_SIZE {
        return Err(ApkRunnerError::DexUnexpectedEof);
    }
    if &bytes[0..4] != b"dex\n" || bytes[7] != 0 {
        return Err(ApkRunnerError::InvalidDexMagic);
    }

    let expected_checksum = LittleEndian::read_u32(&bytes[8..12]);
    let actual_checksum = adler32(&bytes[12..]);
    if expected_checksum != actual_checksum {
        return Err(ApkRunnerError::InvalidDexChecksum {
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }

    let file_size = read_u32(bytes, 32)?;
    let header_size = read_u32(bytes, 36)?;
    let endian_tag = read_u32(bytes, 40)?;
    if file_size as usize > bytes.len() || header_size as usize > bytes.len() {
        return Err(ApkRunnerError::DexUnexpectedEof);
    }
    if header_size < DEX_HEADER_SIZE as u32 {
        return Err(ApkRunnerError::DexParsingError(
            "DEX header size is smaller than expected".to_string(),
        ));
    }
    if endian_tag != ENDIAN_CONSTANT {
        return Err(ApkRunnerError::DexParsingError(
            "unsupported DEX endian tag".to_string(),
        ));
    }

    let string_ids_size = read_u32(bytes, 56)?;
    let string_ids_off = read_u32(bytes, 60)?;
    let type_ids_size = read_u32(bytes, 64)?;
    let type_ids_off = read_u32(bytes, 68)?;
    let method_ids_size = read_u32(bytes, 88)?;
    let class_defs_size = read_u32(bytes, 96)?;
    let class_defs_off = read_u32(bytes, 100)?;
    let strings = parse_strings(bytes, string_ids_size, string_ids_off)?;
    let classes = parse_classes(
        bytes,
        class_defs_size,
        class_defs_off,
        type_ids_size,
        type_ids_off,
        &strings,
    )?;

    Ok(DexFile {
        path: path.into(),
        header: DexHeader {
            magic: String::from_utf8_lossy(&bytes[0..8]).to_string(),
            checksum: expected_checksum,
            signature_sha1: hex_lower(&bytes[12..32]),
            file_size,
            header_size,
            endian_tag,
            string_ids_size,
            type_ids_size,
            method_ids_size,
            class_defs_size,
        },
        classes,
        strings,
    })
}

pub fn adler32(bytes: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in bytes {
        a = (a + u32::from(*byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn read_u32(bytes: &[u8], offset: usize) -> ApkRunnerResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(ApkRunnerError::DexUnexpectedEof)?;
    if end > bytes.len() {
        return Err(ApkRunnerError::DexUnexpectedEof);
    }
    Ok(LittleEndian::read_u32(&bytes[offset..end]))
}

fn parse_strings(bytes: &[u8], size: u32, offset: u32) -> ApkRunnerResult<Vec<String>> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let table_offset = offset as usize;
    let table_bytes = (size as usize)
        .checked_mul(4)
        .ok_or(ApkRunnerError::DexUnexpectedEof)?;
    if table_offset
        .checked_add(table_bytes)
        .ok_or(ApkRunnerError::DexUnexpectedEof)?
        > bytes.len()
    {
        return Err(ApkRunnerError::DexUnexpectedEof);
    }

    let mut strings = Vec::with_capacity(size as usize);
    for index in 0..size as usize {
        let string_offset = read_u32(bytes, table_offset + index * 4)? as usize;
        strings.push(read_dex_string(bytes, string_offset)?);
    }
    Ok(strings)
}

fn read_dex_string(bytes: &[u8], offset: usize) -> ApkRunnerResult<String> {
    let mut cursor = offset;
    read_uleb128(bytes, &mut cursor)?;
    let start = cursor;
    while cursor < bytes.len() && bytes[cursor] != 0 {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return Err(ApkRunnerError::DexUnexpectedEof);
    }
    Ok(String::from_utf8_lossy(&bytes[start..cursor]).into_owned())
}

fn parse_classes(
    bytes: &[u8],
    class_defs_size: u32,
    class_defs_off: u32,
    type_ids_size: u32,
    type_ids_off: u32,
    strings: &[String],
) -> ApkRunnerResult<Vec<DexClassSummary>> {
    if class_defs_size == 0 {
        return Ok(Vec::new());
    }
    let class_table = class_defs_off as usize;
    let class_table_bytes = (class_defs_size as usize)
        .checked_mul(32)
        .ok_or(ApkRunnerError::DexUnexpectedEof)?;
    if class_table
        .checked_add(class_table_bytes)
        .ok_or(ApkRunnerError::DexUnexpectedEof)?
        > bytes.len()
    {
        return Err(ApkRunnerError::DexUnexpectedEof);
    }

    let mut classes = Vec::with_capacity(class_defs_size as usize);
    for index in 0..class_defs_size as usize {
        let class_def = class_table + index * 32;
        let class_idx = read_u32(bytes, class_def)?;
        let class_data_off = read_u32(bytes, class_def + 24)?;
        let descriptor = type_descriptor(bytes, type_ids_size, type_ids_off, strings, class_idx)?;
        let method_count = if class_data_off == 0 {
            0
        } else {
            class_data_method_count(bytes, class_data_off as usize)?
        };
        classes.push(DexClassSummary {
            class_name: descriptor_to_class_name(&descriptor),
            method_count,
        });
    }
    Ok(classes)
}

fn type_descriptor(
    bytes: &[u8],
    type_ids_size: u32,
    type_ids_off: u32,
    strings: &[String],
    type_index: u32,
) -> ApkRunnerResult<String> {
    if type_index >= type_ids_size {
        return Err(ApkRunnerError::DexParsingError(
            "class type index out of range".to_string(),
        ));
    }
    let offset = type_ids_off as usize + type_index as usize * 4;
    let descriptor_idx = read_u32(bytes, offset)? as usize;
    strings.get(descriptor_idx).cloned().ok_or_else(|| {
        ApkRunnerError::DexParsingError("descriptor string index out of range".to_string())
    })
}

fn descriptor_to_class_name(descriptor: &str) -> String {
    descriptor
        .strip_prefix('L')
        .and_then(|value| value.strip_suffix(';'))
        .unwrap_or(descriptor)
        .replace('/', ".")
}

fn class_data_method_count(bytes: &[u8], offset: usize) -> ApkRunnerResult<u32> {
    let mut cursor = offset;
    let static_fields = read_uleb128(bytes, &mut cursor)?;
    let instance_fields = read_uleb128(bytes, &mut cursor)?;
    let direct_methods = read_uleb128(bytes, &mut cursor)?;
    let virtual_methods = read_uleb128(bytes, &mut cursor)?;
    skip_encoded_fields(bytes, &mut cursor, static_fields)?;
    skip_encoded_fields(bytes, &mut cursor, instance_fields)?;
    Ok(direct_methods + virtual_methods)
}

fn skip_encoded_fields(bytes: &[u8], cursor: &mut usize, count: u32) -> ApkRunnerResult<()> {
    for _ in 0..count {
        read_uleb128(bytes, cursor)?;
        read_uleb128(bytes, cursor)?;
    }
    Ok(())
}

fn read_uleb128(bytes: &[u8], cursor: &mut usize) -> ApkRunnerResult<u32> {
    let mut result = 0u32;
    let mut shift = 0u32;
    for _ in 0..5 {
        let byte = bytes
            .get(*cursor)
            .copied()
            .ok_or(ApkRunnerError::DexUnexpectedEof)?;
        *cursor += 1;
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
    }
    Err(ApkRunnerError::DexParsingError(
        "ULEB128 value is too large".to_string(),
    ))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
pub(crate) fn minimal_valid_dex() -> Vec<u8> {
    use byteorder::WriteBytesExt;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"dex\n035\0");
    bytes.write_u32::<LittleEndian>(0).expect("checksum");
    bytes.extend_from_slice(&[0u8; 20]);
    bytes
        .write_u32::<LittleEndian>(DEX_HEADER_SIZE as u32)
        .expect("file size");
    bytes
        .write_u32::<LittleEndian>(DEX_HEADER_SIZE as u32)
        .expect("header size");
    bytes
        .write_u32::<LittleEndian>(ENDIAN_CONSTANT)
        .expect("endian");
    for _ in 0..17 {
        bytes.write_u32::<LittleEndian>(0).expect("header field");
    }
    let checksum = adler32(&bytes[12..]);
    LittleEndian::write_u32(&mut bytes[8..12], checksum);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_dex_header() {
        let dex = parse_dex_file("classes.dex", &minimal_valid_dex()).expect("dex should parse");
        assert_eq!(dex.header.file_size, DEX_HEADER_SIZE as u32);
        assert_eq!(dex.header.class_defs_size, 0);
    }

    #[test]
    fn invalid_magic_returns_structured_dex_error() {
        let mut bytes = minimal_valid_dex();
        bytes[0] = b'x';
        let error = parse_dex_file("classes.dex", &bytes).expect_err("invalid magic should fail");
        assert!(matches!(error, ApkRunnerError::InvalidDexMagic));
    }

    #[test]
    fn invalid_checksum_returns_structured_dex_error() {
        let mut bytes = minimal_valid_dex();
        bytes[20] = 1;
        let error = parse_dex_file("classes.dex", &bytes).expect_err("checksum should fail");
        assert!(matches!(error, ApkRunnerError::InvalidDexChecksum { .. }));
    }
}

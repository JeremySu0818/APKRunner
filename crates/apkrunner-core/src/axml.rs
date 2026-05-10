use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};

use crate::error::{ApkRunnerError, ApkRunnerResult};

const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_XML_TYPE: u16 = 0x0003;
const RES_XML_START_NAMESPACE_TYPE: u16 = 0x0100;
const RES_XML_END_NAMESPACE_TYPE: u16 = 0x0101;
const RES_XML_START_ELEMENT_TYPE: u16 = 0x0102;
const RES_XML_END_ELEMENT_TYPE: u16 = 0x0103;
const RES_XML_RESOURCE_MAP_TYPE: u16 = 0x0180;
const UTF8_FLAG: u32 = 1 << 8;
const NO_INDEX: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxmlDocument {
    pub root: AxmlElement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxmlElement {
    pub name: String,
    pub attributes: Vec<AxmlAttribute>,
    pub children: Vec<AxmlElement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxmlAttribute {
    pub namespace: Option<String>,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy)]
struct ChunkHeader {
    chunk_type: u16,
    header_size: usize,
    size: usize,
}

pub fn parse_axml(bytes: &[u8]) -> ApkRunnerResult<AxmlDocument> {
    let file_header = read_chunk_header(bytes, 0)?;
    if file_header.chunk_type != RES_XML_TYPE {
        return Err(ApkRunnerError::InvalidAxmlMagic(file_header.chunk_type));
    }

    let mut strings = Vec::<String>::new();
    let mut stack = Vec::<AxmlElement>::new();
    let mut root: Option<AxmlElement> = None;
    let mut offset = file_header.header_size;

    while offset < bytes.len() {
        let header = read_chunk_header(bytes, offset)?;
        let chunk_end = checked_chunk_end(bytes, offset, header.size)?;
        match header.chunk_type {
            RES_STRING_POOL_TYPE => {
                strings = parse_string_pool(bytes, offset, header)?;
            }
            RES_XML_RESOURCE_MAP_TYPE
            | RES_XML_START_NAMESPACE_TYPE
            | RES_XML_END_NAMESPACE_TYPE => {}
            RES_XML_START_ELEMENT_TYPE => {
                let element = parse_start_element(bytes, offset, chunk_end, &strings)?;
                stack.push(element);
            }
            RES_XML_END_ELEMENT_TYPE => {
                let element = stack.pop().ok_or_else(|| {
                    ApkRunnerError::AxmlParsingError(
                        "end element without matching start".to_string(),
                    )
                })?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(element);
                } else if root.is_none() {
                    root = Some(element);
                } else {
                    return Err(ApkRunnerError::AxmlParsingError(
                        "multiple root elements found".to_string(),
                    ));
                }
            }
            other => return Err(ApkRunnerError::UnsupportedAxmlChunkType(other)),
        }
        offset = chunk_end;
    }

    if !stack.is_empty() {
        return Err(ApkRunnerError::AxmlParsingError(
            "unclosed element at end of document".to_string(),
        ));
    }

    root.map(|root| AxmlDocument { root }).ok_or_else(|| {
        ApkRunnerError::AxmlParsingError("AXML document has no root element".to_string())
    })
}

impl AxmlElement {
    pub fn attribute(&self, local_name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| local_attribute_name(&attribute.name) == local_name)
            .map(|attribute| attribute.value.as_str())
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a AxmlElement> {
        self.children
            .iter()
            .filter(move |child| local_attribute_name(&child.name) == name)
    }
}

fn local_attribute_name(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, local)| local)
}

fn read_chunk_header(bytes: &[u8], offset: usize) -> ApkRunnerResult<ChunkHeader> {
    let end = offset
        .checked_add(8)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if end > bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    let chunk_type = LittleEndian::read_u16(&bytes[offset..offset + 2]);
    let header_size = LittleEndian::read_u16(&bytes[offset + 2..offset + 4]) as usize;
    let size = LittleEndian::read_u32(&bytes[offset + 4..offset + 8]) as usize;
    if header_size < 8 || size < header_size {
        return Err(ApkRunnerError::AxmlParsingError(
            "invalid chunk header size".to_string(),
        ));
    }
    Ok(ChunkHeader {
        chunk_type,
        header_size,
        size,
    })
}

fn checked_chunk_end(bytes: &[u8], offset: usize, size: usize) -> ApkRunnerResult<usize> {
    let end = offset
        .checked_add(size)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if end > bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    Ok(end)
}

fn read_u16(bytes: &[u8], offset: usize) -> ApkRunnerResult<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if end > bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    Ok(LittleEndian::read_u16(&bytes[offset..end]))
}

fn read_u32(bytes: &[u8], offset: usize) -> ApkRunnerResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if end > bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    Ok(LittleEndian::read_u32(&bytes[offset..end]))
}

fn parse_string_pool(
    bytes: &[u8],
    offset: usize,
    header: ChunkHeader,
) -> ApkRunnerResult<Vec<String>> {
    let chunk_end = checked_chunk_end(bytes, offset, header.size)?;
    let header_end = offset
        .checked_add(28)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if header_end > chunk_end {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }

    let string_count = read_u32(bytes, offset + 8)? as usize;
    let style_count = read_u32(bytes, offset + 12)? as usize;
    let flags = read_u32(bytes, offset + 16)?;
    let strings_start = read_u32(bytes, offset + 20)? as usize;
    let offsets_start = offset
        .checked_add(header.header_size)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    let data_start = offset
        .checked_add(strings_start)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if offsets_start
        .checked_add((string_count + style_count).saturating_mul(4))
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?
        > chunk_end
        || data_start > chunk_end
    {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }

    let mut strings = Vec::with_capacity(string_count);
    for index in 0..string_count {
        let string_offset = read_u32(bytes, offsets_start + index * 4)? as usize;
        let absolute = data_start
            .checked_add(string_offset)
            .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
        if absolute >= chunk_end {
            return Err(ApkRunnerError::AxmlUnexpectedEof);
        }
        let value = if flags & UTF8_FLAG != 0 {
            decode_utf8_string(&bytes[absolute..chunk_end])?
        } else {
            decode_utf16_string(&bytes[absolute..chunk_end])?
        };
        strings.push(value);
    }
    Ok(strings)
}

fn decode_utf8_string(bytes: &[u8]) -> ApkRunnerResult<String> {
    let mut offset = 0;
    read_length8(bytes, &mut offset)?;
    let byte_len = read_length8(bytes, &mut offset)?;
    let end = offset
        .checked_add(byte_len)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if end >= bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    String::from_utf8(bytes[offset..end].to_vec())
        .map_err(|source| ApkRunnerError::AxmlParsingError(source.to_string()))
}

fn decode_utf16_string(bytes: &[u8]) -> ApkRunnerResult<String> {
    let mut offset = 0;
    let unit_len = read_length16(bytes, &mut offset)?;
    let byte_len = unit_len
        .checked_mul(2)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    let end = offset
        .checked_add(byte_len)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if end > bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    let mut units = Vec::with_capacity(unit_len);
    for chunk in bytes[offset..end].chunks_exact(2) {
        units.push(LittleEndian::read_u16(chunk));
    }
    String::from_utf16(&units)
        .map_err(|source| ApkRunnerError::AxmlParsingError(source.to_string()))
}

fn read_length8(bytes: &[u8], offset: &mut usize) -> ApkRunnerResult<usize> {
    if *offset >= bytes.len() {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    let first = bytes[*offset];
    *offset += 1;
    if first & 0x80 == 0 {
        Ok(first as usize)
    } else {
        if *offset >= bytes.len() {
            return Err(ApkRunnerError::AxmlUnexpectedEof);
        }
        let second = bytes[*offset];
        *offset += 1;
        Ok((((first & 0x7f) as usize) << 8) | second as usize)
    }
}

fn read_length16(bytes: &[u8], offset: &mut usize) -> ApkRunnerResult<usize> {
    if (*offset).checked_add(2).is_none_or(|end| end > bytes.len()) {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }
    let first = LittleEndian::read_u16(&bytes[*offset..*offset + 2]);
    *offset += 2;
    if first & 0x8000 == 0 {
        Ok(first as usize)
    } else {
        if (*offset).checked_add(2).is_none_or(|end| end > bytes.len()) {
            return Err(ApkRunnerError::AxmlUnexpectedEof);
        }
        let second = LittleEndian::read_u16(&bytes[*offset..*offset + 2]);
        *offset += 2;
        Ok((((first & 0x7fff) as usize) << 16) | second as usize)
    }
}

fn parse_start_element(
    bytes: &[u8],
    offset: usize,
    chunk_end: usize,
    strings: &[String],
) -> ApkRunnerResult<AxmlElement> {
    let attr_ext_offset = offset
        .checked_add(16)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if attr_ext_offset
        .checked_add(20)
        .is_none_or(|end| end > chunk_end)
    {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }

    let name_index = read_u32(bytes, attr_ext_offset + 4)?;
    let attribute_start = read_u16(bytes, attr_ext_offset + 8)? as usize;
    let attribute_size = read_u16(bytes, attr_ext_offset + 10)? as usize;
    let attribute_count = read_u16(bytes, attr_ext_offset + 12)? as usize;
    let attributes_offset = attr_ext_offset
        .checked_add(attribute_start)
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    let attributes_end = attributes_offset
        .checked_add(attribute_size.saturating_mul(attribute_count))
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)?;
    if attribute_size < 20 || attributes_end > chunk_end {
        return Err(ApkRunnerError::AxmlUnexpectedEof);
    }

    let name = string_at(strings, name_index)?;
    let mut attributes = Vec::with_capacity(attribute_count);
    for index in 0..attribute_count {
        let attr_offset = attributes_offset + index * attribute_size;
        let namespace_index = read_u32(bytes, attr_offset)?;
        let attr_name_index = read_u32(bytes, attr_offset + 4)?;
        let raw_value_index = read_u32(bytes, attr_offset + 8)?;
        let data_type = read_byte(bytes, attr_offset + 15)?;
        let data = read_u32(bytes, attr_offset + 16)?;
        let namespace = if namespace_index == NO_INDEX {
            None
        } else {
            Some(string_at(strings, namespace_index)?)
        };
        let attr_name = string_at(strings, attr_name_index)?;
        let value = value_from_typed_data(strings, raw_value_index, data_type, data)?;
        attributes.push(AxmlAttribute {
            namespace,
            name: attr_name,
            value,
        });
    }

    Ok(AxmlElement {
        name,
        attributes,
        children: Vec::new(),
    })
}

fn read_byte(bytes: &[u8], offset: usize) -> ApkRunnerResult<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ApkRunnerError::AxmlUnexpectedEof)
}

fn string_at(strings: &[String], index: u32) -> ApkRunnerResult<String> {
    strings.get(index as usize).cloned().ok_or_else(|| {
        ApkRunnerError::AxmlParsingError(format!("string index {index} is out of range"))
    })
}

fn value_from_typed_data(
    strings: &[String],
    raw_value_index: u32,
    data_type: u8,
    data: u32,
) -> ApkRunnerResult<String> {
    if raw_value_index != NO_INDEX {
        return string_at(strings, raw_value_index);
    }
    match data_type {
        0x03 => string_at(strings, data),
        0x10 => Ok(data.to_string()),
        0x11 => Ok(format!("0x{data:x}")),
        0x12 => Ok((data != 0).to_string()),
        _ => Ok(data.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    fn push_header(out: &mut Vec<u8>, chunk_type: u16, header_size: u16, size: u32) {
        out.write_u16::<LittleEndian>(chunk_type)
            .expect("write type");
        out.write_u16::<LittleEndian>(header_size)
            .expect("write header size");
        out.write_u32::<LittleEndian>(size).expect("write size");
    }

    fn encode_utf8(value: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(value.chars().count() as u8);
        out.push(value.len() as u8);
        out.extend_from_slice(value.as_bytes());
        out.push(0);
        out
    }

    fn string_pool(strings: &[&str]) -> Vec<u8> {
        let encoded: Vec<Vec<u8>> = strings.iter().map(|value| encode_utf8(value)).collect();
        let header_size = 28u32;
        let offsets_size = (strings.len() * 4) as u32;
        let strings_start = header_size + offsets_size;
        let data_len: u32 = encoded.iter().map(|value| value.len() as u32).sum();
        let size = strings_start + data_len;
        let mut out = Vec::new();
        push_header(&mut out, RES_STRING_POOL_TYPE, 28, size);
        out.write_u32::<LittleEndian>(strings.len() as u32)
            .expect("count");
        out.write_u32::<LittleEndian>(0).expect("style count");
        out.write_u32::<LittleEndian>(UTF8_FLAG).expect("flags");
        out.write_u32::<LittleEndian>(strings_start)
            .expect("strings start");
        out.write_u32::<LittleEndian>(0).expect("styles start");
        let mut cursor = 0u32;
        for value in &encoded {
            out.write_u32::<LittleEndian>(cursor).expect("offset");
            cursor += value.len() as u32;
        }
        for value in encoded {
            out.extend_from_slice(&value);
        }
        out
    }

    fn start_element(name: u32, attributes: &[(u32, u32)]) -> Vec<u8> {
        let size = 36 + attributes.len() as u32 * 20;
        let mut out = Vec::new();
        push_header(&mut out, RES_XML_START_ELEMENT_TYPE, 16, size);
        out.write_u32::<LittleEndian>(1).expect("line");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("comment");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("ns");
        out.write_u32::<LittleEndian>(name).expect("name");
        out.write_u16::<LittleEndian>(20).expect("attr start");
        out.write_u16::<LittleEndian>(20).expect("attr size");
        out.write_u16::<LittleEndian>(attributes.len() as u16)
            .expect("attr count");
        out.write_u16::<LittleEndian>(0).expect("id index");
        out.write_u16::<LittleEndian>(0).expect("class index");
        out.write_u16::<LittleEndian>(0).expect("style index");
        for (name_index, value_index) in attributes {
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

    fn end_element(name: u32) -> Vec<u8> {
        let mut out = Vec::new();
        push_header(&mut out, RES_XML_END_ELEMENT_TYPE, 16, 24);
        out.write_u32::<LittleEndian>(1).expect("line");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("comment");
        out.write_u32::<LittleEndian>(NO_INDEX).expect("ns");
        out.write_u32::<LittleEndian>(name).expect("name");
        out
    }

    pub(crate) fn valid_manifest_axml() -> Vec<u8> {
        let pool = string_pool(&["manifest", "package", "com.example"]);
        let start = start_element(0, &[(1, 2)]);
        let end = end_element(0);
        let size = 8 + pool.len() + start.len() + end.len();
        let mut out = Vec::new();
        push_header(&mut out, RES_XML_TYPE, 8, size as u32);
        out.extend(pool);
        out.extend(start);
        out.extend(end);
        out
    }

    #[test]
    fn parses_valid_manifest_chunk() {
        let doc = parse_axml(&valid_manifest_axml()).expect("manifest should parse");
        assert_eq!(doc.root.name, "manifest");
        assert_eq!(doc.root.attribute("package"), Some("com.example"));
    }

    #[test]
    fn invalid_magic_returns_structured_error() {
        let bytes = [0x00, 0x00, 8, 0, 8, 0, 0, 0];
        let error = parse_axml(&bytes).expect_err("invalid magic should fail");
        assert!(matches!(error, ApkRunnerError::InvalidAxmlMagic(0)));
    }
}

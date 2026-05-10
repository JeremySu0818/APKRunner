use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameFormat {
    Rgba8888,
    PlaceholderText,
    Png,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePacket {
    pub surface_size: SurfaceSize,
    pub frame_format: FrameFormat,
    pub payload_base64: String,
    pub metadata: String,
    pub timestamp: String,
}

pub struct PlaceholderFrameRenderer;

impl PlaceholderFrameRenderer {
    pub fn render(width: u32, height: u32) -> FramePacket {
        let pixel_count = width.saturating_mul(height) as usize;
        let mut bytes = Vec::with_capacity(pixel_count.saturating_mul(4));
        for y in 0..height {
            for x in 0..width {
                let r = ((x.saturating_mul(255)) / width.max(1)) as u8;
                let g = ((y.saturating_mul(255)) / height.max(1)) as u8;
                bytes.extend_from_slice(&[r, g, 180, 255]);
            }
        }

        FramePacket {
            surface_size: SurfaceSize { width, height },
            frame_format: FrameFormat::Rgba8888,
            payload_base64: STANDARD.encode(bytes),
            metadata: "Deterministic APKRunner placeholder gradient".to_string(),
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

pub fn parse_png_dimensions(bytes: &[u8]) -> Option<SurfaceSize> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE {
        return None;
    }
    let ihdr_length = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
    if ihdr_length != 13 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some(SurfaceSize {
        width: u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        height: u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_ihdr_parser_extracts_width_and_height() {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&320u32.to_be_bytes());
        png.extend_from_slice(&180u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        png.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(
            parse_png_dimensions(&png),
            Some(SurfaceSize {
                width: 320,
                height: 180
            })
        );
    }
}

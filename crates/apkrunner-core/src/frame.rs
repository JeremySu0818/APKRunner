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

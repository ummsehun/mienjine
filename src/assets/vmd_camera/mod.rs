use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use encoding_rs::SHIFT_JIS;
use glam::Vec3;

const VMD_HEADER_PREFIX: &[u8] = b"Vocaloid Motion Data";
const HEADER_SIZE: usize = 30;
const MODEL_NAME_SIZE: usize = 20;
const BONE_KEYFRAME_SIZE: usize = 111;
const MORPH_KEYFRAME_SIZE: usize = 23;

#[derive(Debug, Clone, Copy)]
pub struct VmdCameraKeyframe {
    pub frame_no: u32,
    pub distance: f32,
    pub position: Vec3,
    pub rotation: Vec3,
    pub interpolation: [u8; 24],
    pub fov_deg: f32,
    pub perspective: bool,
}

#[derive(Debug, Clone)]
pub struct VmdCameraTrack {
    pub model_name: String,
    pub keyframes: Vec<VmdCameraKeyframe>,
    pub max_frame: u32,
}

pub fn parse_vmd_camera(path: &Path) -> Result<VmdCameraTrack> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read VMD: {}", path.display()))?;
    let mut cursor = Cursor::new(&bytes);
    let header = cursor
        .read_exact(HEADER_SIZE)
        .context("invalid VMD: missing header")?;
    if !header.starts_with(VMD_HEADER_PREFIX) {
        bail!(
            "invalid VMD header for camera track: {}",
            String::from_utf8_lossy(header)
        );
    }

    let model_name_bytes = cursor
        .read_exact(MODEL_NAME_SIZE)
        .context("invalid VMD: missing model name")?;
    let model_name = bytes_to_name(model_name_bytes);

    let bone_count = cursor
        .read_u32_le()
        .context("invalid VMD: missing bone keyframe count")? as usize;
    cursor
        .skip_bytes(bone_count.saturating_mul(BONE_KEYFRAME_SIZE))
        .context("invalid VMD: truncated bone section")?;

    let morph_count = cursor
        .read_u32_le()
        .context("invalid VMD: missing morph keyframe count")? as usize;
    cursor
        .skip_bytes(morph_count.saturating_mul(MORPH_KEYFRAME_SIZE))
        .context("invalid VMD: truncated morph section")?;

    let camera_count = cursor
        .read_u32_le()
        .context("invalid VMD: missing camera keyframe count")? as usize;
    let mut keyframes = Vec::with_capacity(camera_count);
    for _ in 0..camera_count {
        let frame_no = cursor
            .read_u32_le()
            .context("invalid VMD camera frame: frame_no")?;
        let distance = cursor
            .read_f32_le()
            .context("invalid VMD camera frame: distance")?;
        let position = Vec3::new(
            cursor
                .read_f32_le()
                .context("invalid VMD camera frame: pos.x")?,
            cursor
                .read_f32_le()
                .context("invalid VMD camera frame: pos.y")?,
            cursor
                .read_f32_le()
                .context("invalid VMD camera frame: pos.z")?,
        );
        let rotation = Vec3::new(
            cursor
                .read_f32_le()
                .context("invalid VMD camera frame: rot.x")?,
            cursor
                .read_f32_le()
                .context("invalid VMD camera frame: rot.y")?,
            cursor
                .read_f32_le()
                .context("invalid VMD camera frame: rot.z")?,
        );
        let interpolation_bytes = cursor
            .read_exact(24)
            .context("invalid VMD camera frame: interpolation")?;
        let mut interpolation = [0_u8; 24];
        interpolation.copy_from_slice(interpolation_bytes);
        let fov = cursor
            .read_u32_le()
            .context("invalid VMD camera frame: fov")?;
        let perspective_flag = cursor
            .read_u8()
            .context("invalid VMD camera frame: perspective")?;
        keyframes.push(VmdCameraKeyframe {
            frame_no,
            distance,
            position,
            rotation,
            interpolation,
            fov_deg: fov as f32,
            perspective: perspective_flag == 0,
        });
    }
    // Keep deterministic order for interpolation sampling.
    keyframes.sort_by_key(|frame| frame.frame_no);
    let max_frame = keyframes.last().map(|frame| frame.frame_no).unwrap_or(0);

    Ok(VmdCameraTrack {
        model_name,
        keyframes,
        max_frame,
    })
}

fn bytes_to_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let raw = &bytes[..end];
    let (decoded, _, _) = SHIFT_JIS.decode(raw);
    decoded.trim().to_owned()
}

#[derive(Debug, Clone, Copy)]
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_exact(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(len)?;
        if end > self.bytes.len() {
            return None;
        }
        let out = &self.bytes[self.offset..end];
        self.offset = end;
        Some(out)
    }

    fn read_u8(&mut self) -> Option<u8> {
        self.read_exact(1).map(|slice| slice[0])
    }

    fn read_u32_le(&mut self) -> Option<u32> {
        let slice = self.read_exact(4)?;
        Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn read_f32_le(&mut self) -> Option<f32> {
        let slice = self.read_exact(4)?;
        Some(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn skip_bytes(&mut self, len: usize) -> Option<()> {
        let _ = self.read_exact(len)?;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::SHIFT_JIS;

    #[test]
    fn parse_minimal_camera_vmd() {
        let mut data = Vec::new();
        let mut header = [0_u8; HEADER_SIZE];
        header[..VMD_HEADER_PREFIX.len()].copy_from_slice(VMD_HEADER_PREFIX);
        data.extend_from_slice(&header);
        data.extend_from_slice(&[0_u8; MODEL_NAME_SIZE]);
        data.extend_from_slice(&(0_u32).to_le_bytes()); // bones
        data.extend_from_slice(&(0_u32).to_le_bytes()); // morphs
        data.extend_from_slice(&(1_u32).to_le_bytes()); // cameras
        data.extend_from_slice(&(120_u32).to_le_bytes()); // frame
        data.extend_from_slice(&(5.0_f32).to_le_bytes()); // distance
        data.extend_from_slice(&(1.0_f32).to_le_bytes());
        data.extend_from_slice(&(2.0_f32).to_le_bytes());
        data.extend_from_slice(&(3.0_f32).to_le_bytes());
        data.extend_from_slice(&(0.1_f32).to_le_bytes());
        data.extend_from_slice(&(0.2_f32).to_le_bytes());
        data.extend_from_slice(&(0.3_f32).to_le_bytes());
        data.extend_from_slice(&[20_u8; 24]);
        data.extend_from_slice(&(45_u32).to_le_bytes());
        data.push(0);

        let path = tempfile::NamedTempFile::new().expect("tempfile");
        fs::write(path.path(), data).expect("write vmd");
        let track = parse_vmd_camera(path.path()).expect("parse camera");
        assert_eq!(track.keyframes.len(), 1);
        assert_eq!(track.max_frame, 120);
        assert!((track.keyframes[0].position.x - 1.0).abs() < 1e-6);
        assert!(track.keyframes[0].perspective);
    }

    #[test]
    fn reject_invalid_header() {
        let path = tempfile::NamedTempFile::new().expect("tempfile");
        fs::write(path.path(), b"bad").expect("write");
        assert!(parse_vmd_camera(path.path()).is_err());
    }

    #[test]
    fn bytes_to_name_decodes_shift_jis() {
        let (encoded, _, _) = SHIFT_JIS.encode("全身");
        let encoded = encoded.into_owned();
        let mut bytes = [0_u8; MODEL_NAME_SIZE];
        bytes[..encoded.len()].copy_from_slice(&encoded);
        assert_eq!(bytes_to_name(&bytes), "全身");
    }
}

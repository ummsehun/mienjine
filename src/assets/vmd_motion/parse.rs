use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use encoding_rs::SHIFT_JIS;
use glam::{Quat, Vec3};

use super::{VmdBoneFrame, VmdMorphFrame, VmdMotion};

const VMD_HEADER_PREFIX: &[u8] = b"Vocaloid Motion Data";
const HEADER_SIZE: usize = 30;
const MODEL_NAME_SIZE: usize = 20;

pub(super) fn parse_vmd_motion(path: &Path) -> Result<VmdMotion> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read VMD: {}", path.display()))?;
    let mut cursor = Cursor::new(&bytes);

    let header = cursor
        .read_exact(HEADER_SIZE)
        .context("invalid VMD: missing header")?;
    if !header.starts_with(VMD_HEADER_PREFIX) {
        bail!("invalid VMD header: {}", String::from_utf8_lossy(header));
    }

    let model_name = bytes_to_name(
        cursor
            .read_exact(MODEL_NAME_SIZE)
            .context("invalid VMD: missing model name")?,
    );

    let bone_count = cursor
        .read_u32_le()
        .context("invalid VMD: missing bone keyframe count")? as usize;
    let mut bone_frames = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        bone_frames.push(read_bone_frame(&mut cursor)?);
    }

    let morph_count = cursor
        .read_u32_le()
        .context("invalid VMD: missing morph keyframe count")? as usize;
    let mut morph_frames = Vec::with_capacity(morph_count);
    for _ in 0..morph_count {
        morph_frames.push(read_morph_frame(&mut cursor)?);
    }

    Ok(VmdMotion {
        model_name,
        bone_frames,
        morph_frames,
    })
}

fn read_bone_frame(cursor: &mut Cursor<'_>) -> Result<VmdBoneFrame> {
    let bone_name = bytes_to_name(cursor.read_exact(15).context("invalid VMD: bone name")?);
    let frame_no = cursor.read_u32_le().context("invalid VMD: frame no")?;
    let translation = Vec3::new(
        cursor.read_f32_le().context("invalid VMD: x")?,
        cursor.read_f32_le().context("invalid VMD: y")?,
        cursor.read_f32_le().context("invalid VMD: z")?,
    );
    let rotation = Quat::from_xyzw(
        cursor.read_f32_le().context("invalid VMD: qx")?,
        cursor.read_f32_le().context("invalid VMD: qy")?,
        cursor.read_f32_le().context("invalid VMD: qz")?,
        cursor.read_f32_le().context("invalid VMD: qw")?,
    );
    cursor
        .skip_bytes(64)
        .context("invalid VMD: bone interpolation")?;
    Ok(VmdBoneFrame {
        bone_name,
        frame_no,
        translation,
        rotation,
    })
}

fn read_morph_frame(cursor: &mut Cursor<'_>) -> Result<VmdMorphFrame> {
    let morph_name = bytes_to_name(cursor.read_exact(15).context("invalid VMD: morph name")?);
    let frame_no = cursor
        .read_u32_le()
        .context("invalid VMD: morph frame no")?;
    let weight = cursor.read_f32_le().context("invalid VMD: morph weight")?;
    Ok(VmdMorphFrame {
        morph_name,
        frame_no,
        weight,
    })
}

pub(super) fn bytes_to_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let (decoded, _, _) = SHIFT_JIS.decode(&bytes[..end]);
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

    fn read_u32_le(&mut self) -> Option<u32> {
        let slice = self.read_exact(4)?;
        Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn read_f32_le(&mut self) -> Option<f32> {
        let slice = self.read_exact(4)?;
        Some(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn skip_bytes(&mut self, len: usize) -> Option<()> {
        self.read_exact(len).map(|_| ())
    }
}

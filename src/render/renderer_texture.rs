use glam::Vec2;

use crate::scene::{
    CameraFocusMode, TextureCpu, TextureSamplingMode, TextureVOrigin, TextureWrapMode,
};

pub(super) fn apply_uv_transform(uv: Vec2, transform: crate::scene::UvTransform2D) -> Vec2 {
    let scaled = Vec2::new(uv.x * transform.scale[0], uv.y * transform.scale[1]);
    let (sin_t, cos_t) = transform.rotation_rad.sin_cos();
    let rotated = Vec2::new(
        scaled.x * cos_t - scaled.y * sin_t,
        scaled.x * sin_t + scaled.y * cos_t,
    );
    Vec2::new(
        rotated.x + transform.offset[0],
        rotated.y + transform.offset[1],
    )
}

pub(super) fn sample_texture_rgba(
    texture: &TextureCpu,
    uv: Vec2,
    mode: TextureSamplingMode,
    v_origin: TextureVOrigin,
    wrap_s: TextureWrapMode,
    wrap_t: TextureWrapMode,
    mip_level: usize,
) -> [f32; 4] {
    let (level_width, level_height, level_data) = texture_level(texture, mip_level);
    let wrap_u = wrap_uv(uv.x, wrap_s);
    let raw_v = match v_origin {
        TextureVOrigin::Gltf => uv.y,
        TextureVOrigin::Legacy => 1.0 - uv.y,
    };
    let wrap_v = wrap_uv(raw_v, wrap_t);
    match mode {
        TextureSamplingMode::Nearest => {
            sample_texture_nearest(level_width, level_height, level_data, wrap_u, wrap_v)
        }
        TextureSamplingMode::Bilinear => {
            sample_texture_bilinear(level_width, level_height, level_data, wrap_u, wrap_v)
        }
    }
}

pub(super) fn sample_texture_nearest(
    width: u32,
    height: u32,
    rgba8: &[u8],
    u: f32,
    v: f32,
) -> [f32; 4] {
    if width == 0 || height == 0 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let x = ((u * width as f32).floor() as i32).rem_euclid(width as i32) as u32;
    let y = ((v * height as f32).floor() as i32).rem_euclid(height as i32) as u32;
    sample_texture_texel(width, height, rgba8, x, y)
}

pub(super) fn sample_texture_bilinear(
    width: u32,
    height: u32,
    rgba8: &[u8],
    u: f32,
    v: f32,
) -> [f32; 4] {
    if width == 0 || height == 0 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let fx = u * width as f32 - 0.5;
    let fy = v * height as f32 - 0.5;
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let c00 = sample_texture_texel(
        width,
        height,
        rgba8,
        x0.rem_euclid(width as i32) as u32,
        y0.rem_euclid(height as i32) as u32,
    );
    let c10 = sample_texture_texel(
        width,
        height,
        rgba8,
        x1.rem_euclid(width as i32) as u32,
        y0.rem_euclid(height as i32) as u32,
    );
    let c01 = sample_texture_texel(
        width,
        height,
        rgba8,
        x0.rem_euclid(width as i32) as u32,
        y1.rem_euclid(height as i32) as u32,
    );
    let c11 = sample_texture_texel(
        width,
        height,
        rgba8,
        x1.rem_euclid(width as i32) as u32,
        y1.rem_euclid(height as i32) as u32,
    );
    [
        bilerp(c00[0], c10[0], c01[0], c11[0], tx, ty),
        bilerp(c00[1], c10[1], c01[1], c11[1], tx, ty),
        bilerp(c00[2], c10[2], c01[2], c11[2], tx, ty),
        bilerp(c00[3], c10[3], c01[3], c11[3], tx, ty),
    ]
}

pub(super) fn sample_texture_texel(
    width: u32,
    height: u32,
    rgba8: &[u8],
    x: u32,
    y: u32,
) -> [f32; 4] {
    let idx = (y as usize)
        .saturating_mul(width as usize)
        .saturating_add(x as usize)
        .saturating_mul(4);
    if width == 0 || height == 0 || idx + 3 >= rgba8.len() {
        return [1.0, 1.0, 1.0, 1.0];
    }
    [
        rgba8[idx] as f32 / 255.0,
        rgba8[idx + 1] as f32 / 255.0,
        rgba8[idx + 2] as f32 / 255.0,
        rgba8[idx + 3] as f32 / 255.0,
    ]
}

pub(super) fn texture_level(texture: &TextureCpu, mip_level: usize) -> (u32, u32, &[u8]) {
    if mip_level == 0 {
        return (texture.width, texture.height, texture.rgba8.as_slice());
    }
    if let Some(level) = texture.mip_levels.get(mip_level.saturating_sub(1)) {
        return (level.width, level.height, level.rgba8.as_slice());
    }
    if let Some(last) = texture.mip_levels.last() {
        return (last.width, last.height, last.rgba8.as_slice());
    }
    (texture.width, texture.height, texture.rgba8.as_slice())
}

pub(super) fn prefer_sampling_for_focus(
    mode: TextureSamplingMode,
    focus: CameraFocusMode,
) -> TextureSamplingMode {
    if matches!(focus, CameraFocusMode::Face | CameraFocusMode::Upper)
        && matches!(mode, TextureSamplingMode::Nearest)
    {
        TextureSamplingMode::Bilinear
    } else {
        mode
    }
}

pub(super) fn select_mip_level(
    texture: &TextureCpu,
    depth: f32,
    mip_bias: f32,
    focus: CameraFocusMode,
) -> usize {
    let max_level = texture.mip_levels.len();
    if max_level == 0 {
        return 0;
    }
    let focus_bias = match focus {
        CameraFocusMode::Face => -1.25,
        CameraFocusMode::Upper => -0.65,
        _ => 0.0,
    };
    let depth_term = depth.clamp(0.0, 1.0) * 6.0;
    let lod = (depth_term + mip_bias + focus_bias).clamp(0.0, max_level as f32);
    lod.floor() as usize
}

pub(super) fn wrap_uv(value: f32, mode: TextureWrapMode) -> f32 {
    match mode {
        TextureWrapMode::Repeat => value - value.floor(),
        TextureWrapMode::MirroredRepeat => {
            let whole = value.floor() as i32;
            let frac = value - value.floor();
            if whole & 1 == 0 {
                frac
            } else {
                1.0 - frac
            }
        }
        TextureWrapMode::ClampToEdge => value.clamp(0.0, 1.0 - 1.0e-6),
    }
}

pub(super) fn bilerp(c00: f32, c10: f32, c01: f32, c11: f32, tx: f32, ty: f32) -> f32 {
    let a = c00 + (c10 - c00) * tx;
    let b = c01 + (c11 - c01) * tx;
    a + (b - a) * ty
}

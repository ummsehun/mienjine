use crate::render::renderer::ThemePalette;
use crate::scene::ClarityProfile;

pub(super) fn mix_color(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
    ]
}

pub(super) fn model_color_for_intensity(intensity: f32, palette: ThemePalette) -> [u8; 3] {
    let t = intensity.clamp(0.0, 1.0);
    if t < 0.58 {
        mix_color(palette.shadow, palette.mid, t / 0.58)
    } else {
        mix_color(palette.mid, palette.highlight, (t - 0.58) / 0.42)
    }
}

pub(super) fn luminance(rgb: [f32; 3]) -> f32 {
    (rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722).clamp(0.0, 1.0)
}

pub(super) fn scale_rgb(rgb: [f32; 3], scale: f32) -> [f32; 3] {
    [
        (rgb[0] * scale).clamp(0.0, 1.0),
        (rgb[1] * scale).clamp(0.0, 1.0),
        (rgb[2] * scale).clamp(0.0, 1.0),
    ]
}

pub(super) fn to_display_rgb(rgb: [f32; 3]) -> [u8; 3] {
    [
        (linear_to_srgb(rgb[0]).clamp(0.0, 1.0) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
        (linear_to_srgb(rgb[1]).clamp(0.0, 1.0) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
        (linear_to_srgb(rgb[2]).clamp(0.0, 1.0) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8,
    ]
}

pub(super) fn color_scale_from_tonemap(base_luma: f32, target_intensity: f32) -> f32 {
    if base_luma <= 1e-4 {
        target_intensity.max(0.12)
    } else {
        (target_intensity / base_luma).clamp(0.35, 2.6)
    }
}

pub(super) fn clarity_saturation_gain(clarity: ClarityProfile) -> f32 {
    match clarity {
        ClarityProfile::Balanced => 1.00,
        ClarityProfile::Sharp => 1.04,
        ClarityProfile::Extreme => 1.10,
    }
}

pub(super) fn srgb_to_linear(c: f32) -> f32 {
    let v = c.clamp(0.0, 1.0);
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn linear_to_srgb(c: f32) -> f32 {
    let v = c.max(0.0);
    if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub(super) fn boost_saturation(rgb: [f32; 3], saturation_gain: f32) -> [f32; 3] {
    let sat = saturation_gain.clamp(0.6, 1.8);
    let l = luminance(rgb);
    [
        (l + (rgb[0] - l) * sat).clamp(0.0, 1.0),
        (l + (rgb[1] - l) * sat).clamp(0.0, 1.0),
        (l + (rgb[2] - l) * sat).clamp(0.0, 1.0),
    ]
}

use crate::scene::{CameraFocusMode, DetailProfile, RenderConfig, TextureSamplingMode};

pub(crate) fn apply_distant_subject_clarity_boost(
    config: &mut RenderConfig,
    subject_height_ratio: f32,
) {
    if !config.quality_auto_distance
        || !subject_height_ratio.is_finite()
        || subject_height_ratio <= 0.0
    {
        return;
    }
    let target = config.subject_target_height_ratio.clamp(0.20, 0.95);
    let distant_threshold = (target * 0.65).clamp(0.14, 0.52);
    let near_threshold = (target * 1.35).clamp(0.45, 0.98);

    if subject_height_ratio < distant_threshold {
        let t = ((distant_threshold - subject_height_ratio) / distant_threshold).clamp(0.0, 1.0);
        config.model_lift = (config.model_lift + 0.10 * t).clamp(0.02, 0.55);
        config.edge_accent_strength = (config.edge_accent_strength + 0.55 * t).clamp(0.0, 2.0);
        config.bg_suppression = (config.bg_suppression + 0.70 * t).clamp(0.0, 1.0);
        config.min_triangle_area_px2 = (config.min_triangle_area_px2 * (1.0 - 0.85 * t)).max(0.0);
        if t > 0.30 {
            config.triangle_stride = config.triangle_stride.saturating_sub(1).max(1);
        }
        if t > 0.70 {
            config.triangle_stride = config.triangle_stride.saturating_sub(1).max(1);
        }
        return;
    }

    if subject_height_ratio > near_threshold {
        let t = ((subject_height_ratio - near_threshold) / near_threshold).clamp(0.0, 1.0);
        config.edge_accent_strength = (config.edge_accent_strength * (1.0 - 0.4 * t)).max(0.05);
        config.bg_suppression = (config.bg_suppression + 0.10 * t).clamp(0.0, 1.0);
    }
}

pub(crate) fn apply_face_focus_detail_boost(config: &mut RenderConfig, subject_height_ratio: f32) {
    if !matches!(config.camera_focus, CameraFocusMode::Face) {
        return;
    }
    let ratio = subject_height_ratio.clamp(0.0, 1.0);
    let t = if ratio < 0.28 {
        ((0.28 - ratio) / 0.28).clamp(0.0, 1.0)
    } else {
        0.0
    };
    config.texture_mip_bias = (config.texture_mip_bias - 0.85 - 0.65 * t).clamp(-2.0, 4.0);
    config.edge_accent_strength = (config.edge_accent_strength + 0.20 + 0.30 * t).clamp(0.0, 2.0);
    config.bg_suppression = (config.bg_suppression + 0.16 + 0.22 * t).clamp(0.0, 1.0);
    if matches!(config.texture_sampling, TextureSamplingMode::Nearest) {
        config.texture_sampling = TextureSamplingMode::Bilinear;
    }
    if config.triangle_stride > 1 {
        config.triangle_stride = config.triangle_stride.saturating_sub(1);
    }
}

pub(crate) fn apply_pmx_surface_guardrails(
    config: &mut RenderConfig,
    is_pmx_scene: bool,
    subject_height_ratio: f32,
) {
    if !is_pmx_scene || !subject_height_ratio.is_finite() || subject_height_ratio <= 0.0 {
        return;
    }

    let target = config.subject_target_height_ratio.clamp(0.20, 0.95);
    let guardrail_threshold = (target * 0.92).clamp(0.35, 0.72);
    if subject_height_ratio >= guardrail_threshold {
        return;
    }

    let t = ((guardrail_threshold - subject_height_ratio) / guardrail_threshold).clamp(0.0, 1.0);
    config.triangle_stride = 1;
    config.min_triangle_area_px2 = (config.min_triangle_area_px2 * (1.0 - 0.75 * t)).max(0.0);
    config.min_triangle_area_px2 = config
        .min_triangle_area_px2
        .min((0.12 - 0.06 * t).max(0.04));
    config.edge_accent_strength = config.edge_accent_strength.min((0.26 - 0.10 * t).max(0.16));
}

pub(crate) fn apply_adaptive_quality_tuning(
    config: &mut RenderConfig,
    base_triangle_stride: usize,
    base_min_triangle_area_px2: f32,
    lod_level: usize,
    is_pmx_scene: bool,
) {
    let mut effective_lod = lod_level;
    if matches!(config.detail_profile, DetailProfile::Perf) {
        effective_lod = effective_lod.max(1);
    }
    config.triangle_stride = base_triangle_stride.max(match effective_lod {
        0 => 1,
        1 => 2,
        _ => 3,
    });
    config.min_triangle_area_px2 = base_min_triangle_area_px2.max(match effective_lod {
        0 => 0.0,
        1 => 0.6,
        _ => 1.2,
    });

    if effective_lod >= 1 {
        config.texture_sampling = TextureSamplingMode::Nearest;
    }
    if is_pmx_scene {
        config.texture_sampling = TextureSamplingMode::Bilinear;
    }
    if !is_pmx_scene && effective_lod >= 2 && matches!(config.detail_profile, DetailProfile::Perf) {
        config.material_color = false;
    }
}

pub(crate) fn jitter_scale_for_lod(lod_level: usize) -> f32 {
    match lod_level {
        0 => 1.0,
        1 => 0.65,
        _ => 0.35,
    }
}

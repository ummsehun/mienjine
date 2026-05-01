use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, PerfProfile,
    RenderBackend, RenderMode, RenderOutputMode, StageQuality, SyncPolicy, SyncSpeedMode,
    TextureSamplingMode, ThemeStyle,
};

use super::super::types::RenderDetailMode;

pub(crate) fn mode_to_text(value: RenderMode) -> String {
    match value {
        RenderMode::Ascii => "ascii",
        RenderMode::Braille => "braille",
    }
    .to_owned()
}

pub(crate) fn parse_mode_text(value: &str) -> RenderMode {
    if value.to_ascii_lowercase().starts_with("asc") {
        RenderMode::Ascii
    } else {
        RenderMode::Braille
    }
}

pub(crate) fn output_mode_to_text(value: RenderOutputMode) -> String {
    match value {
        RenderOutputMode::Text => "text",
        RenderOutputMode::Hybrid => "hybrid",
        RenderOutputMode::KittyHq => "kitty-hq",
    }
    .to_owned()
}

pub(crate) fn graphics_protocol_to_text(value: GraphicsProtocol) -> String {
    match value {
        GraphicsProtocol::Auto => "auto",
        GraphicsProtocol::Kitty => "kitty",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::None => "none",
    }
    .to_owned()
}

pub(crate) fn perf_profile_to_text(value: PerfProfile) -> String {
    match value {
        PerfProfile::Balanced => "balanced",
        PerfProfile::Cinematic => "cinematic",
        PerfProfile::Smooth => "smooth",
    }
    .to_owned()
}

pub(crate) fn detail_profile_to_text(value: DetailProfile) -> String {
    match value {
        DetailProfile::Perf => "perf",
        DetailProfile::Balanced => "balanced",
        DetailProfile::Ultra => "ultra",
    }
    .to_owned()
}

pub(crate) fn clarity_profile_to_text(value: ClarityProfile) -> String {
    match value {
        ClarityProfile::Balanced => "balanced",
        ClarityProfile::Sharp => "sharp",
        ClarityProfile::Extreme => "extreme",
    }
    .to_owned()
}

pub(crate) fn ansi_quantization_to_text(value: AnsiQuantization) -> String {
    match value {
        AnsiQuantization::Q216 => "q216",
        AnsiQuantization::Off => "off",
    }
    .to_owned()
}

pub(crate) fn backend_to_text(value: RenderBackend) -> String {
    match value {
        RenderBackend::Cpu => "cpu",
        RenderBackend::Gpu => "gpu",
    }
    .to_owned()
}

pub(crate) fn color_mode_to_text(value: ColorMode) -> String {
    match value {
        ColorMode::Mono => "mono",
        ColorMode::Ansi => "ansi",
    }
    .to_owned()
}

pub(crate) fn braille_profile_to_text(value: BrailleProfile) -> String {
    match value {
        BrailleProfile::Safe => "safe",
        BrailleProfile::Normal => "normal",
        BrailleProfile::Dense => "dense",
    }
    .to_owned()
}

pub(crate) fn theme_style_to_text(value: ThemeStyle) -> String {
    match value {
        ThemeStyle::Theater => "theater",
        ThemeStyle::Neon => "neon",
        ThemeStyle::Holo => "holo",
    }
    .to_owned()
}

pub(crate) fn center_lock_mode_to_text(value: CenterLockMode) -> String {
    match value {
        CenterLockMode::Root => "root",
        CenterLockMode::Mixed => "mixed",
    }
    .to_owned()
}

pub(crate) fn wasd_mode_to_text(value: CameraControlMode) -> String {
    match value {
        CameraControlMode::Orbit => "orbit",
        CameraControlMode::FreeFly => "freefly",
    }
    .to_owned()
}

pub(crate) fn camera_focus_to_text(value: CameraFocusMode) -> String {
    match value {
        CameraFocusMode::Auto => "auto",
        CameraFocusMode::Full => "full",
        CameraFocusMode::Upper => "upper",
        CameraFocusMode::Face => "face",
        CameraFocusMode::Hands => "hands",
    }
    .to_owned()
}

pub(crate) fn texture_sampling_to_text(value: TextureSamplingMode) -> String {
    match value {
        TextureSamplingMode::Nearest => "nearest",
        TextureSamplingMode::Bilinear => "bilinear",
    }
    .to_owned()
}

pub(crate) fn cell_aspect_mode_to_text(value: CellAspectMode) -> String {
    match value {
        CellAspectMode::Auto => "auto",
        CellAspectMode::Manual => "manual",
    }
    .to_owned()
}

pub(crate) fn contrast_profile_to_text(value: ContrastProfile) -> String {
    match value {
        ContrastProfile::Adaptive => "adaptive",
        ContrastProfile::Fixed => "fixed",
    }
    .to_owned()
}

pub(crate) fn camera_mode_to_text(value: CameraMode) -> String {
    match value {
        CameraMode::Off => "off",
        CameraMode::Vmd => "vmd",
        CameraMode::Blend => "blend",
    }
    .to_owned()
}

pub(crate) fn camera_align_preset_to_text(value: CameraAlignPreset) -> String {
    match value {
        CameraAlignPreset::Std => "std",
        CameraAlignPreset::AltA => "alt-a",
        CameraAlignPreset::AltB => "alt-b",
    }
    .to_owned()
}

pub(crate) fn sync_speed_mode_to_text(value: SyncSpeedMode) -> String {
    match value {
        SyncSpeedMode::AutoDurationFit => "auto",
        SyncSpeedMode::Realtime1x => "realtime",
    }
    .to_owned()
}

pub(crate) fn sync_policy_to_text(value: SyncPolicy) -> String {
    match value {
        SyncPolicy::Continuous => "continuous",
        SyncPolicy::Fixed => "fixed",
        SyncPolicy::Manual => "manual",
    }
    .to_owned()
}

pub(crate) fn audio_reactive_to_text(value: AudioReactiveMode) -> String {
    match value {
        AudioReactiveMode::Off => "off",
        AudioReactiveMode::On => "on",
        AudioReactiveMode::High => "high",
    }
    .to_owned()
}

pub(crate) fn cinematic_camera_to_text(value: CinematicCameraMode) -> String {
    match value {
        CinematicCameraMode::Off => "off",
        CinematicCameraMode::On => "on",
        CinematicCameraMode::Aggressive => "aggressive",
    }
    .to_owned()
}

pub(crate) fn render_detail_mode_to_text(value: RenderDetailMode) -> String {
    match value {
        RenderDetailMode::Quick => "quick",
        RenderDetailMode::Advanced => "advanced",
    }
    .to_owned()
}

pub(crate) fn parse_render_detail_mode_text(value: &str) -> RenderDetailMode {
    if value.to_ascii_lowercase().starts_with("adv") {
        RenderDetailMode::Advanced
    } else {
        RenderDetailMode::Quick
    }
}

#[allow(dead_code)]
pub(crate) fn stage_quality_to_text(value: StageQuality) -> String {
    match value {
        StageQuality::Minimal => "minimal",
        StageQuality::Low => "low",
        StageQuality::Medium => "medium",
        StageQuality::High => "high",
    }
    .to_owned()
}

#[allow(dead_code)]
pub(crate) fn parse_stage_quality_text(value: &str) -> StageQuality {
    let value = value.to_ascii_lowercase();
    if value.starts_with("min") {
        StageQuality::Minimal
    } else if value.starts_with("low") {
        StageQuality::Low
    } else if value.starts_with("high") {
        StageQuality::High
    } else {
        StageQuality::Medium
    }
}

#[cfg(feature = "gpu")]
pub(crate) fn gpu_available_once() -> bool {
    #[cfg(feature = "gpu")]
    {
        crate::render::gpu::GpuRenderer::is_available()
    }
}

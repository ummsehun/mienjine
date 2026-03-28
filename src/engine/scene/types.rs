use glam::Vec3;

pub const DEFAULT_CHARSET: &str = " .:-=+*#%@";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Ascii,
    Braille,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOutputMode {
    Text,
    Hybrid,
    KittyHq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    Auto,
    Kitty,
    Iterm2,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyTransport {
    Shm,
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyCompression {
    None,
    Zlib,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyInternalResPreset {
    R640x360,
    R854x480,
    R1280x720,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyPipelineMode {
    RealPixel,
    GlyphCompat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverStrategy {
    Hard,
    Soft,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageRole {
    Sub,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPolicy {
    Continuous,
    Fixed,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfProfile {
    Balanced,
    Cinematic,
    Smooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailProfile {
    Perf,
    Balanced,
    Ultra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterLockMode {
    Root,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraControlMode {
    Orbit,
    FreeFly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Mono,
    Ansi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrailleProfile {
    Safe,
    Normal,
    Dense,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeStyle {
    Theater,
    Neon,
    Holo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioReactiveMode {
    Off,
    On,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CinematicCameraMode {
    Off,
    On,
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraFocusMode {
    Auto,
    Full,
    Upper,
    Face,
    Hands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellAspectMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastProfile {
    Adaptive,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSpeedMode {
    AutoDurationFit,
    Realtime1x,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureSamplingMode {
    Nearest,
    Bilinear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureVOrigin {
    Gltf,
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureSamplerMode {
    Gltf,
    Override,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureColorSpace {
    Srgb,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureWrapMode {
    Repeat,
    MirroredRepeat,
    ClampToEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilterMode {
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Off,
    Vmd,
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraAlignPreset {
    Std,
    AltA,
    AltB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClarityProfile {
    Balanced,
    Sharp,
    Extreme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiQuantization {
    Q216,
    Off,
}

#[derive(Debug, Clone, Copy)]
pub struct FreeFlyState {
    pub eye: Vec3,
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
}

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub fov_deg: f32,
    pub near: f32,
    pub far: f32,
    pub mode: RenderMode,
    pub output_mode: RenderOutputMode,
    pub graphics_protocol: GraphicsProtocol,
    pub kitty_transport: KittyTransport,
    pub kitty_compression: KittyCompression,
    pub kitty_internal_res: KittyInternalResPreset,
    pub kitty_pipeline_mode: KittyPipelineMode,
    pub recover_strategy: RecoverStrategy,
    pub kitty_scale: f32,
    pub hq_target_fps: u32,
    pub subject_exposure_only: bool,
    pub subject_target_height_ratio: f32,
    pub subject_target_width_ratio: f32,
    pub quality_auto_distance: bool,
    pub texture_mip_bias: f32,
    pub stage_as_sub_only: bool,
    pub stage_role: StageRole,
    pub stage_luma_cap: f32,
    pub recover_color_auto: bool,
    pub perf_profile: PerfProfile,
    pub detail_profile: DetailProfile,
    pub backend: RenderBackend,
    pub color_mode: ColorMode,
    pub ascii_force_color: bool,
    pub braille_profile: BrailleProfile,
    pub theme_style: ThemeStyle,
    pub audio_reactive: AudioReactiveMode,
    pub cinematic_camera: CinematicCameraMode,
    pub camera_focus: CameraFocusMode,
    pub reactive_gain: f32,
    pub reactive_pulse: f32,
    pub exposure_bias: f32,
    pub center_lock: bool,
    pub center_lock_mode: CenterLockMode,
    pub stage_level: u8,
    pub stage_reactive: bool,
    pub material_color: bool,
    pub texture_sampling: TextureSamplingMode,
    pub texture_v_origin: TextureVOrigin,
    pub texture_sampler: TextureSamplerMode,
    pub clarity_profile: ClarityProfile,
    pub ansi_quantization: AnsiQuantization,
    pub model_lift: f32,
    pub edge_accent_strength: f32,
    pub bg_suppression: f32,
    pub braille_aspect_compensation: f32,
    pub charset: String,
    pub cell_aspect: f32,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub fps_cap: u32,
    pub ambient: f32,
    pub diffuse_strength: f32,
    pub specular_strength: f32,
    pub specular_power: f32,
    pub rim_strength: f32,
    pub rim_power: f32,
    pub fog_strength: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_policy: SyncPolicy,
    pub sync_hard_snap_ms: u32,
    pub sync_kp: f32,
    pub contrast_floor: f32,
    pub contrast_gamma: f32,
    pub fog_scale: f32,
    pub triangle_stride: usize,
    pub min_triangle_area_px2: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            fov_deg: 60.0,
            near: 0.1,
            far: 100.0,
            mode: RenderMode::Ascii,
            output_mode: RenderOutputMode::Text,
            graphics_protocol: GraphicsProtocol::Auto,
            kitty_transport: KittyTransport::Shm,
            kitty_compression: KittyCompression::None,
            kitty_internal_res: KittyInternalResPreset::R640x360,
            kitty_pipeline_mode: KittyPipelineMode::RealPixel,
            recover_strategy: RecoverStrategy::Hard,
            kitty_scale: 1.0,
            hq_target_fps: 24,
            subject_exposure_only: true,
            subject_target_height_ratio: 0.66,
            subject_target_width_ratio: 0.42,
            quality_auto_distance: true,
            texture_mip_bias: 0.0,
            stage_as_sub_only: true,
            stage_role: StageRole::Sub,
            stage_luma_cap: 0.35,
            recover_color_auto: true,
            perf_profile: PerfProfile::Balanced,
            detail_profile: DetailProfile::Balanced,
            backend: RenderBackend::Cpu,
            color_mode: ColorMode::Mono,
            ascii_force_color: true,
            braille_profile: BrailleProfile::Safe,
            theme_style: ThemeStyle::Theater,
            audio_reactive: AudioReactiveMode::On,
            cinematic_camera: CinematicCameraMode::On,
            camera_focus: CameraFocusMode::Auto,
            reactive_gain: 0.35,
            reactive_pulse: 0.0,
            exposure_bias: 0.0,
            center_lock: true,
            center_lock_mode: CenterLockMode::Root,
            stage_level: 2,
            stage_reactive: true,
            material_color: true,
            texture_sampling: TextureSamplingMode::Nearest,
            texture_v_origin: TextureVOrigin::Gltf,
            texture_sampler: TextureSamplerMode::Gltf,
            clarity_profile: ClarityProfile::Sharp,
            ansi_quantization: AnsiQuantization::Q216,
            model_lift: 0.12,
            edge_accent_strength: 0.32,
            bg_suppression: 0.35,
            braille_aspect_compensation: 1.00,
            charset: DEFAULT_CHARSET.to_owned(),
            cell_aspect: 0.5,
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.0,
            fps_cap: 30,
            ambient: 0.12,
            diffuse_strength: 0.95,
            specular_strength: 0.25,
            specular_power: 24.0,
            rim_strength: 0.22,
            rim_power: 2.0,
            fog_strength: 0.20,
            contrast_profile: ContrastProfile::Adaptive,
            sync_policy: SyncPolicy::Continuous,
            sync_hard_snap_ms: 120,
            sync_kp: 0.15,
            contrast_floor: 0.10,
            contrast_gamma: 0.90,
            fog_scale: 1.0,
            triangle_stride: 1,
            min_triangle_area_px2: 0.0,
        }
    }
}

pub fn kitty_internal_resolution(preset: KittyInternalResPreset) -> (u16, u16) {
    match preset {
        KittyInternalResPreset::R640x360 => (640, 360),
        KittyInternalResPreset::R854x480 => (854, 480),
        KittyInternalResPreset::R1280x720 => (1280, 720),
    }
}

pub fn estimate_cell_aspect_from_window(
    columns: u16,
    rows: u16,
    width_px: u16,
    height_px: u16,
) -> Option<f32> {
    if columns == 0 || rows == 0 || width_px == 0 || height_px == 0 {
        return None;
    }
    let cell_w = (width_px as f32) / (columns as f32);
    let cell_h = (height_px as f32) / (rows as f32);
    if cell_h <= f32::EPSILON {
        return None;
    }
    Some(cell_w / cell_h)
}

pub fn resolve_cell_aspect(config: &RenderConfig, detected: Option<f32>) -> f32 {
    let trim = config.cell_aspect_trim.clamp(0.70, 1.30);
    match config.cell_aspect_mode {
        CellAspectMode::Manual => config.cell_aspect.clamp(0.30, 1.20),
        CellAspectMode::Auto => detected
            .map(|value| (value * trim).clamp(0.30, 1.20))
            .unwrap_or_else(|| config.cell_aspect.clamp(0.30, 1.20)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialAlphaMode {
    Opaque,
    Mask,
    Blend,
}

#[derive(Debug, Clone, Copy)]
pub struct UvTransform2D {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation_rad: f32,
    pub tex_coord_override: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_cell_aspect_formula_matches_expected_ratio() {
        let aspect = estimate_cell_aspect_from_window(120, 40, 1920, 1200)
            .expect("aspect should be available");
        let expected = (1920.0 / 120.0) / (1200.0 / 40.0);
        assert!((aspect - expected).abs() < 1e-6);
    }

    #[test]
    fn resolve_cell_aspect_auto_uses_detected_value_and_trim() {
        let mut cfg = RenderConfig {
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.10,
            ..RenderConfig::default()
        };
        let value = resolve_cell_aspect(&cfg, Some(0.82));
        assert!((value - 0.902).abs() < 1e-3);

        cfg.cell_aspect_trim = 2.0;
        let clamped = resolve_cell_aspect(&cfg, Some(1.0));
        assert!((clamped - 1.2).abs() < 1e-6);
    }

    #[test]
    fn resolve_cell_aspect_manual_ignores_detected() {
        let cfg = RenderConfig {
            cell_aspect_mode: CellAspectMode::Manual,
            cell_aspect: 0.58,
            ..RenderConfig::default()
        };
        let value = resolve_cell_aspect(&cfg, Some(0.91));
        assert!((value - 0.58).abs() < 1e-6);
    }
}

use std::path::PathBuf;

use crate::scene::DEFAULT_CHARSET;

use super::enums::{
    AnsiQuantizationArg, AudioReactiveArg, BackendArg, BrailleProfileArg, CameraAlignPresetArg,
    CameraFocusArg, CameraModeArg, CellAspectModeArg, CenterLockModeArg, CinematicCameraArg,
    ClarityProfileArg, ColorModeArg, ContrastProfileArg, DetailProfileArg, GraphicsProtocolArg,
    KittyCompressionArg, KittyInternalResArg, KittyPipelineArg, KittyTransportArg, ModeArg,
    OutputModeArg, PerfProfileArg, RecoverColorArg, RecoverStrategyArg, StageRoleArg,
    SyncPolicyArg, SyncProfileModeArg, SyncSpeedModeArg, TextureSamplerModeArg, TextureSamplingArg,
    TextureVOriginArg, ThemeArg, ToggleArg, WasdModeArg,
};

#[derive(Debug, clap::Args)]
pub struct StartArgs {
    #[arg(long, default_value = "assets/glb")]
    pub dir: PathBuf,
    #[arg(long, default_value = "assets/pmx")]
    pub pmx_dir: PathBuf,
    #[arg(long, default_value = "assets/vmd")]
    pub motion_dir: PathBuf,
    #[arg(long, default_value = "assets/music")]
    pub music_dir: PathBuf,
    #[arg(long, default_value = "assets/stage")]
    pub stage_dir: PathBuf,
    #[arg(long)]
    pub stage: Option<String>,
    #[arg(long)]
    pub anim: Option<String>,
    #[arg(long, default_value = "assets/camera")]
    pub camera_dir: PathBuf,
    #[arg(long)]
    pub camera: Option<String>,
    #[arg(long)]
    pub camera_vmd: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub camera_mode: Option<CameraModeArg>,
    #[arg(long, value_enum)]
    pub camera_align_preset: Option<CameraAlignPresetArg>,
    #[arg(long)]
    pub camera_unit_scale: Option<f32>,
    #[arg(long)]
    pub camera_vmd_fps: Option<f32>,
    #[arg(long, value_enum, default_value_t = ModeArg::Braille)]
    pub mode: ModeArg,
    #[arg(long, value_enum)]
    pub output_mode: Option<OutputModeArg>,
    #[arg(long, value_enum)]
    pub kitty_transport: Option<KittyTransportArg>,
    #[arg(long, value_enum)]
    pub kitty_compression: Option<KittyCompressionArg>,
    #[arg(long, value_enum)]
    pub kitty_internal_res: Option<KittyInternalResArg>,
    #[arg(long, value_enum)]
    pub kitty_pipeline: Option<KittyPipelineArg>,
    #[arg(long, value_enum)]
    pub recover_strategy: Option<RecoverStrategyArg>,
    #[arg(long)]
    pub kitty_scale: Option<f32>,
    #[arg(long)]
    pub hq_target_fps: Option<u32>,
    #[arg(long)]
    pub subject_target_height: Option<f32>,
    #[arg(long)]
    pub subject_target_width: Option<f32>,
    #[arg(long, value_enum)]
    pub quality_auto_distance: Option<ToggleArg>,
    #[arg(long)]
    pub texture_mip_bias: Option<f32>,
    #[arg(long, value_enum)]
    pub stage_sub_only: Option<ToggleArg>,
    #[arg(long, value_enum)]
    pub subject_exposure_only: Option<ToggleArg>,
    #[arg(long, value_enum)]
    pub stage_role: Option<StageRoleArg>,
    #[arg(long)]
    pub stage_luma_cap: Option<f32>,
    #[arg(long, value_enum)]
    pub recover_color: Option<RecoverColorArg>,
    #[arg(long, value_enum)]
    pub graphics_protocol: Option<GraphicsProtocolArg>,
    #[arg(long, value_enum)]
    pub color_mode: Option<ColorModeArg>,
    #[arg(long, value_enum)]
    pub ascii_force_color: Option<ToggleArg>,
    #[arg(long, value_enum)]
    pub braille_profile: Option<BrailleProfileArg>,
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    #[arg(long, value_enum)]
    pub audio_reactive: Option<AudioReactiveArg>,
    #[arg(long, value_enum)]
    pub cinematic_camera: Option<CinematicCameraArg>,
    #[arg(long)]
    pub reactive_gain: Option<f32>,
    #[arg(long, value_enum)]
    pub perf_profile: Option<PerfProfileArg>,
    #[arg(long, value_enum)]
    pub detail_profile: Option<DetailProfileArg>,
    #[arg(long, value_enum)]
    pub clarity_profile: Option<ClarityProfileArg>,
    #[arg(long, value_enum)]
    pub ansi_quantization: Option<AnsiQuantizationArg>,
    #[arg(long, value_enum)]
    pub backend: Option<BackendArg>,
    #[arg(long, value_enum)]
    pub center_lock: Option<ToggleArg>,
    #[arg(long, value_enum)]
    pub center_lock_mode: Option<CenterLockModeArg>,
    #[arg(long, value_enum)]
    pub wasd_mode: Option<WasdModeArg>,
    #[arg(long)]
    pub freefly_speed: Option<f32>,
    #[arg(long)]
    pub camera_look_speed: Option<f32>,
    #[arg(long, value_enum)]
    pub camera_focus: Option<CameraFocusArg>,
    #[arg(long, value_enum)]
    pub material_color: Option<ToggleArg>,
    #[arg(long, value_enum)]
    pub texture_sampling: Option<TextureSamplingArg>,
    #[arg(long, value_enum)]
    pub texture_v_origin: Option<TextureVOriginArg>,
    #[arg(long, value_enum)]
    pub texture_sampler: Option<TextureSamplerModeArg>,
    #[arg(long)]
    pub stage_level: Option<u8>,
    #[arg(long)]
    pub exposure_bias: Option<f32>,
    #[arg(long)]
    pub model_lift: Option<f32>,
    #[arg(long)]
    pub edge_accent_strength: Option<f32>,
    #[arg(long, default_value_t = 20)]
    pub fps_cap: u32,
    #[arg(long, default_value_t = 0.5)]
    pub cell_aspect: f32,
    #[arg(long, value_enum)]
    pub cell_aspect_mode: Option<CellAspectModeArg>,
    #[arg(long)]
    pub cell_aspect_trim: Option<f32>,
    #[arg(long, value_enum)]
    pub contrast_profile: Option<ContrastProfileArg>,
    #[arg(long)]
    pub sync_offset_ms: Option<i32>,
    #[arg(long, value_enum)]
    pub sync_speed_mode: Option<SyncSpeedModeArg>,
    #[arg(long, value_enum)]
    pub sync_policy: Option<SyncPolicyArg>,
    #[arg(long)]
    pub sync_profile_dir: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub sync_profile_mode: Option<SyncProfileModeArg>,
    #[arg(long)]
    pub sync_profile_key: Option<String>,
    #[arg(long)]
    pub sync_hard_snap_ms: Option<u32>,
    #[arg(long)]
    pub sync_kp: Option<f32>,
    #[arg(long, default_value_t = 60.0)]
    pub fov_deg: f32,
    #[arg(long, default_value_t = 0.1)]
    pub near: f32,
    #[arg(long, default_value_t = 100.0)]
    pub far: f32,
    #[arg(long, default_value_t = 0.12)]
    pub ambient: f32,
    #[arg(long, default_value_t = 0.95)]
    pub diffuse_strength: f32,
    #[arg(long, default_value_t = 0.25)]
    pub specular_strength: f32,
    #[arg(long, default_value_t = 24.0)]
    pub specular_power: f32,
    #[arg(long, default_value_t = 0.22)]
    pub rim_strength: f32,
    #[arg(long, default_value_t = 2.0)]
    pub rim_power: f32,
    #[arg(long, default_value_t = 0.20)]
    pub fog_strength: f32,
    #[arg(long, default_value_t = 0.0)]
    pub orbit_speed: f32,
    #[arg(long, default_value_t = 0.0)]
    pub orbit_radius: f32,
    #[arg(long, default_value_t = 0.0)]
    pub camera_height: f32,
    #[arg(long, default_value_t = 0.0)]
    pub look_at_y: f32,
    #[arg(long, default_value = DEFAULT_CHARSET)]
    pub charset: String,
}

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::scene::DEFAULT_CHARSET;

#[derive(Debug, Parser)]
#[command(name = "terminal-miku3d")]
#[command(
    about = "Terminal renderer for ASCII/Braille 3D scenes (CPU backend; GPU option falls back)",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start with setup TUI (model/music/mode/fps) and run selected GLB scene.
    Start(StartArgs),
    /// Run interactive terminal rendering for cube/OBJ/GLB/PMX scenes.
    Run(RunArgs),
    /// Launch web preview server (Three.js reference path).
    Preview(PreviewArgs),
    /// Preprocess GLB textures (upscale/sharpen) and write optimized output.
    Preprocess(PreprocessArgs),
    /// Benchmark a scene pipeline without terminal presentation.
    Bench(BenchArgs),
    /// Inspect GLB/glTF scene structure.
    Inspect(InspectArgs),
}

#[derive(Debug, clap::Args)]
pub struct StartArgs {
    /// Directory to scan for .glb/.gltf files.
    #[arg(long, default_value = "assets/glb")]
    pub dir: PathBuf,
    /// Directory to scan for .pmx files (recursively).
    #[arg(long, default_value = "assets/pmx")]
    pub pmx_dir: PathBuf,
    /// Directory to scan for PMX motion .vmd files.
    #[arg(long, default_value = "assets/vmd")]
    pub motion_dir: PathBuf,
    /// Directory to scan for .mp3/.wav files.
    #[arg(long, default_value = "assets/music")]
    pub music_dir: PathBuf,
    /// Directory to scan for stage directories.
    #[arg(long, default_value = "assets/stage")]
    pub stage_dir: PathBuf,
    /// Stage selector: none | auto | <stage-name> | <path>
    #[arg(long)]
    pub stage: Option<String>,
    /// Animation selector by name or index. Defaults to first clip if available.
    #[arg(long)]
    pub anim: Option<String>,
    /// Directory to scan for camera .vmd files.
    #[arg(long, default_value = "assets/camera")]
    pub camera_dir: PathBuf,
    /// Camera selector: none | auto | <name> | <path>
    #[arg(long)]
    pub camera: Option<String>,
    /// Optional camera VMD track path.
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
    /// Directory for sync profile storage (`profiles.json` inside this dir).
    #[arg(long)]
    pub sync_profile_dir: Option<PathBuf>,
    /// Sync profile mode.
    #[arg(long, value_enum)]
    pub sync_profile_mode: Option<SyncProfileModeArg>,
    /// Optional override key for sync profile lookup.
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

#[derive(Debug, clap::Args)]
pub struct RunArgs {
    #[arg(long, value_enum, default_value_t = RunSceneArg::Glb)]
    pub scene: RunSceneArg,
    /// Path to .glb or .gltf file (required for --scene glb).
    #[arg(long)]
    pub glb: Option<PathBuf>,
    /// Path to .obj file (required for --scene obj).
    #[arg(long)]
    pub obj: Option<PathBuf>,
    /// Path to .pmx file (required for --scene pmx).
    #[arg(long)]
    pub pmx: Option<PathBuf>,
    /// Directory to scan for stage directories.
    #[arg(long, default_value = "assets/stage")]
    pub stage_dir: PathBuf,
    /// Stage selector: none | auto | <stage-name> | <path>
    #[arg(long)]
    pub stage: Option<String>,
    /// Animation selector by name or index.
    #[arg(long)]
    pub anim: Option<String>,
    /// Directory to scan for camera .vmd files.
    #[arg(long, default_value = "assets/camera")]
    pub camera_dir: PathBuf,
    /// Camera selector: none | auto | <name> | <path>
    #[arg(long)]
    pub camera: Option<String>,
    /// Optional camera VMD track path.
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
    /// Directory for sync profile storage (`profiles.json` inside this dir).
    #[arg(long)]
    pub sync_profile_dir: Option<PathBuf>,
    /// Sync profile mode.
    #[arg(long, value_enum)]
    pub sync_profile_mode: Option<SyncProfileModeArg>,
    /// Optional override key for sync profile lookup.
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
    #[arg(long, default_value_t = 0.55)]
    pub orbit_speed: f32,
    #[arg(long, default_value_t = 4.0)]
    pub orbit_radius: f32,
    #[arg(long, default_value_t = 1.2)]
    pub camera_height: f32,
    #[arg(long, default_value_t = 1.0)]
    pub look_at_y: f32,
    #[arg(long, default_value = DEFAULT_CHARSET)]
    pub charset: String,
}

#[derive(Debug, clap::Args)]
pub struct BenchArgs {
    #[arg(long, value_enum)]
    pub scene: BenchSceneArg,
    #[arg(long)]
    pub glb: Option<PathBuf>,
    #[arg(long)]
    pub obj: Option<PathBuf>,
    #[arg(long)]
    pub anim: Option<String>,
    #[arg(long, default_value_t = 10.0)]
    pub seconds: f32,
    #[arg(long, value_enum, default_value_t = ModeArg::Ascii)]
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
    #[arg(long, default_value_t = 0.5)]
    pub cell_aspect: f32,
    #[arg(long, value_enum)]
    pub cell_aspect_mode: Option<CellAspectModeArg>,
    #[arg(long)]
    pub cell_aspect_trim: Option<f32>,
    #[arg(long, value_enum)]
    pub contrast_profile: Option<ContrastProfileArg>,
    #[arg(long, default_value_t = 120)]
    pub width: u16,
    #[arg(long, default_value_t = 40)]
    pub height: u16,
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
    #[arg(long, default_value = DEFAULT_CHARSET)]
    pub charset: String,
}

#[derive(Debug, clap::Args)]
pub struct PreviewArgs {
    /// Path to .glb or .gltf.
    #[arg(long)]
    pub glb: PathBuf,
    /// Optional animation selector by name or index.
    #[arg(long)]
    pub anim: Option<String>,
    /// Optional camera VMD track path.
    #[arg(long)]
    pub camera_vmd: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = CameraModeArg::Vmd)]
    pub camera_mode: CameraModeArg,
    #[arg(long, value_enum, default_value_t = CameraAlignPresetArg::Std)]
    pub camera_align_preset: CameraAlignPresetArg,
    #[arg(long, default_value_t = 0.08)]
    pub camera_unit_scale: f32,
    #[arg(long, default_value_t = 30.0)]
    pub camera_vmd_fps: f32,
    #[arg(long, default_value_t = 8787)]
    pub port: u16,
}

#[derive(Debug, clap::Args)]
pub struct PreprocessArgs {
    /// Preprocess preset.
    #[arg(long, value_enum, default_value_t = PreprocessPresetArg::Default)]
    pub preset: PreprocessPresetArg,
    /// Input GLB file.
    #[arg(long)]
    pub glb: PathBuf,
    /// Output GLB file path.
    #[arg(long)]
    pub out: PathBuf,
    /// Texture upscale factor.
    #[arg(long, default_value_t = 2)]
    pub upscale_factor: u32,
    /// Optional unsharp strength (0.0 disables).
    #[arg(long, default_value_t = 0.20)]
    pub upscale_sharpen: f32,
}

#[derive(Debug, clap::Args)]
pub struct InspectArgs {
    /// Path to .glb or .gltf file.
    #[arg(long)]
    pub glb: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum PreprocessPresetArg {
    #[value(name = "default")]
    Default,
    #[value(name = "web-parity")]
    WebParity,
    #[value(name = "face-priority")]
    FacePriority,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BenchSceneArg {
    Cube,
    Obj,
    GlbStatic,
    GlbAnim,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RunSceneArg {
    Cube,
    Obj,
    Glb,
    Pmx,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ModeArg {
    Ascii,
    Braille,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CellAspectModeArg {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ContrastProfileArg {
    Adaptive,
    Fixed,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SyncSpeedModeArg {
    Auto,
    Realtime,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SyncPolicyArg {
    Continuous,
    Fixed,
    Manual,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SyncProfileModeArg {
    Auto,
    Off,
    Write,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputModeArg {
    Text,
    Hybrid,
    #[value(name = "kitty-hq", alias = "graphics")]
    KittyHq,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KittyTransportArg {
    Shm,
    Direct,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KittyCompressionArg {
    None,
    Zlib,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KittyInternalResArg {
    #[value(name = "640x360")]
    R640x360,
    #[value(name = "854x480")]
    R854x480,
    #[value(name = "1280x720")]
    R1280x720,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KittyPipelineArg {
    #[value(name = "real")]
    RealPixel,
    #[value(name = "glyph")]
    GlyphCompat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RecoverStrategyArg {
    Hard,
    Soft,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StageRoleArg {
    Sub,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RecoverColorArg {
    Auto,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GraphicsProtocolArg {
    Auto,
    Kitty,
    #[value(name = "iterm2")]
    Iterm2,
    None,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ColorModeArg {
    Mono,
    Ansi,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BrailleProfileArg {
    Safe,
    Normal,
    Dense,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ThemeArg {
    Theater,
    Neon,
    Holo,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AudioReactiveArg {
    Off,
    On,
    High,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CinematicCameraArg {
    Off,
    On,
    Aggressive,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PerfProfileArg {
    Balanced,
    Cinematic,
    Smooth,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DetailProfileArg {
    Perf,
    Balanced,
    Ultra,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ClarityProfileArg {
    Balanced,
    Sharp,
    Extreme,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AnsiQuantizationArg {
    Q216,
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CenterLockModeArg {
    Root,
    Mixed,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CameraFocusArg {
    Auto,
    Full,
    Upper,
    Face,
    Hands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WasdModeArg {
    Orbit,
    Freefly,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CameraModeArg {
    Off,
    Vmd,
    Blend,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CameraAlignPresetArg {
    Std,
    #[value(name = "alt-a")]
    AltA,
    #[value(name = "alt-b")]
    AltB,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TextureSamplingArg {
    Nearest,
    Bilinear,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TextureVOriginArg {
    Gltf,
    Legacy,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TextureSamplerModeArg {
    Gltf,
    Override,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BackendArg {
    Cpu,
    #[value(name = "gpu", alias = "gpu-preview")]
    Gpu,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ToggleArg {
    On,
    Off,
}

pub fn parse() -> Cli {
    Cli::parse()
}

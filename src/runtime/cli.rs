use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::runtime::sync_profile::SyncProfileMode;
use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DEFAULT_CHARSET, DetailProfile, GraphicsProtocol,
    KittyCompression, KittyInternalResPreset, KittyPipelineMode, KittyTransport, PerfProfile,
    RecoverStrategy, RenderBackend, RenderMode, RenderOutputMode, StageRole, SyncPolicy,
    SyncSpeedMode, TextureSamplerMode, TextureSamplingMode, TextureVOrigin, ThemeStyle,
};

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

impl From<ModeArg> for RenderMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Ascii => RenderMode::Ascii,
            ModeArg::Braille => RenderMode::Braille,
        }
    }
}

impl From<CellAspectModeArg> for CellAspectMode {
    fn from(value: CellAspectModeArg) -> Self {
        match value {
            CellAspectModeArg::Auto => CellAspectMode::Auto,
            CellAspectModeArg::Manual => CellAspectMode::Manual,
        }
    }
}

impl From<ContrastProfileArg> for ContrastProfile {
    fn from(value: ContrastProfileArg) -> Self {
        match value {
            ContrastProfileArg::Adaptive => ContrastProfile::Adaptive,
            ContrastProfileArg::Fixed => ContrastProfile::Fixed,
        }
    }
}

impl From<SyncSpeedModeArg> for SyncSpeedMode {
    fn from(value: SyncSpeedModeArg) -> Self {
        match value {
            SyncSpeedModeArg::Auto => SyncSpeedMode::AutoDurationFit,
            SyncSpeedModeArg::Realtime => SyncSpeedMode::Realtime1x,
        }
    }
}

impl From<SyncPolicyArg> for SyncPolicy {
    fn from(value: SyncPolicyArg) -> Self {
        match value {
            SyncPolicyArg::Continuous => SyncPolicy::Continuous,
            SyncPolicyArg::Fixed => SyncPolicy::Fixed,
            SyncPolicyArg::Manual => SyncPolicy::Manual,
        }
    }
}

impl From<SyncProfileModeArg> for SyncProfileMode {
    fn from(value: SyncProfileModeArg) -> Self {
        match value {
            SyncProfileModeArg::Auto => SyncProfileMode::Auto,
            SyncProfileModeArg::Off => SyncProfileMode::Off,
            SyncProfileModeArg::Write => SyncProfileMode::Write,
        }
    }
}

impl From<OutputModeArg> for RenderOutputMode {
    fn from(value: OutputModeArg) -> Self {
        match value {
            OutputModeArg::Text => RenderOutputMode::Text,
            OutputModeArg::Hybrid => RenderOutputMode::Hybrid,
            OutputModeArg::KittyHq => RenderOutputMode::KittyHq,
        }
    }
}

impl From<KittyTransportArg> for KittyTransport {
    fn from(value: KittyTransportArg) -> Self {
        match value {
            KittyTransportArg::Shm => KittyTransport::Shm,
            KittyTransportArg::Direct => KittyTransport::Direct,
        }
    }
}

impl From<KittyCompressionArg> for KittyCompression {
    fn from(value: KittyCompressionArg) -> Self {
        match value {
            KittyCompressionArg::None => KittyCompression::None,
            KittyCompressionArg::Zlib => KittyCompression::Zlib,
        }
    }
}

impl From<KittyInternalResArg> for KittyInternalResPreset {
    fn from(value: KittyInternalResArg) -> Self {
        match value {
            KittyInternalResArg::R640x360 => KittyInternalResPreset::R640x360,
            KittyInternalResArg::R854x480 => KittyInternalResPreset::R854x480,
            KittyInternalResArg::R1280x720 => KittyInternalResPreset::R1280x720,
        }
    }
}

impl From<StageRoleArg> for StageRole {
    fn from(value: StageRoleArg) -> Self {
        match value {
            StageRoleArg::Sub => StageRole::Sub,
            StageRoleArg::Off => StageRole::Off,
        }
    }
}

impl From<KittyPipelineArg> for KittyPipelineMode {
    fn from(value: KittyPipelineArg) -> Self {
        match value {
            KittyPipelineArg::RealPixel => KittyPipelineMode::RealPixel,
            KittyPipelineArg::GlyphCompat => KittyPipelineMode::GlyphCompat,
        }
    }
}

impl From<RecoverStrategyArg> for RecoverStrategy {
    fn from(value: RecoverStrategyArg) -> Self {
        match value {
            RecoverStrategyArg::Hard => RecoverStrategy::Hard,
            RecoverStrategyArg::Soft => RecoverStrategy::Soft,
            RecoverStrategyArg::Off => RecoverStrategy::Off,
        }
    }
}

impl From<RecoverColorArg> for bool {
    fn from(value: RecoverColorArg) -> Self {
        matches!(value, RecoverColorArg::Auto)
    }
}

impl From<GraphicsProtocolArg> for GraphicsProtocol {
    fn from(value: GraphicsProtocolArg) -> Self {
        match value {
            GraphicsProtocolArg::Auto => GraphicsProtocol::Auto,
            GraphicsProtocolArg::Kitty => GraphicsProtocol::Kitty,
            GraphicsProtocolArg::Iterm2 => GraphicsProtocol::Iterm2,
            GraphicsProtocolArg::None => GraphicsProtocol::None,
        }
    }
}

impl From<ColorModeArg> for ColorMode {
    fn from(value: ColorModeArg) -> Self {
        match value {
            ColorModeArg::Mono => ColorMode::Mono,
            ColorModeArg::Ansi => ColorMode::Ansi,
        }
    }
}

impl From<BrailleProfileArg> for BrailleProfile {
    fn from(value: BrailleProfileArg) -> Self {
        match value {
            BrailleProfileArg::Safe => BrailleProfile::Safe,
            BrailleProfileArg::Normal => BrailleProfile::Normal,
            BrailleProfileArg::Dense => BrailleProfile::Dense,
        }
    }
}

impl From<ThemeArg> for ThemeStyle {
    fn from(value: ThemeArg) -> Self {
        match value {
            ThemeArg::Theater => ThemeStyle::Theater,
            ThemeArg::Neon => ThemeStyle::Neon,
            ThemeArg::Holo => ThemeStyle::Holo,
        }
    }
}

impl From<PerfProfileArg> for PerfProfile {
    fn from(value: PerfProfileArg) -> Self {
        match value {
            PerfProfileArg::Balanced => PerfProfile::Balanced,
            PerfProfileArg::Cinematic => PerfProfile::Cinematic,
            PerfProfileArg::Smooth => PerfProfile::Smooth,
        }
    }
}

impl From<DetailProfileArg> for DetailProfile {
    fn from(value: DetailProfileArg) -> Self {
        match value {
            DetailProfileArg::Perf => DetailProfile::Perf,
            DetailProfileArg::Balanced => DetailProfile::Balanced,
            DetailProfileArg::Ultra => DetailProfile::Ultra,
        }
    }
}

impl From<ClarityProfileArg> for ClarityProfile {
    fn from(value: ClarityProfileArg) -> Self {
        match value {
            ClarityProfileArg::Balanced => ClarityProfile::Balanced,
            ClarityProfileArg::Sharp => ClarityProfile::Sharp,
            ClarityProfileArg::Extreme => ClarityProfile::Extreme,
        }
    }
}

impl From<AnsiQuantizationArg> for AnsiQuantization {
    fn from(value: AnsiQuantizationArg) -> Self {
        match value {
            AnsiQuantizationArg::Q216 => AnsiQuantization::Q216,
            AnsiQuantizationArg::Off => AnsiQuantization::Off,
        }
    }
}

impl From<CenterLockModeArg> for CenterLockMode {
    fn from(value: CenterLockModeArg) -> Self {
        match value {
            CenterLockModeArg::Root => CenterLockMode::Root,
            CenterLockModeArg::Mixed => CenterLockMode::Mixed,
        }
    }
}

impl From<CameraFocusArg> for CameraFocusMode {
    fn from(value: CameraFocusArg) -> Self {
        match value {
            CameraFocusArg::Auto => CameraFocusMode::Auto,
            CameraFocusArg::Full => CameraFocusMode::Full,
            CameraFocusArg::Upper => CameraFocusMode::Upper,
            CameraFocusArg::Face => CameraFocusMode::Face,
            CameraFocusArg::Hands => CameraFocusMode::Hands,
        }
    }
}

impl From<WasdModeArg> for CameraControlMode {
    fn from(value: WasdModeArg) -> Self {
        match value {
            WasdModeArg::Orbit => CameraControlMode::Orbit,
            WasdModeArg::Freefly => CameraControlMode::FreeFly,
        }
    }
}

impl From<CameraModeArg> for CameraMode {
    fn from(value: CameraModeArg) -> Self {
        match value {
            CameraModeArg::Off => CameraMode::Off,
            CameraModeArg::Vmd => CameraMode::Vmd,
            CameraModeArg::Blend => CameraMode::Blend,
        }
    }
}

impl From<CameraAlignPresetArg> for CameraAlignPreset {
    fn from(value: CameraAlignPresetArg) -> Self {
        match value {
            CameraAlignPresetArg::Std => CameraAlignPreset::Std,
            CameraAlignPresetArg::AltA => CameraAlignPreset::AltA,
            CameraAlignPresetArg::AltB => CameraAlignPreset::AltB,
        }
    }
}

impl From<TextureSamplingArg> for TextureSamplingMode {
    fn from(value: TextureSamplingArg) -> Self {
        match value {
            TextureSamplingArg::Nearest => TextureSamplingMode::Nearest,
            TextureSamplingArg::Bilinear => TextureSamplingMode::Bilinear,
        }
    }
}

impl From<TextureVOriginArg> for TextureVOrigin {
    fn from(value: TextureVOriginArg) -> Self {
        match value {
            TextureVOriginArg::Gltf => TextureVOrigin::Gltf,
            TextureVOriginArg::Legacy => TextureVOrigin::Legacy,
        }
    }
}

impl From<TextureSamplerModeArg> for TextureSamplerMode {
    fn from(value: TextureSamplerModeArg) -> Self {
        match value {
            TextureSamplerModeArg::Gltf => TextureSamplerMode::Gltf,
            TextureSamplerModeArg::Override => TextureSamplerMode::Override,
        }
    }
}

impl From<BackendArg> for RenderBackend {
    fn from(value: BackendArg) -> Self {
        match value {
            BackendArg::Cpu => RenderBackend::Cpu,
            BackendArg::Gpu => RenderBackend::Gpu,
        }
    }
}

impl From<ToggleArg> for bool {
    fn from(value: ToggleArg) -> Self {
        matches!(value, ToggleArg::On)
    }
}

impl From<AudioReactiveArg> for AudioReactiveMode {
    fn from(value: AudioReactiveArg) -> Self {
        match value {
            AudioReactiveArg::Off => AudioReactiveMode::Off,
            AudioReactiveArg::On => AudioReactiveMode::On,
            AudioReactiveArg::High => AudioReactiveMode::High,
        }
    }
}

impl From<CinematicCameraArg> for CinematicCameraMode {
    fn from(value: CinematicCameraArg) -> Self {
        match value {
            CinematicCameraArg::Off => CinematicCameraMode::Off,
            CinematicCameraArg::On => CinematicCameraMode::On,
            CinematicCameraArg::Aggressive => CinematicCameraMode::Aggressive,
        }
    }
}

pub fn parse() -> Cli {
    Cli::parse()
}

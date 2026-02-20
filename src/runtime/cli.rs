use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::scene::{
    AudioReactiveMode, BrailleProfile, CameraFocusMode, CellAspectMode, CenterLockMode,
    CinematicCameraMode, ColorMode, ContrastProfile, DEFAULT_CHARSET, DetailProfile, PerfProfile,
    RenderBackend, RenderMode, SyncSpeedMode, TextureSamplingMode, ThemeStyle,
};

#[derive(Debug, Parser)]
#[command(name = "terminal-miku3d")]
#[command(
    about = "Terminal renderer for ASCII/Braille 3D scenes (CPU + optional GPU experimental)",
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
    /// Run interactive terminal rendering for cube/OBJ/GLB scenes.
    Run(RunArgs),
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
    /// Animation selector by name or index. Defaults to first clip if available.
    #[arg(long)]
    pub anim: Option<String>,
    #[arg(long, value_enum, default_value_t = ModeArg::Ascii)]
    pub mode: ModeArg,
    #[arg(long, value_enum)]
    pub color_mode: Option<ColorModeArg>,
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
    #[arg(long)]
    pub stage_level: Option<u8>,
    #[arg(long)]
    pub exposure_bias: Option<f32>,
    #[arg(long, default_value_t = 30)]
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
    /// Animation selector by name or index.
    #[arg(long)]
    pub anim: Option<String>,
    #[arg(long, value_enum, default_value_t = ModeArg::Ascii)]
    pub mode: ModeArg,
    #[arg(long, value_enum)]
    pub color_mode: Option<ColorModeArg>,
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
    #[arg(long)]
    pub stage_level: Option<u8>,
    #[arg(long)]
    pub exposure_bias: Option<f32>,
    #[arg(long, default_value_t = 30)]
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
    pub color_mode: Option<ColorModeArg>,
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
    #[arg(long)]
    pub stage_level: Option<u8>,
    #[arg(long)]
    pub exposure_bias: Option<f32>,
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
pub struct InspectArgs {
    /// Path to .glb or .gltf file.
    #[arg(long)]
    pub glb: PathBuf,
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
pub enum TextureSamplingArg {
    Nearest,
    Bilinear,
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

impl From<TextureSamplingArg> for TextureSamplingMode {
    fn from(value: TextureSamplingArg) -> Self {
        match value {
            TextureSamplingArg::Nearest => TextureSamplingMode::Nearest,
            TextureSamplingArg::Bilinear => TextureSamplingMode::Bilinear,
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

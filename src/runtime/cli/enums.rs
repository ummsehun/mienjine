//! CLI argument enum definitions (ValueEnum variants).
//! All enums in this module are used as `#[arg(long, value_enum)]` types
//! across StartArgs, RunArgs, and BenchArgs.

use clap::ValueEnum;

// ----------------------------------------------------------------------------
// Scene / mode enums
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ModeArg {
    Ascii,
    Braille,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RunSceneArg {
    Cube,
    Obj,
    Glb,
    Pmx,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BenchSceneArg {
    Cube,
    Obj,
    GlbStatic,
    GlbAnim,
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

// ----------------------------------------------------------------------------
// Camera enums
// ----------------------------------------------------------------------------

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
pub enum CameraFocusArg {
    Auto,
    Full,
    Upper,
    Face,
    Hands,
}

// ----------------------------------------------------------------------------
// Output / graphics enums
// ----------------------------------------------------------------------------

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
pub enum GraphicsProtocolArg {
    Auto,
    Kitty,
    #[value(name = "iterm2")]
    Iterm2,
    None,
}

// ----------------------------------------------------------------------------
// Color / theme enums
// ----------------------------------------------------------------------------

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
pub enum AnsiQuantizationArg {
    Q216,
    Off,
}

// ----------------------------------------------------------------------------
// Quality / performance enums
// ----------------------------------------------------------------------------

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
pub enum ContrastProfileArg {
    Adaptive,
    Fixed,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CellAspectModeArg {
    Auto,
    Manual,
}

// ----------------------------------------------------------------------------
// Control / interaction enums
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CenterLockModeArg {
    Root,
    Mixed,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WasdModeArg {
    Orbit,
    Freefly,
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

// ----------------------------------------------------------------------------
// Stage / recovery enums
// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------
// Audio / reactive enums
// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------
// Texture / material enums
// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------
// Sync / profile enums
// ----------------------------------------------------------------------------

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

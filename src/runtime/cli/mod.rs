mod args_bench;
mod args_run;
mod args_simple;
mod args_start;
mod enums;

pub use args_bench::BenchArgs;
pub use args_run::RunArgs;
pub use args_simple::{InspectArgs, PreprocessArgs, PreviewArgs};
pub use args_start::StartArgs;

pub use enums::{
    AnsiQuantizationArg, AudioReactiveArg, BackendArg, BenchSceneArg, BrailleProfileArg,
    CameraAlignPresetArg, CameraFocusArg, CameraModeArg, CellAspectModeArg, CenterLockModeArg,
    CinematicCameraArg, ClarityProfileArg, ColorModeArg, ContrastProfileArg, DetailProfileArg,
    GraphicsProtocolArg, KittyCompressionArg, KittyInternalResArg, KittyPipelineArg,
    KittyTransportArg, ModeArg, OutputModeArg, PerfProfileArg, PreprocessPresetArg,
    RecoverColorArg, RecoverStrategyArg, RunSceneArg, StageRoleArg, SyncPolicyArg,
    SyncProfileModeArg, SyncSpeedModeArg, TextureSamplerModeArg, TextureSamplingArg,
    TextureVOriginArg, ThemeArg, ToggleArg, WasdModeArg,
};

use clap::{Parser, Subcommand};

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
    Start(StartArgs),
    Run(RunArgs),
    Preview(PreviewArgs),
    Preprocess(PreprocessArgs),
    Bench(BenchArgs),
    Inspect(InspectArgs),
}

pub fn parse() -> Cli {
    Cli::parse()
}

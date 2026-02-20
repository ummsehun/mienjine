use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::scene::{DEFAULT_CHARSET, RenderMode};

#[derive(Debug, Parser)]
#[command(name = "terminal-miku3d")]
#[command(about = "CPU-only terminal renderer for ASCII/Braille 3D scenes", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run interactive terminal rendering for cube/OBJ/GLB scenes.
    Run(RunArgs),
    /// Benchmark a scene pipeline without terminal presentation.
    Bench(BenchArgs),
    /// Inspect GLB/glTF scene structure.
    Inspect(InspectArgs),
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
    #[arg(long, default_value_t = 30)]
    pub fps_cap: u32,
    #[arg(long, default_value_t = 0.5)]
    pub cell_aspect: f32,
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
    #[arg(long, default_value_t = 0.5)]
    pub cell_aspect: f32,
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

impl From<ModeArg> for RenderMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Ascii => RenderMode::Ascii,
            ModeArg::Braille => RenderMode::Braille,
        }
    }
}

pub fn parse() -> Cli {
    Cli::parse()
}

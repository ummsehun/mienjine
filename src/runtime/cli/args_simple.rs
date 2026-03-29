use std::path::PathBuf;

use super::enums::{CameraAlignPresetArg, CameraModeArg, PreprocessPresetArg};

#[derive(Debug, clap::Args)]
pub struct PreviewArgs {
    #[arg(long)]
    pub glb: PathBuf,
    #[arg(long)]
    pub anim: Option<String>,
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
    #[arg(long, value_enum, default_value_t = PreprocessPresetArg::Default)]
    pub preset: PreprocessPresetArg,
    #[arg(long)]
    pub glb: PathBuf,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, default_value_t = 2)]
    pub upscale_factor: u32,
    #[arg(long, default_value_t = 0.20)]
    pub upscale_sharpen: f32,
}

#[derive(Debug, clap::Args)]
pub struct InspectArgs {
    #[arg(long)]
    pub glb: PathBuf,
}

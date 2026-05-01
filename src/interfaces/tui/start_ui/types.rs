use std::path::PathBuf;

use crossterm::event::KeyEvent;

use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, PerfProfile,
    RenderBackend, RenderMode, RenderOutputMode, StageQuality, SyncPolicy, SyncSpeedMode,
    TextureSamplingMode, ThemeStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelBranch {
    Glb,
    PmxVmd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartWizardStep {
    Branch,
    Model,
    Motion,
    Music,
    Stage,
    Camera,
    Render,
    AspectCalib,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderDetailMode {
    Quick,
    Advanced,
}

impl StartWizardStep {
    pub(super) fn index(self) -> usize {
        match self {
            StartWizardStep::Branch => 0,
            StartWizardStep::Model => 1,
            StartWizardStep::Motion => 2,
            StartWizardStep::Music => 3,
            StartWizardStep::Stage => 4,
            StartWizardStep::Camera => 5,
            StartWizardStep::Render => 6,
            StartWizardStep::AspectCalib => 7,
            StartWizardStep::Confirm => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Ready,
    NeedsConvert,
    Invalid,
}

#[derive(Debug, Clone, Copy)]
pub struct StageTransform {
    pub offset: [f32; 3],
    pub scale: f32,
    pub rotation_deg: [f32; 3],
}

impl Default for StageTransform {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0, 0.0],
            scale: 1.0,
            rotation_deg: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone)]
pub struct StageChoice {
    pub name: String,
    pub status: StageStatus,
    pub render_path: Option<PathBuf>,
    pub pmx_path: Option<PathBuf>,
    pub transform: StageTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiBreakpoint {
    Wide,
    Normal,
    Compact,
}

#[derive(Debug, Clone, Copy)]
pub enum StartWizardEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
}

#[derive(Debug, Clone)]
pub struct StartWizardDefaults {
    pub mode: RenderMode,
    pub output_mode: RenderOutputMode,
    pub graphics_protocol: GraphicsProtocol,
    pub perf_profile: PerfProfile,
    pub detail_profile: DetailProfile,
    pub clarity_profile: ClarityProfile,
    pub ansi_quantization: AnsiQuantization,
    pub backend: RenderBackend,
    pub center_lock: bool,
    pub center_lock_mode: CenterLockMode,
    pub wasd_mode: CameraControlMode,
    pub freefly_speed: f32,
    pub camera_focus: CameraFocusMode,
    pub material_color: bool,
    pub texture_sampling: TextureSamplingMode,
    pub model_lift: f32,
    pub edge_accent_strength: f32,
    pub braille_aspect_compensation: f32,
    pub stage_level: u8,
    pub stage_reactive: bool,
    pub stage_quality: StageQuality,
    pub color_mode: ColorMode,
    pub braille_profile: BrailleProfile,
    pub theme_style: ThemeStyle,
    pub audio_reactive: AudioReactiveMode,
    pub cinematic_camera: CinematicCameraMode,
    pub reactive_gain: f32,
    pub fps_cap: u32,
    pub cell_aspect: f32,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_offset_ms: i32,
    pub sync_speed_mode: SyncSpeedMode,
    pub sync_policy: SyncPolicy,
    pub sync_hard_snap_ms: u32,
    pub sync_kp: f32,
    pub font_preset_enabled: bool,
    pub camera_mode: CameraMode,
    pub camera_align_preset: CameraAlignPreset,
    pub camera_unit_scale: f32,
    pub camera_vmd_path: Option<PathBuf>,
}

impl Default for StartWizardDefaults {
    fn default() -> Self {
        Self {
            mode: RenderMode::Braille,
            output_mode: RenderOutputMode::Text,
            graphics_protocol: GraphicsProtocol::Auto,
            perf_profile: PerfProfile::Balanced,
            detail_profile: DetailProfile::Balanced,
            clarity_profile: ClarityProfile::Sharp,
            ansi_quantization: AnsiQuantization::Q216,
            backend: RenderBackend::Cpu,
            center_lock: true,
            center_lock_mode: CenterLockMode::Root,
            wasd_mode: CameraControlMode::FreeFly,
            freefly_speed: 1.0,
            camera_focus: CameraFocusMode::Auto,
            material_color: true,
            texture_sampling: TextureSamplingMode::Nearest,
            model_lift: 0.12,
            edge_accent_strength: 0.32,
            braille_aspect_compensation: 1.00,
            stage_level: 2,
            stage_reactive: true,
            stage_quality: StageQuality::Medium,
            color_mode: ColorMode::Mono,
            braille_profile: BrailleProfile::Safe,
            theme_style: ThemeStyle::Theater,
            audio_reactive: AudioReactiveMode::On,
            cinematic_camera: CinematicCameraMode::On,
            reactive_gain: 0.35,
            fps_cap: 20,
            cell_aspect: 0.5,
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.0,
            contrast_profile: ContrastProfile::Adaptive,
            sync_offset_ms: 0,
            sync_speed_mode: SyncSpeedMode::AutoDurationFit,
            sync_policy: SyncPolicy::Continuous,
            sync_hard_snap_ms: 120,
            sync_kp: 0.15,
            font_preset_enabled: false,
            camera_mode: CameraMode::Off,
            camera_align_preset: CameraAlignPreset::Std,
            camera_unit_scale: 0.08,
            camera_vmd_path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StartSelection {
    pub branch: ModelBranch,
    pub glb_path: PathBuf,
    pub pmx_path: Option<PathBuf>,
    pub motion_vmd_path: Option<PathBuf>,
    pub music_path: Option<PathBuf>,
    pub mode: RenderMode,
    pub output_mode: RenderOutputMode,
    pub graphics_protocol: GraphicsProtocol,
    pub perf_profile: PerfProfile,
    pub detail_profile: DetailProfile,
    pub clarity_profile: ClarityProfile,
    pub ansi_quantization: AnsiQuantization,
    pub backend: RenderBackend,
    pub center_lock: bool,
    pub center_lock_mode: CenterLockMode,
    pub wasd_mode: CameraControlMode,
    pub freefly_speed: f32,
    pub camera_focus: CameraFocusMode,
    pub material_color: bool,
    pub texture_sampling: TextureSamplingMode,
    pub model_lift: f32,
    pub edge_accent_strength: f32,
    pub braille_aspect_compensation: f32,
    pub stage_level: u8,
    pub stage_reactive: bool,
    pub stage_quality: StageQuality,
    pub color_mode: ColorMode,
    pub braille_profile: BrailleProfile,
    pub theme_style: ThemeStyle,
    pub audio_reactive: AudioReactiveMode,
    pub cinematic_camera: CinematicCameraMode,
    pub reactive_gain: f32,
    pub fps_cap: u32,
    pub cell_aspect: f32,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_offset_ms: i32,
    pub sync_speed_mode: SyncSpeedMode,
    pub sync_policy: SyncPolicy,
    pub sync_hard_snap_ms: u32,
    pub sync_kp: f32,
    pub stage_choice: Option<StageChoice>,
    pub stage_transform: StageTransform,
    pub apply_font_preset: bool,
    pub camera_vmd_path: Option<PathBuf>,
    pub camera_mode: CameraMode,
    pub camera_align_preset: CameraAlignPreset,
    pub camera_unit_scale: f32,
}

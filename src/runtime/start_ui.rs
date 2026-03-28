use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{
    runtime::{
        config::UiLanguage,
        start_ui_helpers::{
            breakpoint_for, clamp_ratatui_area, closest_u32_index, compute_duration_fit_factor,
            cycle_index, detect_terminal_cell_aspect, format_mib, inspect_audio_duration,
            inspect_clip_duration, inspect_motion_duration, tr, MIN_HEIGHT, MIN_WIDTH,
            RATATUI_SAFE_MAX_CELLS, RENDER_FIELD_COUNT, START_FPS_OPTIONS, SYNC_OFFSET_LIMIT_MS,
            SYNC_OFFSET_STEP_MS,
        },
        terminal::{RatatuiSession, TerminalProfile},
    },
    scene::{
        resolve_cell_aspect, AnsiQuantization, AudioReactiveMode, BrailleProfile,
        CameraAlignPreset, CameraControlMode, CameraFocusMode, CameraMode, CellAspectMode,
        CenterLockMode, CinematicCameraMode, ClarityProfile, ColorMode, ContrastProfile,
        DetailProfile, GraphicsProtocol, PerfProfile, RenderBackend, RenderConfig, RenderMode,
        RenderOutputMode, SyncPolicy, SyncSpeedMode, TextureSamplingMode, ThemeStyle,
    },
};

mod panels;
mod steps;
use panels::{draw_header, draw_help_panel, draw_min_size_screen, draw_summary_panel};
use steps::draw_step_panel;

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

impl StartWizardStep {
    fn index(self) -> usize {
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

#[derive(Debug, Clone)]
struct StartEntry {
    path: PathBuf,
    name: String,
    bytes: u64,
}

impl StartEntry {
    fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<invalid>")
            .to_owned();
        let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        Self {
            path: path.to_path_buf(),
            name,
            bytes,
        }
    }

    fn label(&self) -> String {
        format!("{} ({})", self.name, format_mib(self.bytes))
    }
}

#[derive(Debug, Clone)]
struct StartWizardState {
    step: StartWizardStep,
    branch: ModelBranch,
    model_entries: Vec<StartEntry>,
    pmx_entries: Vec<StartEntry>,
    motion_entries: Vec<StartEntry>,
    music_entries: Vec<StartEntry>,
    stage_entries: Vec<StageChoice>,
    camera_entries: Vec<StartEntry>,
    model_index: usize,
    motion_index: usize,
    music_index: usize,
    stage_index: usize,
    camera_index: usize,
    mode: RenderMode,
    output_mode: RenderOutputMode,
    graphics_protocol: GraphicsProtocol,
    perf_profile: PerfProfile,
    detail_profile: DetailProfile,
    clarity_profile: ClarityProfile,
    ansi_quantization: AnsiQuantization,
    backend: RenderBackend,
    center_lock: bool,
    center_lock_mode: CenterLockMode,
    wasd_mode: CameraControlMode,
    freefly_speed: f32,
    camera_focus: CameraFocusMode,
    material_color: bool,
    texture_sampling: TextureSamplingMode,
    model_lift: f32,
    edge_accent_strength: f32,
    braille_aspect_compensation: f32,
    stage_level: u8,
    stage_reactive: bool,
    color_mode: ColorMode,
    braille_profile: BrailleProfile,
    theme_style: ThemeStyle,
    audio_reactive: AudioReactiveMode,
    cinematic_camera: CinematicCameraMode,
    reactive_gain: f32,
    fps_index: usize,
    manual_cell_aspect: f32,
    cell_aspect_mode: CellAspectMode,
    cell_aspect_trim: f32,
    contrast_profile: ContrastProfile,
    sync_offset_ms: i32,
    sync_speed_mode: SyncSpeedMode,
    sync_policy: SyncPolicy,
    sync_hard_snap_ms: u32,
    sync_kp: f32,
    font_preset_enabled: bool,
    camera_mode: CameraMode,
    camera_align_preset: CameraAlignPreset,
    camera_unit_scale: f32,
    camera_focus_index: usize,
    render_focus_index: usize,
    width: u16,
    height: u16,
    detected_cell_aspect: Option<f32>,
    #[cfg(feature = "gpu")]
    gpu_available: bool,
    clip_duration_cache: HashMap<PathBuf, Option<f32>>,
    audio_duration_cache: HashMap<PathBuf, Option<f32>>,
}

impl StartWizardState {
    fn new(
        model_entries: Vec<StartEntry>,
        pmx_entries: Vec<StartEntry>,
        motion_entries: Vec<StartEntry>,
        music_entries: Vec<StartEntry>,
        stage_entries: Vec<StageChoice>,
        camera_entries: Vec<StartEntry>,
        defaults: StartWizardDefaults,
        width: u16,
        height: u16,
    ) -> Self {
        let camera_index = defaults
            .camera_vmd_path
            .as_ref()
            .and_then(|selected| {
                camera_entries
                    .iter()
                    .position(|entry| entry.path == *selected)
                    .map(|idx| idx + 1)
            })
            .unwrap_or(0);
        Self {
            step: StartWizardStep::Branch,
            branch: ModelBranch::Glb,
            model_entries,
            pmx_entries,
            motion_entries,
            music_entries,
            stage_entries,
            camera_entries,
            model_index: 0,
            motion_index: 0,
            music_index: 0,
            stage_index: 0,
            camera_index,
            mode: defaults.mode,
            output_mode: defaults.output_mode,
            graphics_protocol: defaults.graphics_protocol,
            perf_profile: defaults.perf_profile,
            detail_profile: defaults.detail_profile,
            clarity_profile: defaults.clarity_profile,
            ansi_quantization: defaults.ansi_quantization,
            backend: defaults.backend,
            center_lock: defaults.center_lock,
            center_lock_mode: defaults.center_lock_mode,
            wasd_mode: defaults.wasd_mode,
            freefly_speed: defaults.freefly_speed.clamp(0.1, 8.0),
            camera_focus: defaults.camera_focus,
            material_color: defaults.material_color,
            texture_sampling: defaults.texture_sampling,
            model_lift: defaults.model_lift.clamp(0.02, 0.45),
            edge_accent_strength: defaults.edge_accent_strength.clamp(0.0, 1.5),
            braille_aspect_compensation: defaults.braille_aspect_compensation,
            stage_level: defaults.stage_level.min(4),
            stage_reactive: defaults.stage_reactive,
            color_mode: defaults.color_mode,
            braille_profile: defaults.braille_profile,
            theme_style: defaults.theme_style,
            audio_reactive: defaults.audio_reactive,
            cinematic_camera: defaults.cinematic_camera,
            reactive_gain: defaults.reactive_gain.clamp(0.0, 1.0),
            fps_index: closest_u32_index(defaults.fps_cap, &START_FPS_OPTIONS),
            manual_cell_aspect: defaults.cell_aspect,
            cell_aspect_mode: defaults.cell_aspect_mode,
            cell_aspect_trim: defaults.cell_aspect_trim.clamp(0.70, 1.30),
            contrast_profile: defaults.contrast_profile,
            sync_offset_ms: defaults
                .sync_offset_ms
                .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
            sync_speed_mode: defaults.sync_speed_mode,
            sync_policy: defaults.sync_policy,
            sync_hard_snap_ms: defaults.sync_hard_snap_ms.clamp(10, 2_000),
            sync_kp: defaults.sync_kp.clamp(0.01, 1.0),
            font_preset_enabled: defaults.font_preset_enabled,
            camera_mode: defaults.camera_mode,
            camera_align_preset: defaults.camera_align_preset,
            camera_unit_scale: defaults.camera_unit_scale.clamp(0.01, 2.0),
            camera_focus_index: 0,
            render_focus_index: 0,
            width,
            height,
            detected_cell_aspect: None,
            #[cfg(feature = "gpu")]
            gpu_available: gpu_available_once(),
            clip_duration_cache: HashMap::new(),
            audio_duration_cache: HashMap::new(),
        }
    }

    fn on_resize(&mut self, width: u16, height: u16) {
        self.width = width.max(1);
        self.height = height.max(1);
    }

    fn refresh_runtime_metrics(&mut self, anim_selector: Option<&str>) {
        self.detected_cell_aspect = detect_terminal_cell_aspect();

        let model_path = self
            .model_entries
            .get(self.model_index)
            .map(|entry| entry.path.clone());
        if let Some(path) = model_path {
            self.clip_duration_cache
                .entry(path.clone())
                .or_insert_with(|| inspect_clip_duration(&path, anim_selector));
        }

        let music_path = self.selected_music_path().cloned();
        if let Some(path) = music_path {
            self.audio_duration_cache
                .entry(path.clone())
                .or_insert_with(|| inspect_audio_duration(&path));
        }
    }

    fn apply_event(&mut self, event: StartWizardEvent) -> StartWizardAction {
        match event {
            StartWizardEvent::Resize(width, height) => {
                self.on_resize(width, height);
                StartWizardAction::Continue
            }
            StartWizardEvent::Tick => StartWizardAction::Continue,
            StartWizardEvent::Key(key) => self.apply_key(key),
        }
    }

    fn apply_key(&mut self, key: KeyEvent) -> StartWizardAction {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return StartWizardAction::Continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return StartWizardAction::Cancel,
            _ => {}
        }

        if self.is_too_small() {
            return StartWizardAction::Continue;
        }

        match key.code {
            KeyCode::Tab => {
                self.tab_forward();
                return StartWizardAction::Continue;
            }
            KeyCode::BackTab => {
                self.tab_backward();
                return StartWizardAction::Continue;
            }
            _ => {}
        }

        match self.step {
            StartWizardStep::Branch => self.apply_branch_key(key),
            StartWizardStep::Model => self.apply_model_key(key),
            StartWizardStep::Motion => self.apply_motion_key(key),
            StartWizardStep::Music => self.apply_music_key(key),
            StartWizardStep::Stage => self.apply_stage_key(key),
            StartWizardStep::Camera => self.apply_camera_key(key),
            StartWizardStep::Render => self.apply_render_key(key),
            StartWizardStep::AspectCalib => self.apply_aspect_key(key),
            StartWizardStep::Confirm => self.apply_confirm_key(key),
        }
    }

    fn apply_model_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let model_len = match self.branch {
            ModelBranch::Glb => self.model_entries.len(),
            ModelBranch::PmxVmd => self.pmx_entries.len(),
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.model_index, model_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.model_index, model_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = if matches!(self.branch, ModelBranch::PmxVmd) {
                    StartWizardStep::Motion
                } else {
                    StartWizardStep::Music
                };
                StartWizardAction::Continue
            }
            KeyCode::Esc => StartWizardAction::Cancel,
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_branch_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
                self.branch = match self.branch {
                    ModelBranch::Glb => ModelBranch::PmxVmd,
                    ModelBranch::PmxVmd => ModelBranch::Glb,
                };
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('l') | KeyCode::Char('j') => {
                self.branch = match self.branch {
                    ModelBranch::Glb => ModelBranch::PmxVmd,
                    ModelBranch::PmxVmd => ModelBranch::Glb,
                };
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Model;
                StartWizardAction::Continue
            }
            KeyCode::Esc => StartWizardAction::Cancel,
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_motion_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let motion_len = self.motion_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.motion_index, motion_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.motion_index, motion_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Music;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Model;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_music_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let music_len = self.music_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.music_index, music_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.music_index, music_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Stage;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Model;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_stage_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let stage_len = self.stage_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.stage_index, stage_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.stage_index, stage_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Camera;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Music;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_camera_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let camera_len = self.camera_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.camera_focus_index, 4, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.camera_focus_index, 4, 1);
                StartWizardAction::Continue
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                self.adjust_camera_value(camera_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.adjust_camera_value(camera_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Render;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Stage;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_render_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.render_focus_index, RENDER_FIELD_COUNT, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.render_focus_index, RENDER_FIELD_COUNT, 1);
                StartWizardAction::Continue
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                self.adjust_render_value(-1);
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.adjust_render_value(1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::AspectCalib;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Camera;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_aspect_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                self.cell_aspect_trim = (self.cell_aspect_trim - 0.01).clamp(0.70, 1.30);
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.cell_aspect_trim = (self.cell_aspect_trim + 0.01).clamp(0.70, 1.30);
                StartWizardAction::Continue
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.cell_aspect_trim = 1.0;
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Confirm;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Render;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_confirm_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Enter => StartWizardAction::Submit(self.selection()),
            KeyCode::Esc => {
                self.step = StartWizardStep::AspectCalib;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn tab_forward(&mut self) {
        match self.step {
            StartWizardStep::Branch => self.step = StartWizardStep::Model,
            StartWizardStep::Model => self.step = StartWizardStep::Music,
            StartWizardStep::Motion => self.step = StartWizardStep::Music,
            StartWizardStep::Music => self.step = StartWizardStep::Stage,
            StartWizardStep::Stage => self.step = StartWizardStep::Camera,
            StartWizardStep::Camera => self.step = StartWizardStep::Render,
            StartWizardStep::Render => {
                if self.render_focus_index + 1 < RENDER_FIELD_COUNT {
                    self.render_focus_index += 1;
                } else {
                    self.step = StartWizardStep::AspectCalib;
                }
            }
            StartWizardStep::AspectCalib => self.step = StartWizardStep::Confirm,
            StartWizardStep::Confirm => {}
        }
    }

    fn tab_backward(&mut self) {
        match self.step {
            StartWizardStep::Branch => {}
            StartWizardStep::Model => {}
            StartWizardStep::Motion => self.step = StartWizardStep::Model,
            StartWizardStep::Music => self.step = StartWizardStep::Model,
            StartWizardStep::Stage => self.step = StartWizardStep::Music,
            StartWizardStep::Camera => self.step = StartWizardStep::Stage,
            StartWizardStep::Render => {
                if self.render_focus_index > 0 {
                    self.render_focus_index -= 1;
                } else {
                    self.step = StartWizardStep::Camera;
                }
            }
            StartWizardStep::AspectCalib => self.step = StartWizardStep::Render,
            StartWizardStep::Confirm => self.step = StartWizardStep::AspectCalib,
        }
    }

    fn adjust_camera_value(&mut self, camera_len: usize, delta: i32) {
        match self.camera_focus_index {
            0 => {
                cycle_index(&mut self.camera_index, camera_len, delta);
                if self.camera_index == 0 {
                    self.camera_mode = CameraMode::Off;
                } else if matches!(self.camera_mode, CameraMode::Off) {
                    self.camera_mode = CameraMode::Vmd;
                }
            }
            1 => {
                self.camera_mode = match self.camera_mode {
                    CameraMode::Off => CameraMode::Vmd,
                    CameraMode::Vmd => CameraMode::Blend,
                    CameraMode::Blend => CameraMode::Off,
                };
            }
            2 => {
                self.camera_align_preset = match self.camera_align_preset {
                    CameraAlignPreset::Std => CameraAlignPreset::AltA,
                    CameraAlignPreset::AltA => CameraAlignPreset::AltB,
                    CameraAlignPreset::AltB => CameraAlignPreset::Std,
                };
            }
            3 => {
                self.camera_unit_scale =
                    (self.camera_unit_scale + 0.01 * delta as f32).clamp(0.01, 2.0);
            }
            _ => {}
        }
    }

    fn adjust_render_value(&mut self, delta: i32) {
        match self.render_focus_index {
            0 => {
                self.mode = match self.mode {
                    RenderMode::Ascii => RenderMode::Braille,
                    RenderMode::Braille => RenderMode::Ascii,
                };
                if matches!(self.mode, RenderMode::Ascii) {
                    self.color_mode = ColorMode::Ansi;
                }
            }
            1 => {
                self.perf_profile = match self.perf_profile {
                    PerfProfile::Balanced => PerfProfile::Cinematic,
                    PerfProfile::Cinematic => PerfProfile::Smooth,
                    PerfProfile::Smooth => PerfProfile::Balanced,
                };
            }
            2 => {
                self.detail_profile = match self.detail_profile {
                    DetailProfile::Perf => DetailProfile::Balanced,
                    DetailProfile::Balanced => DetailProfile::Ultra,
                    DetailProfile::Ultra => DetailProfile::Perf,
                };
            }
            3 => {
                self.clarity_profile = match self.clarity_profile {
                    ClarityProfile::Balanced => ClarityProfile::Sharp,
                    ClarityProfile::Sharp => ClarityProfile::Extreme,
                    ClarityProfile::Extreme => ClarityProfile::Balanced,
                };
            }
            4 => {
                self.ansi_quantization = match self.ansi_quantization {
                    AnsiQuantization::Q216 => AnsiQuantization::Off,
                    AnsiQuantization::Off => AnsiQuantization::Q216,
                };
            }
            5 => {
                self.backend = match self.backend {
                    RenderBackend::Cpu => RenderBackend::Gpu,
                    RenderBackend::Gpu => RenderBackend::Cpu,
                };
            }
            6 => {
                self.center_lock = !self.center_lock;
            }
            7 => {
                self.center_lock_mode = match self.center_lock_mode {
                    CenterLockMode::Root => CenterLockMode::Mixed,
                    CenterLockMode::Mixed => CenterLockMode::Root,
                };
            }
            8 => {
                self.wasd_mode = match self.wasd_mode {
                    CameraControlMode::Orbit => CameraControlMode::FreeFly,
                    CameraControlMode::FreeFly => CameraControlMode::Orbit,
                };
            }
            9 => {
                let step = 0.1 * (delta as f32);
                self.freefly_speed = (self.freefly_speed + step).clamp(0.1, 8.0);
            }
            10 => {
                self.camera_focus = match self.camera_focus {
                    CameraFocusMode::Auto => CameraFocusMode::Full,
                    CameraFocusMode::Full => CameraFocusMode::Upper,
                    CameraFocusMode::Upper => CameraFocusMode::Face,
                    CameraFocusMode::Face => CameraFocusMode::Hands,
                    CameraFocusMode::Hands => CameraFocusMode::Auto,
                };
            }
            11 => {
                self.material_color = !self.material_color;
            }
            12 => {
                self.texture_sampling = match self.texture_sampling {
                    TextureSamplingMode::Nearest => TextureSamplingMode::Bilinear,
                    TextureSamplingMode::Bilinear => TextureSamplingMode::Nearest,
                };
            }
            13 => {
                let step = 0.01 * (delta as f32);
                self.model_lift = (self.model_lift + step).clamp(0.02, 0.45);
            }
            14 => {
                let step = 0.05 * (delta as f32);
                self.edge_accent_strength = (self.edge_accent_strength + step).clamp(0.0, 1.5);
            }
            15 => {
                let value = (self.stage_level as i32 + delta).clamp(0, 4);
                self.stage_level = value as u8;
            }
            16 => {
                if matches!(self.mode, RenderMode::Braille) {
                    self.color_mode = match self.color_mode {
                        ColorMode::Mono => ColorMode::Ansi,
                        ColorMode::Ansi => ColorMode::Mono,
                    };
                } else {
                    self.color_mode = ColorMode::Ansi;
                }
            }
            17 => {
                self.braille_profile = match self.braille_profile {
                    BrailleProfile::Safe => BrailleProfile::Normal,
                    BrailleProfile::Normal => BrailleProfile::Dense,
                    BrailleProfile::Dense => BrailleProfile::Safe,
                };
            }
            18 => {
                self.theme_style = match self.theme_style {
                    ThemeStyle::Theater => ThemeStyle::Neon,
                    ThemeStyle::Neon => ThemeStyle::Holo,
                    ThemeStyle::Holo => ThemeStyle::Theater,
                };
            }
            19 => {
                self.audio_reactive = match self.audio_reactive {
                    AudioReactiveMode::Off => AudioReactiveMode::On,
                    AudioReactiveMode::On => AudioReactiveMode::High,
                    AudioReactiveMode::High => AudioReactiveMode::Off,
                };
            }
            20 => {
                self.cinematic_camera = match self.cinematic_camera {
                    CinematicCameraMode::Off => CinematicCameraMode::On,
                    CinematicCameraMode::On => CinematicCameraMode::Aggressive,
                    CinematicCameraMode::Aggressive => CinematicCameraMode::Off,
                };
            }
            21 => {
                let step = 0.05 * (delta as f32);
                self.reactive_gain = (self.reactive_gain + step).clamp(0.0, 1.0);
            }
            22 => cycle_index(&mut self.fps_index, START_FPS_OPTIONS.len(), delta),
            23 => {
                self.contrast_profile = match self.contrast_profile {
                    ContrastProfile::Adaptive => ContrastProfile::Fixed,
                    ContrastProfile::Fixed => ContrastProfile::Adaptive,
                }
            }
            24 => {
                let next = self
                    .sync_offset_ms
                    .saturating_add(delta.saturating_mul(SYNC_OFFSET_STEP_MS));
                self.sync_offset_ms = next.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
            }
            25 => {
                self.sync_speed_mode = match self.sync_speed_mode {
                    SyncSpeedMode::AutoDurationFit => SyncSpeedMode::Realtime1x,
                    SyncSpeedMode::Realtime1x => SyncSpeedMode::AutoDurationFit,
                }
            }
            26 => {
                self.output_mode = match self.output_mode {
                    RenderOutputMode::Text => RenderOutputMode::Hybrid,
                    RenderOutputMode::Hybrid => RenderOutputMode::KittyHq,
                    RenderOutputMode::KittyHq => RenderOutputMode::Text,
                };
            }
            27 => {
                self.graphics_protocol = match self.graphics_protocol {
                    GraphicsProtocol::Auto => GraphicsProtocol::Kitty,
                    GraphicsProtocol::Kitty => GraphicsProtocol::Iterm2,
                    GraphicsProtocol::Iterm2 => GraphicsProtocol::None,
                    GraphicsProtocol::None => GraphicsProtocol::Auto,
                };
            }
            28 => {
                self.sync_policy = match self.sync_policy {
                    SyncPolicy::Continuous => SyncPolicy::Fixed,
                    SyncPolicy::Fixed => SyncPolicy::Manual,
                    SyncPolicy::Manual => SyncPolicy::Continuous,
                };
            }
            29 => {
                let next = (self.sync_hard_snap_ms as i32 + delta * 10).clamp(10, 2_000);
                self.sync_hard_snap_ms = next as u32;
            }
            30 => {
                self.sync_kp = (self.sync_kp + 0.01 * delta as f32).clamp(0.01, 1.0);
            }
            31 => {
                self.cell_aspect_mode = match self.cell_aspect_mode {
                    CellAspectMode::Auto => CellAspectMode::Manual,
                    CellAspectMode::Manual => CellAspectMode::Auto,
                }
            }
            32 => {
                self.font_preset_enabled = !self.font_preset_enabled;
            }
            _ => {}
        }
    }

    fn selection(&self) -> StartSelection {
        let active_model_path = self.selected_model_path().cloned().unwrap_or_default();
        let glb_path = active_model_path.clone();
        let pmx_path = if matches!(self.branch, ModelBranch::PmxVmd) {
            Some(active_model_path.clone())
        } else {
            None
        };
        let motion_vmd_path = self.selected_motion_path().cloned();
        let stage_choice = self.selected_stage_choice();
        let stage_transform = stage_choice
            .as_ref()
            .map(|choice| choice.transform)
            .unwrap_or_default();
        StartSelection {
            branch: self.branch,
            glb_path,
            pmx_path,
            motion_vmd_path,
            music_path: self.selected_music_path().cloned(),
            mode: self.mode,
            output_mode: self.output_mode,
            graphics_protocol: self.graphics_protocol,
            perf_profile: self.perf_profile,
            detail_profile: self.detail_profile,
            clarity_profile: self.clarity_profile,
            ansi_quantization: self.ansi_quantization,
            backend: self.backend,
            center_lock: self.center_lock,
            center_lock_mode: self.center_lock_mode,
            wasd_mode: self.wasd_mode,
            freefly_speed: self.freefly_speed,
            camera_focus: self.camera_focus,
            material_color: self.material_color,
            texture_sampling: self.texture_sampling,
            model_lift: self.model_lift,
            edge_accent_strength: self.edge_accent_strength,
            braille_aspect_compensation: self.braille_aspect_compensation,
            stage_level: self.stage_level,
            stage_reactive: self.stage_reactive,
            color_mode: if matches!(self.mode, RenderMode::Ascii) {
                ColorMode::Ansi
            } else {
                self.color_mode
            },
            braille_profile: self.braille_profile,
            theme_style: self.theme_style,
            audio_reactive: self.audio_reactive,
            cinematic_camera: self.cinematic_camera,
            reactive_gain: self.reactive_gain,
            fps_cap: START_FPS_OPTIONS[self.fps_index],
            cell_aspect: self.manual_cell_aspect,
            cell_aspect_mode: self.cell_aspect_mode,
            cell_aspect_trim: self.cell_aspect_trim,
            contrast_profile: self.contrast_profile,
            sync_offset_ms: self.sync_offset_ms,
            sync_speed_mode: self.sync_speed_mode,
            sync_policy: self.sync_policy,
            sync_hard_snap_ms: self.sync_hard_snap_ms,
            sync_kp: self.sync_kp,
            stage_choice,
            stage_transform,
            apply_font_preset: self.font_preset_enabled,
            camera_vmd_path: self.selected_camera_path().cloned(),
            camera_mode: if self.camera_index == 0 {
                CameraMode::Off
            } else {
                self.camera_mode
            },
            camera_align_preset: self.camera_align_preset,
            camera_unit_scale: self.camera_unit_scale,
        }
    }

    fn selected_model_path(&self) -> Option<&PathBuf> {
        match self.branch {
            ModelBranch::Glb => self
                .model_entries
                .get(self.model_index)
                .map(|entry| &entry.path),
            ModelBranch::PmxVmd => self
                .pmx_entries
                .get(self.model_index)
                .map(|entry| &entry.path),
        }
    }

    fn selected_music_path(&self) -> Option<&PathBuf> {
        if self.music_index == 0 {
            None
        } else {
            self.music_entries
                .get(self.music_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    fn selected_stage_choice(&self) -> Option<StageChoice> {
        if self.stage_index == 0 {
            None
        } else {
            self.stage_entries
                .get(self.stage_index.saturating_sub(1))
                .cloned()
        }
    }

    fn selected_camera_path(&self) -> Option<&PathBuf> {
        if self.camera_index == 0 {
            None
        } else {
            self.camera_entries
                .get(self.camera_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    fn selected_motion_path(&self) -> Option<&PathBuf> {
        if !matches!(self.branch, ModelBranch::PmxVmd) || self.motion_index == 0 {
            None
        } else {
            self.motion_entries
                .get(self.motion_index.saturating_sub(1))
                .map(|entry| &entry.path)
        }
    }

    fn selected_clip_duration_secs(&self) -> Option<f32> {
        match self.branch {
            ModelBranch::Glb => {
                let path = self.model_entries.get(self.model_index)?.path.clone();
                self.clip_duration_cache.get(&path).and_then(|value| *value)
            }
            ModelBranch::PmxVmd => self
                .selected_motion_path()
                .and_then(|path| inspect_motion_duration(path)),
        }
    }

    fn selected_audio_duration_secs(&self) -> Option<f32> {
        let path = self.selected_music_path()?.clone();
        self.audio_duration_cache
            .get(&path)
            .and_then(|value| *value)
    }

    fn expected_sync_speed(&self) -> f32 {
        compute_duration_fit_factor(
            self.selected_clip_duration_secs(),
            self.selected_audio_duration_secs(),
            self.sync_speed_mode,
        )
    }

    fn preview_render_config(&self) -> RenderConfig {
        RenderConfig {
            mode: self.mode,
            output_mode: self.output_mode,
            graphics_protocol: self.graphics_protocol,
            perf_profile: self.perf_profile,
            detail_profile: self.detail_profile,
            clarity_profile: self.clarity_profile,
            ansi_quantization: self.ansi_quantization,
            backend: self.backend,
            center_lock: self.center_lock,
            center_lock_mode: self.center_lock_mode,
            stage_level: self.stage_level,
            stage_reactive: self.stage_reactive,
            material_color: self.material_color,
            texture_sampling: self.texture_sampling,
            model_lift: self.model_lift,
            edge_accent_strength: self.edge_accent_strength,
            braille_aspect_compensation: self.braille_aspect_compensation,
            color_mode: if matches!(self.mode, RenderMode::Ascii) {
                ColorMode::Ansi
            } else {
                self.color_mode
            },
            ascii_force_color: true,
            braille_profile: self.braille_profile,
            theme_style: self.theme_style,
            audio_reactive: self.audio_reactive,
            cinematic_camera: self.cinematic_camera,
            camera_focus: self.camera_focus,
            reactive_gain: self.reactive_gain,
            cell_aspect: self.manual_cell_aspect,
            cell_aspect_mode: self.cell_aspect_mode,
            cell_aspect_trim: self.cell_aspect_trim,
            contrast_profile: self.contrast_profile,
            sync_policy: self.sync_policy,
            sync_hard_snap_ms: self.sync_hard_snap_ms,
            sync_kp: self.sync_kp,
            ..RenderConfig::default()
        }
    }

    fn effective_cell_aspect(&self) -> f32 {
        resolve_cell_aspect(&self.preview_render_config(), self.detected_cell_aspect)
    }

    fn breakpoint(&self) -> UiBreakpoint {
        breakpoint_for(self.width, self.height)
    }

    fn is_too_small(&self) -> bool {
        self.width < MIN_WIDTH || self.height < MIN_HEIGHT
    }
}

#[cfg(feature = "gpu")]
fn gpu_available_once() -> bool {
    #[cfg(feature = "gpu")]
    {
        crate::render::gpu::GpuRenderer::is_available()
    }
}

#[derive(Debug, Clone)]
enum StartWizardAction {
    Continue,
    Cancel,
    Submit(StartSelection),
}

pub fn run_start_wizard(
    model_dir: &Path,
    pmx_dir: &Path,
    motion_dir: &Path,
    music_dir: &Path,
    stage_dir: &Path,
    camera_dir: &Path,
    model_files: &[PathBuf],
    pmx_files: &[PathBuf],
    motion_files: &[PathBuf],
    music_files: &[PathBuf],
    camera_files: &[PathBuf],
    stage_entries: &[StageChoice],
    defaults: StartWizardDefaults,
    ui_language: UiLanguage,
    anim_selector: Option<&str>,
) -> Result<Option<StartSelection>> {
    if model_files.is_empty() {
        return Ok(None);
    }

    let model_entries = model_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let pmx_entries = pmx_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let motion_entries = motion_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let music_entries = music_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let camera_entries = camera_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let stage_entries = stage_entries.to_vec();

    let mut terminal = RatatuiSession::enter_with_profile(TerminalProfile::detect())?;
    let (width, height) = terminal.size()?;
    let mut state = StartWizardState::new(
        model_entries,
        pmx_entries,
        motion_entries,
        music_entries,
        stage_entries,
        camera_entries,
        defaults,
        width,
        height,
    );

    loop {
        state.refresh_runtime_metrics(anim_selector);
        let (current_width, current_height) = terminal.size()?;
        state.on_resize(current_width, current_height);
        if safe_tui_size(current_width, current_height) {
            terminal.draw(|frame| {
                draw_start_wizard(
                    frame,
                    model_dir,
                    pmx_dir,
                    motion_dir,
                    music_dir,
                    stage_dir,
                    camera_dir,
                    &state,
                    ui_language,
                );
            })?;
        } else {
            draw_unsafe_size_fallback(current_width, current_height, ui_language)?;
        }

        let next_event = if event::poll(Duration::from_millis(120))? {
            Some(event::read()?)
        } else {
            None
        };

        let action = match next_event {
            Some(Event::Key(key)) => state.apply_event(StartWizardEvent::Key(key)),
            Some(Event::Resize(width, height)) => {
                state.apply_event(StartWizardEvent::Resize(width, height))
            }
            Some(_) => StartWizardAction::Continue,
            None => state.apply_event(StartWizardEvent::Tick),
        };

        match action {
            StartWizardAction::Continue => {}
            StartWizardAction::Cancel => return Ok(None),
            StartWizardAction::Submit(selection) => return Ok(Some(selection)),
        }
    }
}

fn safe_tui_size(width: u16, height: u16) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    let cells = (width as u32).saturating_mul(height as u32);
    cells < RATATUI_SAFE_MAX_CELLS
}

fn draw_unsafe_size_fallback(width: u16, height: u16, lang: UiLanguage) -> Result<()> {
    let mut stdout = io::stdout();
    let lines = vec![
        tr(
            lang,
            "터미널 크기 안정화 중입니다. 자동 복구를 기다려주세요.",
            "Terminal size is unstable. Waiting for auto recovery.",
        )
        .to_owned(),
        format!(
            "{}: {}x{}",
            tr(lang, "현재 크기", "Current size"),
            width,
            height
        ),
        format!(
            "{}: {}",
            tr(lang, "안전 셀 한계", "Safe cell limit"),
            RATATUI_SAFE_MAX_CELLS
        ),
        tr(lang, "q: 취소", "q: cancel").to_owned(),
    ];
    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            queue!(stdout, Print("\n"))?;
        }
        queue!(stdout, Print(line))?;
    }
    stdout.flush()?;
    Ok(())
}

fn draw_start_wizard(
    frame: &mut Frame,
    model_dir: &Path,
    pmx_dir: &Path,
    motion_dir: &Path,
    music_dir: &Path,
    stage_dir: &Path,
    camera_dir: &Path,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let area = clamp_ratatui_area(frame.area());
    if state.is_too_small() {
        draw_min_size_screen(frame, state, ui_language, area);
        return;
    }
    let breakpoint = state.breakpoint();
    let footer_height = match breakpoint {
        UiBreakpoint::Wide => 5,
        UiBreakpoint::Normal => 4,
        UiBreakpoint::Compact => 3,
    };
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(footer_height),
        ])
        .split(area);

    draw_header(frame, main[0], state, ui_language);

    match breakpoint {
        UiBreakpoint::Wide => {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
                .split(main[1]);
            draw_step_panel(frame, body[0], state, ui_language);
            draw_summary_panel(
                frame,
                body[1],
                model_dir,
                pmx_dir,
                motion_dir,
                music_dir,
                stage_dir,
                camera_dir,
                state,
                ui_language,
            );
        }
        UiBreakpoint::Normal => {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(10)])
                .split(main[1]);
            draw_step_panel(frame, body[0], state, ui_language);
            draw_summary_panel(
                frame,
                body[1],
                model_dir,
                pmx_dir,
                motion_dir,
                music_dir,
                stage_dir,
                camera_dir,
                state,
                ui_language,
            );
        }
        UiBreakpoint::Compact => {
            draw_step_panel(frame, main[1], state, ui_language);
        }
    }

    draw_help_panel(frame, main[2], state, ui_language, breakpoint);
}

#[cfg(test)]
mod tests;

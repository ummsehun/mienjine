use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    queue,
    style::Print,
    terminal::window_size,
    terminal::{Clear, ClearType},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use rodio::{Decoder, Source};

use crate::{
    loader,
    runtime::{config::UiLanguage, terminal::RatatuiSession},
    scene::{
        estimate_cell_aspect_from_window, resolve_cell_aspect, AnsiQuantization, AudioReactiveMode,
        BrailleProfile, CameraAlignPreset, CameraControlMode, CameraFocusMode, CameraMode,
        CellAspectMode, CenterLockMode, CinematicCameraMode, ClarityProfile, ColorMode,
        ContrastProfile, DetailProfile, GraphicsProtocol, PerfProfile, RenderBackend, RenderConfig,
        RenderMode, RenderOutputMode, SyncPolicy, SyncSpeedMode, TextureSamplingMode, ThemeStyle,
    },
};

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 18;
const START_FPS_OPTIONS: [u32; 9] = [0, 15, 20, 24, 30, 40, 60, 90, 120];
const RENDER_FIELD_COUNT: usize = 33;
const SYNC_OFFSET_STEP_MS: i32 = 10;
const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;
const RATATUI_SAFE_MAX_CELLS: u32 = (u16::MAX as u32) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartWizardStep {
    Model,
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
            StartWizardStep::Model => 0,
            StartWizardStep::Music => 1,
            StartWizardStep::Stage => 2,
            StartWizardStep::Camera => 3,
            StartWizardStep::Render => 4,
            StartWizardStep::AspectCalib => 5,
            StartWizardStep::Confirm => 6,
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
    pub glb_path: PathBuf,
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
    model_entries: Vec<StartEntry>,
    music_entries: Vec<StartEntry>,
    stage_entries: Vec<StageChoice>,
    camera_entries: Vec<StartEntry>,
    model_index: usize,
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
            step: StartWizardStep::Model,
            model_entries,
            music_entries,
            stage_entries,
            camera_entries,
            model_index: 0,
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
            StartWizardStep::Model => self.apply_model_key(key),
            StartWizardStep::Music => self.apply_music_key(key),
            StartWizardStep::Stage => self.apply_stage_key(key),
            StartWizardStep::Camera => self.apply_camera_key(key),
            StartWizardStep::Render => self.apply_render_key(key),
            StartWizardStep::AspectCalib => self.apply_aspect_key(key),
            StartWizardStep::Confirm => self.apply_confirm_key(key),
        }
    }

    fn apply_model_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.model_index, self.model_entries.len(), -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.model_index, self.model_entries.len(), 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Music;
                StartWizardAction::Continue
            }
            KeyCode::Esc => StartWizardAction::Cancel,
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
            StartWizardStep::Model => self.step = StartWizardStep::Music,
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
            StartWizardStep::Model => {}
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
        let glb_path = self.model_entries[self.model_index].path.clone();
        let stage_choice = self.selected_stage_choice();
        let stage_transform = stage_choice
            .as_ref()
            .map(|choice| choice.transform)
            .unwrap_or_default();
        StartSelection {
            glb_path,
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

    fn selected_clip_duration_secs(&self) -> Option<f32> {
        let path = self.model_entries.get(self.model_index)?.path.clone();
        self.clip_duration_cache.get(&path).and_then(|value| *value)
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
    music_dir: &Path,
    stage_dir: &Path,
    camera_dir: &Path,
    model_files: &[PathBuf],
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
    let music_entries = music_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let camera_entries = camera_files
        .iter()
        .map(|path| StartEntry::from_path(path))
        .collect::<Vec<_>>();
    let stage_entries = stage_entries.to_vec();

    let mut terminal = RatatuiSession::enter()?;
    let (width, height) = terminal.size()?;
    let mut state = StartWizardState::new(
        model_entries,
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

fn draw_header(frame: &mut Frame, area: Rect, state: &StartWizardState, ui_language: UiLanguage) {
    let title = tr(
        ui_language,
        "Terminal Miku 3D 시작 설정",
        "Terminal Miku 3D Setup",
    );
    let step_name = match state.step {
        StartWizardStep::Model => tr(ui_language, "모델 선택", "Model"),
        StartWizardStep::Music => tr(ui_language, "음악 선택", "Music"),
        StartWizardStep::Stage => tr(ui_language, "스테이지 선택", "Stage"),
        StartWizardStep::Camera => tr(ui_language, "카메라 선택", "Camera"),
        StartWizardStep::Render => tr(ui_language, "렌더 옵션", "Render"),
        StartWizardStep::AspectCalib => tr(ui_language, "비율 보정", "Aspect Calib"),
        StartWizardStep::Confirm => tr(ui_language, "확인/실행", "Confirm"),
    };
    let line = Line::from(vec![
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  •  "),
        Span::raw(format!("{} {}/7", step_name, state.step.index() + 1)),
    ]);

    let para = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(para, area);
}

fn draw_step_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    match state.step {
        StartWizardStep::Model => draw_model_list(frame, area, state, ui_language),
        StartWizardStep::Music => draw_music_list(frame, area, state, ui_language),
        StartWizardStep::Stage => draw_stage_list(frame, area, state, ui_language),
        StartWizardStep::Camera => draw_camera_panel(frame, area, state, ui_language),
        StartWizardStep::Render => draw_render_options(frame, area, state, ui_language),
        StartWizardStep::AspectCalib => draw_aspect_calibration(frame, area, state, ui_language),
        StartWizardStep::Confirm => draw_confirm_panel(frame, area, state, ui_language),
    }
}

fn draw_model_list(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "1) 모델 선택", "1) Select Model");
    let items = state
        .model_entries
        .iter()
        .map(|entry| ListItem::new(entry.label()))
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    list_state.select(Some(state.model_index));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_music_list(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "2) 음악 선택", "2) Select Music");
    let mut items = Vec::with_capacity(state.music_entries.len() + 1);
    items.push(ListItem::new(tr(ui_language, "없음", "None")));
    items.extend(
        state
            .music_entries
            .iter()
            .map(|entry| ListItem::new(entry.label())),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(state.music_index));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_stage_list(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(
        ui_language,
        "3) 스테이지를 선택해 주세요",
        "3) Select Stage",
    );
    let mut items = Vec::with_capacity(state.stage_entries.len() + 1);
    items.push(ListItem::new(tr(ui_language, "없음", "None")));
    items.extend(state.stage_entries.iter().map(|entry| {
        let badge = match entry.status {
            StageStatus::Ready => tr(ui_language, "사용 가능", "Ready"),
            StageStatus::NeedsConvert => tr(ui_language, "PMX 변환 필요", "Needs PMX->GLB"),
            StageStatus::Invalid => tr(ui_language, "사용 불가", "Invalid"),
        };
        ListItem::new(format!("{}  [{}]", entry.name, badge))
    }));
    let mut list_state = ListState::default();
    list_state.select(Some(state.stage_index));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_camera_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "4) 카메라 선택", "4) Select Camera");
    let camera_source = if state.camera_index == 0 {
        tr(ui_language, "없음", "None").to_owned()
    } else {
        state
            .camera_entries
            .get(state.camera_index.saturating_sub(1))
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| tr(ui_language, "없음", "None").to_owned())
    };
    let camera_mode = match state.camera_mode {
        CameraMode::Off => "off",
        CameraMode::Vmd => "vmd",
        CameraMode::Blend => "blend",
    };
    let align = match state.camera_align_preset {
        CameraAlignPreset::Std => "std",
        CameraAlignPreset::AltA => "alt-a",
        CameraAlignPreset::AltB => "alt-b",
    };
    let rows = vec![
        format!("{}: {}", tr(ui_language, "소스", "Source"), camera_source),
        format!(
            "{}: {}",
            tr(ui_language, "모드", "Mode"),
            if state.camera_index == 0 {
                "off"
            } else {
                camera_mode
            }
        ),
        format!("{}: {}", tr(ui_language, "프리셋", "Preset"), align),
        format!(
            "{}: {:.2}",
            tr(ui_language, "유닛 스케일", "Unit Scale"),
            state.camera_unit_scale
        ),
    ];
    let items = rows.into_iter().map(ListItem::new).collect::<Vec<_>>();
    let mut list_state = ListState::default();
    list_state.select(Some(state.camera_focus_index.min(3)));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_render_options(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "4) 렌더 옵션", "4) Render Options");
    let mode = match state.mode {
        RenderMode::Ascii => "ASCII",
        RenderMode::Braille => "Braille",
    };
    let perf_profile = match state.perf_profile {
        PerfProfile::Balanced => tr(ui_language, "균형 30FPS", "Balanced 30FPS"),
        PerfProfile::Cinematic => tr(ui_language, "시네마 20FPS", "Cinematic 20FPS"),
        PerfProfile::Smooth => tr(ui_language, "부드러움 45FPS", "Smooth 45FPS"),
    };
    let detail_profile = match state.detail_profile {
        DetailProfile::Perf => tr(ui_language, "성능", "Perf"),
        DetailProfile::Balanced => tr(ui_language, "균형", "Balanced"),
        DetailProfile::Ultra => tr(ui_language, "고품질", "Ultra"),
    };
    let clarity_profile = match state.clarity_profile {
        ClarityProfile::Balanced => tr(ui_language, "균형", "Balanced"),
        ClarityProfile::Sharp => tr(ui_language, "선명", "Sharp"),
        ClarityProfile::Extreme => tr(ui_language, "극선명", "Extreme"),
    };
    let ansi_quantization = match state.ansi_quantization {
        AnsiQuantization::Q216 => "q216",
        AnsiQuantization::Off => tr(ui_language, "끄기(truecolor)", "off (truecolor)"),
    };
    let backend = match state.backend {
        RenderBackend::Cpu => "CPU",
        #[cfg(feature = "gpu")]
        RenderBackend::Gpu => {
            if state.gpu_available {
                "GPU (Metal)"
            } else {
                "CPU (GPU unavailable)"
            }
        }
        #[cfg(not(feature = "gpu"))]
        RenderBackend::Gpu => "CPU (GPU not compiled)",
    };
    let center_lock = if state.center_lock {
        tr(ui_language, "켜짐", "On")
    } else {
        tr(ui_language, "꺼짐", "Off")
    };
    let color_mode = if matches!(state.mode, RenderMode::Ascii) {
        tr(ui_language, "항상 ON (ANSI)", "Always ON (ANSI)")
    } else {
        match state.color_mode {
            ColorMode::Mono => tr(ui_language, "모노", "Mono"),
            ColorMode::Ansi => tr(ui_language, "ANSI", "ANSI"),
        }
    };
    let braille_profile = match state.braille_profile {
        BrailleProfile::Safe => tr(ui_language, "안전", "Safe"),
        BrailleProfile::Normal => tr(ui_language, "표준", "Normal"),
        BrailleProfile::Dense => tr(ui_language, "고밀도", "Dense"),
    };
    let theme = match state.theme_style {
        ThemeStyle::Theater => tr(ui_language, "극장", "Theater"),
        ThemeStyle::Neon => tr(ui_language, "네온", "Neon"),
        ThemeStyle::Holo => tr(ui_language, "홀로그램", "Hologram"),
    };
    let audio_reactive = match state.audio_reactive {
        AudioReactiveMode::Off => tr(ui_language, "끔", "Off"),
        AudioReactiveMode::On => tr(ui_language, "보통", "On"),
        AudioReactiveMode::High => tr(ui_language, "강함", "High"),
    };
    let cinematic = match state.cinematic_camera {
        CinematicCameraMode::Off => tr(ui_language, "끔", "Off"),
        CinematicCameraMode::On => tr(ui_language, "보통", "On"),
        CinematicCameraMode::Aggressive => tr(ui_language, "강함", "Aggressive"),
    };
    let contrast = match state.contrast_profile {
        ContrastProfile::Adaptive => tr(ui_language, "적응형", "Adaptive"),
        ContrastProfile::Fixed => tr(ui_language, "고정", "Fixed"),
    };
    let sync_mode = match state.sync_speed_mode {
        SyncSpeedMode::AutoDurationFit => tr(ui_language, "자동", "Auto"),
        SyncSpeedMode::Realtime1x => tr(ui_language, "실시간", "Realtime"),
    };
    let output_mode = match state.output_mode {
        RenderOutputMode::Text => tr(ui_language, "텍스트", "Text"),
        RenderOutputMode::Hybrid => tr(ui_language, "하이브리드", "Hybrid"),
        RenderOutputMode::KittyHq => tr(ui_language, "Kitty HQ", "Kitty HQ"),
    };
    let graphics_protocol = match state.graphics_protocol {
        GraphicsProtocol::Auto => "auto",
        GraphicsProtocol::Kitty => "kitty",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::None => "none",
    };
    let sync_policy = match state.sync_policy {
        SyncPolicy::Continuous => tr(ui_language, "연속", "Continuous"),
        SyncPolicy::Fixed => tr(ui_language, "고정", "Fixed"),
        SyncPolicy::Manual => tr(ui_language, "수동", "Manual"),
    };
    let aspect_mode = match state.cell_aspect_mode {
        CellAspectMode::Auto => tr(ui_language, "자동", "Auto"),
        CellAspectMode::Manual => tr(ui_language, "수동", "Manual"),
    };
    let font = if state.font_preset_enabled {
        tr(ui_language, "켜짐", "On")
    } else {
        tr(ui_language, "꺼짐", "Off")
    };
    let center_lock_mode = match state.center_lock_mode {
        CenterLockMode::Root => tr(ui_language, "루트", "Root"),
        CenterLockMode::Mixed => tr(ui_language, "혼합", "Mixed"),
    };
    let camera_focus = match state.camera_focus {
        CameraFocusMode::Auto => tr(ui_language, "자동", "Auto"),
        CameraFocusMode::Full => tr(ui_language, "전신", "Full"),
        CameraFocusMode::Upper => tr(ui_language, "상반신", "Upper"),
        CameraFocusMode::Face => tr(ui_language, "얼굴", "Face"),
        CameraFocusMode::Hands => tr(ui_language, "손", "Hands"),
    };
    let wasd_mode = match state.wasd_mode {
        CameraControlMode::Orbit => tr(ui_language, "오빗", "Orbit"),
        CameraControlMode::FreeFly => tr(ui_language, "자유이동", "FreeFly"),
    };
    let material_color = if state.material_color {
        tr(ui_language, "켜짐", "On")
    } else {
        tr(ui_language, "꺼짐", "Off")
    };
    let texture_sampling = match state.texture_sampling {
        TextureSamplingMode::Nearest => tr(ui_language, "최근접", "Nearest"),
        TextureSamplingMode::Bilinear => tr(ui_language, "쌍선형", "Bilinear"),
    };

    let rows = [
        format!("{}: {}", tr(ui_language, "모드", "Mode"), mode),
        format!(
            "{}: {}",
            tr(ui_language, "성능 프로필", "Perf Profile"),
            perf_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "디테일 프로필", "Detail Profile"),
            detail_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "선명도 프로필", "Clarity Profile"),
            clarity_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "ANSI 양자화", "ANSI Quantization"),
            ansi_quantization
        ),
        format!("{}: {}", tr(ui_language, "백엔드", "Backend"), backend),
        format!(
            "{}: {}",
            tr(ui_language, "중앙 고정", "Center Lock"),
            center_lock
        ),
        format!(
            "{}: {}",
            tr(ui_language, "중앙 고정 기준", "Center Lock Mode"),
            center_lock_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "WASD 모드", "WASD Mode"),
            wasd_mode
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "자유이동 속도", "FreeFly Speed"),
            state.freefly_speed
        ),
        format!(
            "{}: {}",
            tr(ui_language, "카메라 포커스", "Camera Focus"),
            camera_focus
        ),
        format!(
            "{}: {}",
            tr(ui_language, "재질 색상", "Material Color"),
            material_color
        ),
        format!(
            "{}: {}",
            tr(ui_language, "텍스처 샘플링", "Texture Sampling"),
            texture_sampling
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "모델 리프트", "Model Lift"),
            state.model_lift
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "엣지 강조", "Edge Accent"),
            state.edge_accent_strength
        ),
        format!(
            "{}: {}",
            tr(ui_language, "스테이지 레벨", "Stage Level"),
            state.stage_level
        ),
        format!(
            "{}: {}",
            tr(ui_language, "컬러 모드", "Color Mode"),
            color_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "Braille 프로필", "Braille Profile"),
            braille_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "분위기/조명 스타일", "Mood/Lighting"),
            theme
        ),
        format!(
            "{}: {}",
            tr(ui_language, "음악 반응", "Audio Reactive"),
            audio_reactive
        ),
        format!(
            "{}: {}",
            tr(ui_language, "시네마틱 카메라", "Cinematic Camera"),
            cinematic
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "반응 게인", "Reactive Gain"),
            state.reactive_gain
        ),
        format!(
            "{}: {}",
            tr(ui_language, "FPS 제한", "FPS Cap"),
            fps_label(START_FPS_OPTIONS[state.fps_index], ui_language)
        ),
        format!(
            "{}: {}",
            tr(ui_language, "대비 프로필", "Contrast Profile"),
            contrast
        ),
        format!(
            "{}: {} ms",
            tr(ui_language, "동기화 오프셋", "Sync Offset"),
            state.sync_offset_ms
        ),
        format!(
            "{}: {}",
            tr(ui_language, "동기화 속도", "Sync Speed"),
            sync_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "출력 모드", "Output Mode"),
            output_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "그래픽 프로토콜", "Graphics Protocol"),
            graphics_protocol
        ),
        format!(
            "{}: {}",
            tr(ui_language, "동기화 정책", "Sync Policy"),
            sync_policy
        ),
        format!(
            "{}: {} ms",
            tr(ui_language, "하드 스냅", "Hard Snap"),
            state.sync_hard_snap_ms
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "동기화 Kp", "Sync Kp"),
            state.sync_kp
        ),
        format!(
            "{}: {}",
            tr(ui_language, "셀 비율 모드", "Cell Aspect Mode"),
            aspect_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "폰트 프리셋", "Font Preset"),
            font
        ),
    ];

    let items = rows
        .iter()
        .map(|text| ListItem::new(text.clone()))
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    list_state.select(Some(state.render_focus_index));
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_aspect_calibration(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let detected_label = state
        .detected_cell_aspect
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "n/a".to_owned());
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(6)])
        .split(area);

    let info = vec![
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "감지 비율", "Detected"),
            detected_label
        )),
        Line::raw(format!(
            "{}: {:?}",
            tr(ui_language, "모드", "Mode"),
            state.cell_aspect_mode
        )),
        Line::raw(format!(
            "{}: {:.2}",
            tr(ui_language, "Trim", "Trim"),
            state.cell_aspect_trim
        )),
        Line::raw(format!(
            "{}: {:.3}",
            tr(ui_language, "적용 비율", "Applied"),
            state.effective_cell_aspect()
        )),
    ];

    let info_widget = Paragraph::new(info)
        .block(
            Block::default()
                .title(tr(ui_language, "5) 비율 보정", "5) Aspect Calibration"))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(info_widget, chunks[0]);

    let preview = aspect_preview_ascii(
        chunks[1].width.saturating_sub(2),
        chunks[1].height.saturating_sub(2),
        state.effective_cell_aspect(),
    );
    let preview_widget = Paragraph::new(preview)
        .block(
            Block::default()
                .title(tr(ui_language, "원형 프리뷰", "Circle Preview"))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(preview_widget, chunks[1]);
}

fn draw_confirm_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let selection = state.selection();
    let model_name = selection
        .glb_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<invalid>");
    let music_name = selection
        .music_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| tr(ui_language, "없음", "None").to_owned());
    let detected_label = state
        .detected_cell_aspect
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "n/a".to_owned());

    let clip_duration = state.selected_clip_duration_secs();
    let audio_duration = state.selected_audio_duration_secs();
    let speed = state.expected_sync_speed();
    let color_mode = if matches!(selection.mode, RenderMode::Ascii) {
        "ANSI (ASCII fixed)"
    } else {
        match selection.color_mode {
            ColorMode::Mono => "Mono",
            ColorMode::Ansi => "ANSI",
        }
    };
    let perf_profile = match selection.perf_profile {
        PerfProfile::Balanced => "Balanced",
        PerfProfile::Cinematic => "Cinematic",
        PerfProfile::Smooth => "Smooth",
    };
    let detail_profile = match selection.detail_profile {
        DetailProfile::Perf => "Perf",
        DetailProfile::Balanced => "Balanced",
        DetailProfile::Ultra => "Ultra",
    };
    let backend = match selection.backend {
        RenderBackend::Cpu => "CPU",
        #[cfg(feature = "gpu")]
        RenderBackend::Gpu => {
            if state.gpu_available {
                "GPU (Metal)"
            } else {
                "CPU (GPU unavailable)"
            }
        }
        #[cfg(not(feature = "gpu"))]
        RenderBackend::Gpu => "CPU (GPU not compiled)",
    };
    let braille_profile = match selection.braille_profile {
        BrailleProfile::Safe => "Safe",
        BrailleProfile::Normal => "Normal",
        BrailleProfile::Dense => "Dense",
    };
    let theme_style = match selection.theme_style {
        ThemeStyle::Theater => "Theater",
        ThemeStyle::Neon => "Neon",
        ThemeStyle::Holo => "Holo",
    };
    let audio_reactive = match selection.audio_reactive {
        AudioReactiveMode::Off => "Off",
        AudioReactiveMode::On => "On",
        AudioReactiveMode::High => "High",
    };
    let cinematic_camera = match selection.cinematic_camera {
        CinematicCameraMode::Off => "Off",
        CinematicCameraMode::On => "On",
        CinematicCameraMode::Aggressive => "Aggressive",
    };
    let wasd_mode = match selection.wasd_mode {
        CameraControlMode::Orbit => "Orbit",
        CameraControlMode::FreeFly => "FreeFly",
    };
    let clarity_profile = match selection.clarity_profile {
        ClarityProfile::Balanced => "Balanced",
        ClarityProfile::Sharp => "Sharp",
        ClarityProfile::Extreme => "Extreme",
    };
    let color_path = match selection.ansi_quantization {
        AnsiQuantization::Q216 => "ANSI q216",
        AnsiQuantization::Off => "ANSI truecolor",
    };
    let output_mode = match selection.output_mode {
        RenderOutputMode::Text => "Text",
        RenderOutputMode::Hybrid => "Hybrid",
        RenderOutputMode::KittyHq => "KittyHq",
    };
    let graphics_protocol = match selection.graphics_protocol {
        GraphicsProtocol::Auto => "auto",
        GraphicsProtocol::Kitty => "kitty",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::None => "none",
    };
    let sync_policy = match selection.sync_policy {
        SyncPolicy::Continuous => "continuous",
        SyncPolicy::Fixed => "fixed",
        SyncPolicy::Manual => "manual",
    };
    let stage_name = selection
        .stage_choice
        .as_ref()
        .map(|choice| choice.name.as_str())
        .unwrap_or_else(|| tr(ui_language, "없음", "None"));
    let stage_status = selection
        .stage_choice
        .as_ref()
        .map(|choice| match choice.status {
            StageStatus::Ready => tr(ui_language, "사용 가능", "Ready"),
            StageStatus::NeedsConvert => tr(ui_language, "PMX 변환 필요", "Needs PMX->GLB"),
            StageStatus::Invalid => tr(ui_language, "사용 불가", "Invalid"),
        })
        .unwrap_or_else(|| tr(ui_language, "선택 안함", "Not selected"));
    let camera_name = selection
        .camera_vmd_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| tr(ui_language, "없음", "None").to_owned());
    let camera_mode = match selection.camera_mode {
        CameraMode::Off => "off",
        CameraMode::Vmd => "vmd",
        CameraMode::Blend => "blend",
    };
    let camera_align = match selection.camera_align_preset {
        CameraAlignPreset::Std => "std",
        CameraAlignPreset::AltA => "alt-a",
        CameraAlignPreset::AltB => "alt-b",
    };

    let lines = vec![
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "모델", "Model"),
            model_name
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "음악", "Music"),
            music_name
        )),
        Line::raw(format!(
            "{}: {} ({})",
            tr(ui_language, "스테이지", "Stage"),
            stage_name,
            stage_status
        )),
        Line::raw(format!(
            "{}: {} / {} / {} / {:.2}",
            tr(ui_language, "카메라", "Camera"),
            camera_name,
            camera_mode,
            camera_align,
            selection.camera_unit_scale
        )),
        Line::raw(format!(
            "{}: {:?}",
            tr(ui_language, "렌더 모드", "Render"),
            selection.mode
        )),
        Line::raw(format!(
            "{}: {} / {} / {} / {}",
            tr(
                ui_language,
                "프로필/디테일/선명도/백엔드",
                "Profile/Detail/Clarity/Backend"
            ),
            perf_profile,
            detail_profile,
            clarity_profile,
            backend
        )),
        Line::raw(format!(
            "{}: {} ({:?}) / {}",
            tr(ui_language, "중앙고정/스테이지", "Center/Stage"),
            if selection.center_lock { "On" } else { "Off" },
            selection.center_lock_mode,
            selection.stage_level
        )),
        Line::raw(format!(
            "{}: {:?}",
            tr(ui_language, "카메라 포커스", "Camera Focus"),
            selection.camera_focus
        )),
        Line::raw(format!(
            "{}: {} ({:.2})",
            tr(ui_language, "WASD 모드/속도", "WASD Mode/Speed"),
            wasd_mode,
            selection.freefly_speed
        )),
        Line::raw(format!(
            "{}: {} / {:?}",
            tr(ui_language, "재질색상/샘플링", "Material/Sampling"),
            if selection.material_color {
                "On"
            } else {
                "Off"
            },
            selection.texture_sampling
        )),
        Line::raw(format!(
            "{}: {} / {} / {}",
            tr(ui_language, "컬러/프로필/경로", "Color/Profile/Path"),
            color_mode,
            braille_profile,
            color_path
        )),
        Line::raw(format!(
            "{}: {} / {}",
            tr(ui_language, "출력/프로토콜", "Output/Protocol"),
            output_mode,
            graphics_protocol
        )),
        Line::raw(format!(
            "{}: {} / {}",
            tr(ui_language, "분위기/반응", "Mood/Reactive"),
            theme_style,
            audio_reactive
        )),
        Line::raw(format!(
            "{}: {} ({:.2})",
            tr(ui_language, "시네마틱/게인", "Cinematic/Gain"),
            cinematic_camera,
            selection.reactive_gain
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "FPS", "FPS"),
            fps_label(selection.fps_cap, ui_language)
        )),
        Line::raw(format!(
            "{}: {:.1}fps",
            tr(ui_language, "목표 FPS", "Target FPS"),
            target_fps_for_profile(selection.perf_profile)
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "감지 비율", "Detected Aspect"),
            detected_label
        )),
        Line::raw(format!(
            "{}: {:.3}",
            tr(ui_language, "적용 비율", "Applied Aspect"),
            state.effective_cell_aspect()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "클립 길이", "Clip Duration"),
            duration_label(clip_duration)
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "음악 길이", "Audio Duration"),
            duration_label(audio_duration)
        )),
        Line::raw(format!(
            "{}: {:.4}",
            tr(ui_language, "속도 계수", "Speed Factor"),
            speed
        )),
        Line::raw(format!(
            "{}: {} ms",
            tr(ui_language, "동기화 오프셋", "Sync Offset"),
            selection.sync_offset_ms
        )),
        Line::raw(format!(
            "{}: {} / {}ms / kp {:.2}",
            tr(ui_language, "동기화 정책", "Sync Policy"),
            sync_policy,
            selection.sync_hard_snap_ms,
            selection.sync_kp
        )),
        Line::raw(""),
        Line::styled(
            tr(
                ui_language,
                "Enter로 실행, Esc로 이전 단계",
                "Press Enter to run, Esc to go back",
            ),
            Style::default().fg(Color::Cyan),
        ),
    ];

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .title(tr(ui_language, "6) 확인 / 실행", "6) Confirm / Run"))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn draw_summary_panel(
    frame: &mut Frame,
    area: Rect,
    model_dir: &Path,
    music_dir: &Path,
    stage_dir: &Path,
    camera_dir: &Path,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let selection = state.selection();
    let model_name = selection
        .glb_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<invalid>");
    let music_name = selection
        .music_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| tr(ui_language, "없음", "None").to_owned());
    let stage_name = selection
        .stage_choice
        .as_ref()
        .map(|choice| choice.name.as_str())
        .unwrap_or_else(|| tr(ui_language, "없음", "None"));
    let stage_status = selection
        .stage_choice
        .as_ref()
        .map(|choice| match choice.status {
            StageStatus::Ready => tr(ui_language, "사용 가능", "Ready"),
            StageStatus::NeedsConvert => tr(ui_language, "PMX 변환 필요", "Needs PMX->GLB"),
            StageStatus::Invalid => tr(ui_language, "사용 불가", "Invalid"),
        })
        .unwrap_or_else(|| tr(ui_language, "선택 안함", "Not selected"));
    let camera_name = selection
        .camera_vmd_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| tr(ui_language, "없음", "None").to_owned());

    let lines = vec![
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "모델 경로", "Model Dir"),
            model_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "음악 경로", "Music Dir"),
            music_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "스테이지 경로", "Stage Dir"),
            stage_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "카메라 경로", "Camera Dir"),
            camera_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "모델", "Model"),
            model_name
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "음악", "Music"),
            music_name
        )),
        Line::raw(format!(
            "{}: {} ({})",
            tr(ui_language, "스테이지", "Stage"),
            stage_name,
            stage_status
        )),
        Line::raw(format!(
            "{}: {} / {:?} / {:?} / {:.2}",
            tr(ui_language, "카메라", "Camera"),
            camera_name,
            selection.camera_mode,
            selection.camera_align_preset,
            selection.camera_unit_scale
        )),
        Line::raw(format!(
            "{}: {:.3}",
            tr(ui_language, "적용 비율", "Applied Aspect"),
            state.effective_cell_aspect()
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "모드", "Mode"),
            selection.mode,
            selection.color_mode
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "출력/프로토콜", "Output/Protocol"),
            selection.output_mode,
            selection.graphics_protocol
        )),
        Line::raw(format!(
            "{}: {:?} / {:?} / {:?} / {:?}",
            tr(
                ui_language,
                "프로필/디테일/선명도/백엔드",
                "Profile/Detail/Clarity/Backend"
            ),
            selection.perf_profile,
            selection.detail_profile,
            selection.clarity_profile,
            selection.backend
        )),
        Line::raw(format!(
            "{}: {}({:?}) / {}",
            tr(ui_language, "중앙고정/스테이지", "Center/Stage"),
            if selection.center_lock { "On" } else { "Off" },
            selection.center_lock_mode,
            selection.stage_level
        )),
        Line::raw(format!(
            "{}: {:?}",
            tr(ui_language, "카메라 포커스", "Camera Focus"),
            selection.camera_focus
        )),
        Line::raw(format!(
            "{}: {:?} ({:.2})",
            tr(ui_language, "WASD 모드/속도", "WASD Mode/Speed"),
            selection.wasd_mode,
            selection.freefly_speed
        )),
        Line::raw(format!(
            "{}: {} / {:?}",
            tr(ui_language, "재질색상/샘플링", "Material/Sampling"),
            if selection.material_color {
                "On"
            } else {
                "Off"
            },
            selection.texture_sampling
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "Braille/색경로", "Braille/Color Path"),
            selection.braille_profile,
            selection.ansi_quantization
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "테마/반응", "Theme/Reactive"),
            selection.theme_style,
            selection.audio_reactive
        )),
        Line::raw(format!(
            "{}: {:?}",
            tr(ui_language, "시네마틱 카메라", "Cinematic Camera"),
            selection.cinematic_camera
        )),
        Line::raw(format!(
            "{}: {:.2}",
            tr(ui_language, "반응 게인", "Reactive Gain"),
            selection.reactive_gain
        )),
        Line::raw(format!(
            "{}: {}ms",
            tr(ui_language, "Offset", "Offset"),
            state.sync_offset_ms
        )),
        Line::raw(format!(
            "{}: {:.4}",
            tr(ui_language, "Speed", "Speed"),
            state.expected_sync_speed()
        )),
        Line::raw(format!(
            "{}: {:?} / {}ms / kp {:.2}",
            tr(ui_language, "정책", "Policy"),
            selection.sync_policy,
            selection.sync_hard_snap_ms,
            selection.sync_kp
        )),
    ];

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .title(tr(ui_language, "선택 요약", "Selection Summary"))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn draw_help_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
    breakpoint: UiBreakpoint,
) {
    let mut lines = Vec::new();
    lines.push(Line::raw(match state.step {
        StartWizardStep::Model => tr(
            ui_language,
            "모델: ↑/↓ 선택, Enter 다음, Esc 취소",
            "Model: ↑/↓ select, Enter next, Esc cancel",
        ),
        StartWizardStep::Music => tr(
            ui_language,
            "음악: ↑/↓ 선택, Enter 다음, Esc 이전",
            "Music: ↑/↓ select, Enter next, Esc back",
        ),
        StartWizardStep::Stage => tr(
            ui_language,
            "스테이지: ↑/↓ 선택, Enter 다음, Esc 이전",
            "Stage: ↑/↓ select, Enter next, Esc back",
        ),
        StartWizardStep::Camera => tr(
            ui_language,
            "카메라: ↑/↓ 항목, ←/→ 값 변경, Enter 다음, Esc 이전",
            "Camera: ↑/↓ focus, ←/→ change, Enter next, Esc back",
        ),
        StartWizardStep::Render => tr(
            ui_language,
            "옵션: ↑/↓ 항목, ←/→ 값 변경, Enter 다음, Esc 이전",
            "Options: ↑/↓ focus, ←/→ change, Enter next, Esc back",
        ),
        StartWizardStep::AspectCalib => tr(
            ui_language,
            "보정: ←/→ trim, r 리셋, Enter 다음, Esc 이전",
            "Calib: ←/→ trim, r reset, Enter next, Esc back",
        ),
        StartWizardStep::Confirm => tr(
            ui_language,
            "확인: Enter 실행, Esc 이전",
            "Confirm: Enter run, Esc back",
        ),
    }));

    if breakpoint != UiBreakpoint::Compact {
        lines.push(Line::raw(tr(
            ui_language,
            "공통: q 취소, Tab 다음, Shift+Tab 이전",
            "Common: q cancel, Tab next, Shift+Tab prev",
        )));
    }

    let help = Paragraph::new(lines)
        .block(
            Block::default()
                .title(tr(ui_language, "조작", "Help"))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(help, area);
}

fn draw_min_size_screen(
    frame: &mut Frame,
    state: &StartWizardState,
    ui_language: UiLanguage,
    area: Rect,
) {
    let title = tr(
        ui_language,
        "터미널 크기가 너무 작습니다",
        "Terminal is too small",
    );
    let lines = vec![
        Line::raw(format!(
            "{}: {}x{}",
            tr(ui_language, "현재 크기", "Current size"),
            state.width,
            state.height
        )),
        Line::raw(format!(
            "{}: {}x{}",
            tr(ui_language, "최소 요구", "Minimum required"),
            MIN_WIDTH,
            MIN_HEIGHT
        )),
        Line::raw(""),
        Line::raw(tr(
            ui_language,
            "터미널을 늘리면 자동으로 복귀합니다.",
            "Resize terminal and UI will recover automatically.",
        )),
        Line::raw(tr(ui_language, "q: 종료", "q: quit")),
    ];
    let para = Paragraph::new(lines)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn clamp_ratatui_area(area: Rect) -> Rect {
    let cells = (area.width as u32).saturating_mul(area.height as u32);
    if cells <= RATATUI_SAFE_MAX_CELLS {
        return area;
    }
    let aspect = if area.height == 0 {
        1.0
    } else {
        (area.width as f32 / area.height as f32).max(0.1)
    };
    let h = ((RATATUI_SAFE_MAX_CELLS as f32 / aspect).sqrt().floor() as u16).max(1);
    let w = ((h as f32 * aspect).floor() as u16).max(1);
    Rect {
        x: area.x,
        y: area.y,
        width: w,
        height: h,
    }
}

fn target_fps_for_profile(profile: PerfProfile) -> f32 {
    match profile {
        PerfProfile::Balanced => 30.0,
        PerfProfile::Cinematic => 20.0,
        PerfProfile::Smooth => 45.0,
    }
}

fn tr<'a>(lang: UiLanguage, ko: &'a str, en: &'a str) -> &'a str {
    match lang {
        UiLanguage::Ko => ko,
        UiLanguage::En => en,
    }
}

fn cycle_index(index: &mut usize, len: usize, delta: i32) {
    if len == 0 {
        *index = 0;
        return;
    }
    if delta > 0 {
        *index = (*index + 1) % len;
    } else if delta < 0 {
        *index = if *index == 0 { len - 1 } else { *index - 1 };
    }
}

fn closest_u32_index(value: u32, options: &[u32]) -> usize {
    options
        .iter()
        .enumerate()
        .min_by_key(|(_, option)| option.abs_diff(value))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn breakpoint_for(width: u16, height: u16) -> UiBreakpoint {
    if width >= 140 && height >= 40 {
        UiBreakpoint::Wide
    } else if width >= 100 && height >= 28 {
        UiBreakpoint::Normal
    } else {
        UiBreakpoint::Compact
    }
}

fn format_mib(bytes: u64) -> String {
    let mib = (bytes as f64) / (1024.0 * 1024.0);
    format!("{mib:.1} MiB")
}

fn duration_label(seconds: Option<f32>) -> String {
    seconds
        .map(|v| format!("{v:.3}s"))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn fps_label(fps: u32, lang: UiLanguage) -> String {
    if fps == 0 {
        tr(lang, "무제한", "Unlimited").to_owned()
    } else {
        fps.to_string()
    }
}

fn detect_terminal_cell_aspect() -> Option<f32> {
    let ws = window_size().ok()?;
    estimate_cell_aspect_from_window(ws.columns, ws.rows, ws.width, ws.height)
}

fn inspect_clip_duration(path: &Path, anim_selector: Option<&str>) -> Option<f32> {
    let scene = loader::load_gltf(path).ok()?;
    if scene.animations.is_empty() {
        return None;
    }
    if let Some(selector) = anim_selector {
        let index = scene.animation_index_by_selector(Some(selector))?;
        return scene.animations.get(index).map(|clip| clip.duration);
    }
    scene.animations.first().map(|clip| clip.duration)
}

fn inspect_audio_duration(path: &Path) -> Option<f32> {
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    decoder.total_duration().map(|d| d.as_secs_f32())
}

fn compute_duration_fit_factor(
    clip_duration_secs: Option<f32>,
    audio_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> f32 {
    if !matches!(mode, SyncSpeedMode::AutoDurationFit) {
        return 1.0;
    }
    let Some(clip) = clip_duration_secs else {
        return 1.0;
    };
    let Some(audio) = audio_duration_secs else {
        return 1.0;
    };
    if clip <= f32::EPSILON || audio <= f32::EPSILON {
        return 1.0;
    }
    let factor = clip / audio;
    if (0.85..=1.15).contains(&factor) {
        factor
    } else {
        1.0
    }
}

fn aspect_preview_ascii(width: u16, height: u16, aspect: f32) -> String {
    let w = width.max(12) as usize;
    let h = height.max(6) as usize;
    let cx = (w as f32 - 1.0) * 0.5;
    let cy = (h as f32 - 1.0) * 0.5;
    let radius = (w.min(h) as f32) * 0.35;
    let mut out = String::with_capacity(w.saturating_mul(h + 1));

    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - cx) / radius;
            let dy = (y as f32 - cy) / radius;
            let d = ((dx * aspect).powi(2) + dy.powi(2)).sqrt();
            let ch = if (d - 1.0).abs() < 0.08 {
                '@'
            } else if d < 1.0 {
                '.'
            } else {
                ' '
            };
            out.push(ch);
        }
        if y + 1 < h {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::{backend::TestBackend, Terminal};

    fn key(code: KeyCode) -> StartWizardEvent {
        StartWizardEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn test_state() -> StartWizardState {
        let model_entries = vec![StartEntry::from_path(Path::new("miku.glb"))];
        let music_entries = vec![StartEntry::from_path(Path::new("world.mp3"))];
        let camera_entries = vec![StartEntry::from_path(Path::new("world_is_mine.vmd"))];
        let stage_entries = vec![StageChoice {
            name: "default-stage".to_owned(),
            status: StageStatus::Ready,
            render_path: Some(PathBuf::from("assets/stage/default-stage/stage.glb")),
            pmx_path: None,
            transform: StageTransform::default(),
        }];
        StartWizardState::new(
            model_entries,
            music_entries,
            stage_entries,
            camera_entries,
            StartWizardDefaults::default(),
            120,
            35,
        )
    }

    #[test]
    fn transitions_model_to_confirm_with_enter() {
        let mut state = test_state();
        assert_eq!(state.step, StartWizardStep::Model);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::Music);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::Stage);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::Camera);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::Render);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::AspectCalib);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::Confirm);

        assert!(matches!(
            state.apply_event(key(KeyCode::Enter)),
            StartWizardAction::Submit(_)
        ));
    }

    #[test]
    fn esc_moves_back_or_cancels() {
        let mut state = test_state();

        state.step = StartWizardStep::Music;
        assert!(matches!(
            state.apply_event(key(KeyCode::Esc)),
            StartWizardAction::Continue
        ));
        assert_eq!(state.step, StartWizardStep::Model);

        assert!(matches!(
            state.apply_event(key(KeyCode::Esc)),
            StartWizardAction::Cancel
        ));
    }

    #[test]
    fn tab_and_backtab_handle_focus_on_render_step() {
        let mut state = test_state();
        state.step = StartWizardStep::Render;
        state.render_focus_index = 0;

        state.apply_event(key(KeyCode::Tab));
        assert_eq!(state.render_focus_index, 1);

        state.apply_event(key(KeyCode::BackTab));
        assert_eq!(state.render_focus_index, 0);
    }

    #[test]
    fn breakpoint_edges() {
        assert_eq!(breakpoint_for(140, 40), UiBreakpoint::Wide);
        assert_eq!(breakpoint_for(139, 40), UiBreakpoint::Normal);
        assert_eq!(breakpoint_for(100, 28), UiBreakpoint::Normal);
        assert_eq!(breakpoint_for(99, 28), UiBreakpoint::Compact);
    }

    #[test]
    fn aspect_calibration_step_updates_trim() {
        let mut state = test_state();
        state.step = StartWizardStep::AspectCalib;
        let before = state.cell_aspect_trim;
        state.apply_event(key(KeyCode::Right));
        assert!(state.cell_aspect_trim > before);
    }

    #[test]
    fn selecting_camera_source_auto_enables_vmd_mode() {
        let mut state = test_state();
        state.step = StartWizardStep::Camera;
        state.camera_mode = CameraMode::Off;
        state.camera_focus_index = 0;
        state.apply_event(key(KeyCode::Right));
        assert_eq!(state.camera_index, 1);
        assert!(matches!(state.camera_mode, CameraMode::Vmd));
    }

    #[test]
    fn render_wide_normal_compact() {
        for (w, h) in [(432, 102), (120, 35), (80, 22)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            let state = test_state();
            terminal
                .draw(|frame| {
                    draw_start_wizard(
                        frame,
                        Path::new("assets/glb"),
                        Path::new("assets/music"),
                        Path::new("assets/stage"),
                        Path::new("assets/camera"),
                        &state,
                        UiLanguage::Ko,
                    );
                })
                .expect("render should succeed");
        }
    }
}

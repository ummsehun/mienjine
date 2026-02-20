use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    terminal::window_size,
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
        CellAspectMode, ContrastProfile, RenderConfig, RenderMode, SyncSpeedMode,
        estimate_cell_aspect_from_window, resolve_cell_aspect,
    },
};

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 18;
const START_FPS_OPTIONS: [u32; 7] = [15, 20, 24, 30, 40, 50, 60];
const RENDER_FIELD_COUNT: usize = 7;
const SYNC_OFFSET_STEP_MS: i32 = 10;
const SYNC_OFFSET_LIMIT_MS: i32 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartWizardStep {
    Model,
    Music,
    Render,
    AspectCalib,
    Confirm,
}

impl StartWizardStep {
    fn index(self) -> usize {
        match self {
            StartWizardStep::Model => 0,
            StartWizardStep::Music => 1,
            StartWizardStep::Render => 2,
            StartWizardStep::AspectCalib => 3,
            StartWizardStep::Confirm => 4,
        }
    }
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

#[derive(Debug, Clone, Copy)]
pub struct StartWizardDefaults {
    pub mode: RenderMode,
    pub fps_cap: u32,
    pub cell_aspect: f32,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_offset_ms: i32,
    pub sync_speed_mode: SyncSpeedMode,
    pub font_preset_enabled: bool,
}

impl Default for StartWizardDefaults {
    fn default() -> Self {
        Self {
            mode: RenderMode::Ascii,
            fps_cap: 30,
            cell_aspect: 0.5,
            cell_aspect_mode: CellAspectMode::Auto,
            cell_aspect_trim: 1.0,
            contrast_profile: ContrastProfile::Adaptive,
            sync_offset_ms: 0,
            sync_speed_mode: SyncSpeedMode::AutoDurationFit,
            font_preset_enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StartSelection {
    pub glb_path: PathBuf,
    pub music_path: Option<PathBuf>,
    pub mode: RenderMode,
    pub fps_cap: u32,
    pub cell_aspect: f32,
    pub cell_aspect_mode: CellAspectMode,
    pub cell_aspect_trim: f32,
    pub contrast_profile: ContrastProfile,
    pub sync_offset_ms: i32,
    pub sync_speed_mode: SyncSpeedMode,
    pub apply_font_preset: bool,
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
    model_index: usize,
    music_index: usize,
    mode: RenderMode,
    fps_index: usize,
    manual_cell_aspect: f32,
    cell_aspect_mode: CellAspectMode,
    cell_aspect_trim: f32,
    contrast_profile: ContrastProfile,
    sync_offset_ms: i32,
    sync_speed_mode: SyncSpeedMode,
    font_preset_enabled: bool,
    render_focus_index: usize,
    width: u16,
    height: u16,
    detected_cell_aspect: Option<f32>,
    clip_duration_cache: HashMap<PathBuf, Option<f32>>,
    audio_duration_cache: HashMap<PathBuf, Option<f32>>,
}

impl StartWizardState {
    fn new(
        model_entries: Vec<StartEntry>,
        music_entries: Vec<StartEntry>,
        defaults: StartWizardDefaults,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            step: StartWizardStep::Model,
            model_entries,
            music_entries,
            model_index: 0,
            music_index: 0,
            mode: defaults.mode,
            fps_index: closest_u32_index(defaults.fps_cap, &START_FPS_OPTIONS),
            manual_cell_aspect: defaults.cell_aspect,
            cell_aspect_mode: defaults.cell_aspect_mode,
            cell_aspect_trim: defaults.cell_aspect_trim.clamp(0.70, 1.30),
            contrast_profile: defaults.contrast_profile,
            sync_offset_ms: defaults
                .sync_offset_ms
                .clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS),
            sync_speed_mode: defaults.sync_speed_mode,
            font_preset_enabled: defaults.font_preset_enabled,
            render_focus_index: 0,
            width,
            height,
            detected_cell_aspect: None,
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
                self.step = StartWizardStep::Render;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Model;
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
                self.step = StartWizardStep::Music;
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
            StartWizardStep::Music => self.step = StartWizardStep::Render,
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
            StartWizardStep::Render => {
                if self.render_focus_index > 0 {
                    self.render_focus_index -= 1;
                } else {
                    self.step = StartWizardStep::Music;
                }
            }
            StartWizardStep::AspectCalib => self.step = StartWizardStep::Render,
            StartWizardStep::Confirm => self.step = StartWizardStep::AspectCalib,
        }
    }

    fn adjust_render_value(&mut self, delta: i32) {
        match self.render_focus_index {
            0 => {
                self.mode = match self.mode {
                    RenderMode::Ascii => RenderMode::Braille,
                    RenderMode::Braille => RenderMode::Ascii,
                }
            }
            1 => cycle_index(&mut self.fps_index, START_FPS_OPTIONS.len(), delta),
            2 => {
                self.contrast_profile = match self.contrast_profile {
                    ContrastProfile::Adaptive => ContrastProfile::Fixed,
                    ContrastProfile::Fixed => ContrastProfile::Adaptive,
                }
            }
            3 => {
                let next = self
                    .sync_offset_ms
                    .saturating_add(delta.saturating_mul(SYNC_OFFSET_STEP_MS));
                self.sync_offset_ms = next.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
            }
            4 => {
                self.sync_speed_mode = match self.sync_speed_mode {
                    SyncSpeedMode::AutoDurationFit => SyncSpeedMode::Realtime1x,
                    SyncSpeedMode::Realtime1x => SyncSpeedMode::AutoDurationFit,
                }
            }
            5 => {
                self.cell_aspect_mode = match self.cell_aspect_mode {
                    CellAspectMode::Auto => CellAspectMode::Manual,
                    CellAspectMode::Manual => CellAspectMode::Auto,
                }
            }
            6 => {
                self.font_preset_enabled = !self.font_preset_enabled;
            }
            _ => {}
        }
    }

    fn selection(&self) -> StartSelection {
        let glb_path = self.model_entries[self.model_index].path.clone();
        StartSelection {
            glb_path,
            music_path: self.selected_music_path().cloned(),
            mode: self.mode,
            fps_cap: START_FPS_OPTIONS[self.fps_index],
            cell_aspect: self.manual_cell_aspect,
            cell_aspect_mode: self.cell_aspect_mode,
            cell_aspect_trim: self.cell_aspect_trim,
            contrast_profile: self.contrast_profile,
            sync_offset_ms: self.sync_offset_ms,
            sync_speed_mode: self.sync_speed_mode,
            apply_font_preset: self.font_preset_enabled,
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
            cell_aspect: self.manual_cell_aspect,
            cell_aspect_mode: self.cell_aspect_mode,
            cell_aspect_trim: self.cell_aspect_trim,
            contrast_profile: self.contrast_profile,
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

#[derive(Debug, Clone)]
enum StartWizardAction {
    Continue,
    Cancel,
    Submit(StartSelection),
}

pub fn run_start_wizard(
    model_dir: &Path,
    music_dir: &Path,
    model_files: &[PathBuf],
    music_files: &[PathBuf],
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

    let mut terminal = RatatuiSession::enter()?;
    let (width, height) = terminal.size()?;
    let mut state = StartWizardState::new(model_entries, music_entries, defaults, width, height);

    loop {
        state.refresh_runtime_metrics(anim_selector);
        terminal.draw(|frame| {
            draw_start_wizard(frame, model_dir, music_dir, &state, ui_language);
        })?;

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

fn draw_start_wizard(
    frame: &mut Frame,
    model_dir: &Path,
    music_dir: &Path,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    if state.is_too_small() {
        draw_min_size_screen(frame, state, ui_language);
        return;
    }

    let area = frame.area();
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
            draw_summary_panel(frame, body[1], model_dir, music_dir, state, ui_language);
        }
        UiBreakpoint::Normal => {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(8), Constraint::Length(10)])
                .split(main[1]);
            draw_step_panel(frame, body[0], state, ui_language);
            draw_summary_panel(frame, body[1], model_dir, music_dir, state, ui_language);
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
        StartWizardStep::Render => tr(ui_language, "렌더 옵션", "Render"),
        StartWizardStep::AspectCalib => tr(ui_language, "비율 보정", "Aspect Calib"),
        StartWizardStep::Confirm => tr(ui_language, "확인/실행", "Confirm"),
    };
    let line = Line::from(vec![
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  •  "),
        Span::raw(format!("{} {}/5", step_name, state.step.index() + 1)),
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

fn draw_render_options(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "3) 렌더 옵션", "3) Render Options");
    let mode = match state.mode {
        RenderMode::Ascii => "ASCII",
        RenderMode::Braille => "Braille",
    };
    let contrast = match state.contrast_profile {
        ContrastProfile::Adaptive => tr(ui_language, "적응형", "Adaptive"),
        ContrastProfile::Fixed => tr(ui_language, "고정", "Fixed"),
    };
    let sync_mode = match state.sync_speed_mode {
        SyncSpeedMode::AutoDurationFit => tr(ui_language, "자동", "Auto"),
        SyncSpeedMode::Realtime1x => tr(ui_language, "실시간", "Realtime"),
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

    let rows = [
        format!("{}: {}", tr(ui_language, "모드", "Mode"), mode),
        format!(
            "{}: {}",
            tr(ui_language, "FPS 제한", "FPS Cap"),
            START_FPS_OPTIONS[state.fps_index]
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
                .title(tr(ui_language, "4) 비율 보정", "4) Aspect Calibration"))
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
            "{}: {:?}",
            tr(ui_language, "렌더 모드", "Render"),
            selection.mode
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "FPS", "FPS"),
            selection.fps_cap
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
                .title(tr(ui_language, "5) 확인 / 실행", "5) Confirm / Run"))
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
            tr(ui_language, "모델", "Model"),
            model_name
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "음악", "Music"),
            music_name
        )),
        Line::raw(format!(
            "{}: {:.3}",
            tr(ui_language, "적용 비율", "Applied Aspect"),
            state.effective_cell_aspect()
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

fn draw_min_size_screen(frame: &mut Frame, state: &StartWizardState, ui_language: UiLanguage) {
    let area = frame.area();
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
    use ratatui::{Terminal, backend::TestBackend};

    fn key(code: KeyCode) -> StartWizardEvent {
        StartWizardEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn test_state() -> StartWizardState {
        let model_entries = vec![StartEntry::from_path(Path::new("miku.glb"))];
        let music_entries = vec![StartEntry::from_path(Path::new("world.mp3"))];
        StartWizardState::new(
            model_entries,
            music_entries,
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
                        &state,
                        UiLanguage::Ko,
                    );
                })
                .expect("render should succeed");
        }
    }
}

mod input;
mod input_adjust;
mod confirm_panel;
mod panels;
mod state;
mod steps;
mod tests;
mod types;

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event},
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use ratatui::prelude::*;

use crate::runtime::{
    config::UiLanguage,
    start_ui_helpers::{
        aspect_preview_ascii, breakpoint_for, clamp_ratatui_area, closest_u32_index,
        compute_duration_fit_factor, cycle_index, detect_terminal_cell_aspect, duration_label,
        format_mib, fps_label, inspect_audio_duration, inspect_clip_duration,
        inspect_motion_duration, target_fps_for_profile, tr, MIN_HEIGHT, MIN_WIDTH,
        RATATUI_SAFE_MAX_CELLS, RENDER_FIELD_COUNT, START_FPS_OPTIONS, SYNC_OFFSET_LIMIT_MS,
        SYNC_OFFSET_STEP_MS,
    },
    terminal::{RatatuiSession, TerminalProfile},
};

use state::{StartEntry, StartWizardAction, StartWizardState};
pub use types::{
    ModelBranch, StageChoice, StageStatus, StageTransform, StartSelection, StartWizardDefaults,
    StartWizardEvent, StartWizardStep, UiBreakpoint,
};

pub(crate) use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, PerfProfile,
    RenderBackend, RenderMode, RenderOutputMode, SyncPolicy, SyncSpeedMode, TextureSamplingMode,
    ThemeStyle,
};

use panels::{draw_header, draw_help_panel, draw_min_size_screen, draw_summary_panel};
use steps::draw_step_panel;

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

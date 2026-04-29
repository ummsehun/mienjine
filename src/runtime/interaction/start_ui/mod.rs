mod input;
mod input_adjust;
mod panels;
mod state;
mod steps;
mod steps_render;
#[cfg(test)]
mod tests;
mod theme;
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
    terminal::{Clear as CrosstermClear, ClearType},
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear as WidgetClear, Paragraph};

use crate::runtime::{
    config::{UiLanguage, preset::PresetStore},
    start_ui_helpers::{
        QUICK_RENDER_FIELD_COUNT, RATATUI_SAFE_MAX_CELLS, RENDER_FIELD_COUNT, START_FPS_OPTIONS,
        SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_STEP_MS, clamp_ratatui_area, cycle_index, tr,
    },
    terminal::{RatatuiSession, TerminalProfile},
};

use state::{PresetPromptState, StartEntry, StartWizardAction, StartWizardState};
pub use types::{
    ModelBranch, RenderDetailMode, StageChoice, StageStatus, StageTransform, StartSelection,
    StartWizardDefaults, StartWizardEvent, StartWizardStep, UiBreakpoint,
};

pub(crate) use crate::scene::{
    AnsiQuantization, AudioReactiveMode, BrailleProfile, CameraAlignPreset, CameraControlMode,
    CameraFocusMode, CameraMode, CellAspectMode, CenterLockMode, CinematicCameraMode,
    ClarityProfile, ColorMode, ContrastProfile, DetailProfile, GraphicsProtocol, PerfProfile,
    RenderBackend, RenderMode, RenderOutputMode, SyncPolicy, SyncSpeedMode, TextureSamplingMode,
    ThemeStyle,
};

use panels::{
    draw_action_bar, draw_header, draw_min_size_screen, draw_stepper, draw_summary_panel,
};
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
    let preset_store = match PresetStore::load_default() {
        Ok(store) => Some(store),
        Err(error) => {
            eprintln!("warning: preset store unavailable: {error}");
            None
        }
    };

    let mut terminal = RatatuiSession::enter_with_profile(TerminalProfile::detect())?;
    let (width, height) = terminal.size()?;
    let mut state = StartWizardState::new(
        model_entries,
        pmx_entries,
        motion_entries,
        music_entries,
        stage_entries,
        camera_entries,
        preset_store,
        defaults,
        width,
        height,
    );
    state.initialize_presets();

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
    queue!(stdout, MoveTo(0, 0), CrosstermClear(ClearType::All))?;
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
    let action_height = match breakpoint {
        UiBreakpoint::Wide => 5,
        UiBreakpoint::Normal => 4,
        UiBreakpoint::Compact => 3,
    };
    let stepper_height = if matches!(breakpoint, UiBreakpoint::Compact) {
        1
    } else {
        2
    };
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(stepper_height),
            Constraint::Min(8),
            Constraint::Length(action_height),
        ])
        .split(area);

    draw_header(frame, main[0], state, ui_language);
    draw_stepper(frame, main[1], state, ui_language, breakpoint);

    match breakpoint {
        UiBreakpoint::Wide => {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(main[2]);
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
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(main[2]);
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
            draw_step_panel(frame, main[2], state, ui_language);
        }
    }

    draw_action_bar(frame, main[3], state, ui_language, breakpoint);

    if let Some(status) = state.status_message.as_ref() {
        let area = Rect {
            x: area.x,
            y: area.y.saturating_add(area.height.saturating_sub(1)),
            width: area.width,
            height: 1,
        };
        let line = Line::from(vec![
            Span::styled("● ", Style::default().fg(Color::Cyan)),
            Span::raw(status.as_str()),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    match state.preset_prompt {
        PresetPromptState::Inactive => {}
        PresetPromptState::EnterName { ref buffer } => {
            let width = area.width.min(76);
            let height = 5;
            let x = area.x.saturating_add((area.width.saturating_sub(width)) / 2);
            let y = area
                .y
                .saturating_add((area.height.saturating_sub(height)) / 2);
            let dialog = Rect {
                x,
                y,
                width,
                height,
            };
            frame.render_widget(WidgetClear, dialog);
            let prompt = Paragraph::new(vec![
                Line::raw(tr(
                    ui_language,
                    "프리셋 이름을 입력하세요 (Enter 저장 / Esc 취소)",
                    "Enter preset name (Enter save / Esc cancel)",
                )),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        format!("{}: ", tr(ui_language, "이름", "Name")),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(buffer.clone(), Style::default().fg(Color::White)),
                ]),
            ])
            .block(
                Block::default()
                    .title(tr(ui_language, "Preset Save", "Preset Save"))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
            frame.render_widget(prompt, dialog);
        }
        PresetPromptState::ConfirmOverwrite { ref name } => {
            let width = area.width.min(76);
            let height = 5;
            let x = area.x.saturating_add((area.width.saturating_sub(width)) / 2);
            let y = area
                .y
                .saturating_add((area.height.saturating_sub(height)) / 2);
            let dialog = Rect {
                x,
                y,
                width,
                height,
            };
            frame.render_widget(WidgetClear, dialog);
            let prompt = Paragraph::new(vec![
                Line::raw(format!(
                    "{} '{name}'",
                    tr(
                        ui_language,
                        "동일 이름 preset이 이미 존재합니다:",
                        "Preset already exists:"
                    )
                )),
                Line::raw(""),
                Line::raw(tr(
                    ui_language,
                    "Enter=덮어쓰기 / Esc=취소",
                    "Enter=overwrite / Esc=cancel",
                )),
            ])
            .block(
                Block::default()
                    .title(tr(ui_language, "Overwrite Confirm", "Overwrite Confirm"))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
            frame.render_widget(prompt, dialog);
        }
    }
}

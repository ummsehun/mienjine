use super::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use super::confirm_panel::draw_confirm_panel;
use super::panels::draw_aspect_calibration;
use super::steps_render::draw_render_options;

pub(super) fn draw_step_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    match state.step {
        StartWizardStep::Branch => draw_branch_panel(frame, area, state, ui_language),
        StartWizardStep::Model => draw_model_list(frame, area, state, ui_language),
        StartWizardStep::Motion => draw_motion_list(frame, area, state, ui_language),
        StartWizardStep::Music => draw_music_list(frame, area, state, ui_language),
        StartWizardStep::Stage => draw_stage_list(frame, area, state, ui_language),
        StartWizardStep::Camera => draw_camera_panel(frame, area, state, ui_language),
        StartWizardStep::Render => draw_render_options(frame, area, state, ui_language),
        StartWizardStep::AspectCalib => draw_aspect_calibration(frame, area, state, ui_language),
        StartWizardStep::Confirm => draw_confirm_panel(frame, area, state, ui_language),
    }
}

fn draw_branch_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "0) 모델 종류 선택", "0) Select Source");
    let items = vec![
        ListItem::new(tr(ui_language, "GLB (.glb/.gltf)", "GLB (.glb/.gltf)")),
        ListItem::new(tr(ui_language, "PMX + VMD", "PMX + VMD")),
    ];
    let mut list_state = ListState::default();
    list_state.select(Some(match state.branch {
        ModelBranch::Glb => 0,
        ModelBranch::PmxVmd => 1,
    }));
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

fn draw_model_list(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "1) 모델 선택", "1) Select Model");
    let entries = match state.branch {
        ModelBranch::Glb => &state.model_entries,
        ModelBranch::PmxVmd => &state.pmx_entries,
    };
    let items = entries
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

fn draw_motion_list(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let title = tr(ui_language, "2) 모션 선택", "2) Select Motion");
    let mut items = Vec::with_capacity(state.motion_entries.len() + 1);
    items.push(ListItem::new(tr(ui_language, "없음", "None")));
    items.extend(
        state
            .motion_entries
            .iter()
            .map(|entry| ListItem::new(entry.label())),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(state.motion_index));
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

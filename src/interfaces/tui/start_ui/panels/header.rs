use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::super::state::StartWizardState;
use super::super::theme::start_ui_theme;
use super::super::tr;
use super::super::types::{StartWizardStep, UiBreakpoint};
use crate::runtime::config::UiLanguage;

pub fn draw_header(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let theme = start_ui_theme();
    let title = tr(
        ui_language,
        "Terminal Miku 3D 시작 설정",
        "Terminal Miku 3D Setup",
    );
    let step_name = match state.step {
        StartWizardStep::Branch => tr(ui_language, "브랜치 선택", "Branch"),
        StartWizardStep::Model => tr(ui_language, "모델 선택", "Model"),
        StartWizardStep::Motion => tr(ui_language, "모션 선택", "Motion"),
        StartWizardStep::Music => tr(ui_language, "음악 선택", "Music"),
        StartWizardStep::Stage => tr(ui_language, "스테이지 선택", "Stage"),
        StartWizardStep::Camera => tr(ui_language, "카메라 선택", "Camera"),
        StartWizardStep::Render => tr(ui_language, "렌더 옵션", "Render"),
        StartWizardStep::AspectCalib => tr(ui_language, "비율 보정", "Aspect Calib"),
        StartWizardStep::Confirm => tr(ui_language, "확인/실행", "Confirm"),
    };
    let line = Line::from(vec![
        Span::styled(title, theme.text_primary.add_modifier(Modifier::BOLD)),
        Span::raw("  •  "),
        Span::styled(
            format!("{} {}/9", step_name, state.step.index() + 1),
            Style::default().fg(theme.accent),
        ),
    ]);

    let para = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_active),
    );
    frame.render_widget(para, area);
}

pub fn draw_stepper(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
    breakpoint: UiBreakpoint,
) {
    let theme = start_ui_theme();
    let labels_ko = [
        "브랜치",
        "모델",
        "모션",
        "음악",
        "스테이지",
        "카메라",
        "렌더",
        "비율",
        "확인",
    ];
    let labels_en = [
        "Branch", "Model", "Motion", "Music", "Stage", "Camera", "Render", "Aspect", "Confirm",
    ];
    let labels = if matches!(ui_language, UiLanguage::Ko) {
        labels_ko
    } else {
        labels_en
    };

    if matches!(breakpoint, UiBreakpoint::Compact) {
        let text = format!(
            "{} {}/9  {}",
            tr(ui_language, "단계", "Step"),
            state.step.index() + 1,
            labels[state.step.index()]
        );
        frame.render_widget(Paragraph::new(text), area);
        return;
    }

    let mut spans = Vec::new();
    for (idx, label) in labels.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(" - ", theme.text_dim));
        }
        let done = idx < state.step.index();
        let current = idx == state.step.index();
        let style = if current {
            Style::default()
                .fg(theme.focus)
                .add_modifier(Modifier::BOLD)
        } else if done {
            Style::default().fg(theme.success)
        } else {
            theme.text_dim
        };
        spans.push(Span::styled(format!("[{:02} {}]", idx + 1, label), style));
    }
    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

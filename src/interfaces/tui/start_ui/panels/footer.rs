use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::super::state::StartWizardState;
use super::super::theme::start_ui_theme;
use super::super::tr;
use super::super::types::{RenderDetailMode, StartWizardStep, UiBreakpoint};
use crate::runtime::config::UiLanguage;

pub fn draw_action_bar(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
    breakpoint: UiBreakpoint,
) {
    let theme = start_ui_theme();
    let step_hint = match state.step {
        StartWizardStep::Branch => tr(
            ui_language,
            "브랜치 선택: 좌/우, 프리셋: 위/아래, Enter 진행",
            "Branch: left/right, Preset: up/down, Enter next",
        ),
        StartWizardStep::Model => tr(
            ui_language,
            "모델 선택: 위/아래, Enter 진행",
            "Model: up/down, Enter next",
        ),
        StartWizardStep::Motion => tr(
            ui_language,
            "모션 선택: 위/아래, Enter 진행",
            "Motion: up/down, Enter next",
        ),
        StartWizardStep::Music => tr(
            ui_language,
            "음악 선택: 위/아래, Enter 진행",
            "Music: up/down, Enter next",
        ),
        StartWizardStep::Stage => tr(
            ui_language,
            "스테이지 선택: 위/아래, Enter 진행",
            "Stage: up/down, Enter next",
        ),
        StartWizardStep::Camera => tr(
            ui_language,
            "카메라 조정: 위/아래 항목, 좌/우 값 변경",
            "Camera: up/down focus, left/right change",
        ),
        StartWizardStep::Render => tr(
            ui_language,
            "렌더 조정: 위/아래 항목, 좌/우 값 변경, Tab Quick/Advanced",
            "Render: up/down focus, left/right change, Tab Quick/Advanced",
        ),
        StartWizardStep::AspectCalib => tr(
            ui_language,
            "비율 보정: 좌/우 trim, r 리셋",
            "Aspect: left/right trim, r reset",
        ),
        StartWizardStep::Confirm => tr(
            ui_language,
            "최종 확인: Enter 실행",
            "Final check: Enter to run",
        ),
    };

    let mode_badge = if matches!(state.step, StartWizardStep::Render) {
        match state.render_detail_mode {
            RenderDetailMode::Quick => {
                Span::styled(tr(ui_language, " QUICK ", " QUICK "), theme.badge_quick)
            }
            RenderDetailMode::Advanced => Span::styled(
                tr(ui_language, " ADVANCED ", " ADVANCED "),
                theme.badge_advanced,
            ),
        }
    } else {
        Span::raw("")
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(step_hint, theme.text_primary),
        Span::raw(" "),
        mode_badge,
    ])];
    if let Some(status) = state.status_message.as_ref() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", tr(ui_language, "상태", "Status")),
                theme.text_secondary,
            ),
            Span::styled(status.clone(), theme.text_primary),
        ]));
    }
    if !matches!(breakpoint, UiBreakpoint::Compact) {
        lines.push(Line::from(vec![
            Span::styled("Enter", Style::default().fg(theme.accent)),
            Span::styled(
                tr(ui_language, " 실행/다음", " confirm/next"),
                theme.text_secondary,
            ),
            Span::raw("  "),
            Span::styled("Esc", Style::default().fg(theme.accent)),
            Span::styled(tr(ui_language, " 이전", " back"), theme.text_secondary),
            Span::raw("  "),
            Span::styled("q", Style::default().fg(theme.accent)),
            Span::styled(tr(ui_language, " 취소", " cancel"), theme.text_secondary),
            Span::raw("  "),
            Span::styled("Ctrl+S", Style::default().fg(theme.accent)),
            Span::styled(
                tr(ui_language, " preset 저장", " save preset"),
                theme.text_secondary,
            ),
        ]));
    }

    let help = Paragraph::new(lines)
        .block(
            Block::default()
                .title(tr(ui_language, "조작", "Actions"))
                .borders(Borders::ALL)
                .border_style(theme.border_idle),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(help, area);
}

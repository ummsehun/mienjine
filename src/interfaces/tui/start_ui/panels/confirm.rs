use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::super::ModelBranch;
use super::super::state::StartWizardState;
use super::super::theme::start_ui_theme;
use super::super::tr;
use super::super::types::StageStatus;
use super::summary_kv_line;
use crate::interfaces::tui::helpers::{duration_label, fps_label};
use crate::runtime::config::UiLanguage;
use crate::scene::RenderBackend;

pub fn draw_confirm_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let theme = start_ui_theme();
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
        .unwrap_or(tr(ui_language, "없음", "None"));
    let motion_name = selection
        .motion_vmd_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or(tr(ui_language, "없음", "None"));
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

    let backend = match selection.backend {
        RenderBackend::Cpu => "CPU".to_owned(),
        #[cfg(feature = "gpu")]
        RenderBackend::Gpu => {
            if state.gpu_available {
                "GPU (Metal)".to_owned()
            } else {
                tr(
                    ui_language,
                    "GPU 불가, CPU 대체",
                    "GPU unavailable, CPU fallback",
                )
                .to_owned()
            }
        }
        #[cfg(not(feature = "gpu"))]
        RenderBackend::Gpu => tr(
            ui_language,
            "GPU 미컴파일, CPU 대체",
            "GPU not compiled, CPU fallback",
        )
        .to_owned(),
    };

    let mut summary_lines: Vec<Line> = vec![
        summary_kv_line(ui_language, theme, "모델", "Model", model_name),
        summary_kv_line(ui_language, theme, "모션", "Motion", motion_name),
        summary_kv_line(ui_language, theme, "음악", "Music", music_name),
        summary_kv_line(
            ui_language,
            theme,
            "스테이지",
            "Stage",
            format!("{} ({})", stage_name, stage_status),
        ),
        summary_kv_line(
            ui_language,
            theme,
            "카메라",
            "Camera",
            format!(
                "{:?} / {:?} / {:.2}",
                selection.camera_mode, selection.camera_align_preset, selection.camera_unit_scale
            ),
        ),
        summary_kv_line(
            ui_language,
            theme,
            "렌더",
            "Render",
            format!(
                "{:?} / {:?} / {:?}",
                selection.mode, selection.perf_profile, selection.detail_profile
            ),
        ),
        summary_kv_line(ui_language, theme, "백엔드", "Backend", backend),
        summary_kv_line(
            ui_language,
            theme,
            "FPS",
            "FPS",
            fps_label(selection.fps_cap, ui_language),
        ),
        summary_kv_line(
            ui_language,
            theme,
            "클립 길이",
            "Clip Duration",
            duration_label(state.selected_clip_duration_secs()),
        ),
        summary_kv_line(
            ui_language,
            theme,
            "오디오 길이",
            "Audio Duration",
            duration_label(state.selected_audio_duration_secs()),
        ),
        summary_kv_line(
            ui_language,
            theme,
            "속도 계수",
            "Speed Factor",
            format!("{:.4}", state.expected_sync_speed()),
        ),
        summary_kv_line(
            ui_language,
            theme,
            "비율",
            "Aspect",
            format!(
                "{:.3} (mode: {:?})",
                state.effective_cell_aspect(),
                selection.cell_aspect_mode
            ),
        ),
    ];

    summary_lines.push(Line::raw(""));
    summary_lines.push(Line::styled(
        tr(
            ui_language,
            "Enter로 실행, Esc로 이전 단계",
            "Press Enter to run, Esc to go back",
        ),
        Style::default().fg(theme.accent),
    ));

    let mut warning_lines: Vec<Line> = Vec::new();

    if matches!(selection.branch, ModelBranch::PmxVmd) && selection.motion_vmd_path.is_none() {
        warning_lines.push(Line::styled(
            tr(
                ui_language,
                "주의: PMX 브랜치에서 모션이 비어 있습니다.",
                "Warning: PMX branch has no motion selected.",
            ),
            Style::default().fg(Color::LightYellow),
        ));
    }

    if let Some(choice) = selection.stage_choice.as_ref() {
        match choice.status {
            StageStatus::Ready => {}
            StageStatus::NeedsConvert => warning_lines.push(Line::styled(
                tr(
                    ui_language,
                    "주의: 선택 스테이지는 PMX->GLB 변환이 필요합니다.",
                    "Warning: selected stage requires PMX->GLB conversion.",
                ),
                Style::default().fg(Color::LightYellow),
            )),
            StageStatus::Invalid => warning_lines.push(Line::styled(
                tr(
                    ui_language,
                    "위험: 선택 스테이지 상태가 유효하지 않습니다.",
                    "Risk: selected stage is marked invalid.",
                ),
                Style::default().fg(Color::LightRed),
            )),
        }
    }

    if selection.music_path.is_none() {
        warning_lines.push(Line::styled(
            tr(
                ui_language,
                "참고: 음악 없이 실행하면 반응/동기화 품질이 낮아질 수 있습니다.",
                "Note: without music, reactive/sync quality can be reduced.",
            ),
            theme.text_secondary,
        ));
    }

    #[cfg(feature = "gpu")]
    if matches!(selection.backend, RenderBackend::Gpu) && !state.gpu_available {
        warning_lines.push(Line::styled(
            tr(
                ui_language,
                "주의: GPU를 선택했지만 현재 환경에서 사용 불가합니다.",
                "Warning: GPU selected but unavailable in this environment.",
            ),
            Style::default().fg(Color::LightYellow),
        ));
    }

    if (state.expected_sync_speed() - 1.0).abs() > 0.12 {
        warning_lines.push(Line::styled(
            tr(
                ui_language,
                "참고: 클립/오디오 길이 차이가 커서 속도 보정이 적용됩니다.",
                "Note: clip/audio duration mismatch applies speed compensation.",
            ),
            theme.text_secondary,
        ));
    }

    if warning_lines.is_empty() {
        warning_lines.push(Line::styled(
            tr(
                ui_language,
                "검증 결과: 주요 경고 없음",
                "Validation: no major warnings",
            ),
            Style::default().fg(theme.success),
        ));
    }

    let chunks = if area.width >= 96 {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(66),
                ratatui::layout::Constraint::Percentage(34),
            ])
            .split(area)
    } else {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Percentage(72),
                ratatui::layout::Constraint::Percentage(28),
            ])
            .split(area)
    };

    let summary = Paragraph::new(summary_lines)
        .block(
            Block::default()
                .title(tr(ui_language, "6) 확인 / 실행", "6) Confirm / Run"))
                .borders(Borders::ALL)
                .border_style(theme.border_active),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(summary, chunks[0]);

    let warnings = Paragraph::new(warning_lines)
        .block(
            Block::default()
                .title(tr(ui_language, "검증 / 경고", "Validation / Warnings"))
                .borders(Borders::ALL)
                .border_style(theme.border_idle),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(warnings, chunks[1]);
}

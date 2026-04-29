use super::*;

use super::state::StartWizardState;
use super::theme::{start_ui_theme, StartUiTheme};
use super::types::{RenderDetailMode, StageStatus, StartWizardStep, UiBreakpoint};
use crate::runtime::start_ui_helpers::{
    aspect_preview_ascii, duration_label, fps_label, MIN_HEIGHT, MIN_WIDTH,
};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

fn summary_kv_line(
    ui_language: UiLanguage,
    theme: StartUiTheme,
    ko: &str,
    en: &str,
    value: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}: ", tr(ui_language, ko, en)),
            theme.text_secondary,
        ),
        Span::styled(value.into(), theme.text_primary),
    ])
}

pub(super) fn draw_header(
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

pub(super) fn draw_stepper(
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

pub(super) fn draw_action_bar(
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

pub(super) fn draw_min_size_screen(
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

pub(super) fn draw_aspect_calibration(
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

pub(super) fn draw_summary_panel(
    frame: &mut Frame,
    area: Rect,
    model_dir: &Path,
    pmx_dir: &Path,
    motion_dir: &Path,
    music_dir: &Path,
    stage_dir: &Path,
    camera_dir: &Path,
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
    let motion_name = selection
        .motion_vmd_path
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| tr(ui_language, "없음", "None").to_owned());
    let branch_name = match selection.branch {
        ModelBranch::Glb => "GLB",
        ModelBranch::PmxVmd => "PMX + VMD",
    };
    let render_mode = match selection.mode {
        RenderMode::Ascii => "ASCII",
        RenderMode::Braille => "Braille",
    };
    let backend = match selection.backend {
        RenderBackend::Cpu => "CPU".to_owned(),
        #[cfg(feature = "gpu")]
        RenderBackend::Gpu => {
            if state.gpu_available {
                "GPU (Metal)".to_owned()
            } else {
                tr(
                    ui_language,
                    "GPU 미사용 가능, CPU 대체",
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
    let detail_mode = match state.render_detail_mode {
        RenderDetailMode::Quick => "Quick",
        RenderDetailMode::Advanced => "Advanced",
    };
    let speed_factor = format!("{:.4}", state.expected_sync_speed());
    let clip_label = duration_label(state.selected_clip_duration_secs());
    let audio_label = duration_label(state.selected_audio_duration_secs());

    let stage_state_style = match selection.stage_choice.as_ref().map(|choice| choice.status) {
        Some(StageStatus::Ready) | None => theme.text_primary,
        Some(StageStatus::NeedsConvert) => {
            Style::default().fg(theme.badge_advanced.fg.unwrap_or(Color::Yellow))
        }
        Some(StageStatus::Invalid) => Style::default().fg(Color::LightRed),
    };
    let title = match state.step {
        StartWizardStep::Branch => tr(ui_language, "브랜치 컨텍스트", "Branch Context"),
        StartWizardStep::Model => tr(ui_language, "모델 컨텍스트", "Model Context"),
        StartWizardStep::Motion => tr(ui_language, "모션 컨텍스트", "Motion Context"),
        StartWizardStep::Music => tr(ui_language, "음악 컨텍스트", "Music Context"),
        StartWizardStep::Stage => tr(ui_language, "스테이지 컨텍스트", "Stage Context"),
        StartWizardStep::Camera => tr(ui_language, "카메라 컨텍스트", "Camera Context"),
        StartWizardStep::Render => tr(ui_language, "렌더 컨텍스트", "Render Context"),
        StartWizardStep::AspectCalib => tr(ui_language, "비율 컨텍스트", "Aspect Context"),
        StartWizardStep::Confirm => tr(ui_language, "실행 전 요약", "Pre-run Summary"),
    };

    let mut lines: Vec<Line> = Vec::new();
    match state.step {
        StartWizardStep::Branch => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "현재 브랜치",
                "Current Branch",
                branch_name,
            ));
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                tr(
                    ui_language,
                    "GLB: 모델 -> 음악 -> 스테이지 -> 카메라 -> 렌더",
                    "GLB: model -> music -> stage -> camera -> render",
                ),
                theme.text_secondary,
            ));
            lines.push(Line::styled(
                tr(
                    ui_language,
                    "PMX: 모델 -> 모션 -> 음악 -> 스테이지 -> 카메라 -> 렌더",
                    "PMX: model -> motion -> music -> stage -> camera -> render",
                ),
                theme.text_secondary,
            ));
            lines.push(Line::raw(""));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "GLB 루트",
                "GLB Root",
                model_dir.display().to_string(),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "PMX 루트",
                "PMX Root",
                pmx_dir.display().to_string(),
            ));
        }
        StartWizardStep::Model => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "브랜치",
                "Branch",
                branch_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "선택 모델",
                "Selected Model",
                model_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "다음 단계",
                "Next Step",
                if matches!(selection.branch, ModelBranch::PmxVmd) {
                    tr(ui_language, "모션 선택", "Motion selection")
                } else {
                    tr(ui_language, "음악 선택", "Music selection")
                },
            ));
        }
        StartWizardStep::Motion => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모델",
                "Model",
                model_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모션",
                "Motion",
                motion_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "클립 길이",
                "Clip Duration",
                clip_label,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모션 루트",
                "Motion Root",
                motion_dir.display().to_string(),
            ));
        }
        StartWizardStep::Music => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모델",
                "Model",
                model_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "음악",
                "Music",
                music_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "클립 길이",
                "Clip Duration",
                clip_label,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "오디오 길이",
                "Audio Duration",
                audio_label,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "예상 속도 계수",
                "Estimated Speed",
                speed_factor,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "음악 루트",
                "Music Root",
                music_dir.display().to_string(),
            ));
        }
        StartWizardStep::Stage => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모델",
                "Model",
                model_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "스테이지",
                "Stage",
                stage_name,
            ));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}: ", tr(ui_language, "상태", "Status")),
                    theme.text_secondary,
                ),
                Span::styled(stage_status.to_owned(), stage_state_style),
            ]));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "스테이지 루트",
                "Stage Root",
                stage_dir.display().to_string(),
            ));
        }
        StartWizardStep::Camera => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "카메라",
                "Camera",
                camera_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모드",
                "Mode",
                format!("{:?}", selection.camera_mode),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "프리셋",
                "Preset",
                format!("{:?}", selection.camera_align_preset),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "유닛 스케일",
                "Unit Scale",
                format!("{:.2}", selection.camera_unit_scale),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "카메라 루트",
                "Camera Root",
                camera_dir.display().to_string(),
            ));
        }
        StartWizardStep::Render => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모드",
                "Mode",
                render_mode,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "옵션 뷰",
                "Option View",
                detail_mode,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "프로필",
                "Profile",
                format!(
                    "{:?} / {:?}",
                    selection.perf_profile, selection.detail_profile
                ),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "백엔드",
                "Backend",
                backend,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "FPS",
                "FPS",
                fps_label(selection.fps_cap, ui_language),
            ));
            lines.push(Line::styled(
                tr(
                    ui_language,
                    "Tab으로 Quick/Advanced 전환",
                    "Press Tab to toggle Quick/Advanced",
                ),
                theme.text_secondary,
            ));
        }
        StartWizardStep::AspectCalib => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "감지 비율",
                "Detected Aspect",
                state
                    .detected_cell_aspect
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_else(|| "n/a".to_owned()),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "적용 비율",
                "Applied Aspect",
                format!("{:.3}", state.effective_cell_aspect()),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모드",
                "Mode",
                format!("{:?}", selection.cell_aspect_mode),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "Trim",
                "Trim",
                format!("{:.2}", selection.cell_aspect_trim),
            ));
        }
        StartWizardStep::Confirm => {
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "브랜치",
                "Branch",
                branch_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "모델",
                "Model",
                model_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "음악",
                "Music",
                music_name,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "스테이지",
                "Stage",
                format!("{} ({})", stage_name, stage_status),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "렌더",
                "Render",
                format!("{} / {:?}", render_mode, selection.perf_profile),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "백엔드",
                "Backend",
                backend,
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "클립/오디오",
                "Clip/Audio",
                format!("{} / {}", clip_label, audio_label),
            ));
            lines.push(summary_kv_line(
                ui_language,
                theme,
                "속도 계수",
                "Speed Factor",
                speed_factor,
            ));
        }
    }

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(theme.border_idle),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

pub(super) fn draw_confirm_panel(
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
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
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

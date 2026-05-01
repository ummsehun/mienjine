use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::super::ModelBranch;
use super::super::state::StartWizardState;
use super::super::theme::start_ui_theme;
use super::super::tr;
use super::super::types::{StageStatus, StartWizardStep};
use super::summary_kv_line;
use crate::interfaces::tui::helpers::{duration_label, fps_label};
use crate::runtime::config::UiLanguage;
use crate::scene::{RenderBackend, RenderMode};

pub fn draw_summary_panel(
    frame: &mut Frame,
    area: Rect,
    model_dir: &std::path::Path,
    pmx_dir: &std::path::Path,
    motion_dir: &std::path::Path,
    music_dir: &std::path::Path,
    stage_dir: &std::path::Path,
    camera_dir: &std::path::Path,
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
        super::super::types::RenderDetailMode::Quick => "Quick",
        super::super::types::RenderDetailMode::Advanced => "Advanced",
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

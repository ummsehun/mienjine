use super::*;

use crate::runtime::start_ui_helpers::{duration_label, fps_label, target_fps_for_profile};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub(super) fn draw_confirm_panel(
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

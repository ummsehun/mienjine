use super::theme::start_ui_theme;
use super::*;
use crate::interfaces::tui::helpers::fps_label;
use crate::scene::StageQuality;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

pub(super) fn draw_render_options(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
    let theme = start_ui_theme();
    let mode_tag = match state.render_detail_mode {
        RenderDetailMode::Quick => tr(ui_language, "Quick", "Quick"),
        RenderDetailMode::Advanced => tr(ui_language, "Advanced", "Advanced"),
    };
    let title = format!(
        "{} [{}]",
        tr(ui_language, "4) 렌더 옵션", "4) Render Options"),
        mode_tag
    );
    let mode = match state.mode {
        RenderMode::Ascii => "ASCII",
        RenderMode::Braille => "Braille",
    };
    let perf_profile = match state.perf_profile {
        PerfProfile::Balanced => tr(ui_language, "균형 30FPS", "Balanced 30FPS"),
        PerfProfile::Cinematic => tr(ui_language, "시네마 20FPS", "Cinematic 20FPS"),
        PerfProfile::Smooth => tr(ui_language, "부드러움 45FPS", "Smooth 45FPS"),
    };
    let detail_profile = match state.detail_profile {
        DetailProfile::Perf => tr(ui_language, "성능", "Perf"),
        DetailProfile::Balanced => tr(ui_language, "균형", "Balanced"),
        DetailProfile::Ultra => tr(ui_language, "고품질", "Ultra"),
    };
    let clarity_profile = match state.clarity_profile {
        ClarityProfile::Balanced => tr(ui_language, "균형", "Balanced"),
        ClarityProfile::Sharp => tr(ui_language, "선명", "Sharp"),
        ClarityProfile::Extreme => tr(ui_language, "극선명", "Extreme"),
    };
    let ansi_quantization = match state.ansi_quantization {
        AnsiQuantization::Q216 => "q216",
        AnsiQuantization::Off => tr(ui_language, "끄기(truecolor)", "off (truecolor)"),
    };
    let backend = match state.backend {
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
    let center_lock = if state.center_lock {
        tr(ui_language, "켜짐", "On")
    } else {
        tr(ui_language, "꺼짐", "Off")
    };
    let color_mode = if matches!(state.mode, RenderMode::Ascii) {
        tr(ui_language, "항상 ON (ANSI)", "Always ON (ANSI)")
    } else {
        match state.color_mode {
            ColorMode::Mono => tr(ui_language, "모노", "Mono"),
            ColorMode::Ansi => tr(ui_language, "ANSI", "ANSI"),
        }
    };
    let braille_profile = match state.braille_profile {
        BrailleProfile::Safe => tr(ui_language, "안전", "Safe"),
        BrailleProfile::Normal => tr(ui_language, "표준", "Normal"),
        BrailleProfile::Dense => tr(ui_language, "고밀도", "Dense"),
    };
    let theme_style = match state.theme_style {
        ThemeStyle::Theater => tr(ui_language, "극장", "Theater"),
        ThemeStyle::Neon => tr(ui_language, "네온", "Neon"),
        ThemeStyle::Holo => tr(ui_language, "홀로그램", "Hologram"),
    };
    let audio_reactive = match state.audio_reactive {
        AudioReactiveMode::Off => tr(ui_language, "끔", "Off"),
        AudioReactiveMode::On => tr(ui_language, "보통", "On"),
        AudioReactiveMode::High => tr(ui_language, "강함", "High"),
    };
    let cinematic = match state.cinematic_camera {
        CinematicCameraMode::Off => tr(ui_language, "끔", "Off"),
        CinematicCameraMode::On => tr(ui_language, "보통", "On"),
        CinematicCameraMode::Aggressive => tr(ui_language, "강함", "Aggressive"),
    };
    let contrast = match state.contrast_profile {
        ContrastProfile::Adaptive => tr(ui_language, "적응형", "Adaptive"),
        ContrastProfile::Fixed => tr(ui_language, "고정", "Fixed"),
    };
    let sync_mode = match state.sync_speed_mode {
        SyncSpeedMode::AutoDurationFit => tr(ui_language, "자동", "Auto"),
        SyncSpeedMode::Realtime1x => tr(ui_language, "실시간", "Realtime"),
    };
    let output_mode = match state.output_mode {
        RenderOutputMode::Text => tr(ui_language, "텍스트", "Text"),
        RenderOutputMode::Hybrid => tr(ui_language, "하이브리드", "Hybrid"),
        RenderOutputMode::KittyHq => tr(ui_language, "Kitty HQ", "Kitty HQ"),
    };
    let graphics_protocol = match state.graphics_protocol {
        GraphicsProtocol::Auto => "auto",
        GraphicsProtocol::Kitty => "kitty",
        GraphicsProtocol::Iterm2 => "iterm2",
        GraphicsProtocol::None => "none",
    };
    let sync_policy = match state.sync_policy {
        SyncPolicy::Continuous => tr(ui_language, "연속", "Continuous"),
        SyncPolicy::Fixed => tr(ui_language, "고정", "Fixed"),
        SyncPolicy::Manual => tr(ui_language, "수동", "Manual"),
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
    let center_lock_mode = match state.center_lock_mode {
        CenterLockMode::Root => tr(ui_language, "루트", "Root"),
        CenterLockMode::Mixed => tr(ui_language, "혼합", "Mixed"),
    };
    let camera_focus = match state.camera_focus {
        CameraFocusMode::Auto => tr(ui_language, "자동", "Auto"),
        CameraFocusMode::Full => tr(ui_language, "전신", "Full"),
        CameraFocusMode::Upper => tr(ui_language, "상반신", "Upper"),
        CameraFocusMode::Face => tr(ui_language, "얼굴", "Face"),
        CameraFocusMode::Hands => tr(ui_language, "손", "Hands"),
    };
    let wasd_mode = match state.wasd_mode {
        CameraControlMode::Orbit => tr(ui_language, "오빗", "Orbit"),
        CameraControlMode::FreeFly => tr(ui_language, "자유이동", "FreeFly"),
    };
    let material_color = if state.material_color {
        tr(ui_language, "켜짐", "On")
    } else {
        tr(ui_language, "꺼짐", "Off")
    };
    let texture_sampling = match state.texture_sampling {
        TextureSamplingMode::Nearest => tr(ui_language, "최근접", "Nearest"),
        TextureSamplingMode::Bilinear => tr(ui_language, "쌍선형", "Bilinear"),
    };
    let stage_quality = match state.stage_quality {
        StageQuality::Minimal => tr(ui_language, "최소", "Minimal"),
        StageQuality::Low => tr(ui_language, "낮음", "Low"),
        StageQuality::Medium => tr(ui_language, "중간", "Medium"),
        StageQuality::High => tr(ui_language, "높음", "High"),
    };

    let rows = vec![
        format!("{}: {}", tr(ui_language, "모드", "Mode"), mode),
        format!(
            "{}: {}",
            tr(ui_language, "성능 프로필", "Perf Profile"),
            perf_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "디테일 프로필", "Detail Profile"),
            detail_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "선명도 프로필", "Clarity Profile"),
            clarity_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "ANSI 양자화", "ANSI Quantization"),
            ansi_quantization
        ),
        format!("{}: {}", tr(ui_language, "백엔드", "Backend"), backend),
        format!(
            "{}: {}",
            tr(ui_language, "중앙 고정", "Center Lock"),
            center_lock
        ),
        format!(
            "{}: {}",
            tr(ui_language, "중앙 고정 기준", "Center Lock Mode"),
            center_lock_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "WASD 모드", "WASD Mode"),
            wasd_mode
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "자유이동 속도", "FreeFly Speed"),
            state.freefly_speed
        ),
        format!(
            "{}: {}",
            tr(ui_language, "카메라 포커스", "Camera Focus"),
            camera_focus
        ),
        format!(
            "{}: {}",
            tr(ui_language, "재질 색상", "Material Color"),
            material_color
        ),
        format!(
            "{}: {}",
            tr(ui_language, "텍스처 샘플링", "Texture Sampling"),
            texture_sampling
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "모델 리프트", "Model Lift"),
            state.model_lift
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "엣지 강조", "Edge Accent"),
            state.edge_accent_strength
        ),
        format!(
            "{}: {}",
            tr(ui_language, "스테이지 레벨", "Stage Level"),
            state.stage_level
        ),
        format!(
            "{}: {}",
            tr(ui_language, "스테이지 품질", "Stage Quality"),
            stage_quality
        ),
        format!(
            "{}: {}",
            tr(ui_language, "컬러 모드", "Color Mode"),
            color_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "Braille 프로필", "Braille Profile"),
            braille_profile
        ),
        format!(
            "{}: {}",
            tr(ui_language, "분위기/조명 스타일", "Mood/Lighting"),
            theme_style
        ),
        format!(
            "{}: {}",
            tr(ui_language, "음악 반응", "Audio Reactive"),
            audio_reactive
        ),
        format!(
            "{}: {}",
            tr(ui_language, "시네마틱 카메라", "Cinematic Camera"),
            cinematic
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "반응 게인", "Reactive Gain"),
            state.reactive_gain
        ),
        format!(
            "{}: {}",
            tr(ui_language, "FPS 제한", "FPS Cap"),
            fps_label(START_FPS_OPTIONS[state.fps_index], ui_language)
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
            tr(ui_language, "출력 모드", "Output Mode"),
            output_mode
        ),
        format!(
            "{}: {}",
            tr(ui_language, "그래픽 프로토콜", "Graphics Protocol"),
            graphics_protocol
        ),
        format!(
            "{}: {}",
            tr(ui_language, "동기화 정책", "Sync Policy"),
            sync_policy
        ),
        format!(
            "{}: {} ms",
            tr(ui_language, "하드 스냅", "Hard Snap"),
            state.sync_hard_snap_ms
        ),
        format!(
            "{}: {:.2}",
            tr(ui_language, "동기화 Kp", "Sync Kp"),
            state.sync_kp
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

    let visible_rows = if matches!(state.render_detail_mode, RenderDetailMode::Quick) {
        rows.iter()
            .take(QUICK_RENDER_FIELD_COUNT)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        rows
    };

    let items = visible_rows
        .iter()
        .map(|text| ListItem::new(text.clone()))
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    list_state.select(Some(state.render_focus_index));
    let list = List::new(items)
        .style(theme.text_secondary)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(theme.border_active),
        )
        .highlight_style(theme.list_selected)
        .highlight_symbol(theme.list_selected_symbol);
    frame.render_stateful_widget(list, area, &mut list_state);
}

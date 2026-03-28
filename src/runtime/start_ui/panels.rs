use super::*;
use crate::runtime::start_ui_helpers::{
    aspect_preview_ascii, duration_label, fps_label, target_fps_for_profile, MIN_HEIGHT, MIN_WIDTH,
};

pub(super) fn draw_header(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
) {
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
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  •  "),
        Span::raw(format!("{} {}/9", step_name, state.step.index() + 1)),
    ]);

    let para = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(para, area);
}

pub(super) fn draw_help_panel(
    frame: &mut Frame,
    area: Rect,
    state: &StartWizardState,
    ui_language: UiLanguage,
    breakpoint: UiBreakpoint,
) {
    let mut lines = Vec::new();
    lines.push(Line::raw(match state.step {
        StartWizardStep::Branch => tr(
            ui_language,
            "브랜치: 좌/우 선택, Enter 다음, Esc 취소",
            "Branch: left/right select, Enter next, Esc cancel",
        ),
        StartWizardStep::Model => tr(
            ui_language,
            "모델: ↑/↓ 선택, Enter 다음, Esc 취소",
            "Model: ↑/↓ select, Enter next, Esc cancel",
        ),
        StartWizardStep::Motion => tr(
            ui_language,
            "모션: ↑/↓ 선택, Enter 다음, Esc 이전",
            "Motion: ↑/↓ select, Enter next, Esc back",
        ),
        StartWizardStep::Music => tr(
            ui_language,
            "음악: ↑/↓ 선택, Enter 다음, Esc 이전",
            "Music: ↑/↓ select, Enter next, Esc back",
        ),
        StartWizardStep::Stage => tr(
            ui_language,
            "스테이지: ↑/↓ 선택, Enter 다음, Esc 이전",
            "Stage: ↑/↓ select, Enter next, Esc back",
        ),
        StartWizardStep::Camera => tr(
            ui_language,
            "카메라: ↑/↓ 항목, ←/→ 값 변경, Enter 다음, Esc 이전",
            "Camera: ↑/↓ focus, ←/→ change, Enter next, Esc back",
        ),
        StartWizardStep::Render => tr(
            ui_language,
            "옵션: ↑/↓ 항목, ←/→ 값 변경, Enter 다음, Esc 이전",
            "Options: ↑/↓ focus, ←/→ change, Enter next, Esc back",
        ),
        StartWizardStep::AspectCalib => tr(
            ui_language,
            "보정: ←/→ trim, r 리셋, Enter 다음, Esc 이전",
            "Calib: ←/→ trim, r reset, Enter next, Esc back",
        ),
        StartWizardStep::Confirm => tr(
            ui_language,
            "확인: Enter 실행, Esc 이전",
            "Confirm: Enter run, Esc back",
        ),
    }));

    if breakpoint != UiBreakpoint::Compact {
        lines.push(Line::raw(tr(
            ui_language,
            "공통: q 취소, Tab 다음, Shift+Tab 이전",
            "Common: q cancel, Tab next, Shift+Tab prev",
        )));
    }

    let help = Paragraph::new(lines)
        .block(
            Block::default()
                .title(tr(ui_language, "조작", "Help"))
                .borders(Borders::ALL),
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

    let lines = vec![
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "모델 경로", "Model Dir"),
            model_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "PMX 경로", "PMX Dir"),
            pmx_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "모션 경로", "Motion Dir"),
            motion_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "음악 경로", "Music Dir"),
            music_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "스테이지 경로", "Stage Dir"),
            stage_dir.display()
        )),
        Line::raw(format!(
            "{}: {}",
            tr(ui_language, "카메라 경로", "Camera Dir"),
            camera_dir.display()
        )),
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
            "{}: {} / {:?} / {:?} / {:.2}",
            tr(ui_language, "카메라", "Camera"),
            camera_name,
            selection.camera_mode,
            selection.camera_align_preset,
            selection.camera_unit_scale
        )),
        Line::raw(format!(
            "{}: {:.3}",
            tr(ui_language, "적용 비율", "Applied Aspect"),
            state.effective_cell_aspect()
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "모드", "Mode"),
            selection.mode,
            selection.color_mode
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "출력/프로토콜", "Output/Protocol"),
            selection.output_mode,
            selection.graphics_protocol
        )),
        Line::raw(format!(
            "{}: {:?} / {:?} / {:?} / {:?}",
            tr(
                ui_language,
                "프로필/디테일/선명도/백엔드",
                "Profile/Detail/Clarity/Backend"
            ),
            selection.perf_profile,
            selection.detail_profile,
            selection.clarity_profile,
            selection.backend
        )),
        Line::raw(format!(
            "{}: {}({:?}) / {}",
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
            "{}: {:?} ({:.2})",
            tr(ui_language, "WASD 모드/속도", "WASD Mode/Speed"),
            selection.wasd_mode,
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
            "{}: {:?} / {:?}",
            tr(ui_language, "Braille/색경로", "Braille/Color Path"),
            selection.braille_profile,
            selection.ansi_quantization
        )),
        Line::raw(format!(
            "{}: {:?} / {:?}",
            tr(ui_language, "테마/반응", "Theme/Reactive"),
            selection.theme_style,
            selection.audio_reactive
        )),
        Line::raw(format!(
            "{}: {:?}",
            tr(ui_language, "시네마틱 카메라", "Cinematic Camera"),
            selection.cinematic_camera
        )),
        Line::raw(format!(
            "{}: {:.2}",
            tr(ui_language, "반응 게인", "Reactive Gain"),
            selection.reactive_gain
        )),
        Line::raw(format!(
            "{}: {}ms",
            tr(ui_language, "Offset", "Offset"),
            state.sync_offset_ms
        )),
        Line::raw(format!(
            "{}: {:.4}",
            tr(ui_language, "Speed", "Speed"),
            state.expected_sync_speed()
        )),
        Line::raw(format!(
            "{}: {:?} / {}ms / kp {:.2}",
            tr(ui_language, "정책", "Policy"),
            selection.sync_policy,
            selection.sync_hard_snap_ms,
            selection.sync_kp
        )),
    ];

    let para = Paragraph::new(lines)
        .block(
            Block::default()
                .title(tr(ui_language, "선택 요약", "Selection Summary"))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

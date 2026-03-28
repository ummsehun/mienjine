use super::*;
use crossterm::event::KeyModifiers;
use ratatui::{backend::TestBackend, Terminal};

fn key(code: KeyCode) -> StartWizardEvent {
    StartWizardEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn test_state() -> StartWizardState {
    let model_entries = vec![StartEntry::from_path(Path::new("miku.glb"))];
    let pmx_entries = vec![StartEntry::from_path(Path::new("miku.pmx"))];
    let motion_entries = vec![StartEntry::from_path(Path::new("dance.vmd"))];
    let music_entries = vec![StartEntry::from_path(Path::new("world.mp3"))];
    let camera_entries = vec![StartEntry::from_path(Path::new("world_is_mine.vmd"))];
    let stage_entries = vec![StageChoice {
        name: "default-stage".to_owned(),
        status: StageStatus::Ready,
        render_path: Some(PathBuf::from("assets/stage/default-stage/stage.glb")),
        pmx_path: None,
        transform: StageTransform::default(),
    }];
    StartWizardState::new(
        model_entries,
        pmx_entries,
        motion_entries,
        music_entries,
        stage_entries,
        camera_entries,
        StartWizardDefaults::default(),
        120,
        35,
    )
}

#[test]
fn transitions_model_to_confirm_with_enter() {
    let mut state = test_state();
    assert_eq!(state.step, StartWizardStep::Branch);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Model);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Music);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Stage);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Camera);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Render);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::AspectCalib);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Confirm);

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Submit(_)
    ));
}

#[test]
fn esc_moves_back_or_cancels() {
    let mut state = test_state();

    state.step = StartWizardStep::Branch;
    state.branch = ModelBranch::PmxVmd;
    assert!(matches!(
        state.apply_event(key(KeyCode::Left)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.branch, ModelBranch::Glb);

    state.step = StartWizardStep::Motion;
    assert!(matches!(
        state.apply_event(key(KeyCode::Esc)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Model);

    state.step = StartWizardStep::Branch;
    assert!(matches!(
        state.apply_event(key(KeyCode::Esc)),
        StartWizardAction::Cancel
    ));
}

#[test]
fn pmx_branch_inserts_motion_step() {
    let mut state = test_state();
    state.branch = ModelBranch::PmxVmd;
    state.step = StartWizardStep::Model;

    assert!(matches!(
        state.apply_event(key(KeyCode::Enter)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Motion);
}

#[test]
fn motion_step_esc_returns_to_model() {
    let mut state = test_state();
    state.step = StartWizardStep::Motion;

    assert!(matches!(
        state.apply_event(key(KeyCode::Esc)),
        StartWizardAction::Continue
    ));
    assert_eq!(state.step, StartWizardStep::Model);
}

#[test]
fn tab_and_backtab_handle_focus_on_render_step() {
    let mut state = test_state();
    state.step = StartWizardStep::Render;
    state.render_focus_index = 0;

    state.apply_event(key(KeyCode::Tab));
    assert_eq!(state.render_focus_index, 1);

    state.apply_event(key(KeyCode::BackTab));
    assert_eq!(state.render_focus_index, 0);
}

#[test]
fn breakpoint_edges() {
    assert_eq!(breakpoint_for(140, 40), UiBreakpoint::Wide);
    assert_eq!(breakpoint_for(139, 40), UiBreakpoint::Normal);
    assert_eq!(breakpoint_for(100, 28), UiBreakpoint::Normal);
    assert_eq!(breakpoint_for(99, 28), UiBreakpoint::Compact);
}

#[test]
fn aspect_calibration_step_updates_trim() {
    let mut state = test_state();
    state.step = StartWizardStep::AspectCalib;
    let before = state.cell_aspect_trim;
    state.apply_event(key(KeyCode::Right));
    assert!(state.cell_aspect_trim > before);
}

#[test]
fn selecting_camera_source_auto_enables_vmd_mode() {
    let mut state = test_state();
    state.step = StartWizardStep::Camera;
    state.camera_mode = CameraMode::Off;
    state.camera_focus_index = 0;
    state.apply_event(key(KeyCode::Right));
    assert_eq!(state.camera_index, 1);
    assert!(matches!(state.camera_mode, CameraMode::Vmd));
}

#[test]
fn render_wide_normal_compact() {
    for (w, h) in [(432, 102), (120, 35), (80, 22)] {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let state = test_state();
        terminal
            .draw(|frame| {
                draw_start_wizard(
                    frame,
                    Path::new("assets/glb"),
                    Path::new("assets/pmx"),
                    Path::new("assets/vmd"),
                    Path::new("assets/music"),
                    Path::new("assets/stage"),
                    Path::new("assets/camera"),
                    &state,
                    UiLanguage::Ko,
                );
            })
            .expect("render should succeed");
    }
}

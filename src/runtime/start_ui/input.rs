use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use super::*;

impl StartWizardState {
    pub(super) fn apply_event(&mut self, event: StartWizardEvent) -> StartWizardAction {
        match event {
            StartWizardEvent::Resize(width, height) => {
                self.on_resize(width, height);
                StartWizardAction::Continue
            }
            StartWizardEvent::Tick => StartWizardAction::Continue,
            StartWizardEvent::Key(key) => self.apply_key(key),
        }
    }

    fn apply_key(&mut self, key: KeyEvent) -> StartWizardAction {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return StartWizardAction::Continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return StartWizardAction::Cancel,
            _ => {}
        }

        if self.is_too_small() {
            return StartWizardAction::Continue;
        }

        match key.code {
            KeyCode::Tab => {
                self.tab_forward();
                return StartWizardAction::Continue;
            }
            KeyCode::BackTab => {
                self.tab_backward();
                return StartWizardAction::Continue;
            }
            _ => {}
        }

        match self.step {
            StartWizardStep::Branch => self.apply_branch_key(key),
            StartWizardStep::Model => self.apply_model_key(key),
            StartWizardStep::Motion => self.apply_motion_key(key),
            StartWizardStep::Music => self.apply_music_key(key),
            StartWizardStep::Stage => self.apply_stage_key(key),
            StartWizardStep::Camera => self.apply_camera_key(key),
            StartWizardStep::Render => self.apply_render_key(key),
            StartWizardStep::AspectCalib => self.apply_aspect_key(key),
            StartWizardStep::Confirm => self.apply_confirm_key(key),
        }
    }

    fn apply_model_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let model_len = match self.branch {
            ModelBranch::Glb => self.model_entries.len(),
            ModelBranch::PmxVmd => self.pmx_entries.len(),
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.model_index, model_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.model_index, model_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = if matches!(self.branch, ModelBranch::PmxVmd) {
                    StartWizardStep::Motion
                } else {
                    StartWizardStep::Music
                };
                StartWizardAction::Continue
            }
            KeyCode::Esc => StartWizardAction::Cancel,
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_branch_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
                self.branch = match self.branch {
                    ModelBranch::Glb => ModelBranch::PmxVmd,
                    ModelBranch::PmxVmd => ModelBranch::Glb,
                };
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('l') | KeyCode::Char('j') => {
                self.branch = match self.branch {
                    ModelBranch::Glb => ModelBranch::PmxVmd,
                    ModelBranch::PmxVmd => ModelBranch::Glb,
                };
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Model;
                StartWizardAction::Continue
            }
            KeyCode::Esc => StartWizardAction::Cancel,
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_motion_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let motion_len = self.motion_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.motion_index, motion_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.motion_index, motion_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Music;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = if matches!(self.branch, ModelBranch::PmxVmd) {
                    StartWizardStep::Motion
                } else {
                    StartWizardStep::Model
                };
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_music_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let music_len = self.music_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.music_index, music_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.music_index, music_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Stage;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = if matches!(self.branch, ModelBranch::PmxVmd) {
                    StartWizardStep::Motion
                } else {
                    StartWizardStep::Model
                };
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_stage_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let stage_len = self.stage_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.stage_index, stage_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.stage_index, stage_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Camera;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Music;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_camera_key(&mut self, key: KeyEvent) -> StartWizardAction {
        let camera_len = self.camera_entries.len() + 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.camera_focus_index, 4, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.camera_focus_index, 4, 1);
                StartWizardAction::Continue
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                self.adjust_camera_value(camera_len, -1);
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.adjust_camera_value(camera_len, 1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Render;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Stage;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_render_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                cycle_index(&mut self.render_focus_index, RENDER_FIELD_COUNT, -1);
                StartWizardAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                cycle_index(&mut self.render_focus_index, RENDER_FIELD_COUNT, 1);
                StartWizardAction::Continue
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                self.adjust_render_value(-1);
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.adjust_render_value(1);
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::AspectCalib;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Camera;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_aspect_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') => {
                self.cell_aspect_trim = (self.cell_aspect_trim - 0.01).clamp(0.70, 1.30);
                StartWizardAction::Continue
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') => {
                self.cell_aspect_trim = (self.cell_aspect_trim + 0.01).clamp(0.70, 1.30);
                StartWizardAction::Continue
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.cell_aspect_trim = 1.0;
                StartWizardAction::Continue
            }
            KeyCode::Enter => {
                self.step = StartWizardStep::Confirm;
                StartWizardAction::Continue
            }
            KeyCode::Esc => {
                self.step = StartWizardStep::Render;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn apply_confirm_key(&mut self, key: KeyEvent) -> StartWizardAction {
        match key.code {
            KeyCode::Enter => StartWizardAction::Submit(self.selection()),
            KeyCode::Esc => {
                self.step = StartWizardStep::AspectCalib;
                StartWizardAction::Continue
            }
            _ => StartWizardAction::Continue,
        }
    }

    fn tab_forward(&mut self) {
        match self.step {
            StartWizardStep::Branch => self.step = StartWizardStep::Model,
            StartWizardStep::Model => self.step = StartWizardStep::Music,
            StartWizardStep::Motion => self.step = StartWizardStep::Music,
            StartWizardStep::Music => self.step = StartWizardStep::Stage,
            StartWizardStep::Stage => self.step = StartWizardStep::Camera,
            StartWizardStep::Camera => self.step = StartWizardStep::Render,
            StartWizardStep::Render => {
                if self.render_focus_index + 1 < RENDER_FIELD_COUNT {
                    self.render_focus_index += 1;
                } else {
                    self.step = StartWizardStep::AspectCalib;
                }
            }
            StartWizardStep::AspectCalib => self.step = StartWizardStep::Confirm,
            StartWizardStep::Confirm => {}
        }
    }

    fn tab_backward(&mut self) {
        match self.step {
            StartWizardStep::Branch => {}
            StartWizardStep::Model => {}
            StartWizardStep::Motion => self.step = StartWizardStep::Model,
            StartWizardStep::Music => self.step = StartWizardStep::Model,
            StartWizardStep::Stage => self.step = StartWizardStep::Music,
            StartWizardStep::Camera => self.step = StartWizardStep::Stage,
            StartWizardStep::Render => {
                if self.render_focus_index > 0 {
                    self.render_focus_index -= 1;
                } else {
                    self.step = StartWizardStep::Camera;
                }
            }
            StartWizardStep::AspectCalib => self.step = StartWizardStep::Render,
            StartWizardStep::Confirm => self.step = StartWizardStep::AspectCalib,
        }
    }
}

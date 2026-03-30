use super::*;

impl StartWizardState {
    pub(super) fn adjust_camera_value(&mut self, camera_len: usize, delta: i32) {
        match self.camera_focus_index {
            0 => {
                cycle_index(&mut self.camera_index, camera_len, delta);
                if self.camera_index == 0 {
                    self.camera_mode = CameraMode::Off;
                } else if matches!(self.camera_mode, CameraMode::Off) {
                    self.camera_mode = CameraMode::Vmd;
                }
            }
            1 => {
                self.camera_mode = match self.camera_mode {
                    CameraMode::Off => CameraMode::Vmd,
                    CameraMode::Vmd => CameraMode::Blend,
                    CameraMode::Blend => CameraMode::Off,
                };
            }
            2 => {
                self.camera_align_preset = match self.camera_align_preset {
                    CameraAlignPreset::Std => CameraAlignPreset::AltA,
                    CameraAlignPreset::AltA => CameraAlignPreset::AltB,
                    CameraAlignPreset::AltB => CameraAlignPreset::Std,
                };
            }
            3 => {
                self.camera_unit_scale =
                    (self.camera_unit_scale + 0.01 * delta as f32).clamp(0.01, 2.0);
            }
            _ => {}
        }
    }

    pub(super) fn adjust_render_value(&mut self, delta: i32) {
        match self.render_focus_index {
            0 => {
                self.mode = match self.mode {
                    RenderMode::Ascii => RenderMode::Braille,
                    RenderMode::Braille => RenderMode::Ascii,
                };
                if matches!(self.mode, RenderMode::Ascii) {
                    self.color_mode = ColorMode::Ansi;
                }
            }
            1 => {
                self.perf_profile = match self.perf_profile {
                    PerfProfile::Balanced => PerfProfile::Cinematic,
                    PerfProfile::Cinematic => PerfProfile::Smooth,
                    PerfProfile::Smooth => PerfProfile::Balanced,
                };
            }
            2 => {
                self.detail_profile = match self.detail_profile {
                    DetailProfile::Perf => DetailProfile::Balanced,
                    DetailProfile::Balanced => DetailProfile::Ultra,
                    DetailProfile::Ultra => DetailProfile::Perf,
                };
            }
            3 => {
                self.clarity_profile = match self.clarity_profile {
                    ClarityProfile::Balanced => ClarityProfile::Sharp,
                    ClarityProfile::Sharp => ClarityProfile::Extreme,
                    ClarityProfile::Extreme => ClarityProfile::Balanced,
                };
            }
            4 => {
                self.ansi_quantization = match self.ansi_quantization {
                    AnsiQuantization::Q216 => AnsiQuantization::Off,
                    AnsiQuantization::Off => AnsiQuantization::Q216,
                };
            }
            5 => {
                self.backend = match self.backend {
                    RenderBackend::Cpu => RenderBackend::Gpu,
                    RenderBackend::Gpu => RenderBackend::Cpu,
                };
            }
            6 => {
                self.center_lock = !self.center_lock;
            }
            7 => {
                self.center_lock_mode = match self.center_lock_mode {
                    CenterLockMode::Root => CenterLockMode::Mixed,
                    CenterLockMode::Mixed => CenterLockMode::Root,
                };
            }
            8 => {
                self.wasd_mode = match self.wasd_mode {
                    CameraControlMode::Orbit => CameraControlMode::FreeFly,
                    CameraControlMode::FreeFly => CameraControlMode::Orbit,
                };
            }
            9 => {
                let step = 0.1 * (delta as f32);
                self.freefly_speed = (self.freefly_speed + step).clamp(0.1, 8.0);
            }
            10 => {
                self.camera_focus = match self.camera_focus {
                    CameraFocusMode::Auto => CameraFocusMode::Full,
                    CameraFocusMode::Full => CameraFocusMode::Upper,
                    CameraFocusMode::Upper => CameraFocusMode::Face,
                    CameraFocusMode::Face => CameraFocusMode::Hands,
                    CameraFocusMode::Hands => CameraFocusMode::Auto,
                };
            }
            11 => {
                self.material_color = !self.material_color;
            }
            12 => {
                self.texture_sampling = match self.texture_sampling {
                    TextureSamplingMode::Nearest => TextureSamplingMode::Bilinear,
                    TextureSamplingMode::Bilinear => TextureSamplingMode::Nearest,
                };
            }
            13 => {
                let step = 0.01 * (delta as f32);
                self.model_lift = (self.model_lift + step).clamp(0.02, 0.45);
            }
            14 => {
                let step = 0.05 * (delta as f32);
                self.edge_accent_strength = (self.edge_accent_strength + step).clamp(0.0, 1.5);
            }
            15 => {
                let value = (self.stage_level as i32 + delta).clamp(0, 4);
                self.stage_level = value as u8;
            }
            16 => {
                if matches!(self.mode, RenderMode::Braille) {
                    self.color_mode = match self.color_mode {
                        ColorMode::Mono => ColorMode::Ansi,
                        ColorMode::Ansi => ColorMode::Mono,
                    };
                } else {
                    self.color_mode = ColorMode::Ansi;
                }
            }
            17 => {
                self.braille_profile = match self.braille_profile {
                    BrailleProfile::Safe => BrailleProfile::Normal,
                    BrailleProfile::Normal => BrailleProfile::Dense,
                    BrailleProfile::Dense => BrailleProfile::Safe,
                };
            }
            18 => {
                self.theme_style = match self.theme_style {
                    ThemeStyle::Theater => ThemeStyle::Neon,
                    ThemeStyle::Neon => ThemeStyle::Holo,
                    ThemeStyle::Holo => ThemeStyle::Theater,
                };
            }
            19 => {
                self.audio_reactive = match self.audio_reactive {
                    AudioReactiveMode::Off => AudioReactiveMode::On,
                    AudioReactiveMode::On => AudioReactiveMode::High,
                    AudioReactiveMode::High => AudioReactiveMode::Off,
                };
            }
            20 => {
                self.cinematic_camera = match self.cinematic_camera {
                    CinematicCameraMode::Off => CinematicCameraMode::On,
                    CinematicCameraMode::On => CinematicCameraMode::Aggressive,
                    CinematicCameraMode::Aggressive => CinematicCameraMode::Off,
                };
            }
            21 => {
                let step = 0.05 * (delta as f32);
                self.reactive_gain = (self.reactive_gain + step).clamp(0.0, 1.0);
            }
            22 => cycle_index(&mut self.fps_index, START_FPS_OPTIONS.len(), delta),
            23 => {
                self.contrast_profile = match self.contrast_profile {
                    ContrastProfile::Adaptive => ContrastProfile::Fixed,
                    ContrastProfile::Fixed => ContrastProfile::Adaptive,
                }
            }
            24 => {
                let next = self
                    .sync_offset_ms
                    .saturating_add(delta.saturating_mul(SYNC_OFFSET_STEP_MS));
                self.sync_offset_ms = next.clamp(-SYNC_OFFSET_LIMIT_MS, SYNC_OFFSET_LIMIT_MS);
            }
            25 => {
                self.sync_speed_mode = match self.sync_speed_mode {
                    SyncSpeedMode::AutoDurationFit => SyncSpeedMode::Realtime1x,
                    SyncSpeedMode::Realtime1x => SyncSpeedMode::AutoDurationFit,
                }
            }
            26 => {
                self.output_mode = match self.output_mode {
                    RenderOutputMode::Text => RenderOutputMode::Hybrid,
                    RenderOutputMode::Hybrid => RenderOutputMode::KittyHq,
                    RenderOutputMode::KittyHq => RenderOutputMode::Text,
                };
            }
            27 => {
                self.graphics_protocol = match self.graphics_protocol {
                    GraphicsProtocol::Auto => GraphicsProtocol::Kitty,
                    GraphicsProtocol::Kitty => GraphicsProtocol::Iterm2,
                    GraphicsProtocol::Iterm2 => GraphicsProtocol::None,
                    GraphicsProtocol::None => GraphicsProtocol::Auto,
                };
            }
            28 => {
                self.sync_policy = match self.sync_policy {
                    SyncPolicy::Continuous => SyncPolicy::Fixed,
                    SyncPolicy::Fixed => SyncPolicy::Manual,
                    SyncPolicy::Manual => SyncPolicy::Continuous,
                };
            }
            29 => {
                let next = (self.sync_hard_snap_ms as i32 + delta * 10).clamp(10, 2_000);
                self.sync_hard_snap_ms = next as u32;
            }
            30 => {
                self.sync_kp = (self.sync_kp + 0.01 * delta as f32).clamp(0.01, 1.0);
            }
            31 => {
                self.cell_aspect_mode = match self.cell_aspect_mode {
                    CellAspectMode::Auto => CellAspectMode::Manual,
                    CellAspectMode::Manual => CellAspectMode::Auto,
                }
            }
            32 => {
                self.font_preset_enabled = !self.font_preset_enabled;
            }
            _ => {}
        }
    }
}

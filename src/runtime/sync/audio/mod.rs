use std::{fs::File, io::BufReader, path::Path};

use rodio::{Decoder, OutputStream, Sink, Source};

use crate::{
    assets::vmd_camera::parse_vmd_camera,
    engine::camera_track::{CameraTrackSampler, MmdCameraTransform},
    runtime::state::{ContinuousSyncState, RuntimeCameraSettings},
    scene::{CameraMode, SyncPolicy, SyncSpeedMode},
};

#[derive(Debug, Clone)]
pub(crate) struct AudioEnvelope {
    pub(crate) fps: u32,
    pub(crate) values: Vec<f32>,
    pub(crate) duration_secs: f32,
}

impl AudioEnvelope {
    pub(crate) fn sample(&self, time_secs: f32) -> f32 {
        if self.values.is_empty() || self.duration_secs <= f32::EPSILON || self.fps == 0 {
            return 0.0;
        }
        let wrapped = time_secs.rem_euclid(self.duration_secs.max(f32::EPSILON));
        let idx = ((wrapped * self.fps as f32).floor() as usize) % self.values.len();
        self.values[idx].clamp(0.0, 1.0)
    }
}

pub(crate) struct MusicPlayback {
    pub(crate) _stream: OutputStream,
    pub(crate) sink: Sink,
    duration_secs: Option<f32>,
}

impl Drop for MusicPlayback {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

pub(crate) struct AudioSyncRuntime {
    pub(crate) playback: MusicPlayback,
    pub(crate) speed_factor: f32,
    pub(crate) envelope: Option<AudioEnvelope>,
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedCameraTrack {
    pub(crate) sampler: CameraTrackSampler,
    pub(crate) transform: MmdCameraTransform,
}

fn start_music_playback(path: Option<&Path>) -> Option<MusicPlayback> {
    let path = path?;
    let stream = OutputStream::try_default().ok()?;
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let duration_secs = decoder.total_duration().map(|d| d.as_secs_f32());
    let sink = Sink::try_new(&stream.1).ok()?;
    sink.pause();
    sink.append(decoder.repeat_infinite());
    Some(MusicPlayback {
        _stream: stream.0,
        sink,
        duration_secs,
    })
}

fn build_audio_envelope(path: Option<&Path>, fps: u32) -> Option<AudioEnvelope> {
    let path = path?;
    if fps == 0 {
        return None;
    }
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let channels = decoder.channels().max(1) as usize;
    let sample_rate = decoder.sample_rate().max(1);
    let total_duration = decoder
        .total_duration()
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    let samples_per_bucket =
        ((sample_rate as f32 * channels as f32) / (fps as f32)).round() as usize;
    let bucket_size = samples_per_bucket.max(channels);

    let mut values = Vec::new();
    let mut acc = 0.0_f32;
    let mut count = 0_usize;
    for sample in decoder {
        let s = (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0);
        acc += s * s;
        count += 1;
        if count >= bucket_size {
            let rms = (acc / (count as f32)).sqrt();
            values.push(rms);
            acc = 0.0;
            count = 0;
        }
    }
    if count > 0 {
        values.push((acc / (count as f32)).sqrt());
    }
    if values.is_empty() {
        return None;
    }

    let max = values
        .iter()
        .copied()
        .fold(0.0_f32, |a, b| if b > a { b } else { a });
    if max > f32::EPSILON {
        for value in &mut values {
            *value = (*value / max).clamp(0.0, 1.0);
        }
    }
    let duration_secs = if total_duration > f32::EPSILON {
        total_duration
    } else {
        (values.len() as f32) / (fps as f32)
    };
    Some(AudioEnvelope {
        fps,
        values,
        duration_secs,
    })
}

pub(crate) fn prepare_audio_sync(
    music_path: Option<&Path>,
    clip_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> Option<AudioSyncRuntime> {
    let envelope = build_audio_envelope(music_path, 60);
    let playback = start_music_playback(music_path)?;
    let speed_factor =
        compute_animation_speed_factor(clip_duration_secs, playback.duration_secs, mode);
    if matches!(mode, SyncSpeedMode::AutoDurationFit) && (speed_factor - 1.0).abs() > 1e-4 {
        tracing::info!(
            "audio sync speed factor applied {:.4} (clip={:?}s, audio={:?}s)",
            speed_factor, clip_duration_secs, playback.duration_secs
        );
    }
    Some(AudioSyncRuntime {
        playback,
        speed_factor,
        envelope,
    })
}

pub(crate) fn compute_animation_speed_factor(
    clip_duration_secs: Option<f32>,
    audio_duration_secs: Option<f32>,
    mode: SyncSpeedMode,
) -> f32 {
    if !matches!(mode, SyncSpeedMode::AutoDurationFit) {
        return 1.0;
    }
    let Some(clip) = clip_duration_secs else {
        return 1.0;
    };
    let Some(audio) = audio_duration_secs else {
        return 1.0;
    };
    if clip <= f32::EPSILON || audio <= f32::EPSILON {
        return 1.0;
    }
    (clip / audio).clamp(0.25, 4.0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_animation_time(
    state: &mut ContinuousSyncState,
    policy: SyncPolicy,
    dt_wall: f32,
    elapsed_wall: f32,
    elapsed_audio: Option<f32>,
    speed_factor: f32,
    sync_offset_ms: i32,
    hard_snap_ms: u32,
    sync_kp: f32,
    clip_duration: Option<f32>,
) -> f32 {
    let offset = (sync_offset_ms as f32) / 1000.0;
    let hard_snap_sec = (hard_snap_ms as f32 / 1000.0).clamp(0.005, 5.0);
    let kp = sync_kp.clamp(0.01, 1.0);
    let dt = dt_wall.max(0.0);

    let target_audio = elapsed_audio.map(|seconds| seconds * speed_factor + offset);

    match policy {
        SyncPolicy::Manual => {
            if !state.initialized {
                state.anim_time = elapsed_wall + offset;
                state.initialized = true;
            } else {
                state.anim_time += dt;
            }
            state.drift_ema *= 0.92;
        }
        SyncPolicy::Fixed => {
            state.anim_time = target_audio.unwrap_or(elapsed_wall + offset);
            state.initialized = true;
            state.drift_ema *= 0.92;
        }
        SyncPolicy::Continuous => {
            if let Some(target) = target_audio {
                if !state.initialized {
                    state.anim_time = target;
                    state.initialized = true;
                    state.drift_ema = 0.0;
                } else {
                    let err = target - state.anim_time;
                    state.drift_ema += (err.abs() - state.drift_ema) * 0.08;
                    if err.abs() > hard_snap_sec {
                        state.anim_time = target;
                        state.hard_snap_count = state.hard_snap_count.saturating_add(1);
                    } else {
                        let drift_gain = (state.drift_ema / hard_snap_sec).clamp(0.0, 1.0);
                        let long_drift_term = (err * 0.18 * drift_gain).clamp(-0.16, 0.16);
                        let rate = (speed_factor + kp * err + long_drift_term).clamp(0.25, 4.0);
                        state.anim_time += dt * rate;
                    }
                }
            } else {
                state.anim_time = elapsed_wall + offset;
                state.initialized = true;
                state.drift_ema *= 0.92;
            }
        }
    }

    if let Some(duration) = clip_duration.filter(|value| *value > f32::EPSILON) {
        state.anim_time = state.anim_time.rem_euclid(duration);
    }
    state.anim_time
}

pub(crate) fn load_camera_track(settings: &RuntimeCameraSettings) -> Option<LoadedCameraTrack> {
    if matches!(settings.mode, CameraMode::Off) {
        return None;
    }
    let path = settings.vmd_path.as_deref()?;
    let track = parse_vmd_camera(path).ok()?;
    let sampler = CameraTrackSampler::from_vmd(&track, settings.vmd_fps)?;
    let transform = MmdCameraTransform::from_preset(settings.align_preset, settings.unit_scale);
    Some(LoadedCameraTrack { sampler, transform })
}

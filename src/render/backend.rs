use glam::Mat4;

use crate::renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, RenderStats};
use crate::scene::{RenderBackend, RenderConfig, SceneCpu};

use super::{backend_cpu, backend_gpu};

pub fn render_frame_with_backend(
    gpu_renderer_state: &mut backend_gpu::GpuRendererState,
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    material_morph_weights: &[f32],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    match config.backend {
        RenderBackend::Cpu => backend_cpu::render_frame_cpu(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            material_morph_weights,
            glyph_ramp,
            scratch,
            camera,
            model_rotation_y,
        ),
        RenderBackend::Gpu => match backend_gpu::render_frame_gpu(
            gpu_renderer_state,
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            glyph_ramp,
            scratch,
            camera,
            model_rotation_y,
        ) {
            Ok(stats) => stats,
            Err(err) => {
                eprintln!("warning: gpu backend failed ({err:?}); falling back to cpu.");

                let mut cpu_cfg = config.clone();
                cpu_cfg.backend = RenderBackend::Cpu;
                backend_cpu::render_frame_cpu(
                    frame,
                    &cpu_cfg,
                    scene,
                    global_matrices,
                    skin_matrices,
                    instance_morph_weights,
                    material_morph_weights,
                    glyph_ramp,
                    scratch,
                    camera,
                    model_rotation_y,
                )
            }
        },
    }
}

#[cfg(all(test, feature = "gpu"))]
#[path = "backend_tests.rs"]
mod tests;

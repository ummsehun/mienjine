#[cfg(all(feature = "gpu", target_os = "macos"))]
use std::sync::Once;

use glam::Mat4;

use crate::renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, RenderStats};
use crate::scene::{RenderConfig, SceneCpu};

#[derive(Debug)]
pub enum GpuBackendError {
    Unsupported,
}

pub fn render_frame_gpu(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> Result<RenderStats, GpuBackendError> {
    #[cfg(all(feature = "gpu", target_os = "macos"))]
    {
        static WARN: Once = Once::new();
        WARN.call_once(|| {
            eprintln!(
                "warning: gpu backend is experimental. using compatibility raster path for now."
            );
        });
        // TODO: Replace with real Metal(wgpu) triangle raster pass.
        Ok(crate::renderer::render_frame(
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
        ))
    }

    #[cfg(not(all(feature = "gpu", target_os = "macos")))]
    {
        let _ = (
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
        );
        Err(GpuBackendError::Unsupported)
    }
}

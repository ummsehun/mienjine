use std::collections::HashMap;

use glam::{Mat3, Mat4};

use crate::renderer::{exposure_bias_multiplier, Camera, FrameBuffers, GlyphRamp, PixelFrame, RenderScratch, RenderStats};
use crate::scene::{
    MaterialAlphaMode, MeshLayer, RenderConfig, SceneCpu, StageRole, TextureSamplingMode,
    TextureVOrigin, TextureWrapMode,
};
use crate::render::backend_gpu::GpuRendererState;

mod cache;

use super::{
    device::{GpuContext, GpuError},
    pipeline::GpuPipeline,
    pipeline::Uniforms,
    resources::{GpuMesh, GpuTexture},
    texture::{GpuTexture as RenderTarget, TextureSize},
};

use cache::{SceneSignature, TextureBindingKey};

#[cfg(feature = "gpu")]
use super::stats::compute_gpu_render_stats;

pub struct GpuRenderer {
    ctx: GpuContext,
    pipeline: Option<GpuPipeline>,
    mesh_cache: HashMap<usize, GpuMesh>,
    morph_mesh_cache: HashMap<usize, GpuMesh>,
    texture_cache: HashMap<usize, GpuTexture>,
    texture_bind_groups: HashMap<TextureBindingKey, wgpu::BindGroup>,
    default_texture: Option<GpuTexture>,
    default_bind_group: Option<wgpu::BindGroup>,
    cached_render_target: Option<RenderTarget>,
    cached_render_target_size: Option<(u32, u32)>,
    cached_scene_sig: Option<SceneSignature>,
}

impl GpuRenderer {
    pub fn new() -> Result<Self, GpuError> {
        let ctx = GpuContext::new()?;
        Ok(Self {
            ctx,
            pipeline: None,
            mesh_cache: HashMap::new(),
            morph_mesh_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            texture_bind_groups: HashMap::new(),
            default_texture: None,
            default_bind_group: None,
            cached_render_target: None,
            cached_render_target_size: None,
            cached_scene_sig: None,
        })
    }

    pub fn is_available() -> bool {
        std::thread::spawn(|| {
            futures::executor::block_on(async {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::METAL,
                    ..Default::default()
                });
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        ..Default::default()
                    })
                    .await
                    .is_some()
            })
        })
        .join()
        .unwrap_or(false)
    }

    fn ensure_pipeline(&mut self) -> Result<(), GpuError> {
        if self.pipeline.is_none() {
            self.pipeline = Some(GpuPipeline::new(&self.ctx, wgpu::TextureFormat::Rgba8UnormSrgb)?);
        }
        
        // Initialize default texture (1x1 white) if not done yet
        if self.default_texture.is_none() {
            let default_tex = GpuTexture::placeholder(&self.ctx);
            self.default_bind_group = Some(self.create_texture_bind_group(&default_tex));
            self.default_texture = Some(default_tex);
        }

        Ok(())
    }

    pub fn render(
        &mut self,
        config: &RenderConfig,
        scene: &SceneCpu,
        global_matrices: &[Mat4],
        skin_matrices: &[Vec<Mat4>],
        instance_morph_weights: &[Vec<f32>],
        camera: Camera,
        model_rotation_y: f32,
        width: u32,
        height: u32,
    ) -> Result<PixelFrame, GpuError> {
        self.ensure_pipeline()?;
        self.ensure_scene_cache(scene);
        self.cache_textures_for_scene(scene, config);
        
        let pipeline = self.pipeline.as_ref().unwrap();

        // Reuse or create render target based on size
        let needs_new_target = self.cached_render_target_size != Some((width, height));
        if needs_new_target {
            self.cached_render_target = Some(RenderTarget::new(&self.ctx, TextureSize::new(width, height))?);
            self.cached_render_target_size = Some((width, height));
        }
        let render_target = self.cached_render_target.as_ref().unwrap();

        let aspect = (width as f32 * config.cell_aspect).max(1.0) / height as f32;
        let projection = crate::math::perspective_matrix(config.fov_deg, aspect, config.near, config.far);
        let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
        let view_projection = projection * view;
        let model_rotation = Mat4::from_rotation_y(model_rotation_y);

        let mut had_draw = false;
        for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
            if matches!(instance.layer, MeshLayer::Stage)
                && matches!(config.stage_role, StageRole::Off)
            {
                continue;
            }
            let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
                continue;
            };

            let node_global = global_matrices
                .get(instance.node_index)
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            let model = model_rotation * node_global;
            let mvp = view_projection * model;
            let normal_matrix = Mat3::from_mat4(model).inverse().transpose();

            let material = self.get_material_params(scene, mesh.material_index, config);

            let morph_weights = instance_morph_weights.get(instance_index).map(|v| v.as_slice());
            let has_morph = morph_weights.is_some_and(|w| !w.is_empty()) && !mesh.morph_targets.is_empty();
            let has_skin = instance
                .skin_index
                .and_then(|skin_idx| skin_matrices.get(skin_idx))
                .is_some_and(|joints| !joints.is_empty());

            let gpu_mesh = if has_morph {
                let gpu_mesh = self
                    .morph_mesh_cache
                    .entry(instance.mesh_index)
                    .or_insert_with(|| GpuMesh::new_with_morph(&self.ctx, mesh, morph_weights));
                gpu_mesh.update_vertices(&self.ctx, mesh, morph_weights);
                gpu_mesh
            } else {
                self.mesh_cache
                    .entry(instance.mesh_index)
                    .or_insert_with(|| GpuMesh::new(&self.ctx, mesh))
            };

            let mut joint_matrix_data = vec![0.0f32; 512 * 16];
            if let Some(skin_idx) = instance.skin_index {
                if let Some(joints) = skin_matrices.get(skin_idx) {
                    for (i, mat) in joints.iter().take(512).enumerate() {
                        let offset = i * 16;
                        joint_matrix_data[offset..offset + 16].copy_from_slice(&mat.to_cols_array());
                    }
                }
            }
            if let Some(pipeline_ref) = self.pipeline.as_ref() {
                self.ctx.queue.write_buffer(
                    &pipeline_ref.joint_matrix_buffer,
                    0,
                    bytemuck::cast_slice(&joint_matrix_data),
                );
            }

            let uniforms = Uniforms {
                mvp_matrix: mvp.to_cols_array_2d(),
                model_matrix: model.to_cols_array_2d(),
                normal_matrix: [
                    [normal_matrix.x_axis.x, normal_matrix.x_axis.y, normal_matrix.x_axis.z, 0.0],
                    [normal_matrix.y_axis.x, normal_matrix.y_axis.y, normal_matrix.y_axis.z, 0.0],
                    [normal_matrix.z_axis.x, normal_matrix.z_axis.y, normal_matrix.z_axis.z, 0.0],
                ],
                camera_pos: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
                light_dir: [0.3, 0.7, 0.6, 0.0],
                lighting_params: [config.ambient.max(0.0), config.diffuse_strength.max(0.0), config.specular_strength.max(0.0), config.specular_power.max(1.0)],
                material_color: material.color,
                fog_params: [0.0, 100.0, config.fog_strength.max(0.0), 0.0],
                uv_transform: [
                    material.uv_offset[0],
                    material.uv_offset[1],
                    material.uv_scale[0],
                    material.uv_scale[1],
                ],
                uv_params: [material.uv_set as f32, material.uv_rotation, 0.0, 0.0],
                alpha_params: [
                    match material.alpha_mode {
                        MaterialAlphaMode::Opaque => 0.0,
                        MaterialAlphaMode::Mask => 1.0,
                        MaterialAlphaMode::Blend => 2.0,
                    },
                    material.alpha_cutoff,
                    0.0,
                    0.0,
                ],
                texture_params: [
                    config.texture_mip_bias,
                    Self::focus_lod_bias(config),
                    material
                        .texture_index
                    .and_then(|tex_idx| scene.textures.get(tex_idx))
                        .map(|texture| texture.mip_levels.len() as f32)
                        .unwrap_or(0.0),
                    0.0,
                ],
                exposure: exposure_bias_multiplier(config.exposure_bias),
                has_skin: if has_skin { 1 } else { 0 },
                _pad2: [0.0; 2],
            };

            let mut uniforms = uniforms;
            uniforms.uv_params[2] = if matches!(config.texture_v_origin, TextureVOrigin::Legacy) {
                1.0
            } else {
                0.0
            };

            pipeline.update_uniforms(&self.ctx.queue, &uniforms);

            let mut encoder = self.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });
            let mut render_pass = render_target.begin_render_pass(&mut encoder, !had_draw);
            render_pass.set_pipeline(&pipeline.render_pipeline);
            render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
            let texture_bind_group = material.texture_index
                .and_then(|tex_idx| {
                    let key = TextureBindingKey {
                        texture_index: tex_idx,
                        wrap_s: match material.wrap_s {
                            TextureWrapMode::Repeat => 0,
                            TextureWrapMode::MirroredRepeat => 1,
                            TextureWrapMode::ClampToEdge => 2,
                        },
                        wrap_t: match material.wrap_t {
                            TextureWrapMode::Repeat => 0,
                            TextureWrapMode::MirroredRepeat => 1,
                            TextureWrapMode::ClampToEdge => 2,
                        },
                        sampling_mode: match material.sampling_mode {
                            TextureSamplingMode::Nearest => 0,
                            TextureSamplingMode::Bilinear => 1,
                        },
                    };
                    self.texture_bind_groups.get(&key)
                })
                .or(self.default_bind_group.as_ref());
            if let Some(bg) = texture_bind_group {
                render_pass.set_bind_group(1, bg, &[]);
            }
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
            drop(render_pass);
            self.ctx.queue.submit(std::iter::once(encoder.finish()));
            had_draw = true;
        }

        if !had_draw {
            let mut clear_encoder = self.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_clear_encoder"),
            });
            let _ = render_target.begin_render_pass(&mut clear_encoder, true);
            self.ctx.queue.submit(std::iter::once(clear_encoder.finish()));
        }

        let rgba_data = render_target.readback(&self.ctx.device, &self.ctx.queue)?;

        let mut pixel_frame = PixelFrame::new(width, height);
        pixel_frame.rgba8.copy_from_slice(&rgba_data);

        Ok(pixel_frame)
    }
}

#[cfg(feature = "gpu")]
pub fn render_frame_gpu(
    renderer_state: &mut GpuRendererState,
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    glyph_ramp: &GlyphRamp,
    _scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> Result<RenderStats, GpuError> {
    let width = u32::from(frame.width).max(1);
    let height = u32::from(frame.height).max(1);

    let pixel_frame = {
        let renderer = renderer_state.renderer_mut()?;
        renderer.render(
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            camera,
            model_rotation_y,
            width,
            height,
        )?
    };

    convert_pixel_frame_to_ascii(&pixel_frame, frame, config, glyph_ramp);

    Ok(compute_gpu_render_stats(
        &pixel_frame,
        config,
        scene,
        global_matrices,
        skin_matrices,
        instance_morph_weights,
        camera,
        model_rotation_y,
    ))
}

#[cfg(not(feature = "gpu"))]
pub fn render_frame_gpu(
    _frame: &mut FrameBuffers,
    _config: &RenderConfig,
    _scene: &SceneCpu,
    _global_matrices: &[Mat4],
    _skin_matrices: &[Vec<Mat4>],
    _instance_morph_weights: &[Vec<f32>],
    _glyph_ramp: &GlyphRamp,
    _scratch: &mut RenderScratch,
    _camera: Camera,
    _model_rotation_y: f32,
) -> Result<RenderStats, GpuError> {
    Err(GpuError::NotImplemented)
}

#[cfg(feature = "gpu")]
fn convert_pixel_frame_to_ascii(
    pixel_frame: &PixelFrame,
    frame: &mut FrameBuffers,
    _config: &RenderConfig,
    _glyph_ramp: &GlyphRamp,
) {
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let px_width = pixel_frame.width_px as usize;
    let cell_width = (px_width / width).max(1);
    let cell_height = (pixel_frame.height_px as usize / height).max(1);

    for y in 0..height {
        for x in 0..width {
            let px_x = x * cell_width;
            let px_y = y * cell_height;
            let idx = y * width + x;
            let px_idx = (px_y.min(pixel_frame.height_px as usize - 1)) * px_width
                + px_x.min(pixel_frame.width_px as usize - 1);

            let r = pixel_frame.rgba8[px_idx * 4];
            let g = pixel_frame.rgba8[px_idx * 4 + 1];
            let b = pixel_frame.rgba8[px_idx * 4 + 2];
            let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;

            frame.glyphs[idx] = if luminance > 200.0 {
                '█'
            } else if luminance > 150.0 {
                '▓'
            } else if luminance > 100.0 {
                '▒'
            } else if luminance > 50.0 {
                '░'
            } else {
                ' '
            };
            frame.fg_rgb[idx] = [r, g, b];
        }
    }
    frame.has_color = true;
}

#[cfg(all(test, feature = "gpu"))]
mod tests {
    use super::*;
    use crate::engine::pipeline::FramePipeline;
    use crate::scene::{cube_scene, MeshInstance, MeshLayer, MorphTargetCpu, Node, RenderConfig, RenderMode, RenderBackend, SceneCpu};
    use glam::{Quat, Vec3};

    fn build_mixed_morph_scene() -> SceneCpu {
        let mut scene = cube_scene();
        scene.meshes[0].morph_targets = vec![MorphTargetCpu {
            name: Some("stretch".to_owned()),
            position_deltas: scene.meshes[0]
                .positions
                .iter()
                .map(|p| Vec3::new(0.0, if p.y > 0.0 { 0.45 } else { 0.0 }, 0.0))
                .collect(),
            normal_deltas: vec![Vec3::ZERO; scene.meshes[0].positions.len()],
        }];
        scene.nodes = vec![
            Node {
                name: Some("left".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::new(-0.55, 0.0, 0.0),
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
            Node {
                name: Some("right".to_owned()),
                parent: None,
                children: Vec::new(),
                base_translation: Vec3::new(0.55, 0.0, 0.0),
                base_rotation: Quat::IDENTITY,
                base_scale: Vec3::ONE,
            },
        ];
        scene.mesh_instances = vec![
            MeshInstance {
                mesh_index: 0,
                node_index: 0,
                skin_index: None,
                default_morph_weights: Vec::new(),
                layer: MeshLayer::Subject,
            },
            MeshInstance {
                mesh_index: 0,
                node_index: 1,
                skin_index: None,
                default_morph_weights: vec![1.0],
                layer: MeshLayer::Subject,
            },
        ];
        scene.root_center_node = Some(0);
        scene
    }

    #[test]
    fn mixed_morph_instances_populate_separate_gpu_caches() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping cache split capture");
            return;
        }

        let scene = build_mixed_morph_scene();
        let mut renderer = GpuRenderer::new().expect("gpu renderer");
        let mut config = RenderConfig::default();
        config.backend = RenderBackend::Gpu;
        config.mode = RenderMode::Ascii;

        let mut pipeline = FramePipeline::new(&scene);
        pipeline.prepare_frame(&scene, 0.0, None, None, 0.0);

        let _ = renderer
            .render(
                &config,
                &scene,
                pipeline.globals(),
                pipeline.skin_matrices(),
                pipeline.morph_weights_by_instance(),
                Camera::default(),
                0.0,
                88,
                50,
            )
            .expect("gpu render");

        assert_eq!(renderer.mesh_cache.len(), 1);
        assert_eq!(renderer.morph_mesh_cache.len(), 1);
        assert!(renderer.mesh_cache.contains_key(&0));
        assert!(renderer.morph_mesh_cache.contains_key(&0));
    }
}

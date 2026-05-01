use glam::{Mat3, Mat4};

use crate::renderer::{Camera, PixelFrame, exposure_bias_multiplier};
use crate::scene::{
    MaterialAlphaMode, MeshLayer, RenderConfig, SceneCpu, StageRole, TextureSamplingMode,
    TextureVOrigin, TextureWrapMode,
};

mod cache;
mod output;
mod skinning;
mod state;

use super::{
    device::GpuError,
    pipeline::Uniforms,
    resources::GpuMesh,
    texture::{GpuTexture as RenderTarget, TextureSize},
};

use cache::TextureBindingKey;
pub use output::render_frame_gpu;
use skinning::upload_joint_matrices;
pub use state::GpuRenderer;

impl GpuRenderer {
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
        self.cache_textures_for_scene(scene, config)?;

        let pipeline = self
            .pipeline
            .as_ref()
            .ok_or_else(|| GpuError::Render("pipeline not initialized".to_string()))?;

        // Reuse or create render target based on size
        let needs_new_target = self.cached_render_target_size != Some((width, height))
            || self.cached_render_target.is_none();
        if needs_new_target {
            self.cached_render_target = Some(RenderTarget::new(
                &self.ctx,
                TextureSize::new(width, height),
            )?);
            self.cached_render_target_size = Some((width, height));
        }
        let render_target = self
            .cached_render_target
            .as_ref()
            .ok_or_else(|| GpuError::Render("render target not available".to_string()))?;

        let aspect = (width as f32 * config.cell_aspect).max(1.0) / height as f32;
        let projection =
            crate::math::perspective_matrix(config.fov_deg, aspect, config.near, config.far);
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

            let morph_weights = instance_morph_weights
                .get(instance_index)
                .map(|v| v.as_slice());
            let has_morph =
                morph_weights.is_some_and(|w| !w.is_empty()) && !mesh.morph_targets.is_empty();
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

            let has_skin = if let Some(pipeline_ref) = self.pipeline.as_ref() {
                upload_joint_matrices(&self.ctx, pipeline_ref, skin_matrices, instance)
            } else {
                false
            };

            let uniforms = Uniforms {
                mvp_matrix: mvp.to_cols_array_2d(),
                model_matrix: model.to_cols_array_2d(),
                normal_matrix: [
                    [
                        normal_matrix.x_axis.x,
                        normal_matrix.x_axis.y,
                        normal_matrix.x_axis.z,
                        0.0,
                    ],
                    [
                        normal_matrix.y_axis.x,
                        normal_matrix.y_axis.y,
                        normal_matrix.y_axis.z,
                        0.0,
                    ],
                    [
                        normal_matrix.z_axis.x,
                        normal_matrix.z_axis.y,
                        normal_matrix.z_axis.z,
                        0.0,
                    ],
                ],
                camera_pos: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
                light_dir: [0.3, 0.7, 0.6, 0.0],
                lighting_params: [
                    config.ambient.max(0.0),
                    config.diffuse_strength.max(0.0),
                    config.specular_strength.max(0.0),
                    config.specular_power.max(1.0),
                ],
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

            let mut encoder =
                self.ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("render_encoder"),
                    });
            let mut render_pass = render_target.begin_render_pass(&mut encoder, !had_draw);
            render_pass.set_pipeline(&pipeline.render_pipeline);
            render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
            let texture_bind_group = material
                .texture_index
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
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
            drop(render_pass);
            self.ctx.queue.submit(std::iter::once(encoder.finish()));
            had_draw = true;
        }

        if !had_draw {
            let mut clear_encoder =
                self.ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("render_clear_encoder"),
                    });
            let _ = render_target.begin_render_pass(&mut clear_encoder, true);
            self.ctx
                .queue
                .submit(std::iter::once(clear_encoder.finish()));
        }

        let rgba_data = render_target.readback(&self.ctx.device, &self.ctx.queue)?;

        let mut pixel_frame = PixelFrame::new(width, height);
        pixel_frame.rgba8.copy_from_slice(&rgba_data);

        Ok(pixel_frame)
    }
}

#[cfg(all(test, feature = "gpu"))]
mod tests;

//! GPU rendering backend using wgpu (Metal on macOS).

mod device;
mod pipeline;
mod resources;
mod texture;

pub use device::{AdapterInfo, GpuContext, GpuError};
pub use pipeline::{GpuPipeline, Uniforms, Vertex};
pub use resources::{GpuMesh, GpuTexture};
pub use texture::{GpuTexture as RenderTarget, TextureSize};

use std::collections::HashMap;

use glam::{Mat3, Mat4, Vec3, Vec4};

use crate::renderer::{exposure_bias_multiplier, Camera, FrameBuffers, GlyphRamp, PixelFrame, RenderScratch, RenderStats};
use crate::scene::{
    MaterialAlphaMode, MeshLayer, RenderConfig, SceneCpu, StageRole, TextureCpu,
    TextureFilterMode, TextureSamplerMode, TextureSamplingMode, TextureVOrigin, TextureWrapMode,
};
use crate::render::backend_gpu::GpuRendererState;

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

#[derive(Clone, Copy)]
struct MaterialGpuParams {
    color: [f32; 4],
    texture_index: Option<usize>,
    uv_set: u32,
    uv_offset: [f32; 2],
    uv_scale: [f32; 2],
    uv_rotation: f32,
    alpha_mode: MaterialAlphaMode,
    alpha_cutoff: f32,
    wrap_s: TextureWrapMode,
    wrap_t: TextureWrapMode,
    sampling_mode: TextureSamplingMode,
}

impl Default for MaterialGpuParams {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            texture_index: None,
            uv_set: 0,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
            uv_rotation: 0.0,
            alpha_mode: MaterialAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            wrap_s: TextureWrapMode::Repeat,
            wrap_t: TextureWrapMode::Repeat,
            sampling_mode: TextureSamplingMode::Nearest,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TextureBindingKey {
    texture_index: usize,
    wrap_s: u8,
    wrap_t: u8,
    sampling_mode: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SceneSignature {
    meshes_ptr: usize,
    materials_ptr: usize,
    textures_ptr: usize,
    skins_ptr: usize,
    nodes_ptr: usize,
    instances_ptr: usize,
    animations_ptr: usize,
    meshes_len: usize,
    materials_len: usize,
    textures_len: usize,
    skins_len: usize,
    nodes_len: usize,
    instances_len: usize,
    animations_len: usize,
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

    fn scene_signature(scene: &SceneCpu) -> SceneSignature {
        SceneSignature {
            meshes_ptr: scene.meshes.as_ptr() as usize,
            materials_ptr: scene.materials.as_ptr() as usize,
            textures_ptr: scene.textures.as_ptr() as usize,
            skins_ptr: scene.skins.as_ptr() as usize,
            nodes_ptr: scene.nodes.as_ptr() as usize,
            instances_ptr: scene.mesh_instances.as_ptr() as usize,
            animations_ptr: scene.animations.as_ptr() as usize,
            meshes_len: scene.meshes.len(),
            materials_len: scene.materials.len(),
            textures_len: scene.textures.len(),
            skins_len: scene.skins.len(),
            nodes_len: scene.nodes.len(),
            instances_len: scene.mesh_instances.len(),
            animations_len: scene.animations.len(),
        }
    }

    fn ensure_scene_cache(&mut self, scene: &SceneCpu) {
        let sig = Self::scene_signature(scene);
        if self.cached_scene_sig != Some(sig) {
            self.mesh_cache.clear();
            self.morph_mesh_cache.clear();
            self.texture_cache.clear();
            self.texture_bind_groups.clear();
            self.cached_render_target = None;
            self.cached_render_target_size = None;
            self.cached_scene_sig = Some(sig);
        }
    }

    fn create_texture_bind_group(&self, texture: &GpuTexture) -> wgpu::BindGroup {
        self.create_texture_bind_group_with_sampler(texture, None)
    }

    fn create_texture_bind_group_with_sampler(
        &self,
        texture: &GpuTexture,
        sampler: Option<&wgpu::Sampler>,
    ) -> wgpu::BindGroup {
        let pipeline = self.pipeline.as_ref().unwrap();
        let sampler = sampler.unwrap_or(&texture.sampler);
        self.ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture_bind_group"),
            layout: &pipeline.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn wrap_mode_to_wgpu(mode: TextureWrapMode) -> wgpu::AddressMode {
        match mode {
            TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
            TextureWrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            TextureWrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        }
    }

    fn filter_mode_to_wgpu(mode: TextureSamplingMode) -> wgpu::FilterMode {
        match mode {
            TextureSamplingMode::Nearest => wgpu::FilterMode::Nearest,
            TextureSamplingMode::Bilinear => wgpu::FilterMode::Linear,
        }
    }

    fn resolve_sampling_mode(
        config: &RenderConfig,
        min_filter: TextureFilterMode,
        mag_filter: TextureFilterMode,
    ) -> TextureSamplingMode {
        let mode = match config.texture_sampler {
            TextureSamplerMode::Override => config.texture_sampling,
            TextureSamplerMode::Gltf => {
                if matches!(min_filter, TextureFilterMode::Nearest)
                    || matches!(mag_filter, TextureFilterMode::Nearest)
                {
                    TextureSamplingMode::Nearest
                } else {
                    TextureSamplingMode::Bilinear
                }
            }
        };

        if matches!(config.camera_focus, crate::scene::CameraFocusMode::Face | crate::scene::CameraFocusMode::Upper)
            && matches!(mode, TextureSamplingMode::Nearest)
        {
            TextureSamplingMode::Bilinear
        } else {
            mode
        }
    }

    fn focus_lod_bias(config: &RenderConfig) -> f32 {
        match config.camera_focus {
            crate::scene::CameraFocusMode::Face => -1.25,
            crate::scene::CameraFocusMode::Upper => -0.65,
            _ => 0.0,
        }
    }

    fn get_or_create_texture(&mut self, key: TextureBindingKey, texture: &TextureCpu) {
        if !self.texture_bind_groups.contains_key(&key) {
            if !self.texture_cache.contains_key(&key.texture_index) {
                let gpu_texture = GpuTexture::new(
                    &self.ctx,
                    texture.width,
                    texture.height,
                    &texture.rgba8,
                    &texture.mip_levels,
                    texture.color_space,
                );
                self.texture_cache.insert(key.texture_index, gpu_texture);
            }

            let texture_ref = self
                .texture_cache
                .get(&key.texture_index)
                .expect("texture inserted above");
            let sampler = self.ctx.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("material_sampler"),
                address_mode_u: Self::wrap_mode_to_wgpu(match key.wrap_s {
                    0 => TextureWrapMode::Repeat,
                    1 => TextureWrapMode::MirroredRepeat,
                    _ => TextureWrapMode::ClampToEdge,
                }),
                address_mode_v: Self::wrap_mode_to_wgpu(match key.wrap_t {
                    0 => TextureWrapMode::Repeat,
                    1 => TextureWrapMode::MirroredRepeat,
                    _ => TextureWrapMode::ClampToEdge,
                }),
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: Self::filter_mode_to_wgpu(match key.sampling_mode {
                    0 => TextureSamplingMode::Nearest,
                    _ => TextureSamplingMode::Bilinear,
                }),
                min_filter: Self::filter_mode_to_wgpu(match key.sampling_mode {
                    0 => TextureSamplingMode::Nearest,
                    _ => TextureSamplingMode::Bilinear,
                }),
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            let bind_group = self.create_texture_bind_group_with_sampler(texture_ref, Some(&sampler));
            self.texture_bind_groups.insert(key, bind_group);
        }
    }

    fn cache_textures_for_scene(&mut self, scene: &SceneCpu, config: &RenderConfig) {
        if !config.material_color {
            return;
        }
        for instance in &scene.mesh_instances {
            if let Some(mesh) = scene.meshes.get(instance.mesh_index) {
                if let Some(mat_idx) = mesh.material_index {
                    if let Some(material) = scene.materials.get(mat_idx) {
                        if let Some(tex_idx) = material.base_color_texture {
                            if let Some(texture) = scene.textures.get(tex_idx) {
                                let sampling_mode = Self::resolve_sampling_mode(
                                    config,
                                    material.base_color_min_filter,
                                    material.base_color_mag_filter,
                                );
                                let key = TextureBindingKey {
                                    texture_index: tex_idx,
                                    wrap_s: match material.base_color_wrap_s {
                                        TextureWrapMode::Repeat => 0,
                                        TextureWrapMode::MirroredRepeat => 1,
                                        TextureWrapMode::ClampToEdge => 2,
                                    },
                                    wrap_t: match material.base_color_wrap_t {
                                        TextureWrapMode::Repeat => 0,
                                        TextureWrapMode::MirroredRepeat => 1,
                                        TextureWrapMode::ClampToEdge => 2,
                                    },
                                    sampling_mode: match sampling_mode {
                                        TextureSamplingMode::Nearest => 0,
                                        TextureSamplingMode::Bilinear => 1,
                                    },
                                };
                                self.get_or_create_texture(key, texture);
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_material_params(
        &self,
        scene: &SceneCpu,
        material_index: Option<usize>,
        config: &RenderConfig,
    ) -> MaterialGpuParams {
        if !config.material_color {
            return MaterialGpuParams::default();
        }

        let mut out = MaterialGpuParams::default();
        let Some(mat_idx) = material_index else {
            return out;
        };
        let Some(material) = scene.materials.get(mat_idx) else {
            return out;
        };

        out.color = material.base_color_factor;
        out.texture_index = material.base_color_texture;
        out.uv_set = material
            .base_color_uv_transform
            .and_then(|transform| transform.tex_coord_override)
            .unwrap_or(material.base_color_tex_coord)
            .min(1);
        if let Some(transform) = material.base_color_uv_transform {
            out.uv_offset = transform.offset;
            out.uv_scale = transform.scale;
            out.uv_rotation = transform.rotation_rad;
        }
        out.alpha_mode = material.alpha_mode;
        out.alpha_cutoff = material.alpha_cutoff.clamp(0.0, 1.0);
        out.wrap_s = material.base_color_wrap_s;
        out.wrap_t = material.base_color_wrap_t;
        out.sampling_mode = Self::resolve_sampling_mode(
            config,
            material.base_color_min_filter,
            material.base_color_mag_filter,
        );
        out
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

#[cfg(feature = "gpu")]
fn compute_gpu_render_stats(
    pixel_frame: &PixelFrame,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    let mut stats = RenderStats::default();
    let width = pixel_frame.width_px as usize;
    let height = pixel_frame.height_px as usize;
    if width == 0 || height == 0 {
        return stats;
    }

    let model_rotation = Mat4::from_rotation_y(model_rotation_y);
    if let Some((x, y, depth)) = project_root_screen_gpu(
        scene,
        global_matrices,
        model_rotation,
        config,
        camera,
        pixel_frame.width_px,
        pixel_frame.height_px,
    ) {
        stats.root_screen_px = Some((x, y));
        stats.root_depth = Some(depth);
    }

    if let Some(subject) = project_subject_metrics_gpu(
        scene,
        global_matrices,
        skin_matrices,
        instance_morph_weights,
        model_rotation,
        config,
        camera,
        pixel_frame.width_px,
        pixel_frame.height_px,
    ) {
        stats.subject_visible_ratio = subject.visible_ratio;
        stats.subject_visible_height_ratio = subject.height_ratio;
        stats.subject_centroid_px = Some(subject.centroid);
        stats.subject_bbox_px = Some(subject.bbox);
    }

    let mut visible = 0usize;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let alpha = pixel_frame.rgba8.get(idx + 3).copied().unwrap_or(0);
            if alpha == 0 {
                continue;
            }
            visible = visible.saturating_add(1);
            sum_x += x as f32 + 0.5;
            sum_y += y as f32 + 0.5;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    let total = width.saturating_mul(height).max(1);
    stats.visible_cell_ratio = (visible as f32) / (total as f32);
    stats.visible_centroid_px = stats.root_screen_px;
    stats.visible_bbox_px = None;
    stats.visible_bbox_aspect = 0.0;
    stats.visible_height_ratio = 0.0;
    if visible > 0 {
        if stats.visible_centroid_px.is_none() {
            stats.visible_centroid_px = Some((sum_x / visible as f32, sum_y / visible as f32));
        }
        let bbox_w = (max_x.saturating_sub(min_x) + 1) as f32;
        let bbox_h = (max_y.saturating_sub(min_y) + 1) as f32;
        stats.visible_bbox_px = Some((
            min_x as u16,
            min_y as u16,
            max_x.min(width.saturating_sub(1)) as u16,
            max_y.min(height.saturating_sub(1)) as u16,
        ));
        stats.visible_bbox_aspect = if bbox_h > f32::EPSILON {
            bbox_w / bbox_h
        } else {
            0.0
        };
        stats.visible_height_ratio = (bbox_h / height as f32).clamp(0.0, 1.0);
    }

    stats.triangles_total = count_gpu_triangles(scene, config);
    stats.pixels_drawn = visible;
    stats
}

#[cfg(feature = "gpu")]
fn project_root_screen_gpu(
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    model_rotation: Mat4,
    config: &RenderConfig,
    camera: Camera,
    width: u32,
    height: u32,
) -> Option<(f32, f32, f32)> {
    let node_index = scene.root_center_node?;
    let global = global_matrices
        .get(node_index)
        .copied()
        .unwrap_or(Mat4::IDENTITY);
    let world = (model_rotation * global).transform_point3(glam::Vec3::ZERO);
    let aspect = ((width as f32) * config.cell_aspect).max(1.0) / (height as f32).max(1.0);
    let projection = crate::math::perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let clip = projection * view * world.extend(1.0);
    if clip.w <= 1e-5 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    let x = (ndc.x * 0.5 + 0.5) * ((width as f32) - 1.0);
    let y = (1.0 - (ndc.y * 0.5 + 0.5)) * ((height as f32) - 1.0);
    let depth = (ndc.z + 1.0) * 0.5;
    Some((x, y, depth))
}

#[cfg(feature = "gpu")]
fn count_gpu_triangles(scene: &SceneCpu, config: &RenderConfig) -> usize {
    let mut total = 0usize;
    for instance in &scene.mesh_instances {
        if matches!(instance.layer, MeshLayer::Stage)
            && matches!(config.stage_role, StageRole::Off)
        {
            continue;
        }
        let Some(mesh) = scene.meshes.get(instance.mesh_index) else {
            continue;
        };
        let alpha_mode = mesh
            .material_index
            .and_then(|material_index| scene.materials.get(material_index))
            .map(|material| material.alpha_mode)
            .unwrap_or(MaterialAlphaMode::Opaque);
        if matches!(instance.layer, MeshLayer::Stage) && matches!(alpha_mode, MaterialAlphaMode::Blend) {
            continue;
        }
        total = total.saturating_add(mesh.indices.len());
    }
    total
}

#[cfg(feature = "gpu")]
struct SubjectMetrics {
    centroid: (f32, f32),
    bbox: (u16, u16, u16, u16),
    visible_ratio: f32,
    height_ratio: f32,
}

#[cfg(feature = "gpu")]
fn project_subject_metrics_gpu(
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    model_rotation: Mat4,
    config: &RenderConfig,
    camera: Camera,
    width: u32,
    height: u32,
) -> Option<SubjectMetrics> {
    let aspect = ((width as f32) * config.cell_aspect).max(1.0) / (height as f32).max(1.0);
    let projection = crate::math::perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let view_projection = projection * view;

    let mut visible = 0usize;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut min_x = width as usize;
    let mut min_y = height as usize;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for (instance_index, instance) in scene.mesh_instances.iter().enumerate() {
        if !matches!(instance.layer, MeshLayer::Subject) {
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
        let morph_weights = instance_morph_weights.get(instance_index).map(|v| v.as_slice());
        let skin = instance.skin_index.and_then(|idx| skin_matrices.get(idx));

        for (vertex_index, position) in mesh.positions.iter().enumerate() {
            let mut pos = *position;
            if let Some(weights) = morph_weights {
                pos = apply_morph_position(mesh, vertex_index, pos, weights);
            }
            pos = apply_skin_position(mesh, vertex_index, pos, skin);

            let world = model.transform_point3(pos);
            let clip = view_projection * world.extend(1.0);
            if clip.w <= 1e-5 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.z < -1.0 || ndc.z > 1.0 {
                continue;
            }

            let x = (ndc.x * 0.5 + 0.5) * ((width as f32) - 1.0);
            let y = (1.0 - (ndc.y * 0.5 + 0.5)) * ((height as f32) - 1.0);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }

            visible = visible.saturating_add(1);
            sum_x += x;
            sum_y += y;
            let px = x.clamp(0.0, (width.saturating_sub(1)) as f32).floor() as usize;
            let py = y.clamp(0.0, (height.saturating_sub(1)) as f32).floor() as usize;
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
        }
    }

    if visible == 0 {
        return None;
    }

    let bbox_w = (max_x.saturating_sub(min_x) + 1) as f32;
    let bbox_h = (max_y.saturating_sub(min_y) + 1) as f32;
    let frame_area = (width as f32).max(1.0) * (height as f32).max(1.0);
    Some(SubjectMetrics {
        centroid: (sum_x / visible as f32, sum_y / visible as f32),
        bbox: (
            min_x as u16,
            min_y as u16,
            max_x.min(width.saturating_sub(1) as usize) as u16,
            max_y.min(height.saturating_sub(1) as usize) as u16,
        ),
        visible_ratio: (bbox_w * bbox_h / frame_area).clamp(0.0, 1.0),
        height_ratio: (bbox_h / (height as f32)).clamp(0.0, 1.0),
    })
}

#[cfg(feature = "gpu")]
fn apply_morph_position(
    mesh: &crate::scene::MeshCpu,
    vertex_index: usize,
    base_position: Vec3,
    weights: &[f32],
) -> Vec3 {
    if mesh.morph_targets.is_empty() || weights.is_empty() {
        return base_position;
    }
    let mut out = base_position;
    for (target_index, target) in mesh.morph_targets.iter().enumerate() {
        let weight = weights.get(target_index).copied().unwrap_or(0.0);
        if weight.abs() <= 1e-5 {
            continue;
        }
        if let Some(delta) = target.position_deltas.get(vertex_index) {
            out += *delta * weight;
        }
    }
    out
}

#[cfg(feature = "gpu")]
fn apply_skin_position(
    mesh: &crate::scene::MeshCpu,
    vertex_index: usize,
    position: Vec3,
    skin_matrices: Option<&Vec<Mat4>>,
) -> Vec3 {
    let Some(joints) = mesh.joints4.as_ref() else {
        return position;
    };
    let Some(weights) = mesh.weights4.as_ref() else {
        return position;
    };
    let Some(skin_matrices) = skin_matrices else {
        return position;
    };

    let Some(joints) = joints.get(vertex_index) else {
        return position;
    };
    let Some(weights) = weights.get(vertex_index) else {
        return position;
    };

    let mut skinned = Vec4::ZERO;
    let mut accumulated = 0.0;
    for i in 0..4 {
        let weight = weights[i];
        if weight <= 0.0 {
            continue;
        }
        let Some(joint_matrix) = skin_matrices.get(joints[i] as usize) else {
            continue;
        };
        skinned += (*joint_matrix * position.extend(1.0)) * weight;
        accumulated += weight;
    }
    if accumulated <= f32::EPSILON {
        return position;
    }
    if skinned.w.abs() > 1e-6 {
        skinned.truncate() / skinned.w
    } else {
        skinned.truncate()
    }
}

#[derive(Debug)]
pub enum GpuBackendError {
    Gpu(GpuError),
    NotImplemented,
    Unsupported,
}

#[cfg(feature = "gpu")]
impl From<GpuError> for GpuBackendError {
    fn from(e: GpuError) -> Self {
        Self::Gpu(e)
    }
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
        pipeline.prepare_frame(&scene, 0.0, None);

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

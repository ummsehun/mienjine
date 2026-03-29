use crate::scene::{
    CameraFocusMode, MaterialAlphaMode, RenderConfig, SceneCpu, TextureCpu, TextureFilterMode,
    TextureSamplerMode, TextureSamplingMode, TextureWrapMode,
};

use super::{GpuRenderer, GpuTexture};

#[derive(Clone, Copy)]
pub(super) struct MaterialGpuParams {
    pub(super) color: [f32; 4],
    pub(super) texture_index: Option<usize>,
    pub(super) uv_set: u32,
    pub(super) uv_offset: [f32; 2],
    pub(super) uv_scale: [f32; 2],
    pub(super) uv_rotation: f32,
    pub(super) alpha_mode: MaterialAlphaMode,
    pub(super) alpha_cutoff: f32,
    pub(super) wrap_s: TextureWrapMode,
    pub(super) wrap_t: TextureWrapMode,
    pub(super) sampling_mode: TextureSamplingMode,
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
pub(super) struct TextureBindingKey {
    pub(super) texture_index: usize,
    pub(super) wrap_s: u8,
    pub(super) wrap_t: u8,
    pub(super) sampling_mode: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneSignature {
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
    pub(super) fn scene_signature(scene: &SceneCpu) -> SceneSignature {
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

    pub(super) fn ensure_scene_cache(&mut self, scene: &SceneCpu) {
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

    pub(super) fn create_texture_bind_group(&self, texture: &GpuTexture) -> wgpu::BindGroup {
        self.create_texture_bind_group_with_sampler(texture, None)
    }

    pub(super) fn create_texture_bind_group_with_sampler(
        &self,
        texture: &GpuTexture,
        sampler: Option<&wgpu::Sampler>,
    ) -> wgpu::BindGroup {
        let pipeline = self.pipeline.as_ref().unwrap();
        let sampler = sampler.unwrap_or(&texture.sampler);
        self.ctx
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
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

        if matches!(
            config.camera_focus,
            CameraFocusMode::Face | CameraFocusMode::Upper
        ) && matches!(mode, TextureSamplingMode::Nearest)
        {
            TextureSamplingMode::Bilinear
        } else {
            mode
        }
    }

    fn focus_lod_bias(config: &RenderConfig) -> f32 {
        match config.camera_focus {
            CameraFocusMode::Face => -1.25,
            CameraFocusMode::Upper => -0.65,
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
            let bind_group =
                self.create_texture_bind_group_with_sampler(texture_ref, Some(&sampler));
            self.texture_bind_groups.insert(key, bind_group);
        }
    }

    pub(super) fn cache_textures_for_scene(&mut self, scene: &SceneCpu, config: &RenderConfig) {
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

    pub(super) fn get_material_params(
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
}

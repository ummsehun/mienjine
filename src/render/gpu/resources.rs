use super::device::GpuContext;
use crate::scene::{MeshCpu, MorphTargetCpu};
use glam::{Vec2, Vec3};

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl GpuMesh {
    pub fn new(ctx: &GpuContext, mesh: &MeshCpu) -> Self {
        Self::new_with_morph(ctx, mesh, None)
    }

    pub fn new_with_morph(ctx: &GpuContext, mesh: &MeshCpu, morph_weights: Option<&[f32]>) -> Self {
        let vertices = build_vertices(mesh, morph_weights);

        let vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertex_buffer"),
            size: (vertices.len() * std::mem::size_of::<super::pipeline::Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue
            .write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let indices: Vec<u32> = mesh
            .indices
            .iter()
            .flat_map(|[a, b, c]| [*a, *b, *c])
            .collect();

        let index_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("index_buffer"),
            size: (indices.len() * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue
            .write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));

        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    pub fn update_vertices(
        &mut self,
        ctx: &GpuContext,
        mesh: &MeshCpu,
        morph_weights: Option<&[f32]>,
    ) {
        let vertices = build_vertices(mesh, morph_weights);

        ctx.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }
}

fn build_vertices(mesh: &MeshCpu, morph_weights: Option<&[f32]>) -> Vec<super::pipeline::Vertex> {
    mesh.positions
        .iter()
        .enumerate()
        .map(|(i, pos)| {
            let normal = mesh
                .normals
                .get(i)
                .copied()
                .unwrap_or(Vec3::new(0.0, 1.0, 0.0));
            let uv = mesh
                .uv0
                .as_ref()
                .and_then(|values| values.get(i).copied())
                .unwrap_or(Vec2::ZERO);
            let uv1 = mesh
                .uv1
                .as_ref()
                .and_then(|values| values.get(i).copied())
                .unwrap_or(uv);
            let joint_indices = mesh
                .joints4
                .as_ref()
                .and_then(|values| values.get(i).copied())
                .map(|indices| {
                    [
                        indices[0] as u32,
                        indices[1] as u32,
                        indices[2] as u32,
                        indices[3] as u32,
                    ]
                })
                .unwrap_or([0, 0, 0, 0]);
            let joint_weights = mesh
                .weights4
                .as_ref()
                .and_then(|values| values.get(i).copied())
                .unwrap_or([0.0, 0.0, 0.0, 0.0]);
            let (morphed_pos, morphed_normal) = apply_morph_targets(
                mesh.morph_targets.as_slice(),
                i,
                *pos,
                normal,
                morph_weights,
            );
            super::pipeline::Vertex {
                position: [morphed_pos.x, morphed_pos.y, morphed_pos.z],
                normal: [morphed_normal.x, morphed_normal.y, morphed_normal.z],
                uv0: [uv.x, uv.y],
                uv1: [uv1.x, uv1.y],
                joint_indices,
                joint_weights,
            }
        })
        .collect()
}

fn apply_morph_targets(
    morph_targets: &[MorphTargetCpu],
    vertex_index: usize,
    base_position: Vec3,
    base_normal: Vec3,
    weights: Option<&[f32]>,
) -> (Vec3, Vec3) {
    let Some(weights) = weights else {
        return (base_position, base_normal);
    };
    if morph_targets.is_empty() || weights.is_empty() {
        return (base_position, base_normal);
    }

    let mut out_position = base_position;
    let mut out_normal = base_normal;
    for (target_index, target) in morph_targets.iter().enumerate() {
        let weight = weights.get(target_index).copied().unwrap_or(0.0);
        if weight.abs() <= 1e-5 {
            continue;
        }
        if let Some(delta) = target.position_deltas.get(vertex_index) {
            out_position += *delta * weight;
        }
        if let Some(delta) = target.normal_deltas.get(vertex_index) {
            out_normal += *delta * weight;
        }
    }
    (out_position, out_normal.normalize_or_zero())
}

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl GpuTexture {
    pub fn new(
        ctx: &GpuContext,
        width: u32,
        height: u32,
        rgba_data: &[u8],
        mip_levels: &[crate::scene::TextureLevelCpu],
        color_space: crate::scene::TextureColorSpace,
    ) -> Self {
        let format = match color_space {
            crate::scene::TextureColorSpace::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            crate::scene::TextureColorSpace::Linear => wgpu::TextureFormat::Rgba8Unorm,
        };
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(" diffuse_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: (mip_levels.len() as u32).saturating_add(1),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        ctx.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        for (level_idx, level) in mip_levels.iter().enumerate() {
            if level.width == 0 || level.height == 0 {
                continue;
            }
            ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: (level_idx as u32).saturating_add(1),
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level.rgba8,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * 4),
                    rows_per_image: Some(level.height),
                },
                wgpu::Extent3d {
                    width: level.width,
                    height: level.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("diffuse_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    pub fn placeholder(ctx: &GpuContext) -> Self {
        let white_1x1 = [255u8; 4];
        Self::new(
            ctx,
            1,
            1,
            &white_1x1,
            &[],
            crate::scene::TextureColorSpace::Srgb,
        )
    }
}

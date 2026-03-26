use super::device::{GpuContext, GpuError};

#[derive(Debug, Clone, Copy)]
pub struct TextureSize {
    pub width: u32,
    pub height: u32,
}

impl TextureSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

pub struct GpuTexture {
    pub color_texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub staging_buffer: wgpu::Buffer,
    pub size: TextureSize,
}

impl GpuTexture {
    fn padded_bytes_per_row(width: u32) -> u32 {
        let unpadded = width.saturating_mul(4);
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        unpadded.div_ceil(align).saturating_mul(align)
    }

    pub fn new(ctx: &GpuContext, size: TextureSize) -> Result<Self, GpuError> {
        if size.width == 0 || size.height == 0 {
            return Err(GpuError::Render("Invalid texture size".into()));
        }

        let color_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color_target"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_target"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let padded_bytes_per_row = Self::padded_bytes_per_row(size.width);
        let staging_buffer_size =
            u64::from(padded_bytes_per_row).saturating_mul(u64::from(size.height));

        let staging_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: staging_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            color_texture,
            color_view,
            depth_texture,
            depth_view,
            staging_buffer,
            size,
        })
    }

    pub fn begin_render_pass<'a>(
        &self,
        encoder: &'a mut wgpu::CommandEncoder,
        clear: bool,
    ) -> wgpu::RenderPass<'a> {
        let color_load = if clear {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            })
        } else {
            wgpu::LoadOp::Load
        };

        let depth_load = if clear {
            wgpu::LoadOp::Clear(1.0)
        } else {
            wgpu::LoadOp::Load
        };

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &self.color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: color_load,
                store: wgpu::StoreOp::Store,
            },
        };

        let depth_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: &self.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: depth_load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        };

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gpu_render_pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: Some(depth_attachment),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    pub fn readback(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Vec<u8>, GpuError> {
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = self.size.width.saturating_mul(bytes_per_pixel);
        let padded_bytes_per_row = Self::padded_bytes_per_row(self.size.width);
        let buffer_size =
            u64::from(padded_bytes_per_row).saturating_mul(u64::from(self.size.height));

        let command_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback_encoder"),
        });

        let texture_extent = wgpu::Extent3d {
            width: self.size.width,
            height: self.size.height,
            depth_or_array_layers: 1,
        };

        let mut encoder = command_encoder;
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.size.height),
                },
            },
            texture_extent,
        );

        queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = self.staging_buffer.slice(0..buffer_size);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });
        device.poll(wgpu::Maintain::Wait);

        rx.recv()
            .map_err(|_| GpuError::Render("Channel error".into()))?
            .map_err(|e| GpuError::Render(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let row_count = self.size.height as usize;
        let unpadded = unpadded_bytes_per_row as usize;
        let padded = padded_bytes_per_row as usize;

        let mut result = vec![0u8; unpadded.saturating_mul(row_count)];
        for row in 0..row_count {
            let src_offset = row.saturating_mul(padded);
            let dst_offset = row.saturating_mul(unpadded);
            let src_end = src_offset.saturating_add(unpadded);
            let dst_end = dst_offset.saturating_add(unpadded);
            result[dst_offset..dst_end].copy_from_slice(&data[src_offset..src_end]);
        }
        drop(data);
        self.staging_buffer.unmap();

        Ok(result)
    }
}

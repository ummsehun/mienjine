use crate::scene::{
    KittyCompression, KittyTransport, RenderBackend, RenderConfig, RenderOutputMode,
};

pub(crate) fn resolve_runtime_backend(requested: RenderBackend) -> RenderBackend {
    match requested {
        RenderBackend::Cpu => RenderBackend::Cpu,
        RenderBackend::Gpu => {
            #[cfg(feature = "gpu")]
            {
                use crate::render::gpu::GpuRenderer;
                if GpuRenderer::is_available() {
                    RenderBackend::Gpu
                } else {
                    eprintln!(
                        "warning: gpu backend requested but no suitable gpu found; falling back to cpu."
                    );
                    RenderBackend::Cpu
                }
            }
            #[cfg(not(feature = "gpu"))]
            {
                eprintln!(
                    "warning: gpu backend requested but gpu feature not enabled; falling back to cpu."
                );
                RenderBackend::Cpu
            }
        }
    }
}

pub(crate) fn normalize_graphics_settings(config: &mut RenderConfig) -> Option<String> {
    if !matches!(
        config.output_mode,
        RenderOutputMode::Hybrid | RenderOutputMode::KittyHq
    ) {
        return None;
    }
    if matches!(config.kitty_transport, KittyTransport::Shm)
        && matches!(config.kitty_compression, KittyCompression::Zlib)
    {
        config.kitty_compression = KittyCompression::None;
        return Some("kitty transport=shm forces compression=none".to_owned());
    }
    None
}

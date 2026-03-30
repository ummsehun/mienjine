use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};

use crate::{
    renderer::{pixel_frame_from_cells, FrameBuffers},
    scene::{
        GraphicsProtocol, KittyCompression, KittyPipelineMode, KittyTransport, RecoverStrategy,
    },
};

const CELL_PIXELS_W_BASE: u32 = 2;
const CELL_PIXELS_H_BASE: u32 = 4;
const BACKGROUND_RGB: [u8; 3] = [26, 32, 44];
const KITTY_CHUNK_LEN: usize = 4096;
const SHM_PREFIX: &str = "gascii-kitty-shm-";
const SHM_STALE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Copy)]
pub struct GraphicsPresentOptions {
    pub transport: KittyTransport,
    pub compression: KittyCompression,
    pub pipeline_mode: KittyPipelineMode,
    pub recover_strategy: RecoverStrategy,
    pub scale: f32,
    pub display_cells: Option<(u16, u16)>,
    pub force_reupload: bool,
}

impl Default for GraphicsPresentOptions {
    fn default() -> Self {
        Self {
            transport: KittyTransport::Shm,
            compression: KittyCompression::None,
            pipeline_mode: KittyPipelineMode::RealPixel,
            recover_strategy: RecoverStrategy::Hard,
            scale: 1.0,
            display_cells: None,
            force_reupload: false,
        }
    }
}

#[derive(Debug)]
struct EncodedPngFrame {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct KittyGraphicsState {
    image_id: u32,
    placement_id: u32,
    uploaded: bool,
    last_cells: Option<(u16, u16)>,
}

impl KittyGraphicsState {
    fn new() -> Self {
        let pid = std::process::id();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u32)
            .unwrap_or(0);
        let base = pid ^ nonce ^ 0x5A51_83C1;
        Self {
            image_id: base.saturating_add(1),
            placement_id: base.saturating_add(2),
            uploaded: false,
            last_cells: None,
        }
    }
}

#[derive(Debug)]
struct ShmFrameBuffer {
    path: PathBuf,
    capacity: usize,
    used_len: usize,
}

impl ShmFrameBuffer {
    fn create(capacity: usize) -> io::Result<Self> {
        let path = next_shm_path();
        let capacity = capacity.max(1024);
        fs::write(&path, vec![0_u8; capacity])?;
        Ok(Self {
            path,
            capacity,
            used_len: 0,
        })
    }

    fn ensure_capacity(&mut self, required: usize) -> io::Result<()> {
        if required <= self.capacity {
            return Ok(());
        }
        self.capacity = required.max(self.capacity.saturating_mul(2)).max(1024);
        fs::write(&self.path, vec![0_u8; self.capacity])?;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.ensure_capacity(bytes.len())?;
        fs::write(&self.path, bytes)?;
        self.used_len = bytes.len();
        Ok(())
    }
}

impl Drop for ShmFrameBuffer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

static SHM_REGISTRY: OnceLock<Mutex<Option<ShmFrameBuffer>>> = OnceLock::new();
static KITTY_STATE: OnceLock<Mutex<KittyGraphicsState>> = OnceLock::new();

pub fn cleanup_shm_registry() {
    if let Some(lock) = SHM_REGISTRY.get() {
        if let Ok(mut guard) = lock.lock() {
            let _ = guard.take();
        }
    }
}

pub fn cleanup_orphan_shm_files() -> usize {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return 0;
    };
    let now = SystemTime::now();
    let mut cleaned = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if !file_name.starts_with(SHM_PREFIX) {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_none_or(|age| age > SHM_STALE_TTL);
        if !stale {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            cleaned = cleaned.saturating_add(1);
        }
    }
    cleaned
}

pub fn detect_supported_protocol(requested: GraphicsProtocol) -> Option<GraphicsProtocol> {
    match requested {
        GraphicsProtocol::None => None,
        GraphicsProtocol::Kitty => supports_kitty().then_some(GraphicsProtocol::Kitty),
        GraphicsProtocol::Iterm2 => supports_iterm2().then_some(GraphicsProtocol::Iterm2),
        GraphicsProtocol::Auto => {
            if supports_kitty() {
                Some(GraphicsProtocol::Kitty)
            } else if supports_iterm2() {
                Some(GraphicsProtocol::Iterm2)
            } else {
                None
            }
        }
    }
}

pub fn write_graphics_frame(
    writer: &mut impl Write,
    frame: &FrameBuffers,
    protocol: GraphicsProtocol,
    options: GraphicsPresentOptions,
) -> io::Result<()> {
    let scale = options.scale.clamp(0.5, 2.0);
    let cell_px_w = ((CELL_PIXELS_W_BASE as f32) * scale).round().max(1.0) as u32;
    let cell_px_h = ((CELL_PIXELS_H_BASE as f32) * scale).round().max(1.0) as u32;
    let encoded = encode_png_frame(frame, cell_px_w, cell_px_h, options.pipeline_mode)?;
    let (cells_w, cells_h) = options
        .display_cells
        .unwrap_or((frame.width.max(1), frame.height.max(1)));
    match protocol {
        GraphicsProtocol::Kitty => write_kitty_frame(
            writer,
            &encoded.bytes,
            cells_w,
            cells_h,
            encoded.width,
            encoded.height,
            options.transport,
            options.compression,
            options.recover_strategy,
            options.force_reupload,
        ),
        GraphicsProtocol::Iterm2 => write_iterm2_frame(writer, &encoded.bytes),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "graphics protocol is not available",
        )),
    }
}

fn supports_kitty() -> bool {
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    term.contains("kitty") || term_program.contains("ghostty")
}

fn supports_iterm2() -> bool {
    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    term_program.contains("iterm")
}

fn encode_png_frame(
    frame: &FrameBuffers,
    cell_px_w: u32,
    cell_px_h: u32,
    pipeline_mode: KittyPipelineMode,
) -> io::Result<EncodedPngFrame> {
    let pixel = pixel_frame_from_cells(frame, cell_px_w, cell_px_h, pipeline_mode, BACKGROUND_RGB);

    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);
    encoder
        .write_image(
            &pixel.rgba8,
            pixel.width_px,
            pixel.height_px,
            ColorType::Rgba8.into(),
        )
        .map_err(io::Error::other)?;
    Ok(EncodedPngFrame {
        bytes,
        width: pixel.width_px,
        height: pixel.height_px,
    })
}

fn write_kitty_frame(
    writer: &mut impl Write,
    png: &[u8],
    cells_w: u16,
    cells_h: u16,
    source_px_w: u32,
    source_px_h: u32,
    transport: KittyTransport,
    compression: KittyCompression,
    recover_strategy: RecoverStrategy,
    force_reupload: bool,
) -> io::Result<()> {
    let lock = KITTY_STATE.get_or_init(|| Mutex::new(KittyGraphicsState::new()));
    let mut state = lock
        .lock()
        .map_err(|_| io::Error::other("failed to lock kitty state"))?;
    let size_changed = state.last_cells != Some((cells_w, cells_h));
    let should_reupload = force_reupload || size_changed || !state.uploaded;
    if should_reupload && state.uploaded && matches!(recover_strategy, RecoverStrategy::Hard) {
        write_kitty_delete(writer, state.image_id, state.placement_id)?;
        state.uploaded = false;
    }

    let effective_compression = if matches!(transport, KittyTransport::Shm) {
        KittyCompression::None
    } else {
        compression
    };

    let result = match transport {
        KittyTransport::Shm => write_kitty_frame_mmap_file(
            writer,
            png,
            cells_w,
            cells_h,
            source_px_w,
            source_px_h,
            state.image_id,
            state.placement_id,
        ),
        KittyTransport::Direct => write_kitty_frame_direct(
            writer,
            png,
            cells_w,
            cells_h,
            source_px_w,
            source_px_h,
            effective_compression,
            state.image_id,
            state.placement_id,
        ),
    };

    if result.is_ok() {
        state.uploaded = true;
        state.last_cells = Some((cells_w, cells_h));
    }
    result
}

fn write_kitty_frame_mmap_file(
    writer: &mut impl Write,
    png: &[u8],
    cells_w: u16,
    cells_h: u16,
    source_px_w: u32,
    source_px_h: u32,
    image_id: u32,
    placement_id: u32,
) -> io::Result<()> {
    let lock = SHM_REGISTRY.get_or_init(|| Mutex::new(None));
    let mut guard = lock
        .lock()
        .map_err(|_| io::Error::other("failed to lock SHM registry"))?;
    if guard.is_none() {
        *guard = Some(ShmFrameBuffer::create(png.len().max(1024))?);
    }
    let frame = guard
        .as_mut()
        .ok_or_else(|| io::Error::other("failed to create SHM buffer"))?;
    frame.write_bytes(png)?;

    let payload = STANDARD.encode(frame.path.to_string_lossy().as_bytes());
    let px_w = source_px_w.max(1);
    let px_h = source_px_h.max(1);
    write!(writer, "\x1b[H")?;
    write!(
        writer,
        "\x1b_Ga=T,i={image_id},p={placement_id},f=100,t=f,c={cells_w},r={cells_h},s={px_w},v={px_h},S={};{}\x1b\\",
        frame.used_len, payload
    )?;
    writer.flush()
}

fn write_kitty_frame_direct(
    writer: &mut impl Write,
    png: &[u8],
    cells_w: u16,
    cells_h: u16,
    source_px_w: u32,
    source_px_h: u32,
    compression: KittyCompression,
    image_id: u32,
    placement_id: u32,
) -> io::Result<()> {
    // Current runtime keeps direct path uncompressed for lower CPU cost; zlib is reserved for
    // future optimization passes once transport overhead is measured.
    let _ = compression;
    let data = STANDARD.encode(png);
    let px_w = source_px_w.max(1);
    let px_h = source_px_h.max(1);

    write!(writer, "\x1b[H")?;

    if data.len() <= KITTY_CHUNK_LEN {
        write!(
            writer,
            "\x1b_Ga=T,i={image_id},p={placement_id},f=100,t=d,c={cells_w},r={cells_h},s={px_w},v={px_h};{data}\x1b\\"
        )?;
        writer.flush()?;
        return Ok(());
    }

    let mut offset = 0usize;
    let mut first = true;
    while offset < data.len() {
        let end = (offset + KITTY_CHUNK_LEN).min(data.len());
        let chunk = &data[offset..end];
        let more = if end < data.len() { 1 } else { 0 };
        if first {
            write!(
                writer,
                "\x1b_Ga=T,i={image_id},p={placement_id},f=100,t=d,c={cells_w},r={cells_h},s={px_w},v={px_h},m={more};{chunk}\x1b\\"
            )?;
            first = false;
        } else {
            write!(writer, "\x1b_Gm={more};{chunk}\x1b\\")?;
        }
        offset = end;
    }
    writer.flush()
}

fn write_kitty_delete(writer: &mut impl Write, image_id: u32, placement_id: u32) -> io::Result<()> {
    write!(writer, "\x1b_Ga=d,d=P,p={placement_id}\x1b\\")?;
    write!(writer, "\x1b_Ga=d,d=I,i={image_id}\x1b\\")?;
    writer.flush()
}

fn write_iterm2_frame(writer: &mut impl Write, png: &[u8]) -> io::Result<()> {
    let data = STANDARD.encode(png);
    write!(writer, "\x1b[H")?;
    write!(
        writer,
        "\x1b]1337;File=inline=1;width=100%;height=100%;preserveAspectRatio=0:{data}\x07"
    )?;
    writer.flush()
}

fn next_shm_path() -> PathBuf {
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{SHM_PREFIX}{pid}-{nonce}.bin"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::FrameBuffers;

    #[test]
    fn encode_png_has_data() {
        let mut frame = FrameBuffers::new(2, 1);
        frame.glyphs[0] = '@';
        frame.glyphs[1] = '.';
        frame.fg_rgb[0] = [255, 64, 64];
        frame.fg_rgb[1] = [64, 255, 255];
        let png = encode_png_frame(&frame, 2, 4, KittyPipelineMode::RealPixel).expect("png");
        assert_eq!(png.width, 4);
        assert_eq!(png.height, 4);
        assert!(png.bytes.len() > 32);
    }
}

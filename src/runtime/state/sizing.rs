pub(crate) const MAX_RENDER_COLS: u16 = 4096;
pub(crate) const MAX_RENDER_ROWS: u16 = 2048;

pub(crate) fn cap_render_size(width: u16, height: u16) -> (u16, u16, bool) {
    if width == 0 || height == 0 {
        return (1, 1, false);
    }
    if width <= MAX_RENDER_COLS && height <= MAX_RENDER_ROWS {
        return (width, height, false);
    }
    let scale_w = (MAX_RENDER_COLS as f32) / (width as f32);
    let scale_h = (MAX_RENDER_ROWS as f32) / (height as f32);
    let scale = scale_w.min(scale_h).clamp(0.01, 1.0);
    let capped_w = ((width as f32) * scale).floor() as u16;
    let capped_h = ((height as f32) * scale).floor() as u16;
    (capped_w.max(1), capped_h.max(1), true)
}

pub(crate) fn is_terminal_size_unstable(width: u16, height: u16) -> bool {
    if width == 0 || height == 0 {
        return true;
    }
    if width == u16::MAX || height == u16::MAX {
        return true;
    }
    let w = width as u32;
    let h = height as u32;
    let max_w = (MAX_RENDER_COLS as u32) * 8;
    let max_h = (MAX_RENDER_ROWS as u32) * 8;
    w > max_w || h > max_h
}

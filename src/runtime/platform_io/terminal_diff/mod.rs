use crate::{renderer::FrameBuffers, scene::AnsiQuantization};

#[derive(Debug)]
pub(crate) struct DiffSegment {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) start_idx: usize,
    pub(crate) end_idx_exclusive: usize,
    pub(crate) payload: String,
}

pub(crate) fn build_diff_segments(
    frame: &FrameBuffers,
    previous_glyphs: &[char],
    previous_rgb: &[[u8; 3]],
    use_ansi: bool,
    quantization: AnsiQuantization,
) -> Vec<DiffSegment> {
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    for y in 0..height {
        let row_start = y * width;
        let row_end = row_start + width;
        let mut x = 0usize;
        while row_start + x < row_end {
            let idx = row_start + x;
            if !cell_changed(
                idx,
                frame,
                previous_glyphs,
                previous_rgb,
                use_ansi,
                quantization,
            ) {
                x += 1;
                continue;
            }

            let run_start_x = x;
            let run_start_idx = idx;
            let mut payload = String::new();
            let mut current_rgb: Option<[u8; 3]> = None;
            while row_start + x < row_end {
                let ridx = row_start + x;
                if !cell_changed(
                    ridx,
                    frame,
                    previous_glyphs,
                    previous_rgb,
                    use_ansi,
                    quantization,
                ) {
                    break;
                }
                if use_ansi {
                    let rgb = quantize_rgb(frame.fg_rgb[ridx], quantization);
                    if current_rgb != Some(rgb) {
                        push_fg_ansi(&mut payload, rgb);
                        current_rgb = Some(rgb);
                    }
                }
                payload.push(frame.glyphs[ridx]);
                x += 1;
            }
            if use_ansi {
                payload.push_str("\x1b[0m");
            }
            out.push(DiffSegment {
                x: run_start_x as u16,
                y: y as u16,
                start_idx: run_start_idx,
                end_idx_exclusive: row_start + x,
                payload,
            });
        }
    }
    out
}

fn cell_changed(
    idx: usize,
    frame: &FrameBuffers,
    previous_glyphs: &[char],
    previous_rgb: &[[u8; 3]],
    use_ansi: bool,
    quantization: AnsiQuantization,
) -> bool {
    if frame.glyphs.get(idx).copied().unwrap_or(' ')
        != previous_glyphs.get(idx).copied().unwrap_or(' ')
    {
        return true;
    }
    if use_ansi {
        let curr = quantize_rgb(
            frame.fg_rgb.get(idx).copied().unwrap_or([255, 255, 255]),
            quantization,
        );
        let prev = previous_rgb.get(idx).copied().unwrap_or([255, 255, 255]);
        return curr != prev;
    }
    false
}

fn push_fg_ansi(out: &mut String, rgb: [u8; 3]) {
    use std::fmt::Write as _;
    let _ = write!(out, "\x1b[38;2;{};{};{}m", rgb[0], rgb[1], rgb[2]);
}

pub(crate) fn quantize_rgb(rgb: [u8; 3], quantization: AnsiQuantization) -> [u8; 3] {
    if matches!(quantization, AnsiQuantization::Off) {
        return rgb;
    }
    fn q(c: u8) -> u8 {
        let bucket = ((c as u16 * 5 + 127) / 255) as u8;
        bucket * 51
    }
    [q(rgb[0]), q(rgb[1]), q(rgb[2])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_segments_include_only_changed_runs() {
        let mut frame = FrameBuffers::new(5, 1);
        frame.glyphs.clone_from_slice(&['a', 'b', 'c', 'd', 'e']);
        frame.fg_rgb.fill([255, 255, 255]);
        let prev_glyphs = vec!['a', 'x', 'c', 'd', 'e'];
        let prev_rgb = vec![[255, 255, 255]; 5];

        let segments = build_diff_segments(
            &frame,
            &prev_glyphs,
            &prev_rgb,
            false,
            AnsiQuantization::Q216,
        );
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].x, 1);
        assert_eq!(segments[0].payload, "b");
    }

    #[test]
    fn ansi_diff_quantizes_colors_before_compare() {
        let mut frame = FrameBuffers::new(2, 1);
        frame.glyphs.clone_from_slice(&['@', '#']);
        frame.fg_rgb[0] = [250, 10, 10];
        frame.fg_rgb[1] = [240, 15, 20];

        let prev_glyphs = vec!['@', '#'];
        let prev_rgb = vec![
            quantize_rgb([255, 0, 0], AnsiQuantization::Q216),
            quantize_rgb([255, 0, 0], AnsiQuantization::Q216),
        ];

        let segments = build_diff_segments(
            &frame,
            &prev_glyphs,
            &prev_rgb,
            true,
            AnsiQuantization::Q216,
        );
        assert!(segments.is_empty());
    }
}

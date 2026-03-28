use crate::render::renderer::{FrameBuffers, RenderStats};

pub(super) fn visible_cell_ratio(frame: &FrameBuffers) -> f32 {
    let total = frame.depth.len();
    if total == 0 {
        return 0.0;
    }
    let visible = frame.depth.iter().filter(|depth| depth.is_finite()).count();
    (visible as f32) / (total as f32)
}

pub(super) fn apply_visible_metrics(
    stats: &mut RenderStats,
    frame: &FrameBuffers,
    subject_depth_cells: &[f32],
    frame_width: u16,
    frame_height: u16,
) {
    stats.visible_cell_ratio = visible_cell_ratio(frame);
    stats.visible_centroid_px = stats.root_screen_px;
    stats.visible_bbox_px = None;
    stats.visible_bbox_aspect = 0.0;
    stats.visible_height_ratio = 0.0;
    stats.subject_visible_ratio = 0.0;
    stats.subject_visible_height_ratio = 0.0;
    stats.subject_centroid_px = None;
    stats.subject_bbox_px = None;
    if frame.width == 0 || frame.height == 0 {
        return;
    }

    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let mut visible = 0usize;
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if !frame.depth[idx].is_finite() {
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
    if visible == 0 {
        return;
    }

    let silhouette_centroid = (sum_x / visible as f32, sum_y / visible as f32);
    if stats.visible_centroid_px.is_none() {
        stats.visible_centroid_px = Some(silhouette_centroid);
    }
    stats.visible_bbox_px = Some((
        min_x as u16,
        min_y as u16,
        max_x.min(width.saturating_sub(1)) as u16,
        max_y.min(height.saturating_sub(1)) as u16,
    ));
    let bbox_w = (max_x.saturating_sub(min_x) + 1) as f32;
    let bbox_h = (max_y.saturating_sub(min_y) + 1) as f32;
    stats.visible_bbox_aspect = if bbox_h > f32::EPSILON {
        bbox_w / bbox_h
    } else {
        0.0
    };
    stats.visible_height_ratio = (bbox_h / (frame.height as f32)).clamp(0.0, 1.0);

    let fw = usize::from(frame_width.max(1));
    let fh = usize::from(frame_height.max(1));
    if subject_depth_cells.len() < fw.saturating_mul(fh) {
        return;
    }
    let mut subject_visible = 0usize;
    let mut subject_sum_x = 0.0f32;
    let mut subject_sum_y = 0.0f32;
    let mut smin_x = fw;
    let mut smin_y = fh;
    let mut smax_x = 0usize;
    let mut smax_y = 0usize;
    for y in 0..fh {
        for x in 0..fw {
            let idx = y * fw + x;
            if !subject_depth_cells[idx].is_finite() {
                continue;
            }
            subject_visible = subject_visible.saturating_add(1);
            subject_sum_x += x as f32 + 0.5;
            subject_sum_y += y as f32 + 0.5;
            smin_x = smin_x.min(x);
            smin_y = smin_y.min(y);
            smax_x = smax_x.max(x);
            smax_y = smax_y.max(y);
        }
    }
    if subject_visible == 0 {
        return;
    }
    stats.subject_visible_ratio = (subject_visible as f32) / (fw.saturating_mul(fh).max(1) as f32);
    let sbbox_h = (smax_y.saturating_sub(smin_y) + 1) as f32;
    stats.subject_visible_height_ratio = (sbbox_h / (fh as f32)).clamp(0.0, 1.0);
    stats.subject_centroid_px = Some((
        subject_sum_x / subject_visible as f32,
        subject_sum_y / subject_visible as f32,
    ));
    stats.subject_bbox_px = Some((
        smin_x as u16,
        smin_y as u16,
        smax_x.min(fw.saturating_sub(1)) as u16,
        smax_y.min(fh.saturating_sub(1)) as u16,
    ));
    if stats.visible_centroid_px.is_none() {
        stats.visible_centroid_px = stats.subject_centroid_px;
    }
}

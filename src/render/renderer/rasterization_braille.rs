use crate::math::depth_less;
use crate::scene::{ColorMode, MeshCpu, RenderConfig, SceneCpu};

use super::braille::BrailleThresholds;
use super::rasterization::perp_dot;
use super::{FrameBuffers, ProjectedVertex, RasterPass, RenderStats, ThemePalette};
use crate::render::renderer_color::{
    boost_saturation, clarity_saturation_gain, color_scale_from_tonemap, luminance,
    model_color_for_intensity, scale_rgb, srgb_to_linear, to_display_rgb,
};
use crate::render::renderer_exposure::{push_histogram, tone_map_intensity};
use crate::render::renderer_material::{resolve_material_props, sample_material};

pub(super) fn rasterize_braille_mesh(
    mesh: &MeshCpu,
    projected_vertices: &[Option<ProjectedVertex>],
    subpixels: &mut super::BrailleSubpixelBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    material_morph_weights: &[f32],
    stats: &mut RenderStats,
    shading: crate::render::renderer::shading::ShadingParams,
    contrast: crate::render::renderer::shading::ContrastParams,
    palette: ThemePalette,
    exposure: f32,
    threshold: BrailleThresholds,
    histogram: &mut [u32; 64],
    histogram_count: &mut u32,
    triangle_order: &mut Vec<usize>,
    triangle_depth_sorted: &mut Vec<(usize, f32)>,
    subject_depth_cells: &mut [f32],
    is_subject_layer: bool,
    cell_width: u16,
    pass: RasterPass,
) {
    let width = i32::from(subpixels.width);
    let height = i32::from(subpixels.height);
    let width_usize = usize::from(subpixels.width);
    let base_stride = config.triangle_stride.max(1);
    let triangle_stride = if is_subject_layer {
        if matches!(pass, RasterPass::Blend) {
            (base_stride / 2).max(1)
        } else {
            base_stride.min(2)
        }
    } else {
        base_stride.saturating_mul(6).clamp(2, 32)
    };
    let min_triangle_area_px2 = if is_subject_layer {
        (config.min_triangle_area_px2 * 0.35).max(0.0)
    } else {
        (config.min_triangle_area_px2.max(0.2) * 2.2).max(0.2)
    };
    if width <= 0 || height <= 0 {
        return;
    }
    let material_props = resolve_material_props(scene, mesh.material_index);
    if !pass.matches(material_props.alpha_mode) {
        return;
    }
    if !is_subject_layer && matches!(pass, RasterPass::Blend) {
        return;
    }
    stats.triangles_total += mesh.indices.len();
    let write_depth = !matches!(pass, RasterPass::Blend);
    let layer_luma_scale = if is_subject_layer {
        1.0
    } else {
        ((1.0 - config.bg_suppression.clamp(0.0, 1.0) * 0.88).clamp(0.12, 1.0))
            .min(config.stage_luma_cap.clamp(0.0, 1.0))
    };

    triangle_order.clear();
    if matches!(pass, RasterPass::Blend) {
        triangle_depth_sorted.clear();
        for (triangle_index, tri) in mesh.indices.iter().enumerate() {
            if triangle_stride > 1 && (triangle_index % triangle_stride) != 0 {
                continue;
            }
            let (Some(v0), Some(v1), Some(v2)) = (
                projected_vertices.get(tri[0] as usize).copied().flatten(),
                projected_vertices.get(tri[1] as usize).copied().flatten(),
                projected_vertices.get(tri[2] as usize).copied().flatten(),
            ) else {
                continue;
            };
            let avg_depth = (v0.depth + v1.depth + v2.depth) * (1.0 / 3.0);
            triangle_depth_sorted.push((triangle_index, avg_depth));
        }
        triangle_depth_sorted
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        triangle_order.extend(triangle_depth_sorted.iter().map(|(idx, _)| *idx));
    } else {
        triangle_order.extend((0..mesh.indices.len()).filter(|triangle_index| {
            triangle_stride <= 1 || (triangle_index % triangle_stride) == 0
        }));
    }

    for triangle_index in triangle_order.iter() {
        let tri = &mesh.indices[*triangle_index];
        let (Some(v0), Some(v1), Some(v2)) = (
            projected_vertices.get(tri[0] as usize).copied().flatten(),
            projected_vertices.get(tri[1] as usize).copied().flatten(),
            projected_vertices.get(tri[2] as usize).copied().flatten(),
        ) else {
            continue;
        };

        let signed_area = perp_dot(v1.screen - v0.screen, v2.screen - v0.screen);
        if !material_props.double_sided && signed_area >= 0.0 {
            stats.triangles_culled += 1;
            continue;
        }
        if signed_area.abs() < 1e-8 || signed_area.abs() < min_triangle_area_px2 {
            stats.triangles_culled += 1;
            continue;
        }
        let inv_area = -1.0 / signed_area;
        let min_x = v0
            .screen
            .x
            .min(v1.screen.x.min(v2.screen.x))
            .floor()
            .max(0.0) as i32;
        let max_x = v0
            .screen
            .x
            .max(v1.screen.x.max(v2.screen.x))
            .ceil()
            .min((width - 1) as f32) as i32;
        let min_y = v0
            .screen
            .y
            .min(v1.screen.y.min(v2.screen.y))
            .floor()
            .max(0.0) as i32;
        let max_y = v0
            .screen
            .y
            .max(v1.screen.y.max(v2.screen.y))
            .ceil()
            .min((height - 1) as f32) as i32;

        if min_x > max_x || min_y > max_y {
            continue;
        }
        let edge0_a = v2.screen.y - v1.screen.y;
        let edge0_b = v1.screen.x - v2.screen.x;
        let edge0_c = v2.screen.x * v1.screen.y - v1.screen.x * v2.screen.y;
        let edge1_a = v0.screen.y - v2.screen.y;
        let edge1_b = v2.screen.x - v0.screen.x;
        let edge1_c = v0.screen.x * v2.screen.y - v2.screen.x * v0.screen.y;
        let edge2_a = v1.screen.y - v0.screen.y;
        let edge2_b = v0.screen.x - v1.screen.x;
        let edge2_c = v1.screen.x * v0.screen.y - v0.screen.x * v1.screen.y;
        let start_x = min_x as f32 + 0.5;
        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let mut edge0 = edge0_a * start_x + edge0_b * py + edge0_c;
            let mut edge1 = edge1_a * start_x + edge1_b * py + edge1_c;
            let mut edge2 = edge2_a * start_x + edge2_b * py + edge2_c;
            for x in min_x..=max_x {
                let w0 = edge0 * inv_area;
                let w1 = edge1 * inv_area;
                let w2 = edge2 * inv_area;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    edge0 += edge0_a;
                    edge1 += edge1_a;
                    edge2 += edge2_a;
                    continue;
                }
                let mut depth = v0.depth * w0 + v1.depth * w1 + v2.depth * w2;
                if !(0.0..=1.0).contains(&depth) {
                    edge0 += edge0_a;
                    edge1 += edge1_a;
                    edge2 += edge2_a;
                    continue;
                }
                if !is_subject_layer {
                    depth = (depth + 6.0e-4).min(1.0);
                }
                let idx = (y as usize) * width_usize + (x as usize);
                let depth_pass = if write_depth {
                    depth_less(subpixels.depth[idx], depth)
                } else {
                    depth <= subpixels.depth[idx]
                };
                if depth_pass {
                    if write_depth {
                        subpixels.depth[idx] = depth;
                    }
                    if is_subject_layer {
                        let cell_x = (x as usize) / 2;
                        let cell_y = (y as usize) / 4;
                        let fw = usize::from(cell_width.max(1));
                        let cidx = cell_y.saturating_mul(fw).saturating_add(cell_x);
                        if cidx < subject_depth_cells.len()
                            && write_depth
                            && depth_less(subject_depth_cells[cidx], depth)
                        {
                            subject_depth_cells[cidx] = depth;
                        }
                    }
                    let world_pos = v0.world_pos * w0 + v1.world_pos * w1 + v2.world_pos * w2;
                    let world_normal =
                        (v0.world_normal * w0 + v1.world_normal * w1 + v2.world_normal * w2)
                            .normalize_or_zero();
                    let uv0 = v0.uv0 * w0 + v1.uv0 * w1 + v2.uv0 * w2;
                    let uv1 = v0.uv1 * w0 + v1.uv1 * w1 + v2.uv1 * w2;
                    let vertex_color = [
                        v0.vertex_color[0] * w0 + v1.vertex_color[0] * w1 + v2.vertex_color[0] * w2,
                        v0.vertex_color[1] * w0 + v1.vertex_color[1] * w1 + v2.vertex_color[1] * w2,
                        v0.vertex_color[2] * w0 + v1.vertex_color[2] * w1 + v2.vertex_color[2] * w2,
                        v0.vertex_color[3] * w0 + v1.vertex_color[3] * w1 + v2.vertex_color[3] * w2,
                    ];
                    let material_index = v0
                        .material_index
                        .or(v1.material_index)
                        .or(v2.material_index);
                    let sample = sample_material(
                        scene,
                        material_index,
                        uv0,
                        uv1,
                        depth,
                        vertex_color,
                        config,
                        material_morph_weights,
                    );
                    if matches!(pass, RasterPass::Mask) && sample.alpha < sample.alpha_cutoff {
                        edge0 += edge0_a;
                        edge1 += edge1_a;
                        edge2 += edge2_a;
                        continue;
                    }
                    if matches!(pass, RasterPass::Blend) && sample.alpha <= 0.01 {
                        edge0 += edge0_a;
                        edge1 += edge1_a;
                        edge2 += edge2_a;
                        continue;
                    }
                    let lighting = crate::render::renderer::shading::shade_lighting(
                        world_normal,
                        world_pos,
                        shading,
                    )
                    .clamp(0.0, 1.0);
                    let view_dir = (shading.camera_pos - world_pos).normalize_or_zero();
                    let edge_factor = (1.0_f32 - world_normal.dot(view_dir).abs()).powf(1.6_f32);
                    let fog = depth.powf(1.7) * shading.fog_strength * contrast.fog_scale;
                    let base_light = ((lighting
                        + edge_factor * config.edge_accent_strength * 0.22)
                        * (1.0 - fog.clamp(0.0, 1.0)))
                    .clamp(0.0, 1.0)
                        * layer_luma_scale;
                    let mut shaded_rgb = [
                        sample.albedo_linear[0] * base_light + sample.emissive_linear[0],
                        sample.albedo_linear[1] * base_light + sample.emissive_linear[1],
                        sample.albedo_linear[2] * base_light + sample.emissive_linear[2],
                    ];
                    if !is_subject_layer {
                        shaded_rgb = scale_rgb(shaded_rgb, layer_luma_scale);
                    }
                    if config.material_color && is_subject_layer {
                        let sat_gain = clarity_saturation_gain(config.clarity_profile)
                            + edge_factor * config.edge_accent_strength * 0.18;
                        shaded_rgb = boost_saturation(shaded_rgb, sat_gain);
                    }
                    let base = luminance(shaded_rgb);
                    if is_subject_layer || !config.subject_exposure_only {
                        push_histogram(histogram, histogram_count, base);
                    }
                    let floor = if is_subject_layer {
                        threshold.floor.max(config.model_lift.clamp(0.02, 0.45))
                    } else {
                        (threshold.floor * 0.45).clamp(0.01, 0.18)
                    };
                    let mut intensity = tone_map_intensity(base, floor, threshold.gamma, exposure);
                    if matches!(pass, RasterPass::Blend) {
                        let alpha = sample.alpha.clamp(0.0, 1.0);
                        intensity = intensity * alpha + subpixels.intensity[idx] * (1.0 - alpha);
                    }
                    subpixels.intensity[idx] = intensity;
                    let mut out_rgb = if config.material_color {
                        let color_scale = if is_subject_layer {
                            color_scale_from_tonemap(base, intensity)
                        } else {
                            color_scale_from_tonemap(base, intensity).min(1.35)
                        };
                        to_display_rgb(scale_rgb(shaded_rgb, color_scale))
                    } else {
                        model_color_for_intensity(intensity, palette)
                    };
                    if matches!(pass, RasterPass::Blend) {
                        let alpha = sample.alpha.clamp(0.0, 1.0);
                        let dst = [
                            srgb_to_linear(subpixels.color_rgb[idx][0] as f32 / 255.0),
                            srgb_to_linear(subpixels.color_rgb[idx][1] as f32 / 255.0),
                            srgb_to_linear(subpixels.color_rgb[idx][2] as f32 / 255.0),
                        ];
                        let src = [
                            srgb_to_linear(out_rgb[0] as f32 / 255.0),
                            srgb_to_linear(out_rgb[1] as f32 / 255.0),
                            srgb_to_linear(out_rgb[2] as f32 / 255.0),
                        ];
                        let mixed = [
                            src[0] * alpha + dst[0] * (1.0 - alpha),
                            src[1] * alpha + dst[1] * (1.0 - alpha),
                            src[2] * alpha + dst[2] * (1.0 - alpha),
                        ];
                        out_rgb = to_display_rgb(mixed);
                    }
                    subpixels.color_rgb[idx] = out_rgb;
                    stats.pixels_drawn += 1;
                }
                edge0 += edge0_a;
                edge1 += edge1_a;
                edge2 += edge2_a;
            }
        }
    }
}

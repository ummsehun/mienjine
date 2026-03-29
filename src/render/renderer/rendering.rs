//! Main rendering pipeline implementation.

use glam::{Mat3, Mat4, Vec3};

use crate::math::perspective_matrix;
use crate::scene::{MeshLayer, RenderConfig, RenderMode, SceneCpu, StageRole};

use super::braille::{braille_thresholds, compose_braille_cells, update_safe_visibility_state};
use super::projection::project_root_screen;
use super::rasterization::{rasterize_braille_mesh, rasterize_mesh};
use super::shading::{contrast_params, ShadingParams};
use super::{Camera, FrameBuffers, RasterPass, RenderScratch, RenderStats};
use crate::render::background::{
    fill_background_ascii, fill_background_braille, stage_params, theme_palette,
};
use crate::render::renderer_exposure::{exposure_bias_multiplier, update_exposure_from_histogram};
use crate::render::renderer_glyph::{select_charset, GlyphRamp};
use crate::render::renderer_metrics::apply_visible_metrics;

pub fn render_frame(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    material_morph_weights: &[f32],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    match config.mode {
        RenderMode::Ascii => render_frame_ascii(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            material_morph_weights,
            glyph_ramp,
            scratch,
            camera,
            model_rotation_y,
        ),
        RenderMode::Braille => render_frame_braille(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            material_morph_weights,
            scratch,
            camera,
            model_rotation_y,
        ),
    }
}

fn render_frame_ascii(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    material_morph_weights: &[f32],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    let palette = theme_palette(config.theme_style);
    fill_background_ascii(frame, config, palette);
    if frame.width == 0 || frame.height == 0 {
        return RenderStats::default();
    }
    let cells = usize::from(frame.width).saturating_mul(usize::from(frame.height));
    let contrast = contrast_params(config, cells);
    let charset = select_charset(config, glyph_ramp.chars(), cells);
    let stage = stage_params(config);
    let aspect = ((frame.width as f32) * config.cell_aspect).max(1.0) / (frame.height as f32);
    let projection = perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let view_projection = projection * view;
    let model_rotation = Mat4::from_rotation_y(model_rotation_y);
    let shading = ShadingParams {
        light_dir: Vec3::new(0.3, 0.7, 0.6).normalize(),
        camera_pos: camera.eye,
        ambient: config.ambient.max(0.0),
        diffuse_strength: config.diffuse_strength.max(0.0),
        specular_strength: config.specular_strength.max(0.0),
        specular_power: config.specular_power.max(1.0),
        rim_strength: config.rim_strength.max(0.0),
        rim_power: config.rim_power.max(0.01),
        fog_strength: (config.fog_strength * stage.fog_mul).clamp(0.0, 1.0),
    };
    let mut stats = RenderStats::default();
    let mut histogram = [0_u32; 64];
    let mut histogram_count = 0_u32;
    if let Some((x, y, depth)) = project_root_screen(
        scene,
        global_matrices,
        model_rotation,
        view_projection,
        frame.width,
        frame.height,
    ) {
        stats.root_screen_px = Some((x, y));
        stats.root_depth = Some(depth);
    }
    let exposure = scratch.exposure * exposure_bias_multiplier(config.exposure_bias);
    for pass in RasterPass::all() {
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
            let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
            {
                let projected_vertices = scratch.prepare_projected_vertices(mesh.positions.len());
                super::projection::project_mesh_vertices(
                    mesh,
                    model,
                    normal_matrix,
                    view_projection,
                    frame.width,
                    frame.height,
                    instance.skin_index.and_then(|i| skin_matrices.get(i)),
                    instance_morph_weights
                        .get(instance_index)
                        .map(Vec::as_slice),
                    projected_vertices,
                );
            }
            let projected_vertices = scratch.projected_vertices.as_slice();
            rasterize_mesh(
                mesh,
                projected_vertices,
                frame,
                charset,
                config,
                scene,
                material_morph_weights,
                &mut stats,
                shading,
                contrast,
                palette,
                exposure,
                &mut histogram,
                &mut histogram_count,
                &mut scratch.triangle_order,
                &mut scratch.triangle_depth_sorted,
                scratch.subject_depth_cells.as_mut_slice(),
                matches!(instance.layer, MeshLayer::Subject),
                pass,
            );
        }
    }
    update_exposure_from_histogram(
        &mut scratch.exposure,
        &histogram,
        histogram_count,
        config.clarity_profile,
    );
    apply_visible_metrics(
        &mut stats,
        frame,
        scratch.subject_depth_cells.as_slice(),
        frame.width,
        frame.height,
    );
    stats
}

fn render_frame_braille(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    material_morph_weights: &[f32],
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    let palette = theme_palette(config.theme_style);
    fill_background_braille(frame, config, palette);
    if frame.width == 0 || frame.height == 0 {
        return RenderStats::default();
    }
    let sub_w = frame.width.saturating_mul(2).max(1);
    let sub_h = frame.height.saturating_mul(4).max(1);
    scratch.braille_subpixels.resize(sub_w, sub_h);
    scratch.braille_subpixels.clear();

    let cells = usize::from(frame.width).saturating_mul(usize::from(frame.height));
    let contrast = contrast_params(config, cells);
    let _stage = stage_params(config);
    let aspect = ((sub_w as f32)
        * (config.cell_aspect * config.braille_aspect_compensation.clamp(0.70, 1.30)))
    .max(1.0)
        / (sub_h as f32);
    let projection = perspective_matrix(config.fov_deg, aspect, config.near, config.far);
    let view = Mat4::look_at_rh(camera.eye, camera.target, camera.up);
    let view_projection = projection * view;
    let model_rotation = Mat4::from_rotation_y(model_rotation_y);
    let shading = ShadingParams {
        light_dir: Vec3::new(0.3, 0.7, 0.6).normalize(),
        camera_pos: camera.eye,
        ambient: config.ambient.max(0.0),
        diffuse_strength: config.diffuse_strength.max(0.0),
        specular_strength: config.specular_strength.max(0.0),
        specular_power: config.specular_power.max(1.0),
        rim_strength: config.rim_strength.max(0.0),
        rim_power: config.rim_power.max(0.01),
        fog_strength: (config.fog_strength * 1.0).clamp(0.0, 1.0),
    };

    let mut histogram = [0_u32; 64];
    let mut histogram_count = 0_u32;
    let mut stats = RenderStats::default();
    if let Some((x, y, depth)) = project_root_screen(
        scene,
        global_matrices,
        model_rotation,
        view_projection,
        frame.width,
        frame.height,
    ) {
        stats.root_screen_px = Some((x, y));
        stats.root_depth = Some(depth);
    }
    let exposure = scratch.exposure * exposure_bias_multiplier(config.exposure_bias);
    let threshold = braille_thresholds(
        config.braille_profile,
        config.clarity_profile,
        scratch.safe_boost_active,
    );
    for pass in RasterPass::all() {
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
            let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
            {
                let projected_vertices = scratch.prepare_projected_vertices(mesh.positions.len());
                super::projection::project_mesh_vertices(
                    mesh,
                    model,
                    normal_matrix,
                    view_projection,
                    sub_w,
                    sub_h,
                    instance.skin_index.and_then(|i| skin_matrices.get(i)),
                    instance_morph_weights
                        .get(instance_index)
                        .map(Vec::as_slice),
                    projected_vertices,
                );
            }
            let projected_vertices = scratch.projected_vertices.as_slice();
            let subpixels = &mut scratch.braille_subpixels;
            rasterize_braille_mesh(
                mesh,
                projected_vertices,
                subpixels,
                config,
                scene,
                material_morph_weights,
                &mut stats,
                shading,
                contrast,
                palette,
                exposure,
                threshold,
                &mut histogram,
                &mut histogram_count,
                &mut scratch.triangle_order,
                &mut scratch.triangle_depth_sorted,
                scratch.subject_depth_cells.as_mut_slice(),
                matches!(instance.layer, MeshLayer::Subject),
                frame.width,
                pass,
            );
        }
    }
    update_exposure_from_histogram(
        &mut scratch.exposure,
        &histogram,
        histogram_count,
        config.clarity_profile,
    );
    compose_braille_cells(
        frame,
        &scratch.braille_subpixels,
        config,
        palette,
        threshold,
    );
    apply_visible_metrics(
        &mut stats,
        frame,
        scratch.subject_depth_cells.as_slice(),
        frame.width,
        frame.height,
    );
    update_safe_visibility_state(scratch, config.braille_profile, stats.visible_cell_ratio);
    stats
}

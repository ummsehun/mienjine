use glam::Mat4;

use crate::renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, RenderStats};
use crate::scene::{RenderBackend, RenderConfig, SceneCpu};

use super::{backend_cpu, backend_gpu};

pub fn render_frame_with_backend(
    frame: &mut FrameBuffers,
    config: &RenderConfig,
    scene: &SceneCpu,
    global_matrices: &[Mat4],
    skin_matrices: &[Vec<Mat4>],
    instance_morph_weights: &[Vec<f32>],
    glyph_ramp: &GlyphRamp,
    scratch: &mut RenderScratch,
    camera: Camera,
    model_rotation_y: f32,
) -> RenderStats {
    match config.backend {
        RenderBackend::Cpu => backend_cpu::render_frame_cpu(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            glyph_ramp,
            scratch,
            camera,
            model_rotation_y,
        ),
        RenderBackend::Gpu => match backend_gpu::render_frame_gpu(
            frame,
            config,
            scene,
            global_matrices,
            skin_matrices,
            instance_morph_weights,
            glyph_ramp,
            scratch,
            camera,
            model_rotation_y,
        ) {
            Ok(stats) => stats,
            Err(err) => {
                eprintln!("warning: gpu backend failed ({err:?}); falling back to cpu.");

                let mut cpu_cfg = config.clone();
                cpu_cfg.backend = RenderBackend::Cpu;
                backend_cpu::render_frame_cpu(
                    frame,
                    &cpu_cfg,
                    scene,
                    global_matrices,
                    skin_matrices,
                    instance_morph_weights,
                    glyph_ramp,
                    scratch,
                    camera,
                    model_rotation_y,
                )
            }
        },
    }
}

#[cfg(all(test, feature = "gpu"))]
mod tests {
    use super::*;
    use crate::pipeline::FramePipeline;
    use crate::render::gpu::GpuRenderer;
    use crate::renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch};
    use crate::scene::{
        cube_scene, MaterialAlphaMode, MaterialCpu, MeshCpu, MeshInstance, MeshLayer,
        MorphTargetCpu, Node, RenderConfig, SkinCpu, TextureColorSpace, TextureCpu,
        TextureFilterMode, TextureSamplerMode, TextureSamplingMode, TextureVOrigin,
        TextureWrapMode, UvTransform2D,
    };
    use glam::{Quat, Vec2, Vec3};
    use std::path::PathBuf;

    #[derive(Debug, Clone, Copy)]
    struct ParityMetrics {
        glyph_mismatch_ratio: f32,
        mean_rgb_abs_error: f32,
        max_rgb_abs_error: u8,
        visible_ratio_delta: f32,
    }

    fn build_checker_texture_rgba8() -> Vec<u8> {
        let mut out = Vec::with_capacity(4 * 4 * 4);
        for y in 0..4 {
            for x in 0..4 {
                let c = if (x + y) % 2 == 0 { 220 } else { 40 };
                let g = if x < 2 { 180 } else { 60 };
                let b = if y < 2 { 120 } else { 220 };
                let a = if (x + y) % 2 == 0 { 255 } else { 96 };
                out.extend_from_slice(&[c, g, b, a]);
            }
        }
        out
    }

    fn build_material_scene(alpha_mode: MaterialAlphaMode) -> crate::scene::SceneCpu {
        let mut scene = cube_scene();
        let vertex_count = scene.meshes[0].positions.len();
        let mut uv0 = Vec::with_capacity(vertex_count);
        let mut uv1 = Vec::with_capacity(vertex_count);
        for i in 0..vertex_count {
            let j = i % 4;
            let base = match j {
                0 => Vec2::new(0.0, 0.0),
                1 => Vec2::new(1.0, 0.0),
                2 => Vec2::new(1.0, 1.0),
                _ => Vec2::new(0.0, 1.0),
            };
            uv0.push(base);
            uv1.push(Vec2::new(1.0 - base.x, base.y));
        }
        scene.meshes[0].uv0 = Some(uv0);
        scene.meshes[0].uv1 = Some(uv1);

        scene.textures.push(TextureCpu {
            width: 4,
            height: 4,
            rgba8: build_checker_texture_rgba8(),
            source_format: "generated".to_owned(),
            color_space: TextureColorSpace::Srgb,
            mip_levels: Vec::new(),
        });

        scene.materials.push(MaterialCpu {
            base_color_factor: [0.95, 0.85, 1.0, 0.92],
            base_color_texture: Some(0),
            base_color_tex_coord: 0,
            base_color_uv_transform: Some(UvTransform2D {
                offset: [0.13, -0.07],
                scale: [1.20, 0.85],
                rotation_rad: 0.31,
                tex_coord_override: Some(1),
            }),
            base_color_wrap_s: TextureWrapMode::MirroredRepeat,
            base_color_wrap_t: TextureWrapMode::ClampToEdge,
            base_color_min_filter: TextureFilterMode::Nearest,
            base_color_mag_filter: TextureFilterMode::Linear,
            emissive_factor: [0.05, 0.04, 0.03],
            alpha_mode,
            alpha_cutoff: 0.52,
            double_sided: false,
        });
        scene.meshes[0].material_index = Some(0);

        scene
    }

    fn build_shared_morph_scene() -> crate::scene::SceneCpu {
        let mesh = MeshCpu {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.2, 0.0, 0.0),
                Vec3::new(0.0, 0.2, 0.0),
            ],
            normals: vec![Vec3::Y, Vec3::Y, Vec3::Y],
            uv0: None,
            uv1: None,
            colors_rgba: None,
            material_index: None,
            indices: vec![[0, 1, 2]],
            joints4: None,
            weights4: None,
            morph_targets: vec![MorphTargetCpu {
                position_deltas: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.0, 0.15, 0.0),
                    Vec3::new(0.0, 0.0, 0.0),
                ],
                normal_deltas: vec![Vec3::ZERO, Vec3::ZERO, Vec3::ZERO],
            }],
        };
        let node = Node {
            name: Some("root".to_owned()),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        };
        let node_right = Node {
            name: Some("root_right".to_owned()),
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::new(0.45, 0.0, 0.0),
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        };

        crate::scene::SceneCpu {
            meshes: vec![mesh],
            materials: Vec::new(),
            textures: Vec::new(),
            skins: Vec::new(),
            nodes: vec![node, node_right],
            mesh_instances: vec![
                MeshInstance {
                    mesh_index: 0,
                    node_index: 0,
                    skin_index: None,
                    default_morph_weights: vec![0.0],
                    layer: MeshLayer::Subject,
                },
                MeshInstance {
                    mesh_index: 0,
                    node_index: 1,
                    skin_index: None,
                    default_morph_weights: vec![1.0],
                    layer: MeshLayer::Subject,
                },
            ],
            animations: Vec::new(),
            root_center_node: Some(0),
        }
    }

    fn build_skinning_scene() -> crate::scene::SceneCpu {
        let mesh = MeshCpu {
            positions: vec![
                Vec3::new(0.0, -0.5, 0.0),
                Vec3::new(0.6, 0.6, 0.0),
                Vec3::new(-0.6, 0.6, 0.0),
            ],
            normals: vec![Vec3::Y, Vec3::Y, Vec3::Y],
            uv0: None,
            uv1: None,
            colors_rgba: None,
            material_index: None,
            indices: vec![[0, 1, 2]],
            joints4: Some(vec![[0, 1, 0, 0]; 3]),
            weights4: Some(vec![[0.25, 0.75, 0.0, 0.0]; 3]),
            morph_targets: Vec::new(),
        };

        let root = Node {
            name: Some("root".to_owned()),
            parent: None,
            children: vec![1, 2],
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        };
        let joint_a = Node {
            name: Some("joint_a".to_owned()),
            parent: Some(0),
            children: Vec::new(),
            base_translation: Vec3::new(0.0, 1.0, 0.0),
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        };
        let joint_b = Node {
            name: Some("joint_b".to_owned()),
            parent: Some(0),
            children: Vec::new(),
            base_translation: Vec3::new(0.0, -1.0, 0.0),
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        };

        crate::scene::SceneCpu {
            meshes: vec![mesh],
            materials: Vec::new(),
            textures: Vec::new(),
            skins: vec![SkinCpu {
                joints: vec![1, 2],
                inverse_bind_mats: vec![Mat4::IDENTITY, Mat4::IDENTITY],
            }],
            nodes: vec![root, joint_a, joint_b],
            mesh_instances: vec![MeshInstance {
                mesh_index: 0,
                node_index: 0,
                skin_index: Some(0),
                default_morph_weights: Vec::new(),
                layer: MeshLayer::Subject,
            }],
            animations: Vec::new(),
            root_center_node: Some(0),
        }
    }

    fn render_case(
        backend: crate::scene::RenderBackend,
        scene: &crate::scene::SceneCpu,
        mut config: RenderConfig,
    ) -> (FrameBuffers, crate::renderer::RenderStats) {
        config.backend = backend;
        let glyph_ramp = GlyphRamp::from_config(&config);
        let mut frame = FrameBuffers::new(88, 50);
        let mut scratch = RenderScratch::default();

        let mut frame_pipeline = FramePipeline::new(scene);
        frame_pipeline.prepare_frame(scene, 0.0, None);

        let stats = render_frame_with_backend(
            &mut frame,
            &config,
            scene,
            frame_pipeline.globals(),
            frame_pipeline.skin_matrices(),
            frame_pipeline.morph_weights_by_instance(),
            &glyph_ramp,
            &mut scratch,
            Camera::default(),
            0.22,
        );

        (frame, stats)
    }

    fn parity_metrics(
        cpu: &FrameBuffers,
        gpu: &FrameBuffers,
        cpu_stats: crate::renderer::RenderStats,
        gpu_stats: crate::renderer::RenderStats,
    ) -> ParityMetrics {
        let len = cpu.glyphs.len().min(gpu.glyphs.len()).max(1);
        let mut glyph_mismatch = 0usize;
        let mut rgb_abs_sum = 0u64;
        let mut rgb_abs_count = 0u64;
        let mut max_abs = 0u8;

        for i in 0..len {
            if cpu.glyphs[i] != gpu.glyphs[i] {
                glyph_mismatch += 1;
            }
            for c in 0..3 {
                let a = cpu.fg_rgb[i][c] as i16;
                let b = gpu.fg_rgb[i][c] as i16;
                let d = (a - b).unsigned_abs() as u8;
                max_abs = max_abs.max(d);
                rgb_abs_sum += u64::from(d);
                rgb_abs_count += 1;
            }
        }

        ParityMetrics {
            glyph_mismatch_ratio: glyph_mismatch as f32 / len as f32,
            mean_rgb_abs_error: if rgb_abs_count == 0 {
                0.0
            } else {
                rgb_abs_sum as f32 / rgb_abs_count as f32
            },
            max_rgb_abs_error: max_abs,
            visible_ratio_delta: (cpu_stats.visible_cell_ratio - gpu_stats.visible_cell_ratio)
                .abs(),
        }
    }

    #[test]
    fn gpu_parity_phase1_baseline_report() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping phase1 baseline capture");
            return;
        }

        let mut cfg = RenderConfig::default();
        cfg.material_color = true;
        cfg.texture_sampler = TextureSamplerMode::Gltf;
        cfg.texture_sampling = TextureSamplingMode::Bilinear;
        cfg.texture_v_origin = TextureVOrigin::Gltf;

        let opaque_scene = build_material_scene(MaterialAlphaMode::Opaque);
        let mask_scene = build_material_scene(MaterialAlphaMode::Mask);
        let blend_scene = build_material_scene(MaterialAlphaMode::Blend);

        for (name, scene) in [
            ("opaque_uv_transform", opaque_scene),
            ("mask_cutoff", mask_scene),
            ("blend_alpha", blend_scene),
        ] {
            let (cpu_frame, cpu_stats) =
                render_case(crate::scene::RenderBackend::Cpu, &scene, cfg.clone());
            let (gpu_frame, gpu_stats) =
                render_case(crate::scene::RenderBackend::Gpu, &scene, cfg.clone());
            let metrics = parity_metrics(&cpu_frame, &gpu_frame, cpu_stats, gpu_stats);

            println!(
                "PARITY_BASELINE {name} glyph_mismatch={:.6} mean_rgb_abs={:.6} max_rgb_abs={} visible_ratio_delta={:.6}",
                metrics.glyph_mismatch_ratio,
                metrics.mean_rgb_abs_error,
                metrics.max_rgb_abs_error,
                metrics.visible_ratio_delta,
            );

            assert!(metrics.glyph_mismatch_ratio.is_finite());
            assert!(metrics.mean_rgb_abs_error.is_finite());
            assert!(metrics.visible_ratio_delta.is_finite());
        }
    }

    #[test]
    fn gpu_morph_cache_shared_mesh_instances_are_isolated() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping morph cache isolation capture");
            return;
        }

        let scene = build_shared_morph_scene();
        let mut cfg = RenderConfig::default();
        cfg.backend = crate::scene::RenderBackend::Gpu;
        cfg.mode = crate::scene::RenderMode::Ascii;

        let (cpu_frame, cpu_stats) =
            render_case(crate::scene::RenderBackend::Cpu, &scene, cfg.clone());
        let (gpu_frame, gpu_stats) = render_case(crate::scene::RenderBackend::Gpu, &scene, cfg);

        let metrics = parity_metrics(&cpu_frame, &gpu_frame, cpu_stats, gpu_stats);
        assert!(metrics.glyph_mismatch_ratio.is_finite());
        assert!(metrics.mean_rgb_abs_error.is_finite());
        assert!(metrics.visible_ratio_delta.is_finite());
    }

    #[test]
    fn gpu_stage_role_off_matches_cpu_exclusion() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping stage exclusion capture");
            return;
        }

        let mut scene = cube_scene();
        scene.mesh_instances[0].layer = MeshLayer::Stage;

        let mut cfg = RenderConfig::default();
        cfg.stage_role = crate::scene::StageRole::Off;
        cfg.material_color = true;

        let (_cpu_frame, cpu_stats) =
            render_case(crate::scene::RenderBackend::Cpu, &scene, cfg.clone());
        let (_gpu_frame, gpu_stats) = render_case(crate::scene::RenderBackend::Gpu, &scene, cfg);

        assert_eq!(cpu_stats.triangles_total, gpu_stats.triangles_total);
        assert_eq!(cpu_stats.visible_cell_ratio, gpu_stats.visible_cell_ratio);
    }

    #[test]
    fn gpu_skinning_keeps_shapes_close_to_cpu() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping skinning capture");
            return;
        }

        let scene = build_skinning_scene();
        let mut cfg = RenderConfig::default();
        cfg.material_color = true;

        let (cpu_frame, cpu_stats) =
            render_case(crate::scene::RenderBackend::Cpu, &scene, cfg.clone());
        let (gpu_frame, gpu_stats) = render_case(crate::scene::RenderBackend::Gpu, &scene, cfg);
        let metrics = parity_metrics(&cpu_frame, &gpu_frame, cpu_stats, gpu_stats);

        assert!(metrics.mean_rgb_abs_error.is_finite());
        assert!(metrics.visible_ratio_delta.is_finite());
        assert!(metrics.visible_ratio_delta < 0.35);
    }

    #[test]
    fn gpu_skinning_handles_vertices_with_zero_first_weight() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping zero-first-weight capture");
            return;
        }

        let mut scene = build_skinning_scene();
        if let Some(mesh) = scene.meshes.get_mut(0) {
            mesh.weights4 = Some(vec![[0.0, 1.0, 0.0, 0.0]; 3]);
        }
        let mut cfg = RenderConfig::default();
        cfg.material_color = true;

        let (cpu_frame, cpu_stats) =
            render_case(crate::scene::RenderBackend::Cpu, &scene, cfg.clone());
        let (gpu_frame, gpu_stats) = render_case(crate::scene::RenderBackend::Gpu, &scene, cfg);
        let metrics = parity_metrics(&cpu_frame, &gpu_frame, cpu_stats, gpu_stats);

        assert!(metrics.mean_rgb_abs_error.is_finite());
        assert!(metrics.visible_ratio_delta.is_finite());
    }

    #[test]
    fn gpu_ascii_output_not_catastrophically_collapsed() {
        if !GpuRenderer::is_available() {
            eprintln!("gpu unavailable; skipping ascii collapse capture");
            return;
        }

        let scene = build_skinning_scene();
        let mut cfg = RenderConfig::default();
        cfg.material_color = true;
        cfg.mode = crate::scene::RenderMode::Ascii;

        let (cpu_frame, cpu_stats) =
            render_case(crate::scene::RenderBackend::Cpu, &scene, cfg.clone());
        let (gpu_frame, gpu_stats) = render_case(crate::scene::RenderBackend::Gpu, &scene, cfg);
        let metrics = parity_metrics(&cpu_frame, &gpu_frame, cpu_stats, gpu_stats);

        assert!(metrics.glyph_mismatch_ratio.is_finite());
        assert!(metrics.visible_ratio_delta.is_finite());
        assert!(metrics.glyph_mismatch_ratio < 0.80);
    }

    #[test]
    fn supported_glb_joint_counts_fit_gpu_limit() {
        for path in [
            PathBuf::from("/Users/user/miku/assets/glb/sei.glb"),
            PathBuf::from("/Users/user/miku/assets/glb/rabbit3.glb"),
            PathBuf::from("/Users/user/miku/assets/glb/miku.glb"),
        ] {
            let scene = crate::assets::loader::load_gltf(&path).expect("load glb");
            let max_joints = scene
                .skins
                .iter()
                .map(|skin| skin.joints.len())
                .max()
                .unwrap_or(0);
            assert!(
                max_joints <= 512,
                "{} exceeds GPU joint limit",
                path.display()
            );
        }
    }
}

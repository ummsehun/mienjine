use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::{
    cli::{BenchArgs, BenchSceneArg, Cli, Commands, InspectArgs, RunArgs, RunSceneArg},
    loader,
    pipeline::FramePipeline,
    renderer::{Camera, FrameBuffers, GlyphRamp, RenderScratch, render_frame},
    scene::{RenderConfig, SceneCpu},
    terminal::TerminalSession,
};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Run(args) => run_interactive(args),
        Commands::Bench(args) => bench(args),
        Commands::Inspect(args) => inspect(args),
    }
}

fn run_interactive(args: RunArgs) -> Result<()> {
    let (scene, animation_index, rotates_without_animation) = load_scene_for_run(&args)?;
    let config = RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode: args.mode.into(),
        charset: args.charset,
        cell_aspect: args.cell_aspect,
        fps_cap: args.fps_cap.max(1),
        ambient: args.ambient,
        diffuse_strength: args.diffuse_strength,
        specular_strength: args.specular_strength,
        specular_power: args.specular_power,
        rim_strength: args.rim_strength,
        rim_power: args.rim_power,
        fog_strength: args.fog_strength,
    };

    let mut terminal = TerminalSession::enter()?;
    let (mut width, mut height) = terminal.size()?;
    if width == 0 || height == 0 {
        width = 120;
        height = 40;
    }
    let mut frame = FrameBuffers::new(width, height);
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let orbit_speed = args.orbit_speed.max(0.0);
    let orbit_radius = args.orbit_radius.max(0.1);
    let camera_height = args.camera_height;
    let look_at_y = args.look_at_y;

    let start = Instant::now();
    let frame_budget = Duration::from_secs_f32(1.0 / (config.fps_cap as f32));

    loop {
        let frame_start = Instant::now();
        if should_exit_frame_loop(&mut frame)? {
            break;
        }

        let elapsed = start.elapsed().as_secs_f32();
        pipeline.prepare_frame(&scene, elapsed, animation_index);
        let rotation = if animation_index.is_some() {
            0.0
        } else if rotates_without_animation {
            elapsed * 0.9
        } else {
            0.0
        };
        let camera = orbit_camera(orbit_speed, orbit_radius, camera_height, look_at_y, elapsed);
        render_frame(
            &mut frame,
            &config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
            &glyph_ramp,
            &mut render_scratch,
            camera,
            rotation,
        );
        let output = pipeline.text_buffer_mut();
        frame.write_text(output);
        terminal.draw_frame(output)?;

        let elapsed_frame = frame_start.elapsed();
        if elapsed_frame < frame_budget {
            thread::sleep(frame_budget - elapsed_frame);
        }
    }
    Ok(())
}

fn load_scene_for_run(args: &RunArgs) -> Result<(SceneCpu, Option<usize>, bool)> {
    match args.scene {
        RunSceneArg::Cube => Ok((crate::scene::cube_scene(), None, true)),
        RunSceneArg::Obj => {
            let path = required_path(args.obj.as_deref(), "--obj is required for --scene obj")?;
            Ok((loader::load_obj(path)?, None, true))
        }
        RunSceneArg::Glb => {
            let path = required_path(args.glb.as_deref(), "--glb is required for --scene glb")?;
            let scene = loader::load_gltf(path)?;
            let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
            Ok((scene, animation_index, true))
        }
    }
}

fn should_exit_frame_loop(frame: &mut FrameBuffers) -> Result<bool> {
    while event::poll(Duration::from_millis(0))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(true),
                _ => {}
            },
            Event::Resize(width, height) => frame.resize(width.max(1), height.max(1)),
            _ => {}
        }
    }
    Ok(false)
}

fn bench(args: BenchArgs) -> Result<()> {
    let (scene, animation_index, rotates) = load_scene_for_bench(&args)?;
    let config = RenderConfig {
        fov_deg: args.fov_deg,
        near: args.near,
        far: args.far,
        mode: args.mode.into(),
        charset: args.charset,
        cell_aspect: args.cell_aspect,
        fps_cap: u32::MAX,
        ambient: args.ambient,
        diffuse_strength: args.diffuse_strength,
        specular_strength: args.specular_strength,
        specular_power: args.specular_power,
        rim_strength: args.rim_strength,
        rim_power: args.rim_power,
        fog_strength: args.fog_strength,
    };
    let mut frame = FrameBuffers::new(args.width.max(1), args.height.max(1));
    let mut pipeline = FramePipeline::new(&scene);
    let glyph_ramp = GlyphRamp::from_config(&config);
    let mut render_scratch = RenderScratch::with_capacity(max_scene_vertices(&scene));
    let camera = Camera::default();

    let benchmark_duration = Duration::from_secs_f32(args.seconds.max(0.1));
    let started = Instant::now();
    let mut frames: u64 = 0;
    let mut triangles: u64 = 0;
    let mut pixels: u64 = 0;

    while started.elapsed() < benchmark_duration {
        let elapsed = started.elapsed().as_secs_f32();
        pipeline.prepare_frame(&scene, elapsed, animation_index);
        let stats = render_frame(
            &mut frame,
            &config,
            &scene,
            pipeline.globals(),
            pipeline.skin_matrices(),
            &glyph_ramp,
            &mut render_scratch,
            camera,
            if rotates { elapsed * 0.9 } else { 0.0 },
        );
        frames += 1;
        triangles += stats.triangles_total as u64;
        pixels += stats.pixels_drawn as u64;
    }

    let elapsed = started.elapsed().as_secs_f64();
    let fps = (frames as f64) / elapsed;
    println!("scene: {:?}", args.scene);
    println!("seconds: {:.2}", elapsed);
    println!("frames: {}", frames);
    println!("fps: {:.2}", fps);
    println!(
        "avg_triangles_per_frame: {:.2}",
        triangles as f64 / (frames.max(1) as f64)
    );
    println!(
        "avg_pixels_per_frame: {:.2}",
        pixels as f64 / (frames.max(1) as f64)
    );
    Ok(())
}

fn inspect(args: InspectArgs) -> Result<()> {
    let scene = loader::load_gltf(&args.glb)?;
    println!("file: {}", args.glb.display());
    println!("meshes: {}", scene.meshes.len());
    println!("mesh_instances: {}", scene.mesh_instances.len());
    println!("nodes: {}", scene.nodes.len());
    println!("skins: {}", scene.skins.len());
    println!("animations: {}", scene.animations.len());
    println!("total_vertices: {}", scene.total_vertices());
    println!("total_triangles: {}", scene.total_triangles());
    println!("total_joints: {}", scene.total_joints());
    for (index, animation) in scene.animations.iter().enumerate() {
        println!(
            "animation[{index}]: name={} duration={:.3}s channels={}",
            animation.name.as_deref().unwrap_or("<unnamed>"),
            animation.duration,
            animation.channels.len()
        );
    }
    Ok(())
}

fn resolve_animation_index(scene: &SceneCpu, selector: Option<&str>) -> Result<Option<usize>> {
    if let Some(selector) = selector {
        let index = scene
            .animation_index_by_selector(Some(selector))
            .with_context(|| format!("animation selector not found: {selector}"))?;
        return Ok(Some(index));
    }
    Ok((!scene.animations.is_empty()).then_some(0))
}

fn load_scene_for_bench(args: &BenchArgs) -> Result<(SceneCpu, Option<usize>, bool)> {
    match args.scene {
        BenchSceneArg::Cube => Ok((crate::scene::cube_scene(), None, true)),
        BenchSceneArg::Obj => {
            let path = required_path(args.obj.as_deref(), "--obj is required for --scene obj")?;
            Ok((loader::load_obj(path)?, None, true))
        }
        BenchSceneArg::GlbStatic => {
            let path = required_path(
                args.glb.as_deref(),
                "--glb is required for --scene glb-static",
            )?;
            Ok((loader::load_gltf(path)?, None, false))
        }
        BenchSceneArg::GlbAnim => {
            let path = required_path(
                args.glb.as_deref(),
                "--glb is required for --scene glb-anim",
            )?;
            let scene = loader::load_gltf(path)?;
            let animation_index = resolve_animation_index(&scene, args.anim.as_deref())?;
            if animation_index.is_none() {
                bail!("scene has no animation clips: {}", path.display());
            }
            Ok((scene, animation_index, false))
        }
    }
}

fn required_path<'a>(path: Option<&'a Path>, message: &str) -> Result<&'a Path> {
    path.ok_or_else(|| anyhow::anyhow!("{message}"))
}

fn orbit_camera(
    orbit_speed: f32,
    orbit_radius: f32,
    camera_height: f32,
    look_at_y: f32,
    time: f32,
) -> Camera {
    let angle = time * orbit_speed;
    let eye = glam::Vec3::new(
        angle.cos() * orbit_radius,
        camera_height,
        angle.sin() * orbit_radius,
    );
    let target = glam::Vec3::new(0.0, look_at_y, 0.0);
    Camera {
        eye,
        target,
        up: glam::Vec3::Y,
    }
}

fn max_scene_vertices(scene: &SceneCpu) -> usize {
    scene
        .meshes
        .iter()
        .map(|mesh| mesh.positions.len())
        .max()
        .unwrap_or(0)
}

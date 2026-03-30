use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::{
    animation::ChannelTarget,
    cli::{BenchArgs, BenchSceneArg, RunArgs, RunSceneArg},
    loader,
    scene::SceneCpu,
};

pub(crate) fn resolve_animation_index(
    scene: &SceneCpu,
    selector: Option<&str>,
) -> Result<Option<usize>> {
    if let Some(selector) = selector {
        let index = scene
            .animation_index_by_selector(Some(selector))
            .with_context(|| format!("animation selector not found: {selector}"))?;
        return Ok(Some(index));
    }
    Ok(default_body_animation_index(scene))
}

pub(crate) fn default_body_animation_index(scene: &SceneCpu) -> Option<usize> {
    scene
        .animations
        .iter()
        .enumerate()
        .find(|(_, clip)| {
            !clip.channels.is_empty()
                && clip
                    .channels
                    .iter()
                    .any(|channel| channel.target != ChannelTarget::MorphWeights)
        })
        .map(|(index, _)| index)
        .or_else(|| (!scene.animations.is_empty()).then_some(0))
}

pub(crate) fn load_scene_for_run(args: &RunArgs) -> Result<(SceneCpu, Option<usize>, bool)> {
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
        RunSceneArg::Pmx => {
            let path = required_path(args.pmx.as_deref(), "--pmx is required for --scene pmx")?;
            let scene = loader::load_pmx(path)?;
            Ok((scene, None, true))
        }
    }
}

pub(crate) fn load_scene_for_bench(args: &BenchArgs) -> Result<(SceneCpu, Option<usize>, bool)> {
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

pub(crate) fn required_path<'a>(path: Option<&'a Path>, message: &str) -> Result<&'a Path> {
    path.ok_or_else(|| anyhow::anyhow!("{message}"))
}

use anyhow::{Context, Result};

use crate::{
    assets::vmd_motion::parse_vmd_motion, interfaces::tui::start_ui::StartSelection, loader,
    runtime::pmx_log, scene::SceneCpu,
};

pub(super) fn load_selected_model(selection: &StartSelection) -> Result<SceneCpu> {
    match selection.branch {
        crate::interfaces::tui::start_ui::ModelBranch::Glb => {
            loader::load_gltf(&selection.glb_path)
        }
        crate::interfaces::tui::start_ui::ModelBranch::PmxVmd => load_pmx_with_vmd(selection),
    }
}

fn load_pmx_with_vmd(selection: &StartSelection) -> Result<SceneCpu> {
    let pmx_path = selection
        .pmx_path
        .as_deref()
        .context("PMX branch selected without pmx_path")?;

    pmx_log::start_session("=== PMX+VMD import session start ===");
    pmx_log::info(format!("PMX path: {}", pmx_path.display()));
    if let Some(motion_vmd_path) = selection.motion_vmd_path.as_deref() {
        pmx_log::info(format!("VMD path: {}", motion_vmd_path.display()));
    } else {
        pmx_log::warn("PMX branch selected without a VMD motion; model will load static.");
    }

    let mut scene = match loader::load_pmx(pmx_path) {
        Ok(scene) => scene,
        Err(err) => {
            pmx_log::error(format!("failed to load PMX {}: {err}", pmx_path.display()));
            return Err(err);
        }
    };

    log_pmx_stats(&scene);

    if let Some(motion_vmd_path) = selection.motion_vmd_path.as_deref() {
        match parse_vmd_motion(motion_vmd_path) {
            Ok(vmd) => {
                pmx_log::info(format!(
                    "VMD parsed: model_name='{}', bone_frames={}, morph_frames={}, duration={:.3}s",
                    vmd.model_name,
                    vmd.bone_frames.len(),
                    vmd.morph_frames.len(),
                    vmd.duration_secs()
                ));
                if !vmd.bone_frames.is_empty() || !vmd.morph_frames.is_empty() {
                    let clip = vmd.to_clip_for_scene(&scene);
                    pmx_log::info(format!(
                        "VMD clip built: channels={}, duration={:.3}s",
                        clip.channels.len(),
                        clip.duration
                    ));
                    if clip.channels.is_empty() {
                        pmx_log::warn(
                            "VMD clip has no matched channels; bone/morph names may not match this PMX.",
                        );
                    }
                    scene.animations.push(clip);
                } else {
                    pmx_log::warn(format!(
                        "VMD {} contains no bone or morph frames.",
                        motion_vmd_path.display()
                    ));
                }
            }
            Err(err) => {
                pmx_log::error(format!(
                    "failed to parse VMD {}: {err}",
                    motion_vmd_path.display()
                ));
            }
        }
    }

    pmx_log::info(format!(
        "PMX+VMD scene animations={}",
        scene.animations.len()
    ));

    Ok(scene)
}

fn log_pmx_stats(scene: &SceneCpu) {
    let morph_target_count = scene
        .meshes
        .iter()
        .map(|mesh| mesh.morph_targets.len())
        .sum::<usize>();

    let (
        bone_count,
        bone_ik_count,
        bone_append_count,
        bone_fixed_axis_count,
        bone_local_axis_count,
        bone_external_parent_count,
    ) = scene
        .pmx_rig_meta
        .as_ref()
        .map(|meta| {
            (
                meta.bones.len(),
                meta.count_bones_with_ik(),
                meta.count_bones_with_append(),
                meta.count_bones_with_fixed_axis(),
                meta.count_bones_with_local_axis(),
                meta.count_bones_with_external_parent(),
            )
        })
        .unwrap_or((0, 0, 0, 0, 0, 0));

    let ik_chain_count = scene
        .pmx_rig_meta
        .as_ref()
        .map(|meta| meta.ik_chains.len())
        .unwrap_or(0);

    let rigid_body_count = scene
        .pmx_physics_meta
        .as_ref()
        .map(|meta| meta.rigid_bodies.len())
        .unwrap_or(0);

    let joint_count = scene
        .pmx_physics_meta
        .as_ref()
        .map(|meta| meta.joints.len())
        .unwrap_or(0);

    pmx_log::info(format!(
        "PMX loaded: nodes={}, skins={}, meshes={}, vertices={}, triangles={}, morph_targets={}, material_morphs={}, ik_chains={}, rigid_bodies={}, joints={}",
        scene.nodes.len(),
        scene.skins.len(),
        scene.meshes.len(),
        scene.total_vertices(),
        scene.total_triangles(),
        morph_target_count,
        scene.material_morphs.len(),
        ik_chain_count,
        rigid_body_count,
        joint_count
    ));

    pmx_log::info(format!(
        "PMX rig bones: total={}, ik={}, append={}, fixed_axis={}, local_axis={}, external_parent={}",
        bone_count,
        bone_ik_count,
        bone_append_count,
        bone_fixed_axis_count,
        bone_local_axis_count,
        bone_external_parent_count
    ));
}

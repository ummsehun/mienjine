use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use glam::{Quat, Vec3};

use crate::{
    interfaces::tui::start_ui::{StageChoice, StageStatus, StageTransform},
    loader,
    runtime::config::GasciiConfig,
    scene::{MeshLayer, SceneCpu},
};

pub(crate) fn discover_glb_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    discover_files_recursive(dir, &mut files, &["glb", "gltf", "obj"])?;
    files.sort();
    Ok(files)
}

pub(crate) fn discover_pmx_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    discover_files_recursive(dir, &mut files, &["pmx"])?;
    files.sort();
    Ok(files)
}

pub(crate) fn discover_music_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    discover_files_recursive(dir, &mut files, &["mp3", "wav", "flac", "ogg", "m4a"])?;
    files.sort();
    Ok(files)
}

pub(crate) fn discover_camera_vmds(dir: &Path) -> Vec<PathBuf> {
    discover_files_recursive_lossy(dir, &["vmd"])
}

pub(crate) fn discover_vmd_files(dir: &Path) -> Vec<PathBuf> {
    discover_files_recursive_lossy(dir, &["vmd"])
}

pub(crate) fn discover_default_camera_vmd(dir: &Path) -> Option<PathBuf> {
    let files = discover_camera_vmds(dir);
    files
        .iter()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some("world_is_mine.vmd"))
        .cloned()
        .or_else(|| files.first().cloned())
}

pub(crate) fn resolve_camera_vmd_choice(
    dir: &Path,
    files: &[PathBuf],
    selector: &str,
) -> Option<PathBuf> {
    let trimmed = selector.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.eq_ignore_ascii_case("default")
    {
        return discover_default_camera_vmd(dir);
    }
    resolve_camera_vmd_selector(files, trimmed)
}

pub(crate) fn resolve_camera_vmd_selector(files: &[PathBuf], selector: &str) -> Option<PathBuf> {
    let trimmed = selector.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.eq_ignore_ascii_case("default")
    {
        return files.first().cloned();
    }
    if trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("off") {
        return None;
    }
    let selector_path = Path::new(trimmed);
    if selector_path.exists() {
        let selector_abs = selector_path
            .canonicalize()
            .unwrap_or_else(|_| selector_path.to_path_buf());
        return files
            .iter()
            .find(|path| path.canonicalize().ok().is_some_and(|p| p == selector_abs))
            .cloned();
    }
    files
        .iter()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(trimmed))
        .cloned()
}

pub(crate) fn resolved_stage_dir(cli_stage_dir: &Path, runtime_cfg: &GasciiConfig) -> PathBuf {
    if cli_stage_dir.as_os_str().is_empty() {
        runtime_cfg.stage_dir.clone()
    } else {
        cli_stage_dir.to_path_buf()
    }
}

pub(crate) fn resolved_camera_dir(cli_camera_dir: &Path, runtime_cfg: &GasciiConfig) -> PathBuf {
    if cli_camera_dir.as_os_str().is_empty() {
        runtime_cfg.camera_dir.clone()
    } else {
        cli_camera_dir.to_path_buf()
    }
}

pub(crate) fn discover_stage_sets(root: &Path) -> Vec<StageChoice> {
    let mut stages = Vec::new();
    if !root.exists() {
        return stages;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return stages;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let transform = load_stage_transform(&path.join("stage.meta.toml"));
        let mut renderable_files = Vec::new();
        let mut pmx_files = Vec::new();
        discover_stage_files_recursive(&path, &mut renderable_files, &mut pmx_files);
        let status = if !renderable_files.is_empty() {
            StageStatus::Ready
        } else if !pmx_files.is_empty() {
            StageStatus::NeedsConvert
        } else {
            StageStatus::Invalid
        };
        stages.push(StageChoice {
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<invalid>")
                .to_owned(),
            status,
            render_path: renderable_files.first().cloned(),
            pmx_path: pmx_files.first().cloned(),
            transform,
        });
    }
    stages
}

pub(crate) fn resolved_stage_selector(
    cli_stage: Option<&str>,
    runtime_cfg: &GasciiConfig,
) -> String {
    cli_stage
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| runtime_cfg.stage_selection.clone())
}

pub(crate) fn resolve_stage_choice_from_selector(
    entries: &[StageChoice],
    selector: &str,
) -> Option<StageChoice> {
    let trimmed = selector.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.eq_ignore_ascii_case("default")
    {
        return entries
            .iter()
            .find(|entry| matches!(entry.status, StageStatus::Ready))
            .cloned();
    }
    if trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("off")
        || trimmed == "없음"
    {
        return None;
    }
    let selector_path = Path::new(trimmed);
    if selector_path.exists() {
        if selector_path.is_dir()
            && let Some(dir_name) = selector_path.file_name().and_then(|n| n.to_str())
            && let Some(found) = entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(dir_name))
        {
            return Some(found.clone());
        }
        let selector_abs = selector_path
            .canonicalize()
            .unwrap_or_else(|_| selector_path.to_path_buf());
        if let Some(found) = entries.iter().find(|entry| {
            entry
                .render_path
                .as_ref()
                .and_then(|p| p.canonicalize().ok())
                .is_some_and(|p| p == selector_abs)
        }) {
            return Some(found.clone());
        }
    }
    entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(trimmed))
        .cloned()
}

pub(crate) fn discover_stage_files_recursive(
    root: &Path,
    renderable_files: &mut Vec<PathBuf>,
    pmx_files: &mut Vec<PathBuf>,
) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase());
            match ext.as_deref() {
                Some("glb" | "gltf" | "obj") => renderable_files.push(path),
                Some("pmx") => pmx_files.push(path),
                _ => {}
            }
        }
    }
}

pub(crate) fn load_stage_transform(path: &Path) -> StageTransform {
    let Ok(content) = fs::read_to_string(path) else {
        return StageTransform::default();
    };
    let mut transform = StageTransform::default();
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        let value = raw_value.trim();
        match key.as_str() {
            "offset" => {
                if let Some(v) = parse_meta_vec3(value) {
                    transform.offset = v;
                }
            }
            "rot" | "rotation" | "rotation_deg" => {
                if let Some(v) = parse_meta_vec3(value) {
                    transform.rotation_deg = v;
                }
            }
            "scale" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    transform.scale = parsed.clamp(0.01, 100.0);
                }
            }
            _ => {}
        }
    }
    transform
}

pub(crate) fn parse_meta_vec3(value: &str) -> Option<[f32; 3]> {
    let trimmed = value.trim();
    let body = trimmed
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(trimmed);
    let parts = body
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok())
        .collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    Some([parts[0]?, parts[1]?, parts[2]?])
}

pub(crate) fn apply_stage_transform(scene: &mut SceneCpu, transform: StageTransform) {
    if scene.nodes.is_empty() {
        return;
    }
    let rotation = Quat::from_euler(
        glam::EulerRot::XYZ,
        transform.rotation_deg[0].to_radians(),
        transform.rotation_deg[1].to_radians(),
        transform.rotation_deg[2].to_radians(),
    );
    let root_index = scene.nodes.len();
    let mut children = Vec::new();
    for (index, node) in scene.nodes.iter_mut().enumerate() {
        if node.parent.is_none() {
            node.parent = Some(root_index);
            children.push(index);
        }
    }
    scene.nodes.push(crate::scene::Node {
        name: Some("StageTransformRoot".to_owned()),
        name_en: None,
        parent: None,
        children,
        base_translation: Vec3::new(
            transform.offset[0],
            transform.offset[1],
            transform.offset[2],
        ),
        base_rotation: rotation,
        base_scale: Vec3::splat(transform.scale),
    });
}

pub(crate) fn load_scene_file(path: &Path) -> Result<SceneCpu> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "glb" | "gltf" => loader::load_gltf(path),
        "obj" => loader::load_obj(path),
        "pmx" => loader::load_pmx(path),
        other => bail!(
            "unsupported scene file extension for runtime merge: {} ({other})",
            path.display()
        ),
    }
}

pub(crate) fn merge_scenes(mut base: SceneCpu, mut overlay: SceneCpu) -> SceneCpu {
    let texture_offset = base.textures.len();
    base.textures.append(&mut overlay.textures);

    let material_offset = base.materials.len();
    for material in &mut overlay.materials {
        material.base_color_texture = material.base_color_texture.map(|idx| idx + texture_offset);
    }
    base.materials.append(&mut overlay.materials);

    let mesh_offset = base.meshes.len();
    for mesh in &mut overlay.meshes {
        mesh.material_index = mesh.material_index.map(|idx| idx + material_offset);
    }
    base.meshes.append(&mut overlay.meshes);

    let node_offset = base.nodes.len();
    for node in &mut overlay.nodes {
        node.parent = node.parent.map(|idx| idx + node_offset);
        for child in &mut node.children {
            *child += node_offset;
        }
    }
    let overlay_root = overlay.root_center_node.map(|idx| idx + node_offset);
    base.nodes.append(&mut overlay.nodes);

    let skin_offset = base.skins.len();
    for skin in &mut overlay.skins {
        for joint in &mut skin.joints {
            *joint += node_offset;
        }
    }
    base.skins.append(&mut overlay.skins);

    for instance in &mut overlay.mesh_instances {
        instance.mesh_index += mesh_offset;
        instance.node_index += node_offset;
        instance.skin_index = instance.skin_index.map(|idx| idx + skin_offset);
        instance.layer = MeshLayer::Stage;
    }
    base.mesh_instances.append(&mut overlay.mesh_instances);

    for clip in &mut overlay.animations {
        for channel in &mut clip.channels {
            channel.node_index += node_offset;
        }
    }
    base.animations.append(&mut overlay.animations);

    if base.root_center_node.is_none() {
        base.root_center_node = overlay_root;
    }
    base
}

fn discover_files_recursive(dir: &Path, files: &mut Vec<PathBuf>, exts: &[&str]) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("read_dir failed: {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            discover_files_recursive(&path, files, exts)?;
            continue;
        }
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| v.to_ascii_lowercase());
        if ext.as_deref().is_some_and(|ext| exts.contains(&ext)) {
            files.push(path);
        }
    }
    Ok(())
}

fn discover_files_recursive_lossy(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let _ = discover_files_recursive(dir, &mut files, exts);
    files.sort();
    files
}

use anyhow::Result;

use crate::{
    interfaces::tui::start_ui::{StageChoice, StageStatus},
    runtime::asset_discovery::{apply_stage_transform, load_scene_file, merge_scenes},
    scene::SceneCpu,
};

pub(super) fn apply_stage_selection(
    mut scene: SceneCpu,
    stage_choice: Option<&StageChoice>,
) -> Result<SceneCpu> {
    let Some(stage_choice) = stage_choice else {
        return Ok(scene);
    };

    match stage_choice.status {
        StageStatus::Ready => {
            if let Some(stage_path) = stage_choice.render_path.as_deref() {
                match load_scene_file(stage_path) {
                    Ok(mut stage_scene) => {
                        apply_stage_transform(&mut stage_scene, stage_choice.transform);
                        scene = merge_scenes(scene, stage_scene);
                    }
                    Err(err) => {
                        eprintln!(
                            "warning: failed to load stage {}: {err}",
                            stage_path.display()
                        );
                    }
                }
            }
        }
        StageStatus::NeedsConvert => {
            let pmx = stage_choice
                .pmx_path
                .as_deref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| stage_choice.name.clone());
            anyhow::bail!(
                "선택한 스테이지는 PMX 변환이 필요합니다: {pmx}\nBlender + MMD Tools로 GLB 변환 후 다시 실행하세요."
            );
        }
        StageStatus::Invalid => {
            eprintln!(
                "warning: selected stage '{}' is invalid (no renderable assets). continuing without stage.",
                stage_choice.name
            );
        }
    }

    Ok(scene)
}

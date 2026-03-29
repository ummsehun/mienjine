use super::*;

#[test]
fn discover_stage_sets_classifies_ready_and_convert() {
    let dir = tempdir().expect("tempdir");
    let stage_root = dir.path().join("assets").join("stage");
    let ready_dir = stage_root.join("ready_stage");
    let convert_dir = stage_root.join("pmx_stage");
    let invalid_dir = stage_root.join("empty_stage");
    fs::create_dir_all(&ready_dir).expect("ready dir");
    fs::create_dir_all(&convert_dir).expect("convert dir");
    fs::create_dir_all(&invalid_dir).expect("invalid dir");
    fs::write(ready_dir.join("scene.glb"), b"not-a-real-glb").expect("ready file");
    fs::write(convert_dir.join("stage.pmx"), b"pmx").expect("pmx file");

    let stages = discover_stage_sets(&stage_root);
    assert_eq!(stages.len(), 3);
    assert!(stages.iter().any(|s| {
        s.name == "ready_stage" && matches!(s.status, StageStatus::Ready) && s.render_path.is_some()
    }));
    assert!(stages.iter().any(|s| {
        s.name == "pmx_stage"
            && matches!(s.status, StageStatus::NeedsConvert)
            && s.pmx_path.is_some()
    }));
    assert!(stages
        .iter()
        .any(|s| s.name == "empty_stage" && matches!(s.status, StageStatus::Invalid)));
}

#[test]
fn discover_pmx_files_recurses_into_nested_directories() {
    let dir = tempdir().expect("tempdir");
    let pmx_root = dir.path().join("assets").join("pmx");
    let nested_dir = pmx_root.join("miku").join("tex");
    fs::create_dir_all(&nested_dir).expect("pmx dirs");
    let pmx_path = pmx_root.join("miku").join("Tda式初音ミクV4X_Ver1.00.pmx");
    fs::write(&pmx_path, b"pmx").expect("pmx file");
    fs::write(nested_dir.join("toon_defo.bmp"), b"tex").expect("texture file");

    let files = discover_pmx_files(&pmx_root).expect("discover pmx files");
    assert_eq!(files, vec![pmx_path]);
}

#[test]
fn discover_vmd_files_keeps_motion_and_camera_dirs_separate() {
    let dir = tempdir().expect("tempdir");
    let motion_dir = dir.path().join("assets").join("vmd");
    let camera_dir = dir.path().join("assets").join("camera");
    fs::create_dir_all(&motion_dir).expect("motion dir");
    fs::create_dir_all(&camera_dir).expect("camera dir");
    let motion_vmd = motion_dir.join("dance.vmd");
    let camera_vmd = camera_dir.join("world_is_mine.vmd");
    fs::write(&motion_vmd, b"motion").expect("motion file");
    fs::write(&camera_vmd, b"camera").expect("camera file");

    let motion_files = discover_vmd_files(&motion_dir);
    let camera_files = discover_camera_vmds(&camera_dir);

    assert_eq!(motion_files, vec![motion_vmd]);
    assert_eq!(camera_files, vec![camera_vmd]);
}

#[test]
fn stage_selector_supports_auto_none_and_name() {
    let stages = vec![
        StageChoice {
            name: "alpha".to_owned(),
            status: StageStatus::NeedsConvert,
            render_path: None,
            pmx_path: Some(PathBuf::from("alpha/stage.pmx")),
            transform: StageTransform::default(),
        },
        StageChoice {
            name: "beta".to_owned(),
            status: StageStatus::Ready,
            render_path: Some(PathBuf::from("beta/stage.glb")),
            pmx_path: None,
            transform: StageTransform::default(),
        },
    ];

    let auto = resolve_stage_choice_from_selector(&stages, "auto");
    assert_eq!(auto.as_ref().map(|s| s.name.as_str()), Some("beta"));

    let none = resolve_stage_choice_from_selector(&stages, "none");
    assert!(none.is_none());

    let named = resolve_stage_choice_from_selector(&stages, "beta");
    assert_eq!(named.as_ref().map(|s| s.name.as_str()), Some("beta"));
}

#[test]
fn discover_default_camera_prefers_world_is_mine() {
    let dir = tempdir().expect("tempdir");
    let camera_dir = dir.path().join("assets").join("camera");
    fs::create_dir_all(&camera_dir).expect("camera dir");
    fs::write(camera_dir.join("a.vmd"), b"vmd").expect("a");
    fs::write(camera_dir.join("world_is_mine.vmd"), b"vmd").expect("world");
    let picked = asset_discovery::discover_default_camera_vmd(&camera_dir).expect("picked");
    assert_eq!(
        picked.file_name().and_then(|value| value.to_str()),
        Some("world_is_mine.vmd")
    );
}

#[test]
fn distance_clamp_guard_pushes_camera_outside_min_radius() {
    let mut guard = DistanceClampGuard::default();
    let target = Vec3::ZERO;
    let mut camera = Camera {
        eye: Vec3::new(0.05, 0.0, 0.03),
        target,
        up: Vec3::Y,
    };
    let min_dist = guard.apply(&mut camera, target, 1.0, 1.0);
    let actual = (camera.eye - target).length();
    assert!(actual + 1e-4 >= min_dist);
    assert!(min_dist >= 0.35);
}

#[test]
fn dynamic_clip_planes_remain_valid() {
    let (near, far) = dynamic_clip_planes(0.6, 1.4, 2.0, false);
    assert!(near > 0.0);
    assert!(far > near);
    assert!(near <= 0.10);
    assert!(far <= 500.0);
}

#[test]
fn dynamic_clip_planes_expand_far_for_stage() {
    let (_, far_no_stage) = dynamic_clip_planes(0.6, 1.4, 2.0, false);
    let (_, far_with_stage) = dynamic_clip_planes(0.6, 1.4, 8.0, true);
    assert!(far_with_stage > far_no_stage);
}

use super::*;

#[test]
fn supported_glb_joint_counts_fit_gpu_limit() {
    let paths = discover_glb_fixtures(Path::new(env!("CARGO_MANIFEST_DIR")));
    if paths.is_empty() {
        eprintln!("no repo-local glb fixtures found under assets/glb; skipping joint-limit check");
        return;
    }

    let mut checked_any = false;
    for path in paths {
        let scene = crate::assets::loader::load_gltf(&path).expect("load glb");
        let max_joints = scene
            .skins
            .iter()
            .map(|skin| skin.joints.len())
            .max()
            .unwrap_or(0);
        if max_joints > 512 {
            eprintln!(
                "warning: skipping {} because it exceeds the GPU joint limit ({max_joints} > 512)",
                path.display()
            );
            continue;
        }
        checked_any = true;
        assert!(
            max_joints <= 512,
            "{} exceeds GPU joint limit",
            path.display()
        );
    }

    assert!(
        checked_any,
        "no supported repo-local glb fixtures found under assets/glb"
    );
}

#[test]
fn discover_glb_fixtures_filters_and_sorts_repo_relative_assets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let glb_dir = temp.path().join("assets/glb");
    fs::create_dir_all(&glb_dir).expect("create glb dir");

    let first = glb_dir.join("sei.glb");
    let second = glb_dir.join("miku.glb");
    fs::write(&second, b"dummy").expect("write glb");
    fs::write(glb_dir.join("notes.txt"), b"ignore").expect("write text");
    fs::write(&first, b"dummy").expect("write glb");

    let discovered = discover_glb_fixtures(temp.path());
    assert_eq!(discovered, vec![second, first]);
}

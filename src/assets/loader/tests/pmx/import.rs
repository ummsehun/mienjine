use PMXUtil::pmx_types::{
    Encode, MorphTypes, PMXBone, PMXFace, PMXJoint, PMXJointType, PMXMaterial, PMXMorph, PMXRigid,
    PMXRigidCalcMethod, PMXRigidForm, PMXSphereModeRaw, PMXToonModeRaw, PMXVertex, PMXVertexWeight,
    UVMorph,
};
use PMXUtil::{pmx_loader::PMXLoader, pmx_writer::PMXWriter};
use tempfile::tempdir;

use super::super::super::load_pmx;
use super::super::common::write_minimal_pmx;

#[test]
fn load_pmx_smoke_imports_geometry_skinning_and_morphs() {
    let dir = tempdir().expect("tempdir");
    let pmx_path = dir.path().join("minimal.pmx");
    write_minimal_pmx(&pmx_path);

    let header = PMXLoader::open(&pmx_path).get_header();
    assert_eq!(header.magic, "PMX ");
    assert_eq!(header.version, 2.0);
    assert!(matches!(header.encode, Encode::UTF8));
    assert_eq!(header.additional_uv, 0);

    let scene = load_pmx(&pmx_path).expect("load pmx");
    assert_eq!(scene.meshes.len(), 1);
    assert_eq!(scene.total_triangles(), 1);
    assert_eq!(scene.total_vertices(), 3);
    assert_eq!(scene.skins.len(), 1);
    assert_eq!(scene.nodes.len(), 1);
    assert_eq!(scene.materials.len(), 1);
    assert_eq!(scene.textures.len(), 1);
    assert_eq!(scene.meshes[0].morph_targets.len(), 1);
    assert_eq!(scene.mesh_instances[0].default_morph_weights.len(), 1);
}

#[test]
fn load_pmx_preserves_bone_metadata() {
    let dir = tempdir().expect("tempdir");
    let pmx_path = dir.path().join("bones.pmx");
    let mut writer = PMXWriter::begin_writer(&pmx_path, false);
    writer.set_model_info(Some("BoneModel"), Some("BoneModel"), Some(""), Some(""));
    writer.set_additional_uv(0).expect("additional uv");

    writer.add_vertices(&[
        PMXVertex {
            position: [0.0, 0.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(1),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [0.0, 1.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(1),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [0.0, 2.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 2.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(1),
            edge_mag: 1.0,
        },
    ]);
    writer.add_faces(&[PMXFace {
        vertices: [0, 1, 2],
    }]);
    writer.add_textures(&[]);
    writer.add_materials(&[PMXMaterial {
        name: "Material".to_owned(),
        english_name: "Material".to_owned(),
        diffuse: [0.8, 0.8, 0.8, 1.0],
        specular: [0.0, 0.0, 0.0],
        specular_factor: 1.0,
        ambient: [0.2, 0.2, 0.2],
        draw_mode: 0,
        edge_color: [0.0, 0.0, 0.0, 1.0],
        edge_size: 1.0,
        texture_index: -1,
        sphere_mode_texture_index: -1,
        sphere_mode: PMXSphereModeRaw::None,
        toon_mode: PMXToonModeRaw::Separate,
        toon_texture_index: -1,
        memo: "".to_owned(),
        num_face_vertices: 3,
    }]);
    writer.add_bones(&[
        PMXBone {
            name: "Root".to_owned(),
            english_name: "Root".to_owned(),
            position: [0.0, 0.0, 0.0],
            parent: -1,
            deform_depth: 0,
            boneflag: 0x0100 | 0x0200 | 0x0400 | 0x0800 | 0x2000,
            offset: [0.0, 0.0, 0.0],
            child: 1,
            append_bone_index: 1,
            append_weight: 0.75,
            fixed_axis: [1.0, 0.0, 0.0],
            local_axis_x: [0.0, 1.0, 0.0],
            local_axis_z: [0.0, 0.0, 1.0],
            key_value: 7,
            ik_target_index: -1,
            ik_iter_count: 0,
            ik_limit: 0.0,
            ik_links: Vec::new(),
        },
        PMXBone {
            name: "Child".to_owned(),
            english_name: "Child".to_owned(),
            position: [0.0, 1.0, 0.0],
            parent: 0,
            deform_depth: 1,
            boneflag: 0,
            offset: [0.0, 1.0, 0.0],
            child: -1,
            append_bone_index: -1,
            append_weight: 0.0,
            fixed_axis: [0.0, 0.0, 0.0],
            local_axis_x: [0.0, 0.0, 0.0],
            local_axis_z: [0.0, 0.0, 0.0],
            key_value: 0,
            ik_target_index: -1,
            ik_iter_count: 0,
            ik_limit: 0.0,
            ik_links: Vec::new(),
        },
    ]);
    writer.add_morphs(&[]);
    writer.add_frames(&[]);
    writer.add_rigid_bodies(&[]);
    writer.add_joints(&[]);
    writer.write();

    let scene = load_pmx(&pmx_path).expect("load pmx with bone metadata");
    let meta = scene.pmx_rig_meta.as_ref().expect("rig meta");
    assert_eq!(meta.bones.len(), 2);
    assert_eq!(meta.count_bones_with_append(), 1);
    assert_eq!(meta.count_bones_with_grant(), 1);
    assert_eq!(meta.count_bones_with_local_grant(), 0);
    assert_eq!(meta.count_bones_with_fixed_axis(), 1);
    assert_eq!(meta.count_bones_with_local_axis(), 1);
    assert_eq!(meta.count_bones_with_external_parent(), 1);

    let root = &meta.bones[0];
    assert!(root.uses_append_rotation());
    assert!(root.uses_append_translation());
    assert!(root.uses_fixed_axis());
    assert!(root.uses_local_axis());
    assert!(root.uses_external_parent());
    assert_eq!(root.append_bone_index, 1);
    assert!((root.append_weight - 0.75).abs() < f32::EPSILON);
    let grant = root.grant_transform.as_ref().expect("grant transform");
    assert_eq!(grant.parent_index, 1);
    assert!((grant.weight - 0.75).abs() < f32::EPSILON);
    assert!(!grant.is_local);
    assert!(grant.affects_rotation);
    assert!(grant.affects_translation);
    assert_eq!(root.fixed_axis, glam::Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(root.local_axis_x, glam::Vec3::new(0.0, 1.0, 0.0));
    assert_eq!(root.local_axis_z, glam::Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(root.key_value, 7);
}

#[test]
fn load_pmx_preserves_local_grant_flag() {
    let dir = tempdir().expect("tempdir");
    let pmx_path = dir.path().join("local_grant.pmx");
    let mut writer = PMXWriter::begin_writer(&pmx_path, false);
    writer.set_model_info(Some("GrantModel"), Some("GrantModel"), Some(""), Some(""));
    writer.set_additional_uv(0).expect("additional uv");
    writer.add_vertices(&[
        PMXVertex {
            position: [0.0, 0.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(0),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [1.0, 0.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(0),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [0.0, 1.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(0),
            edge_mag: 1.0,
        },
    ]);
    writer.add_faces(&[PMXFace {
        vertices: [0, 1, 2],
    }]);
    writer.add_textures(&[]);
    writer.add_materials(&[PMXMaterial {
        name: "Material".to_owned(),
        english_name: "Material".to_owned(),
        diffuse: [1.0, 1.0, 1.0, 1.0],
        specular: [0.0, 0.0, 0.0],
        specular_factor: 1.0,
        ambient: [0.2, 0.2, 0.2],
        draw_mode: 0,
        edge_color: [0.0, 0.0, 0.0, 1.0],
        edge_size: 1.0,
        texture_index: -1,
        sphere_mode_texture_index: -1,
        sphere_mode: PMXSphereModeRaw::None,
        toon_mode: PMXToonModeRaw::Separate,
        toon_texture_index: -1,
        memo: "".to_owned(),
        num_face_vertices: 3,
    }]);
    writer.add_bones(&[
        PMXBone {
            name: "Parent".to_owned(),
            english_name: "Parent".to_owned(),
            position: [0.0, 0.0, 0.0],
            parent: -1,
            deform_depth: 0,
            boneflag: 0,
            offset: [0.0, 0.0, 0.0],
            child: 1,
            append_bone_index: -1,
            append_weight: 0.0,
            fixed_axis: [0.0, 0.0, 0.0],
            local_axis_x: [0.0, 0.0, 0.0],
            local_axis_z: [0.0, 0.0, 0.0],
            key_value: 0,
            ik_target_index: -1,
            ik_iter_count: 0,
            ik_limit: 0.0,
            ik_links: Vec::new(),
        },
        PMXBone {
            name: "Child".to_owned(),
            english_name: "Child".to_owned(),
            position: [0.0, 1.0, 0.0],
            parent: 0,
            deform_depth: 1,
            boneflag: 0x0100 | 0x0080,
            offset: [0.0, 1.0, 0.0],
            child: -1,
            append_bone_index: 0,
            append_weight: 0.4,
            fixed_axis: [0.0, 0.0, 0.0],
            local_axis_x: [0.0, 0.0, 0.0],
            local_axis_z: [0.0, 0.0, 0.0],
            key_value: 0,
            ik_target_index: -1,
            ik_iter_count: 0,
            ik_limit: 0.0,
            ik_links: Vec::new(),
        },
    ]);
    writer.add_morphs(&[]);
    writer.add_frames(&[]);
    writer.add_rigid_bodies(&[]);
    writer.add_joints(&[]);
    writer.write();

    let scene = load_pmx(&pmx_path).expect("load pmx with local grant");
    let meta = scene.pmx_rig_meta.as_ref().expect("rig meta");
    assert_eq!(meta.count_bones_with_grant(), 1);
    assert_eq!(meta.count_bones_with_local_grant(), 1);
    let grant = meta.bones[1]
        .grant_transform
        .as_ref()
        .expect("local grant transform");
    assert!(grant.is_local);
    assert!(grant.affects_rotation);
    assert!(!grant.affects_translation);
}

#[test]
fn load_pmx_imports_uv_morph_targets() {
    let dir = tempdir().expect("tempdir");
    let pmx_path = dir.path().join("uv_morph.pmx");
    let mut writer = PMXWriter::begin_writer(&pmx_path, false);
    writer.set_model_info(Some("UvModel"), Some("UvModel"), Some(""), Some(""));
    writer.set_additional_uv(0).expect("additional uv");

    writer.add_vertices(&[
        PMXVertex {
            position: [0.0, 0.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.2, 0.3],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(0),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [1.0, 0.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.4, 0.5],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(0),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [0.0, 1.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.6, 0.7],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(0),
            edge_mag: 1.0,
        },
    ]);
    writer.add_faces(&[PMXFace {
        vertices: [0, 1, 2],
    }]);
    writer.add_textures(&[]);
    writer.add_materials(&[PMXMaterial {
        name: "Material".to_owned(),
        english_name: "Material".to_owned(),
        diffuse: [0.8, 0.8, 0.8, 1.0],
        specular: [0.0, 0.0, 0.0],
        specular_factor: 1.0,
        ambient: [0.2, 0.2, 0.2],
        draw_mode: 0,
        edge_color: [0.0, 0.0, 0.0, 1.0],
        edge_size: 1.0,
        texture_index: -1,
        sphere_mode_texture_index: -1,
        sphere_mode: PMXSphereModeRaw::None,
        toon_mode: PMXToonModeRaw::Separate,
        toon_texture_index: -1,
        memo: "".to_owned(),
        num_face_vertices: 3,
    }]);
    writer.add_bones(&[PMXBone {
        name: "Root".to_owned(),
        english_name: "Root".to_owned(),
        position: [0.0, 0.0, 0.0],
        parent: -1,
        deform_depth: 0,
        boneflag: 0,
        offset: [0.0, 0.0, 0.0],
        child: -1,
        append_bone_index: -1,
        append_weight: 0.0,
        fixed_axis: [0.0, 0.0, 0.0],
        local_axis_x: [0.0, 0.0, 0.0],
        local_axis_z: [0.0, 0.0, 0.0],
        key_value: 0,
        ik_target_index: -1,
        ik_iter_count: 0,
        ik_limit: 0.0,
        ik_links: Vec::new(),
    }]);
    writer.add_morphs(&[PMXMorph {
        name: "UVShift".to_owned(),
        english_name: "UVShift".to_owned(),
        category: 3,
        morph_type: 3,
        offset: 0,
        morph_data: vec![MorphTypes::UV(UVMorph {
            index: 0,
            offset: [0.1, -0.05, 0.0, 0.0],
        })],
    }]);
    writer.add_frames(&[]);
    writer.add_rigid_bodies(&[]);
    writer.add_joints(&[]);
    writer.write();

    let scene = load_pmx(&pmx_path).expect("load pmx with uv morph");
    let mesh = &scene.meshes[0];
    assert_eq!(mesh.morph_targets.len(), 1);
    let morph = &mesh.morph_targets[0];
    assert!(
        morph
            .position_deltas
            .iter()
            .all(|delta| *delta == glam::Vec3::ZERO)
    );
    assert!(
        morph
            .normal_deltas
            .iter()
            .all(|delta| *delta == glam::Vec3::ZERO)
    );
    let uv0 = morph.uv0_deltas.as_ref().expect("uv0 deltas");
    assert_eq!(uv0[0], glam::Vec2::new(0.1, -0.05));
}

#[test]
fn load_pmx_preserves_physics_metadata() {
    let dir = tempdir().expect("tempdir");
    let pmx_path = dir.path().join("physics.pmx");
    let mut writer = PMXWriter::begin_writer(&pmx_path, false);
    writer.set_model_info(
        Some("PhysicsModel"),
        Some("PhysicsModel"),
        Some(""),
        Some(""),
    );
    writer.set_additional_uv(0).expect("additional uv");

    writer.add_vertices(&[
        PMXVertex {
            position: [0.0, 0.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(1),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [0.0, 1.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(1),
            edge_mag: 1.0,
        },
        PMXVertex {
            position: [0.0, 2.0, 0.0],
            norm: [0.0, 1.0, 0.0],
            uv: [0.0, 2.0],
            add_uv: [[0.0; 4]; 4],
            weight_type: PMXVertexWeight::BDEF1(1),
            edge_mag: 1.0,
        },
    ]);
    writer.add_faces(&[PMXFace {
        vertices: [0, 1, 2],
    }]);
    writer.add_textures(&[]);
    writer.add_materials(&[PMXMaterial {
        name: "Material".to_owned(),
        english_name: "Material".to_owned(),
        diffuse: [0.8, 0.8, 0.8, 1.0],
        specular: [0.0, 0.0, 0.0],
        specular_factor: 1.0,
        ambient: [0.2, 0.2, 0.2],
        draw_mode: 0,
        edge_color: [0.0, 0.0, 0.0, 1.0],
        edge_size: 1.0,
        texture_index: -1,
        sphere_mode_texture_index: -1,
        sphere_mode: PMXSphereModeRaw::None,
        toon_mode: PMXToonModeRaw::Separate,
        toon_texture_index: -1,
        memo: "".to_owned(),
        num_face_vertices: 3,
    }]);
    writer.add_bones(&[
        PMXBone {
            name: "Root".to_owned(),
            english_name: "Root".to_owned(),
            position: [0.0, 0.0, 0.0],
            parent: -1,
            deform_depth: 0,
            boneflag: 0,
            offset: [0.0, 0.0, 0.0],
            child: 1,
            append_bone_index: -1,
            append_weight: 0.0,
            fixed_axis: [0.0, 0.0, 0.0],
            local_axis_x: [0.0, 0.0, 0.0],
            local_axis_z: [0.0, 0.0, 0.0],
            key_value: 0,
            ik_target_index: -1,
            ik_iter_count: 0,
            ik_limit: 0.0,
            ik_links: Vec::new(),
        },
        PMXBone {
            name: "Hair".to_owned(),
            english_name: "Hair".to_owned(),
            position: [0.0, 1.0, 0.0],
            parent: 0,
            deform_depth: 1,
            boneflag: 0,
            offset: [0.0, 1.0, 0.0],
            child: -1,
            append_bone_index: -1,
            append_weight: 0.0,
            fixed_axis: [0.0, 0.0, 0.0],
            local_axis_x: [0.0, 0.0, 0.0],
            local_axis_z: [0.0, 0.0, 0.0],
            key_value: 0,
            ik_target_index: -1,
            ik_iter_count: 0,
            ik_limit: 0.0,
            ik_links: Vec::new(),
        },
    ]);
    writer.add_morphs(&[]);
    writer.add_frames(&[]);
    writer.add_rigid_bodies(&[
        PMXRigid {
            name: "RigidRoot".to_owned(),
            name_en: "RigidRoot".to_owned(),
            bone_index: 0,
            group: 0,
            un_collision_group_flag: 0,
            form: PMXRigidForm::Sphere,
            size: [0.1, 0.1, 0.1],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            mass: 0.0,
            move_resist: 0.0,
            rotation_resist: 0.0,
            repulsion: 0.0,
            friction: 0.0,
            calc_method: PMXRigidCalcMethod::Static,
        },
        PMXRigid {
            name: "RigidHair".to_owned(),
            name_en: "RigidHair".to_owned(),
            bone_index: 1,
            group: 0,
            un_collision_group_flag: 0,
            form: PMXRigidForm::Sphere,
            size: [0.1, 0.1, 0.1],
            position: [0.0, 1.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            mass: 1.0,
            move_resist: 0.0,
            rotation_resist: 0.0,
            repulsion: 0.0,
            friction: 0.0,
            calc_method: PMXRigidCalcMethod::Dynamic,
        },
    ]);
    writer.add_joints(&[PMXJoint {
        name: "Joint".to_owned(),
        name_en: "Joint".to_owned(),
        joint_type: PMXJointType::Spring6DOF {
            a_rigid_index: 0,
            b_rigid_index: 1,
            position: [0.0, 0.5, 0.0],
            rotation: [0.0, 0.0, 0.0],
            move_limit_down: [-0.1, -0.1, -0.1],
            move_limit_up: [0.1, 0.1, 0.1],
            rotation_limit_down: [-0.1, -0.1, -0.1],
            rotation_limit_up: [0.1, 0.1, 0.1],
            spring_const_move: [0.0, 0.0, 0.0],
            spring_const_rotation: [0.0, 0.0, 0.0],
        },
    }]);
    writer.write();

    let scene = load_pmx(&pmx_path).expect("load pmx with physics");
    assert_eq!(
        scene
            .pmx_physics_meta
            .as_ref()
            .map(|meta| meta.rigid_bodies.len()),
        Some(2)
    );
    assert_eq!(
        scene
            .pmx_physics_meta
            .as_ref()
            .map(|meta| meta.joints.len()),
        Some(1)
    );
}

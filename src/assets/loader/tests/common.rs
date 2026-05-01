use PMXUtil::pmx_types::{
    BoneMorph, GroupMorph, MaterialMorph, MorphTypes, PMXBone, PMXFace, PMXMaterial, PMXMorph,
    PMXSphereModeRaw, PMXToonModeRaw, PMXVertex, PMXVertexWeight, VertexMorph,
};
use PMXUtil::pmx_writer::PMXWriter;

pub(super) fn write_minimal_pmx(path: &std::path::Path) {
    let mut writer = PMXWriter::begin_writer(path, false);
    writer.set_model_info(Some("TestModel"), Some("TestModel"), Some(""), Some(""));
    writer.set_additional_uv(0).expect("additional uv");

    let vertices = [
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
    ];
    writer.add_vertices(&vertices);
    writer.add_faces(&[PMXFace {
        vertices: [0, 1, 2],
    }]);
    writer.add_textures(&["texture.png".to_owned()]);
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
        texture_index: 0,
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
    writer.add_morphs(&[
        PMXMorph {
            name: "VertexMorph".to_owned(),
            english_name: "VertexMorph".to_owned(),
            category: 1,
            morph_type: 1,
            offset: 1,
            morph_data: vec![MorphTypes::Vertex(VertexMorph {
                index: 0,
                offset: [0.0, 0.05, 0.0],
            })],
        },
        PMXMorph {
            name: "BoneMorph".to_owned(),
            english_name: "BoneMorph".to_owned(),
            category: 2,
            morph_type: 2,
            offset: 1,
            morph_data: vec![MorphTypes::Bone(BoneMorph {
                index: 0,
                translates: [0.0, 0.0, 0.0],
                rotates: [0.0, 0.0, 0.0, 1.0],
            })],
        },
        PMXMorph {
            name: "GroupMorph".to_owned(),
            english_name: "GroupMorph".to_owned(),
            category: 3,
            morph_type: 0,
            offset: 1,
            morph_data: vec![MorphTypes::Group(GroupMorph {
                index: 0,
                morph_factor: 1.0,
            })],
        },
        PMXMorph {
            name: "MaterialMorph".to_owned(),
            english_name: "MaterialMorph".to_owned(),
            category: 4,
            morph_type: 8,
            offset: 1,
            morph_data: vec![MorphTypes::Material(MaterialMorph {
                index: 0,
                formula: 0,
                diffuse: [0.0, 0.0, 0.0, 0.0],
                specular: [0.0, 0.0, 0.0],
                specular_factor: 0.0,
                ambient: [0.0, 0.0, 0.0],
                edge_color: [0.0, 0.0, 0.0, 0.0],
                edge_size: 0.0,
                texture_factor: [0.0, 0.0, 0.0, 0.0],
                sphere_texture_factor: [0.0, 0.0, 0.0, 0.0],
                toon_texture_factor: [0.0, 0.0, 0.0, 0.0],
            })],
        },
    ]);
    writer.add_frames(&[]);
    writer.add_rigid_bodies(&[]);
    writer.add_joints(&[]);
    writer.write();
}

pub(super) fn push_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

pub(super) fn push_u16s(out: &mut Vec<u8>, values: &[u16]) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

pub(super) fn push_u8s(out: &mut Vec<u8>, values: &[u8]) {
    out.extend_from_slice(values);
}

pub(super) fn push_padding(out: &mut Vec<u8>, align: usize) {
    let rem = out.len() % align;
    if rem == 0 {
        return;
    }
    let pad = align - rem;
    out.extend(std::iter::repeat_n(0_u8, pad));
}

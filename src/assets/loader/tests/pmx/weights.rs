use super::super::super::pmx_support::convert_vertex_weight;
use crate::scene::SdefVertexCpu;
use PMXUtil::pmx_types::PMXVertexWeight;

#[test]
fn convert_vertex_weight_bdef1() {
    let weight = PMXVertexWeight::BDEF1(5);
    let (joints, weights, sdef): ([u16; 4], [f32; 4], Option<SdefVertexCpu>) =
        convert_vertex_weight(&weight);
    assert_eq!(joints, [5, 0, 0, 0]);
    assert_eq!(weights, [1.0, 0.0, 0.0, 0.0]);
    assert!(sdef.is_none());
}

#[test]
fn convert_vertex_weight_bdef2() {
    let weight = PMXVertexWeight::BDEF2 {
        bone_index_1: 1,
        bone_index_2: 2,
        bone_weight_1: 0.75,
    };
    let (joints, weights, sdef): ([u16; 4], [f32; 4], Option<SdefVertexCpu>) =
        convert_vertex_weight(&weight);
    assert_eq!(joints, [1, 2, 0, 0]);
    assert!((weights[0] - 0.75).abs() < 1e-6);
    assert!((weights[1] - 0.25).abs() < 1e-6);
    assert!(sdef.is_none());
}

#[test]
fn convert_vertex_weight_bdef4() {
    let weight = PMXVertexWeight::BDEF4 {
        bone_index_1: 0,
        bone_index_2: 1,
        bone_index_3: 2,
        bone_index_4: 3,
        bone_weight_1: 0.5,
        bone_weight_2: 0.3,
        bone_weight_3: 0.15,
        bone_weight_4: 0.05,
    };
    let (joints, weights, sdef): ([u16; 4], [f32; 4], Option<SdefVertexCpu>) =
        convert_vertex_weight(&weight);
    assert_eq!(joints, [0, 1, 2, 3]);
    assert!((weights[0] - 0.5).abs() < 1e-6);
    assert!((weights[1] - 0.3).abs() < 1e-6);
    assert!((weights[2] - 0.15).abs() < 1e-6);
    assert!((weights[3] - 0.05).abs() < 1e-6);
    assert!(sdef.is_none());
}

#[test]
fn convert_vertex_weight_sdef_preserves_aux_data() {
    let weight = PMXVertexWeight::SDEF {
        bone_index_1: 1,
        bone_index_2: 2,
        bone_weight_1: 0.7,
        sdef_c: [0.1, 0.2, 0.3],
        sdef_r0: [0.4, 0.5, 0.6],
        sdef_r1: [0.7, 0.8, 0.9],
    };
    let (joints, weights, sdef): ([u16; 4], [f32; 4], Option<SdefVertexCpu>) =
        convert_vertex_weight(&weight);
    assert_eq!(joints, [1, 2, 0, 0]);
    assert!((weights[0] - 0.7).abs() < 1e-6);
    assert!((weights[1] - 0.3).abs() < 1e-6);
    let sdef = sdef.expect("sdef data");
    assert_eq!(sdef.bone_index_1, 1);
    assert_eq!(sdef.bone_index_2, 2);
    assert!((sdef.bone_weight_1 - 0.7).abs() < 1e-6);
    assert!((sdef.c - glam::Vec3::new(0.1, 0.2, 0.3)).length() < 1e-6);
    assert!((sdef.r0 - glam::Vec3::new(0.4, 0.5, 0.6)).length() < 1e-6);
    assert!((sdef.r1 - glam::Vec3::new(0.7, 0.8, 0.9)).length() < 1e-6);
}

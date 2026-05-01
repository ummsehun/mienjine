use glam::{Quat, Vec3};

use crate::scene::Node;

use super::super::util::find_root_center_node;

#[test]
fn root_center_prefers_center_over_tip_like_nodes() {
    let nodes = vec![
        Node {
            name: Some("センター先".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        },
        Node {
            name: Some("センター".to_owned()),
            name_en: None,
            parent: None,
            children: Vec::new(),
            base_translation: Vec3::ZERO,
            base_rotation: Quat::IDENTITY,
            base_scale: Vec3::ONE,
        },
    ];
    let picked = find_root_center_node(&nodes).expect("root");
    assert_eq!(picked, 1);
}

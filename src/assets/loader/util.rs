use glam::Vec3;

use crate::scene::Node;

pub(super) fn compute_vertex_normals(positions: &[Vec3], indices: &[[u32; 3]]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for index in indices {
        let (Some(a), Some(b), Some(c)) = (
            positions.get(index[0] as usize),
            positions.get(index[1] as usize),
            positions.get(index[2] as usize),
        ) else {
            continue;
        };
        let n = (*b - *a).cross(*c - *a);
        normals[index[0] as usize] += n;
        normals[index[1] as usize] += n;
        normals[index[2] as usize] += n;
    }
    normals
        .into_iter()
        .map(|n| {
            if n.length_squared() <= f32::EPSILON {
                Vec3::new(0.0, 1.0, 0.0)
            } else {
                n.normalize()
            }
        })
        .collect()
}

pub(super) fn find_root_center_node(nodes: &[Node]) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;
    for (index, node) in nodes.iter().enumerate() {
        let Some(name) = node.name.as_deref() else {
            continue;
        };
        let score = root_name_score(name);
        if score <= 0 {
            continue;
        }
        match best {
            Some((_, best_score)) if best_score >= score => {}
            _ => best = Some((index, score)),
        }
    }
    best.map(|(index, _)| index)
}

pub(super) fn root_name_score(name: &str) -> i32 {
    let lower = name.to_ascii_lowercase();
    let is_tip_like = lower.contains("tip")
        || lower.contains("end")
        || lower.contains("target")
        || name.contains("先");

    let mut score = 0_i32;
    if lower == "hips" || lower == "pelvis" {
        score = score.max(140);
    }
    if lower == "root" {
        score = score.max(130);
    }
    if lower == "center" || lower == "centre" || name == "センター" || name == "センタ" {
        score = score.max(120);
    }

    if lower.contains("hips") || lower.contains("pelvis") {
        score = score.max(110);
    }
    if lower.contains("root") || lower.contains("center") || lower.contains("centre") {
        score = score.max(100);
    }
    if name.contains("センター") || name.contains("センタ") {
        score = score.max(95);
    }

    if is_tip_like {
        score -= 80;
    }

    score
}

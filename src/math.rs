use glam::{Mat4, Vec2};

pub fn perspective_matrix(fov_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    Mat4::perspective_rh(fov_deg.to_radians(), aspect, near, far)
}

pub fn barycentric(point: Vec2, a: Vec2, b: Vec2, c: Vec2) -> Option<[f32; 3]> {
    let area = edge(a, b, c);
    if area.abs() < 1e-8 {
        return None;
    }
    let w0 = edge(b, c, point) / area;
    let w1 = edge(c, a, point) / area;
    let w2 = edge(a, b, point) / area;
    Some([w0, w1, w2])
}

pub fn depth_less(current_depth: f32, candidate_depth: f32) -> bool {
    candidate_depth < current_depth
}

fn edge(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3};

    use super::*;

    #[test]
    fn barycentric_inside_triangle() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(2.0, 0.0);
        let c = Vec2::new(0.0, 2.0);
        let p = Vec2::new(0.5, 0.5);
        let bc = barycentric(p, a, b, c).expect("non-degenerate triangle");
        let sum = bc[0] + bc[1] + bc[2];
        assert!((sum - 1.0).abs() < 1e-5);
        assert!(bc.iter().all(|v| *v >= 0.0));
    }

    #[test]
    fn barycentric_outside_triangle() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(2.0, 0.0);
        let c = Vec2::new(0.0, 2.0);
        let p = Vec2::new(2.0, 2.0);
        let bc = barycentric(p, a, b, c).expect("non-degenerate triangle");
        assert!(bc.iter().any(|v| *v < 0.0));
    }

    #[test]
    fn depth_compare_prefers_nearer() {
        assert!(depth_less(0.5, 0.2));
        assert!(!depth_less(0.2, 0.5));
    }

    #[test]
    fn perspective_matrix_maps_forward_point() {
        let proj = perspective_matrix(60.0, 1.0, 0.1, 100.0);
        let clip = proj * Vec3::new(0.0, 0.0, -1.0).extend(1.0);
        assert!(clip.w.abs() > 1e-6);
    }
}

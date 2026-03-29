//! Shading and contrast parameters for the renderer.

use glam::Vec3;

use crate::scene::{ClarityProfile, ContrastProfile, RenderConfig};

/// Parameters controlling lighting calculations.
#[derive(Debug, Clone, Copy)]
pub struct ShadingParams {
    pub light_dir: Vec3,
    pub camera_pos: Vec3,
    pub ambient: f32,
    pub diffuse_strength: f32,
    pub specular_strength: f32,
    pub specular_power: f32,
    pub rim_strength: f32,
    pub rim_power: f32,
    pub fog_strength: f32,
}

/// Parameters controlling contrast/tone mapping.
#[derive(Debug, Clone, Copy)]
pub struct ContrastParams {
    pub floor: f32,
    pub gamma: f32,
    pub fog_scale: f32,
}

/// Computes the lighting intensity at a given point with the given normal.
pub fn shade_lighting(normal: Vec3, world_pos: Vec3, shading: ShadingParams) -> f32 {
    let n = normal.normalize_or_zero();
    let l = shading.light_dir;
    let v = (shading.camera_pos - world_pos).normalize_or_zero();
    let h = (l + v).normalize_or_zero();

    let diffuse = n.dot(l).max(0.0) * shading.diffuse_strength;
    let specular = if shading.specular_strength > 0.0 {
        n.dot(h).max(0.0).powf(shading.specular_power) * shading.specular_strength
    } else {
        0.0
    };
    let rim = if shading.rim_strength > 0.0 {
        (1.0 - n.dot(v).max(0.0)).powf(shading.rim_power) * shading.rim_strength
    } else {
        0.0
    };

    let lit = shading.ambient + diffuse + specular + rim;
    lit.clamp(0.0, 1.0)
}

/// Computes contrast parameters based on config and cell count.
pub fn contrast_params(config: &RenderConfig, cells: usize) -> ContrastParams {
    let (clarity_floor_mul, clarity_gamma_mul, clarity_fog_mul) = match config.clarity_profile {
        ClarityProfile::Balanced => (1.0, 1.0, 1.0),
        ClarityProfile::Sharp => (1.12, 0.92, 0.92),
        ClarityProfile::Extreme => (1.24, 0.86, 0.84),
    };
    match config.contrast_profile {
        ContrastProfile::Fixed => ContrastParams {
            floor: (config.contrast_floor * clarity_floor_mul).clamp(0.0, 0.45),
            gamma: (config.contrast_gamma * clarity_gamma_mul).clamp(0.50, 1.35),
            fog_scale: (config.fog_scale * clarity_fog_mul).clamp(0.20, 1.5),
        },
        ContrastProfile::Adaptive => {
            let (bucket_floor, bucket_gamma, bucket_fog) = if cells < 6_000 {
                (0.18, 0.78, 0.55)
            } else if cells < 12_000 {
                (0.12, 0.86, 0.75)
            } else {
                (0.08, 0.92, 1.00)
            };
            let floor_scale = (config.contrast_floor / 0.10).clamp(0.5, 2.0);
            let gamma_scale = (config.contrast_gamma / 0.90).clamp(0.6, 1.5);
            ContrastParams {
                floor: (bucket_floor * floor_scale * clarity_floor_mul).clamp(0.02, 0.42),
                gamma: (bucket_gamma * gamma_scale * clarity_gamma_mul).clamp(0.50, 1.20),
                fog_scale: (bucket_fog * config.fog_scale.clamp(0.25, 1.5) * clarity_fog_mul)
                    .clamp(0.20, 1.5),
            }
        }
    }
}

use crate::scene::ClarityProfile;

pub(super) fn push_histogram(histogram: &mut [u32; 64], count: &mut u32, value: f32) {
    let v = value.clamp(0.0, 1.0);
    let idx = ((v * ((histogram.len() - 1) as f32)).round() as usize).min(histogram.len() - 1);
    histogram[idx] = histogram[idx].saturating_add(1);
    *count = count.saturating_add(1);
}

pub(super) fn percentile_from_histogram(histogram: &[u32; 64], count: u32, q: f32) -> f32 {
    if count == 0 {
        return 0.5;
    }
    let target = ((count as f32) * q.clamp(0.0, 1.0)).ceil() as u32;
    let mut acc = 0_u32;
    for (i, bin) in histogram.iter().enumerate() {
        acc = acc.saturating_add(*bin);
        if acc >= target {
            return (i as f32) / ((histogram.len() - 1) as f32);
        }
    }
    1.0
}

pub(super) fn update_exposure_from_histogram(
    exposure: &mut f32,
    histogram: &[u32; 64],
    count: u32,
    clarity: ClarityProfile,
) {
    if count == 0 {
        return;
    }
    let p75 = percentile_from_histogram(histogram, count, 0.75).max(1e-3);
    let desired_mid = match clarity {
        ClarityProfile::Balanced => 0.52,
        ClarityProfile::Sharp => 0.58,
        ClarityProfile::Extreme => 0.64,
    };
    let target = (desired_mid / p75).clamp(0.50, 3.2);
    *exposure = (*exposure + (target - *exposure) * 0.14).clamp(0.28, 3.8);
}

pub(super) fn tone_map_intensity(raw: f32, floor: f32, gamma: f32, exposure: f32) -> f32 {
    let boosted = (raw.clamp(0.0, 1.0) * exposure).clamp(0.0, 1.4);
    let mapped = floor + (1.0 - floor) * boosted.clamp(0.0, 1.0).powf(gamma);
    mapped.clamp(0.0, 1.0)
}

pub fn exposure_bias_multiplier(bias: f32) -> f32 {
    let clamped = bias.clamp(-0.5, 0.8);
    (2.0_f32).powf(clamped).clamp(0.70, 1.80)
}

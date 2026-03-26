# GPU/CPU Parity Phase 1 Baseline

Date: 2026-03-25

## Scope

Captured parity baseline metrics for three representative material scenarios in test:

- `opaque_uv_transform`
- `mask_cutoff`
- `blend_alpha`

Test command:

```bash
cargo test --features gpu render::backend::tests::gpu_parity_phase1_baseline_report -- --nocapture
```

## Baseline Metrics

| Case | glyph_mismatch_ratio | mean_rgb_abs_error | max_rgb_abs_error | visible_ratio_delta |
|---|---:|---:|---:|---:|
| opaque_uv_transform | 0.319545 | 51.821968 | 149 | 0.000000 |
| mask_cutoff | 0.202273 | 53.598106 | 149 | 0.000000 |
| blend_alpha | 0.250000 | 51.821968 | 149 | 0.000000 |

## Notes

- `visible_ratio_delta` is already aligned for these scenarios.
- Major remaining gaps are visual parity (`glyph_mismatch_ratio`, `mean_rgb_abs_error`) and should be targeted in Phase 2/3.
- Baseline is intentionally recorded before additional blend/color-space parity passes.

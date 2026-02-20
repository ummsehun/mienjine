# Asset Preparation (Not Committed)

This project does **not** ship model/motion assets.

Use your own local files and keep them outside git tracking:

- Place converted output at `assets-local/*.glb` (or any local path).
- Keep PMX/VMD source files local-only.
- Do not commit restricted assets to the public repository.

## Expected Runtime Inputs

- `terminal-miku3d run --glb /absolute/path/to/model.glb`
- `terminal-miku3d inspect --glb /absolute/path/to/model.glb`
- `terminal-miku3d bench --scene glb-anim --glb /absolute/path/to/model.glb`

## Notes

- Runtime supports GLB/glTF only.
- PMX/VMD conversion is an offline preprocessing step.

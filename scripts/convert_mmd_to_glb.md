# PMX/VMD -> GLB Conversion Guide (Blender + MMD Tools)

This project's runtime is pure Rust and loads GLB/glTF only.
PMX/VMD conversion is done offline with Blender.

## 1. Install Tools

1. Install Blender (3.x+).
2. Install MMD Tools add-on in Blender.
3. Enable glTF 2.0 exporter (bundled with Blender).

## 2. Import Model and Motion

1. Open Blender.
2. Import PMX model via MMD Tools.
3. Import VMD motion and bind it to the model armature.
4. Scrub the timeline and confirm motion plays.

## 3. Bake Animation

1. Select armature and mesh objects.
2. Bake animation to keyframes (visual keying on).
3. Ensure transforms are clean and scale is correct.

## 4. Export GLB

1. File -> Export -> glTF 2.0.
2. Format: `glTF Binary (.glb)`.
3. Include -> Animation: enabled.
4. Skinning: enabled.
5. Export to a local path (example: `assets-local/miku_dance.glb`).

## 5. Validate in Terminal Miku 3D

```bash
cargo run -- inspect --glb assets-local/miku_dance.glb
cargo run -- run --glb assets-local/miku_dance.glb --anim 0 --mode ascii --fps-cap 30
```

## 6. Troubleshooting

- If rig deforms incorrectly, re-bake animation and re-export.
- If animation list is empty, check exporter animation options.
- If performance is low, decimate mesh and reduce animation complexity.

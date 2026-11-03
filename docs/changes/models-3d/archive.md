# Archive: models-3d — DONE (parse-one, render-one)

**Stage banner: DONE — one loose `.SRM` parses and renders.**

The SR1-remaster `.SRM` layout was confirmed byte-exact on real files (magic
`SRM\x01`, 44-byte shader records, 32-byte material records, bone block, 128 flag
bytes, 32-byte vertices, `u16` triangles). `--mesh <FILE.SRM>` parses positions
and triangle indices only and draws a flat-shaded, auto-rotating solid with
orbit/zoom/pan and a fly camera. Playtest file: 86 vertices / 56 triangles.
Evidence: `docs/changes/models-3d/` (proposal, design, spec, tasks).

## What landed
- PR #12 (`feature/models-3d`, merge `d60209e`): `5d427ca` SRM parser, `5a3a7eb`
  mesh pipeline and `--mesh`, `954870f` docs, `6356887` projection fix.
- PR #13 (`Visor-flesh-3d-galeria`, merge `9ee1c81`) added textured meshes and a
  `.SRM` gallery (`--mesh-textured`, `--mesh-all`).

## What stayed UNKNOWN
- Skeleton/animation (the bone block was decoded later; hierarchy is absent).
- UVs, materials and textures in the base `--mesh` path (the textured path has
  them), plus room meshes and multi-mesh browsing in the base viewer.

## Next step
Bone decode / one-bone pose landed in the `animation` slice; full-body animation
remains blocked because `.SRM` has no parent/child relation.

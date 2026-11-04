# u32-indices - widen mesh index buffers so large level groups render

Date: 2026-11-04. Behaviour/format facts only. No game bytes, no decoded
imagery, no absolute install paths.

This pass builds on `docs/notes/level-geometry.md` (face arrays),
`docs/notes/room-textures.md` (per-face expansion) and
`docs/notes/object-models.md` (object polygons).

## Verdict

- **On-disk face indices stay `u16`.** The `.PCM` face array and the `.SRM`
  face stream are still 16-bit values; only the in-memory/index-buffer type
  changed.
- **The renderer now uses `u32` end to end.** `IndexFormat::Uint32` for both
  the flat and textured pipelines, and every producer, merger and upload stage
  carries `u32`. The on-disk `u16` is widened at parse time.
- **The old ceiling was per-face expansion, not the source data.** Textured
  rooms and objects emit three unique vertices per face, tripling vertex
  counts. A group that fits in `u16` source indices can still exceed 65,535
  expanded vertices, which is what the removed guard reported.
- **The `u16` range guards are gone.** With `u32` the practical ceiling is far
  beyond any observed group, so the unreachable `RenderTooLarge` /
  `GroupTooLarge` errors and their `16-bit` messages were deleted rather than
  rewritten.
- **Behaviour below the old ceiling is unchanged.** Same vertices, faces,
  pages, texture counts and output lines.

## Verified on real data

Pre-change (`main`), the textured group path refused with:

```
error: merged level group exceeds the 16-bit vertex index range
```

Post-change, the window opens and the summary line prints:

| group | vertices | faces | pages | textured | unresolved |
|-------|---------:|------:|------:|---------:|-----------:|
| cathy | 285,936 | 95,312 | 178 | 95,108 | 204 |
| aluka | 212,151 | 70,717 | 76 | 70,589 | 128 |

Both exceed 65,535 expanded vertices, so neither could render under `u16`.

The `under --objects --textures` counts are identical before and after:

```
objects 132 (10 meshes, 122 markers)
level group under vertices=25555 faces=9657 pages=17 textured_faces=5068 unresolved_faces=8
```

## Tests

`cargo test`: 313 passed. The regression test
`textured_room_mesh_reaches_past_the_u16_vertex_range` expands a synthetic
room to 66,000 vertices and asserts index values above `u16::MAX`.

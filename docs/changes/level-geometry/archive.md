# Archive: level-geometry — DONE (geometry CONFIRMED)

**Stage banner: DONE — room geometry is confirmed; UVs and orientation are not.**

Header slot `0x00` is the room record. `+0x10/+0x1C` locate a 12-byte vertex
array (three `i16` xyz at `+0/2/4`); `+0x14/+0x20` locate a 12-byte face array
whose first three `u16` are vertex indices. Across 89 units and 11 rooms the
corpus is 78,020 vertices / 139,877 faces, zero out-of-range indices, and
`max_index = vertices - 1` in every unit. Slot `0x03` is refuted as geometry.
Evidence: `docs/notes/level-geometry.md`.

## What landed
- PR #60 (`docs/level-geometry`, merge `8a0ca35`, commit `edb6e34`) — the
  geometry and placement field map.
- PR #61 (`feature/level-geometry`, merge `164e7b2`, commits `5987617`,
  `f1deffc`, `aff9e33`) — `--level-geometry` counts, box and index validation.
- PR #62 (`feature/level-render`, merge `6ab7c40`) — one unit through `show_mesh`.
- PR #66 (`feature/level-objects`, merge `e818214`) — object listing/markers.

## What stayed UNKNOWN
- The trailing vertex triple (not the face normal), the 8-byte array C, and the
  face attribute words.
- The draw submission that turns the arrays into GPU buffers.
- Object orientation (no rotation field; `+0x18` is a cross-level offset).
- The dominant 143,872-byte block, the room-local group records, and slots
  `0x02/0x03`.

## Next step
Trace the renderer submission and the spawner consumer; then name array C.

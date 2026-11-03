# Proposal: level-geometry (outcome: geometry and placement located; docs-only + one reader proposed)

## Intent

Answer where room geometry (vertices/indices) and object placement live in the
`.PCM` level data, and how the engine consumes them. Four byte-level hypotheses
had already failed, so the binary was treated as the source of truth.

## Outcome

**Geometry is CONFIRMED behind header slot `0x00`, not slot `0x03`.** The room
record exposes two 12-byte arrays: a vertex array (three `i16` position at
`+0/2/4`, plus a trailing triple whose role is UNKNOWN) and a face array whose
first three `u16` are vertex indices. The 8-byte array C stays UNKNOWN. Across
89 units the face array holds 139,877 faces and every index is in
`0..vertices-1` (0 out-of-range, 0 repeated-index triangles).

**Slot `0x03` is refuted as the geometry payload.** It is the offset of a
`0x3C`-stride record array counted by slot `0x02` (counts 0..12). Its large
values (179,324 in `LAIR10`, 191,296 in `UNDRCT9`) are positions near the end of
the payload, not payload sizes.

**Placement is data-driven and CONFIRMED at field level.** The room object array
(`+0x08` count, `+0x0C` offset, `0x4C` source) becomes a `0x60`-byte runtime
array at level `+0xD0` (count at `+0xC8`). Each record starts with a 12-byte
name and carries a 3 x `i16` world position at `+0x20`. No rotation matrix,
quaternion or angle pair exists in the record; `+0x18` is a cross-level offset.
Where a non-identity orientation comes from is UNKNOWN.

**The 40-byte class is not reused for objects.** Objects are `0x4C -> 0x60`.
Two unrelated 40-byte families exist: link records (slot `0x34`) and the
36 -> 40 runtime records (slot `0x3A/0x3B`).

**The 36-byte table is not objects.** It appears in 7/89 units and looks like a
colour/parameter table (sequential index, constant `0xFF00FF00` /
`0x00FF0000` / `0x0000FF00` masks, tagged `0xBE 0xEF` blob). Field meaning is
partial.

Full evidence: `docs/notes/level-geometry.md` (ledger G1-G8, P1-P5, B1-B4, L1;
UNKNOWN list; per-unit numbers).

## What changed versus the prior notes

- `level-builder` B13: the i16 -> f32 triplet sub-builder is the
  animation/sub-model path, not the room vertex array.
- `room-geometry` Candidate A: the 12-byte stride is real, but there are two
  12-byte arrays (vertices and texture/descriptors); neither is the 143,872-byte
  dominant block, which stays UNKNOWN.
- `pcm-loader` L16 / level-builder UNKNOWN: placement position is now a concrete
  field (`+0x20`), and orientation is a concrete negative (not in the record).
- The level-wide 36-byte table (slot `0x3A`) and the room-local 36-byte table
  (`+0x44`, BSP groups) are now separated.

## Proposed next slice (counts-only reader)

`--level-geometry <.PCM file | room directory>` prints, per unit:

1. vertex count, face count, C count, object count;
2. vertex stride `0x0C`, index width `u16`, index min/max;
3. validation result: every index in `0..vertices-1` (out-of-range is loud,
   exit 2);
4. level-wide 36-byte table count (0 for most units).

No coordinates, no game bytes, no mesh export. The reader projects fields that
are already CONFIRMED; it does not interpret the trailing vertex triple, array
C, or object orientation.

## Forecast split

| Part | Lines |
|---|---:|
| Production (`src/level.rs` fields + validation, CLI arm) | ~130 |
| Tests (synthetic header/array/index cases, refusals) x1.5 | ~195 |
| Docs (note update + README line) | ~35 |
| **TOTAL** | **~360** |

## PR plan

- **This change:** one docs-only PR, branch `docs/level-geometry`, base `main`
  (note + this proposal + spec + tasks; README note line). No code.
- **Next change:** one single PR for the counts-only reader (~360 lines), based
  on `main` after this PR merges. No chain needed; the slice is inside the
  400-line budget.

## No-go recorded here

- Mesh rendering / GPU submission: not traced. The draw path that consumes the
  vertex array is UNKNOWN.
- Object orientation: not in the record; final transform composition UNKNOWN.
- Trailing vertex triple, array C, sub-object 24 bytes, level-wide 36-byte
  table semantics: UNKNOWN.
- Dominant 143,872-byte block: UNKNOWN.

## Next probes if the slice is not funded

1. Renderer submission (reader of the room vertex/face pointers and of the
   `0x40`-byte BSP node tree).
2. Per-frame spawner (consumer of level `+0xD0`; check whether `+0x18` is added
   to `+0x20`).
3. Array C consumer and the room-local 36-byte group records.
4. Level-wide 36-byte table consumer (7 units are enough to falsify the lighting
   hypothesis).

## Non-goals

Game bytes in the repo, addresses, decompiled listings, coordinate dumps, mesh
export, transform/placement decode, rendering, collision.

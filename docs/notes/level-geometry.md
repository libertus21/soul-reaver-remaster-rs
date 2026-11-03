# level-geometry - where `.PCM` room geometry and object placement live

Bounded, read-only Ghidra 12.1.4 pass on the SR1 remaster binary (`1\sr1.dll`,
AMD64). Behaviour notes only: role names, field offsets, unit names and counts.
No addresses, no disassembly, no decompiled text, no game bytes, no absolute
install paths. Ghidra project, scripts and decompiler output lived in `%TEMP%`
only.

Date: 2026-11-03. Box: three passes (re-pin, builder walk, corpus cross-checks),
inside the ~3 h budget. Evidence class per claim: **CONFIRMED** (code and real
bytes agree), **INFERRED** (one side only), **UNKNOWN**.

## Verdict

- **Room geometry is CONFIRMED and lives behind header slot `0x00`, not behind
  slot `0x03`.** Slot `0x00` is the room/zone record. Its source fields
  `+0x10/+0x1C` count/locate a **12-byte vertex array**; `+0x14/+0x20` locate a
  **12-byte face array whose first three `u16` are vertex indices**;
  `+0x18/+0x24` locate an **8-byte array of unknown role**.
- **Slot `0x03` is a refuted prime suspect.** It is the offset of a
  `0x3C`-stride record array counted by slot `0x02` (counts 0..12, never
  geometry). Its large values (179,324 in `LAIR10`, 191,296 in `UNDRCT9`) are
  data-base-relative positions near the end of the payload, not payload sizes.
- **Object placement is data-driven and CONFIRMED at the field level.** The room
  record's object array (`+0x08` count, `+0x0C` offset, `0x4C`-byte source
  records) becomes the level's `0x60`-byte runtime object array. Each record
  carries a 12-byte name and a **3 x `i16` world position at `+0x20`**. **No
  rotation matrix, quaternion or angle pair exists in the record**; the
  `+0x18` triple is a cross-level offset, not orientation.
- **The 40-byte class is not reused for objects.** Objects are `0x4C` source ->
  `0x60` runtime. Two unrelated 40-byte record families exist (link records at
  slot `0x34` and the 36 -> 40 runtime records at slot `0x3A/0x3B`).
- **The 36-byte table is not objects.** It is a small auxiliary table present in
  7 of 89 units, with a sequential index, constant colour-lane masks and a
  tagged parameter blob. Its semantics are partial (candidate: lighting/colour
  parameters).

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| G1 | Header slot `0x00` is a room/zone record. Its `+0x10` count / `+0x1C` offset are a 12-byte vertex array; `+0x14` / `+0x20` are a 12-byte face array; `+0x18` / `+0x24` are an 8-byte array | CONFIRMED |
| G2 | The face array's first three `u16` are indices into the vertex array. All 139,877 faces in 89 units index `0..vertices-1`; zero out-of-range and zero repeated-index triangles | CONFIRMED |
| G3 | The vertex record is `0x0C` bytes: three `i16` position components at `+0`, `+2`, `+4`, then three `i16` of an unknown attribute at `+6`, `+8`, `+A` | CONFIRMED (stride/position) / UNKNOWN (trailing triple) |
| G4 | The trailing vertex triple is **not** the geometric face normal. Average absolute dot with the face normal is 0.37-0.57 (chance is 0.5) across `LAIR1`, `LAIR5`, `LAIR10` | REFUTED as normal |
| G5 | Every name-keyed geometry patch site in the builder lands on `i16` 0, 1 or 2 of a 12-byte record (offsets mod 12 in {0,2,4}), across five level-name cases | CONFIRMED |
| G6 | The `0x0C`-stride array at room source `+0x34..+0x38` is **not** the vertex array: the texture-streaming pass reads its `u16` at `+2` masked with `0x1FFF` as a texture id | CONFIRMED |
| G7 | Room-local 36-byte records at `+0x44` count / `+0x48` offset become `0x60`-byte group records; a recursive builder turns them into `0x40`-byte BSP nodes with two child pointers, and a leaf node points at one 12-byte face record | CONFIRMED |
| G8 | Slot `0x02/0x03` uses a `0x3C`-byte source record -> `0x58`-byte runtime record (counts 0..12) and is not a mesh | CONFIRMED |
| P1 | Room object array: source `0x4C` record -> runtime `0x60` record; the level exposes the runtime array at level `+0xD0` with count at level `+0xC8` (header slot `0x1E`, equal to the room count in 3/3 sampled units) | CONFIRMED |
| P2 | Object source record layout: name[12] at `+0x00`, `+0x0C` always 0, `+0x10` small index, `+0x14` serial, `+0x18` 3 x `i16` offset, `+0x20` 3 x `i16` position, `+0x26` name code, `+0x28` type/id word, `+0x2C` flags | CONFIRMED (shape/position) |
| P3 | 838 objects across 89 units, 80 distinct names; `+0x20` positions are signed world units and `+0x0C` is zero in all 838 | CONFIRMED |
| P4 | The runtime object pass matches each name against a 48-entry registry and sets or clears bit `0x4000` at record `+0x2C`; a second pass uses name ordinals and the `+0x18` triple to compute cross-level offsets | CONFIRMED |
| P5 | No rotation field exists in the 76-byte object record. The `+0x18` triple is consumed as a 3-vector in cross-level subtraction, not as angles | CONFIRMED (absence) / UNKNOWN (where orientation is applied) |
| B1 | Link records at slot `0x34` are 40 bytes: five `u32`, a 12-byte `<target>,<index>` name, two `u32`; all 89 units carry the slot | CONFIRMED (prior pass re-checked) |
| B2 | Slot `0x3A/0x3B` is a 36-byte source -> 40-byte runtime table in 7 units. Source: blob offset, sequential index, three constant colour-lane masks, then variable/zero words | CONFIRMED (shape) / UNKNOWN (meaning) |
| B3 | Objects are not 40-byte records: source stride is `0x4C`, runtime stride is `0x60`, and each record starts with a name | CONFIRMED |
| B4 | Slots `0x0C/0x0D` (the 40-byte runtime builder with 16-byte sub-entries) are zero in all 89 area units | CONFIRMED |
| L1 | The "dominant block" (up to 143,872 bytes) is not any of the room geometry arrays: the largest vertex array is 44,268 bytes and the largest face array is 81,180 bytes | CONFIRMED (negative) |

## 1. Geometry

### 1.1 Traversal

```
header slot 0x00
  -> room/zone record (0xA0 runtime)
       +0x10 count / +0x1C offset -> vertex array A     (stride 0x0C)
       +0x14 count / +0x20 offset -> face index array B (stride 0x0C)
       +0x18 count / +0x24 offset -> 8-byte array C
       +0x44 count / +0x48 offset -> room-local 36-byte group table
       +0x34 / +0x38              -> 12-byte texture/descriptor array
```

Slot `0x03` is unrelated: slot `0x02` is its count and slot `0x03` its offset
(the `0x3C`-stride record array). It is **not** the vertex/index payload.

### 1.2 Vertex array A

- Stride **12 bytes**. Source count at room `+0x10`, offset at room `+0x1C`.
- `+0`: `i16` x, `+2`: `i16` y, `+4`: `i16` z. **Position is CONFIRMED**: the
  engine's per-level patch code nudges individual `i16` components inside this
  array, and every patch site lands on component 0, 1 or 2 of a 12-byte record.
- `+6`, `+8`, `+A`: three `i16` of unknown meaning. The natural guess (normal)
  is **refuted**: over sampled faces the absolute dot product between this
  triple and the geometric face normal is 0.37-0.57, i.e. chance. Candidates
  left open: vertex colour, UV/atlas pair plus a tag, or a lighting attribute.
- Units: plain signed world units. The engine converts `i16` to `f32` 1:1 in
  the shared animation/sub-model path; no fixed-point divide is applied at that
  conversion.

### 1.3 Face index array B

- Stride **12 bytes**. Count at room `+0x14`, offset at room `+0x20`.
- `+0`, `+2`, `+4`: three **`u16` vertex indices**. Width is 16 bits.
- `+6`, `+8`, `+A`: three `i16` attributes (material/flags family); not decoded.
- Corpus check (89 units, 139,877 faces): every index is in `0..A-1`; no
  negative, no out-of-range, no self-referencing triangle. This is the strongest
  independent proof that B indexes A.

### 1.4 Array C

- Stride **8 bytes** (four `i16`), count at room `+0x18`, offset at room
  `+0x24`. It is copied whole by the room builder.
- Role UNKNOWN. It is not referenced by the face index array. Candidate roles
  (unproven): collision planes, per-face UV pairs, or light samples.

### 1.5 How the engine consumes the mesh records

The room builder copies A, B and C into the runtime room record. It then builds:

1. a recursive **BSP/group tree** from the room-local 36-byte table at
   `+0x44/+0x48`: each source record becomes a `0x60`-byte group record;
2. `0x40`-byte tree nodes with two child pointers; a leaf stores a pointer to
   the corresponding 12-byte face record in B.

The draw submission that consumes vertex array A was not located (see UNKNOWN).
The 12-byte array at room source `+0x34..+0x38` is consumed elsewhere: the
texture-streaming pass walks it, takes the `u16` at `+2` masked with `0x1FFF` as
a texture id, and compares the byte pairs around it. That array is therefore a
texture/descriptor list, not vertices.

### 1.6 Largest payload

Real-unit geometry sizes (bytes = count x stride):

| Unit | vertices | faces | C | objects | A | B | C |
|---|---:|---:|---:|---:|---:|---:|---:|
| `LAIR5` | 81 | 130 | 80 | 1 | 972 | 1,560 | 640 |
| `LAIR1` | 2,829 | 5,571 | 938 | 23 | 33,948 | 66,852 | 7,504 |
| `LAIR10` | 2,660 | 5,976 | 1,828 | 15 | 31,920 | 71,712 | 14,624 |
| `UNDRCT1` | 3,689 | 6,765 | 268 | 30 | 44,268 | 81,180 | 2,144 |

Corpus totals: 78,020 vertices, 139,877 faces, max 3,689 vertices (`UNDRCT1`)
and 6,765 faces (`UNDRCT1`). Largest C array is 29,944 bytes (`LAIR33`).

The earlier "dominant block" of up to 143,872 bytes (`MOVIE3`) is therefore
**not** a geometry array. Its role stays UNKNOWN.

## 2. Placement

### 2.1 Data path

```
header slot 0x00 (room)
  +0x08 count / +0x0C offset
    -> object source records, stride 0x4C
    -> runtime records, stride 0x60
    -> level +0xD0 (array), level +0xC8 (count)
```

The level builder copies the room's runtime array pointer into level `+0xD0`;
the count at level `+0xC8` comes from header slot `0x1E`, which equals the room
object count in every sampled unit. The area loader then walks that runtime
array: it matches the 12-byte name against a 48-entry global registry and
sets/clears bit `0x4000` at `+0x2C`; a second pass uses a name ordinal and the
`+0x18` triple to compute cross-level offsets.

### 2.2 Object record fields (source `0x4C`)

| Offset | Size | Meaning | Evidence |
|---|---|---|---|
| `0x00` | 12 | NUL-padded name (e.g. `raziel`, `steam`, `campath`) | CONFIRMED |
| `0x0C` | 4 | always zero in 838/838 objects | CONFIRMED |
| `0x10` | 4 | small name/link ordinal (0..13 typical) | INFERRED |
| `0x14` | 4 | per-unit instance serial | INFERRED |
| `0x18` | 6 | 3 x `i16` cross-level offset (often zero) | CONFIRMED (use) |
| `0x1E` | 2 | padding/continuation of the offset triple | UNKNOWN |
| `0x20` | 6 | **3 x `i16` world position: x, y, z** | CONFIRMED |
| `0x26` | 2 | per-name code (e.g. `raziel` 512, `steam` 567) | INFERRED |
| `0x28` | 4 | low 16 = per-name/instance id; high 16 = type 0..8 | CONFIRMED (split) |
| `0x2C` | 4 | flags: 0, `0x8`, `0x80`, `0x8000`, `0x10000` | CONFIRMED |
| `0x30` | 4 | offset to a tagged name/parameter blob | CONFIRMED (shape) |
| `0x38` | 4 | offset to a sub-object block | CONFIRMED (shape) |

Position order is x, y, z (the 12-byte vertex record uses the same order and the
same units). Positions are relative to the level origin already copied from the
header; no scale is applied.

### 2.3 Orientation

**No orientation is stored in the object record.** The record contains a second
3 x `i16` triple at `+0x18`, but the runtime code subtracts it component by
component when it resolves cross-level links; it is a translation/offset, not a
yaw/pitch pair. The 24 opaque bytes copied into each `0x30`-byte sub-object
element (via the sub-object builder) are the only remaining candidate for a
local rotation matrix, and their meaning is UNKNOWN.

Consequence: for the common spawn records sampled here the engine can be
described as translating the object to `+0x20`; where a non-identity rotation
comes from (if any) is not proven by this pass. **UNKNOWN.**

### 2.4 Corpus shape

- 838 objects in 89 units, 80 distinct names.
- Examples: a level carries one `raziel` spawn, many `steam` emitters, `staff`,
  `morlock`, `flamed`/`flameb`, `valve`/`spout`, `campath` path nodes.
- In `LAIR1` all 13 `steam` records share the same z component and differ only
  in x/y; that is a genuine placement row, not noise.
- `+0x18` is nonzero in 388/838 records and `+0x2C` in 306/838; `+0x0C` is zero
  in all 838.

## 3. The 40-byte block class

There is no single 40-byte object record. Two unrelated 40-byte families exist:

| Family | Where | Shape |
|---|---|---|
| Link/transition records | header slot `0x34`, all 89 units | 5 `u32`, 12-byte `<target>,<index>` name, 2 `u32` |
| 36 -> 40 runtime records | header slot `0x3A/0x3B`, 7 units | pointer to tagged blob, `u32`, 8 bytes, `u32`, `u32`, `u16`, `u16`, `u32`, `u32` |

Objects are neither: source stride `0x4C`, runtime stride `0x60`, name first.
So the 40-byte size is a coincidence of two different schemas, not a shared
"object block". `0x0C/0x0D` (the third 40-byte runtime builder) is zero in all
89 area units and unused here.

## 4. The 36-byte table

- Present in 7 of 89 units: `ALUKA13` 1, `LAIR1` 6, `LAIR10` 3, `LAIR2` 7,
  `MOVIE1` 1, `MOVIE2` 7, `MRLOCK7` 1. All other units have count 0.
- Source record `0x24` bytes; runtime `0x28` bytes with the first word widened
  to a pointer (the prior 36 -> 40 copy shape).

| Offset | Size | Observed content | Class |
|---|---|---|---|
| `0x00` | 4 | offset to a `0xBE 0xEF` tagged blob, or 0 | CONFIRMED |
| `0x04` | 4 | sequential per-unit index (e.g. 421..426) | CONFIRMED |
| `0x08` | 12 | constant `0xFF00FF00`, `0x00FF0000`, `0x0000FF00` | CONFIRMED |
| `0x14` | 4 | variable | UNKNOWN |
| `0x18` | 2 | variable (e.g. 64941, 3729) | UNKNOWN |
| `0x1A` | 2 | zero | UNKNOWN |
| `0x1C` | 4 | zero | UNKNOWN |
| `0x20` | 4 | zero | UNKNOWN |

The three constant dwords are colour-lane masks. The tagged blob (magic
`0xBE 0xEF`, count byte, typed variable-length entries) holds small integers.
This is **not** the placement array and **not** a mesh: it has no positions, no
indices and no names. Candidate role: lighting/colour parameters; the meaning of
each field is UNKNOWN.

## Excluded / refuted readings

- Slot `0x03` as the geometry payload: **refuted**. It is an offset to a small
  `0x3C`-stride record array (count from slot `0x02`). The large value is where
  that small array sits, not how big the mesh is.
- The trailing vertex triple as the face normal: **refuted** by the dot-product
  test above.
- The 12-byte texture/descriptor array as vertices: **refuted** by the
  texture-streaming consumer, which reads a texture id from its `+2` word.
- Objects as 40-byte records: **refuted** by the `0x4C -> 0x60` copy shape and
  the leading name.
- The 36-byte table as objects: **refuted** by its constant masks, sequential
  index and absent positions/names.

## What changes versus prior notes

- `docs/notes/level-builder.md` B13 ("a shared sub-builder converts i16 triples
  to f32 triples and builds geometry node trees") is refined: that shared path
  is the **animation/sub-model** converter. The room mesh itself is the A/B pair
  described here; the BSP tree is built from B + the room-local 36-byte table.
- `docs/notes/room-geometry.md` Candidate A (dominant 12-byte block) is partly
  explained: there **are** two 12-byte record families (vertices and
  texture/descriptors), but neither is the 143,872-byte dominant block.
- `docs/notes/pcm-loader.md` L16 and the level-builder UNKNOWN list gain a
  concrete answer for placement fields (position at `+0x20`) and a negative
  answer for orientation (not in the record).
- The "36-byte table" in `level-builder` was described only as a copy shape.
  This pass separates the level-wide table (slot `0x3A`, colour/parameters) from
  the room-local 36-byte table (`+0x44`, BSP groups).

## UNKNOWN (explicit)

1. Meaning of the three trailing `i16` in the 12-byte vertex record.
2. Role of the 8-byte array C.
3. The draw submission that turns A/B into GPU vertex buffers (not traced).
4. Where a non-identity object orientation comes from, if anywhere.
5. Meaning of the 24 opaque bytes in each `0x30`-byte sub-object element.
6. Full semantics of the 36-byte level-wide table (slot `0x3A`).
7. Full semantics of the 40-byte link table's two tail words (prior pass).
8. Role of the 143,872-byte dominant block.
9. Role of the room-local 36-byte group records (`+0x44`).
10. Role of the `0x3C`-stride records at slot `0x02/0x03`.
11. Why only 7 units carry the level-wide 36-byte table.
12. Exact semantics of the texture/descriptor array's byte pairs.

## Next probes (ordered by cost)

1. **Renderer submission.** Find the reader of the runtime room's A/B pointers
   and of the `0x40`-byte node tree; that names the trailing vertex triple and
   confirms the GPU vertex layout.
2. **Spawner.** Find the per-frame consumer of level `+0xD0` that composes the
   world transform; check whether `+0x18` is added to `+0x20`.
3. **Array C consumer.** Follow the second argument of the group builder.
4. **Level-wide 36-byte table consumer.** Seven units are enough to falsify a
   lighting/colour hypothesis quickly.
5. **Slot `0x02/0x03` consumer.** Confirm the 60-byte record type (candidate:
   AI/path/trigger nodes).

## Legal

Behaviour notes only. Role names, field offsets, unit names, counts and a few
short engine literals. No addresses, no listings, no decompiled bodies, no game
bytes, no absolute install paths. Scratch stayed in `%TEMP%`. No protection or
packing was encountered, so no line was stopped.

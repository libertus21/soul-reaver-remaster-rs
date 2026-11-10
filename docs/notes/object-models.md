# object-models - where placed-object meshes live in the SR1 archive

Bounded, read-only byte probe of the SR1 `BIGFILE.DAT` index and its `.PCM`
entries, plus a bounded community-source check. Behaviour/format facts only:
entry indices, field offsets, counts and short engine asset names. No game
bytes, no decompiled listings, no absolute install paths. Scratch lived in
`%TEMP%`; game files were opened read-only.

Date: 2026-11-03. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable test), **INFERRED** (one side only), **UNKNOWN**.

## Verdict

- **Placed-object models live in archive `.PCM` object containers**, the same
  `BIGFILE.DAT` entries the engine requests with loader type `1`. They are
  **not `.SRM`** files and they are not level meshes.
- **The outer container is the same relocation-directory shape as a level
  `.PCM`.** The first dword is a count of relocation positions, not records.
  The "first offset is 12, not 0" observation is explained: the object header
  has no self-pointer at offset 0, so the first recorded pointer position is
  `0x0C`.
- **The mesh is CONFIRMED.** After the data base, the object header names the
  model and points at an **8-byte vertex array** (three `i16` positions plus a
  `u16` normal id) and a **12-byte polygon array** (three `u16` vertex indices,
  then a normal byte, a flag byte and a `u32` material offset).
- **Vertex positions are bone-local, not model-space (CONFIRMED).** The 24-byte
  bone table carries a vertex range plus a local translation and a parent id.
  Adding the accumulated ancestor translations to each bone's vertex range
  reconstructs the rest pose. Skipping that step leaves every body part at its
  bone origin and renders as a triangle jumble; applying it turns `raziel` into
  a recognisable upright humanoid. `--object-info` prints the bone count and the
  shared count explicitly.
- **The index test passes at corpus scale.** 394 single-model containers:
  **18,740 vertices / 32,557 polygons, 0 out-of-range indices**. Every one
  satisfies `vertexStart + vertexCount*8 == polygonStart` and
  `polygonStart + polygonCount*12 == endOfPolygons`.
- **Name mapping is CONFIRMED at corpus scale.** Across 364 level units with a
  valid room/object array, **375 of 378 distinct placed-object names** resolve
  to an object container. The three exceptions (`campath`, `marker`, `splob`)
  are marker/path placeholders with no mesh. The fourth dword of a bigfile index
  entry is a **four-character name suffix**, not a loader type.
- **The archived models are the legacy low-poly object models**, not the
  remaster high-detail meshes. For example `raziel` is 299 vertices / 640
  triangles here, while the loose remaster `RAZIEL_1.SRM` is 20,239 vertices /
  36,000 triangles.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| O1 | The bigfile index entry's fourth dword is a 4-byte name suffix: the last four characters of the asset name, uppercased (`raziel` -> `ZIEL`, `undhost` -> `HOST`). Matches 395/397 object containers; two have a leading `\` (`\BOX`, `\EYE`) | CONFIRMED |
| O2 | An object `.PCM` uses the level container: `u32 N`, `N` relocation positions, zero padding, `data_base = 4*roundup512(N)`. Entry 66: `N=1813`, data base 8,192, payload 202,640 | CONFIRMED |
| O3 | The first relocation position is `0x0C` because the first pointer field is at object header `+0x0C`; `+0x00..+0x08` are scalars. There is no record at offset 0 | CONFIRMED |
| O4 | At the data base sits an object header: strings at `+0x4C` and `+0x58` (12 bytes each, `_`-padded), table offset `+0x0C = 100`, name offsets `+0x20 = 88` / `+0x24 = 76`, and a count-minus-one `u16` at `+0x0A` | CONFIRMED (shape) |
| O5 | The table at `+0x64` has `count` data-base-relative positions. For a single-model object, `table[0]` is the model header; the remaining records are auxiliary type-3 records, not mesh data | CONFIRMED (shape) / UNKNOWN (aux role) |
| O6 | Model header `+0x00..+0x1C`: vertex count, vertex start, a `u32` observed as 1 in 374/394 models, end-of-polygons, polygon count, polygon start, bone count, bone start | CONFIRMED (numbers) / UNKNOWN (the `+0x08` field role) |
| O7 | Model header `+0x30` is the material start; polygon `+0x08` is a material offset. 32,370/32,370 textured polygons land on a 16-byte-aligned offset at or after the material start | CONFIRMED |
| O8 | Vertex stride is 8 bytes: `i16 x`, `i16 y`, `i16 z`, `u16 normalID`; every normal id in the corpus is `< 256` | CONFIRMED |
| O9 | Polygon stride is 12 bytes: `u16 v1, v2, v3`, `u8 normal`, `u8 flags`, `u32 materialOffset/colour` | CONFIRMED (indices, material) / UNKNOWN (flag bits beyond `0x02`) |
| O10 | 394 single-model containers: `18,740` vertices, `32,557` polygons, **0 out-of-range indices**, 100% of the two array-boundary identities | CONFIRMED |
| O11 | 397 object containers exist; 394 are single-model and 3 (`human`, `vlgr`, `paths`) have `table[0] == 0` and are a distinct multi-part shape | CONFIRMED (counts) / UNKNOWN (multi-part layout) |
| O12 | 47,629 relocation positions across the 397 object containers; 47,514 (99.76%) resolve to an in-payload offset, 0 resolve to zero, 115 fall between payload end and entry end | CONFIRMED |
| O13 | Of 378 distinct placed-object names across 364 level units, 375 resolve to an object container by name; `campath`, `marker`, `splob` do not | CONFIRMED |
| O14 | The object loader path walks the level's object-name table and builds one object request per name (prior `pcm-loader` L2); the container is self-identifying by its two name fields | CONFIRMED (prior code note + bytes) |
| O15 | Vertex positions are bone-local. Bone record stride 24: `+8` first vertex, `+10` last vertex, `+12` three `i16` local translation, `+18` parent id. Rest pose adds the accumulated ancestor translations per vertex range. Omitting this yields a connected but exploded mesh (normal topology, humanoid silhouette only after applying). The `+18` word is a **signed** parent index (`-1` = root); see `docs/notes/bone-hierarchy.md` for the validated tree, composition and corpus agreement. | CONFIRMED |
| O16 | Double-sided faces are stored as two triangles with the same three indices and opposite winding (`raziel` 105 such pairs of 640 faces). For topology checks they are one surface face; keeping both makes every shared edge appear 4 times. | CONFIRMED |
| O17 | After rest-pose armature, welding equal positions and merging double-sided duplicates: `raziel` V=283 E=816 F=535, Euler 2, 13 non-manifold edges; `morlock` V=166 E=484 F=320, Euler 2, 2 non-manifold edges. | CONFIRMED |

## 1. The container

### 1.1 The fourth index dword is a name suffix, not a type

The bigfile entry is 16 bytes: `[hash][length][offset][fourth]`. The fourth
dword is printable ASCII on every object container and reads as the last four
characters of the asset name, uppercased:

| entry | fourth dword | container name |
|---:|---|---|
| 66 | `ZIEL` | `raziel__` |
| 119 | `HOST` | `undhost_` |
| 140 | `ARPG` | `warpg___` |
| 252 | `LOCK` | `morlock_` |

The rule holds for 395 of 397 containers. Two entries (`box`, `eye`) store a
leading backslash (`\BOX`, `\EYE`); why is UNKNOWN. The loader type (`1`, `2`,
`0x32`) is chosen by the request site, not stored in the index entry. This
corrects the earlier shorthand that the fourth dword was a type tag.

### 1.2 Outer shape

An object `.PCM` uses the same container as a level `.PCM`:

```text
[ u32 N ][ N x u32 relocation positions ][ zero pad to 4*roundup512(N) ][ payload ]
```

Entry 66 has `N = 1813` and a data base of 8,192 bytes. The first relocation
position is `12`, not `0`: the object header's first pointer is at `+0x0C`, so
positions `0`, `4` and `8` are simply not recorded. This is the same
relocation-map rule as `level-builder` B1-B3, not a different container.

Directory integrity over the 397 object containers:

- 47,629 positions total;
- 47,514 resolve to a data-base-relative offset inside the payload;
- 0 resolve to zero;
- 115 (0.24%) resolve to a value between payload end and entry end (UNKNOWN,
  likely a second embedded base).

### 1.3 Object header (payload offset 0)

| Offset | Size | Content | Evidence |
|---|---|---|---|
| `+0x00` | 4 | flags/format word; varies by model (`0x020502E0`, `0`, …) | INFERRED |
| `+0x04` | 4 | zero | CONFIRMED |
| `+0x08` | 2 | small value, often 1 | UNKNOWN |
| `+0x0A` | 2 | auxiliary-table count minus one | CONFIRMED |
| `+0x0C` | 4 | `100` — offset to the auxiliary table at `+0x64` | CONFIRMED |
| `+0x10` | 4 | `104` | UNKNOWN |
| `+0x14` | 8 | two `u16` pairs (e.g. 4000/10000, 6000/12000) | UNKNOWN |
| `+0x1C` | 4 | 0 or a payload offset | INFERRED |
| `+0x20` | 4 | `88` — offset to the second name | CONFIRMED |
| `+0x24` | 4 | `76` — offset to the first name | CONFIRMED |
| `+0x28` | 4 | small value or 0 | UNKNOWN |
| `+0x2C` | 4 | flags | UNKNOWN |
| `+0x30` | 4 | small `u16` pair (`13,49`) | UNKNOWN |
| `+0x34` | 4 | `65` | UNKNOWN |
| `+0x44` | 8 | `0xFFFFFFFF` twice | UNKNOWN |
| `+0x4C` | 12 | first name, `_`-padded to 12 bytes | CONFIRMED |
| `+0x58` | 12 | second name, normally identical | CONFIRMED |
| `+0x64` | `4*count` | auxiliary positions (data-base-relative) | CONFIRMED (shape) |

`count` comes from the `u16` at `+0x0A` plus one. For a single-model object,
`table[0]` points at the model header. For `human`, `vlgr` and `paths`,
`table[0] == 0`; those three are a multi-part shape and are out of scope here.

### 1.4 Auxiliary records

For `raziel`, the 230 records after `table[0]` all start with byte `3` and look
like small colour/parameter records (for example a type byte, an RGB triple and
repeated `u16` pairs). They are not vertices, not polygons and not materials:
the material array is a separate contiguous block at model `+0x30`. Their role
is UNKNOWN.

## 2. The model payload

### 2.1 Model header

For a single-model container, `table[0]` is a data-base-relative offset to:

| Offset | Size | Meaning | Evidence |
|---|---|---|---|
| `+0x00` | 4 | vertex count | CONFIRMED |
| `+0x04` | 4 | vertex start (data-base-relative) | CONFIRMED |
| `+0x08` | 4 | observed `1` in 374/394; role UNKNOWN | UNKNOWN |
| `+0x0C` | 4 | end of polygon array (`polygonStart + polygonCount*12`) | CONFIRMED |
| `+0x10` | 4 | polygon count | CONFIRMED |
| `+0x14` | 4 | polygon start | CONFIRMED |
| `+0x18` | 4 | bone count | CONFIRMED |
| `+0x1C` | 4 | bone start | CONFIRMED (shape) |
| `+0x20` | 4 | mostly 0 | UNKNOWN |
| `+0x24` | 4 | varies | UNKNOWN |
| `+0x28` | 4 | varies | UNKNOWN |
| `+0x2C` | 4 | varies | UNKNOWN |
| `+0x30` | 4 | material start | CONFIRMED |

### 2.2 Vertex array (stride 8)

| Offset | Size | Meaning |
|---|---|---|
| `+0` | 2 | `i16` x |
| `+2` | 2 | `i16` y |
| `+4` | 2 | `i16` z |
| `+6` | 2 | `u16` normal id |

Positions are signed integer **bone-local** units. No fixed-point divide is
applied. Across the corpus every normal id is below 256, which is consistent
with the engine's normal table. A renderer must add the bone chain translation
(§2.5) before the positions describe the model.

### 2.3 Polygon array (stride 12)

| Offset | Size | Meaning |
|---|---|---|
| `+0` | 2 | `u16` vertex index 1 |
| `+2` | 2 | `u16` vertex index 2 |
| `+4` | 2 | `u16` vertex index 3 |
| `+6` | 1 | normal id |
| `+7` | 1 | flags; `0x02` is the texture-used bit |
| `+8` | 4 | material offset (base-relative), or a colour when not textured |

### 2.4 Materials

The material array starts at model `+0x30` and consists of 16-byte records.
Every textured polygon's `+0x08` value lands on a 16-byte-aligned position at or
after that start: **32,370/32,370**. The interior layout (UV bytes, tpage,
attributes, colour) is the legacy `TextureFT3` family; its exact field meaning
is not needed for geometry and is left UNKNOWN.

### 2.5 Validation

For all 394 single-model objects:

- `vertexStart + vertexCount*8 == polygonStart`: **394/394**;
- `polygonStart + polygonCount*12 == endOfPolygons`: **394/394**;
- every polygon index `< vertexCount`: **32,557/32,557 polygons**;
- total: **18,740 vertices, 32,557 polygons**.

The `boneStart + boneCount*24 == vertexStart` identity holds for 226/394
objects. The remaining 168 have a gap between the end of the 24-byte bone table
and the vertex array; its content is UNKNOWN (padding or another array), but it
does not change how the bone records are read. **Bones are required to draw the
mesh**: the vertex array is bone-local.

Rest pose, per bone `b` with `firstVertex != 0xFFFF` and `lastVertex != 0xFFFF`:

```text
world[b] = bone[b].local + world[bone[b].parent]   (0xFFFF or self ends the chain)
position[v] += world[b] for firstVertex <= v <= lastVertex
```

This is a pure translation chain (no rotation/scale in the archived object
model). `raziel`: 58/66 bones carry ranges; `morlock`: 34/37.

**Correction (2026-11-10).** The `+18` word is a **signed `i16` parent index**
(`-1` = root), not an opaque slot word, so the archived object model **does carry
a real bone hierarchy**. The older shorthand that these models have "no hierarchy"
is wrong for the archived `.PCM` objects; it describes loose `.SRM`, whose joint
pool is a flat 128 slots. Verified: `raziel` forms one root, depth 9, 0
topological violations and 0 cycles; the corpus census is 199/394 (50.5%)
strict-valid, with 194 one-bone props storing a self-parent `0` (a second,
UNKNOWN convention) and one two-root model (`matsrevr`). The composed tree drives
hierarchical skinning as `world[child] = world[parent] * local[child]`. See
`docs/notes/bone-hierarchy.md`.

Vertices are duplicated for hard shading, so a surface check must weld equal
rest-pose positions first. Faces are also duplicated with opposite winding for
two-sided drawing; a surface check must merge those pairs. Both normalisations
are test-only; the renderer keeps every triangle because both sides are real
draw calls.

Named examples (counts only):

| Name | Entry | Vertices | Polygons | Bones |
|---|---:|---:|---:|---:|
| `raziel` | 66 | 299 | 640 | 66 |
| `morlock` | 252 | 178 | 334 | 37 |
| `hint` | 118 | 12 | 9 | 6 |
| `undhost` | 119 | 3 | 1 | 1 |
| `warpg` | 140 | 3 | 1 | 1 |
| `eggfx` | 139 | 3 | 1 | 3 |

`undhost` and `warpg` are genuinely one-triangle objects, so they confirm the
container/format but are not useful as shape checks. `morlock` (178/334) is the
second substantial model that confirms the same layout independently.

## 3. Name -> model

1. A level's placed objects are the room array from `level-geometry` P1: 12-byte
   names at source `+0x00`, stride `0x4C`.
2. The area loader walks the level's object-name table and calls the object
   path builder once per name (prior `pcm-loader` L2). The object request uses
   loader type `1`.
3. The resolved archive entry is an object container whose header carries the
   same name twice, `_`-padded to 12 bytes (`raziel` -> `raziel__`).
4. The index entry's fourth dword echoes the last four characters of that name,
   which makes an index dump human-readable but is not itself the lookup key.

Corpus check (this archive): 364 level units with a valid room and object array
contain 6,142 placed-object instances and **378 distinct names**. **375** have a
matching object container. The three without are `campath`, `marker` and
`splob` — path/marker/effect placeholders that should keep a marker (or be
skipped), not a cube or a bogus mesh.

The earlier `level-geometry` note's 89-unit/838-object figure is a smaller
subset; the published `under1` counts (1,468 vertices / 2,922 faces) reproduce
exactly against entry 115 here, which cross-checks the level parse used for the
name coverage.

## 4. Community sources (format facts only, no code copied)

Web search returned HTTP 403, so GitHub API and raw files were used directly.

| Tool | URL | License | Relevance |
|---|---|---|---|
| ModelEx | https://github.com/TheSerioliOfNosgoth/ModelEx | LICENSE present; GitHub reports `NOASSERTION` (no SPDX id) | `CDC/Models/SR1ObjectModel.cs` and `CDC/Models/SR1Model.cs` document the same header (vertex/polygon/bone starts, 3 x `i16` vertices, 12-byte `_MFace`, 16-byte `TextureFT3` materials) |
| Recombobulator | https://github.com/TheSerioliOfNosgoth/Recombobulator | GPL-3.0 | Same SR1 object model family; useful corroboration only |
| io_scene_xrm (remaster addon) | https://github.com/TheDodylectableX/io_scene_xrm | No LICENSE file | Remaster `.SRM`, not the archived `.PCM` objects; no archived-model reader found |

The ModelEx naming (`SR1ObjectModel`) matches the format observed here. This
note is an independent byte-side confirmation on the remaster archive, not a
port of that reader; no source code was copied.

## 5. What changes versus prior assumptions

- **Entry 66 is not "not the level shape".** It is the same container; the
  first relocation position is `12` because the object header has no pointer at
  offset 0.
- **`N = 1813` is not 1,813 records.** It is the relocation count; the payload
  begins at 8,192 and holds the object header, name table, model header and
  mesh arrays.
- **The fourth bigfile dword is a name suffix, not a loader type.** Loader type
  comes from the request site.
- **Archived object models are not `.SRM`.** They are the legacy low-poly
  object format. The remaster's loose `.SRM` files are a separate, much denser
  asset family.
- **`raziel` is a model name.** It is the first of the two 12-byte names in the
  container, and the placed-object name maps to it directly.

## 6. UNKNOWN (explicit)

1. Meaning of the auxiliary table at object `+0x64` (all type `3` for `raziel`;
   likely colour/animation/texture side data).
2. Object-header fields `+0x08`, `+0x14`, `+0x28`, `+0x2C`, `+0x30`, `+0x34`,
   `+0x44..+0x4B`.
3. Model-header `+0x08` role (observed `1` in 374/394).
4. The gap between the end of the bone table and `vertexStart` for the 168
   objects where `boneStart + boneCount*24 != vertexStart` (padding or another
   array; the 24-byte bone records themselves are confirmed).
5. Interior of the 16-byte material record (UVs, tpage, attributes, colour) and
   its texture binding.
6. The three multi-part containers (`human`, `vlgr`, `paths`) and how their
   sub-models are addressed.
7. The 115 relocation positions (0.24%) that resolve between payload end and
   entry end.
8. Why `box` and `eye` carry a leading `\` in the fourth index dword.
9. Where a placed object's world transform is composed beyond the record
   position. The local **vertical** is now confirmed (`docs/notes/object-up-axis.md`:
   object-local up is `+Z`, the same as the world, so placement is a pure
   translation). The facing axis and any authored yaw remain UNKNOWN.
10. Whether any non-type-1 path reads the auxiliary table.

## 7. Reader in this tree

`--object-info --bigfile <DAT> --entry <I>` parses one single-model container in
memory and prints the vertex/polygon counts, the count of distinct 16-byte
material records referenced by textured polygons, both array-boundary identities,
and the out-of-range polygon-index count. Multi-part containers are refused
loudly.

`--object-mesh --bigfile <DAT> --entry <I>` parses the same container, applies
the bone rest-pose translations, widens the `i16` positions 1:1 to `f32` and
renders each polygon as one triangle from its three `u16` indices. Refusals
happen before the window. Without the bone step the triangles are connected but
every part sits at its bone origin, which reads as a chaotic blob.

`--object-mesh --bigfile <DAT> --entry <I> --pose <FILE>` replaces the rest
translation with the skinning path (section 8): every bone's contiguous vertex
range is placed by that bone's 13-float slot pose, and the skinned model-space
bounds are printed before the window. Without `--pose` the same arm keeps the
rest-pose behaviour above.

`--level-render --bigfile <DAT> --group <PREFIX> --objects` builds a name index
from the archive's 4th index dword once per run, resolves each placed object to
its real model, and translates that model by the record's world position (the
record carries no rotation). Unresolved names fall back to the existing cube
marker. A summary line reports the mesh/marker split; without `--objects` the
render output is unchanged.

Rendering, textures and the material interior are not decoded; the 24-byte bone
table and its parent hierarchy **are** decoded (§2.5 above and
`docs/notes/bone-hierarchy.md`). Animation semantics remain out of scope.
Object-local up is now confirmed `+Z` (`docs/notes/object-up-axis.md`), so
placement is a translation with no rotation; the facing/yaw remains undecoded.

## 8. Skeletal skinning (deformation only)

This is the **deformation pipeline**, not animation semantics. How the runtime
chooses a frame, what the mode bits mean and where the pose comes from are
UNKNOWN and deliberately absent; that question is tracked separately from this
pipeline.

### 8.1 Slot pose and matrix

One runtime slot pose is 13 `f32` values:

```text
[ r00 r01 r02   r10 r11 r12   r20 r21 r22 | scale marker | tx ty tz ]
```

`pose::slot_pose_matrix` copies the first nine floats as a row-major 3x3
rotation, keeps the tenth as `scale_marker`, and the last three as a
translation. The rotation is used as supplied: only finiteness is checked, not
orthonormality (the runtime built it).

**The scale marker's meaning is UNKNOWN.** No corpus or trace evidence says
whether it is a uniform scale, an animation flag or padding, so the matrix
carries it for inspection but applies `scale = 1.0`. Treating it as a scale
would invent semantics this slice does not have.

### 8.2 Composition with the rest pose: pose over rest, added

Archived vertices are bone-local and `rest_positions` is a pure translation
chain (`world[b] = local + world[parent]`). A bone pose therefore transforms
its range as:

```text
posed = scale * R * local + rest_offset + translation
```

The translation is **added** to the bone's accumulated rest translation; the
pose does not replace it. Two facts force that choice:

1. An identity pose (`R = I`, `translation = 0`) must reproduce the rest pose
   byte for byte. Adding does; replacing would collapse every part onto its
   bone origin (the known "triangle jumble").
2. The established mesh-space path (`pose::transform_for_joint`) rotates about
   a joint head and re-adds it (`t = head - R * head`). The bone's rest
   translation *is* that head here, so `R * local + rest_offset` is the same
   composition expressed in bone-local coordinates.

A translation moves a bone's range by exactly its delta; a rotation pivots that
range about the bone's rest origin.

### 8.3 Pose file

`--object-mesh --bigfile <DAT> --entry <INDEX> --pose <FILE>` reads a text file
with **one pose per bone, in bone order**:

```text
# bone 0: identity
1 0 0 0 1 0 0 0 1 1 0 0 0
# bone 1: +90 degrees about X
1 0 0 0 0 -1 0 1 0 1 0 0 0
```

Blank lines and `#` comments are skipped; every other line must hold exactly 13
finite floats. Fewer lines than bones leave the missing bones at rest; extra
lines are ignored. The arm prints one witness line before the window:

```text
pose bones=66 supplied=2 moved=37 bbox min=(...) max=(...)
```

An identity pose or an empty file reproduces the rest-pose bounds exactly; a
rotation on one range changes only the bounds that range can reach. `--pose`
combines with the flat renderer only (not `--textures`).

### 8.4 Vertex shape and tests

`pose::skin_positions` returns model-space `f32` positions (the renderer's
input). `pose::skin` returns the archived 8-byte shape with the normal id
preserved, rounding to the nearest integer and clamping to `i16`; the posed
topology is unchanged because the viewer reuses `object::polygon_indices`.

Tests cover the byte-identical identity pose, a single-range rotation, an exact
translation delta, fewer and more poses than bones, a one-bone model, and the
pose-file parser's refusals.

## 9. Legal

Behaviour and format facts only: entry indices, field offsets, counts, array
strides and short asset names. No game bytes, no decompiled text, no listings,
no absolute install paths. Scratch files lived in `%TEMP%`; the game archive was
opened read-only and never copied into the repository.

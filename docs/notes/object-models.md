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

Positions are signed integer model-space units. No fixed-point divide is
applied. Across the corpus every normal id is below 256, which is consistent
with the engine's normal table.

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
objects. The remaining 168 have a different gap between the bone table and the
vertices; the bone stride or an intermediate array is UNKNOWN. Bones are not
needed to draw the mesh.

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
4. Bone record layout for the 168 objects where the 24-byte identity fails.
5. Interior of the 16-byte material record (UVs, tpage, attributes, colour) and
   its texture binding.
6. The three multi-part containers (`human`, `vlgr`, `paths`) and how their
   sub-models are addressed.
7. The 115 relocation positions (0.24%) that resolve between payload end and
   entry end.
8. Why `box` and `eye` carry a leading `\` in the fourth index dword.
9. Where a placed object's world transform (orientation and scale) is composed;
   the level record has position only, and the model is in local space.
10. Whether any non-type-1 path reads the auxiliary table.

## 7. Legal

Behaviour and format facts only: entry indices, field offsets, counts, array
strides and short asset names. No game bytes, no decompiled text, no listings,
no absolute install paths. Scratch files lived in `%TEMP%`; the game archive was
opened read-only and never copied into the repository.

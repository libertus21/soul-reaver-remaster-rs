# room-textures - where `.PCM` room geometry UV and texture data live

Bounded, read-only byte probe of the SR1 remaster archive. Behaviour/format
facts only: field offsets, strides, counts, index arithmetic and a handful of
sampled Texel values. No game bytes, no decoded imagery, no payload dumps, no
absolute install paths. Scratch lived in `%TEMP%\opencode\roomtex`; the game
tree was opened read-only.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable test), **INFERRED** (one side only), **UNKNOWN**.

This pass builds directly on:
`docs/notes/level-geometry.md` (room record, vertex/face arrays),
`docs/notes/object-textures.md` (`TextureFT3` and the `TEXTURES.BIG` page
mapping) and `docs/notes/object-models.md` (object polygon layout).

## Verdict

- **Room UV and texture data ARE present and CONFIRMED.** They live in a
  per-room **12-byte record array** at room record `+0x34` (data-base-relative
  start offset) to `+0x38` (end offset). The array is a `TextureFT3` with the
  trailing colour word removed: three UV byte pairs, a texture word and two
  attribute words.
- **The face tail is two `u16`, and the second one is the array offset.**
  Face record `+0x0A` (`B`) is a **byte offset into the `+0x34` array**, not a
  face index and not a data-base offset. It is 12-aligned for 99.76% of faces.
- **The texture page is `textureWord & 0x07FF` at record `+0x02`.** This is the
  same mask and the same bank as object faces. There is no palette and no
  embedded pixel data anywhere in the room payload.
- **Room pages occupy `TEXTURES.BIG` pages 379..1286; object pages occupy
  2..378** (object-textures T5/T12). The bank header declares 1287 pages. The
  two asset families do not overlap.
- **The room allocation is ordered and contiguous.** 371 of 371 rooms with a
  UV array use a single contiguous page range, and ranges are non-decreasing in
  archive entry order: **0 inversions in 370 comparisons**.
- **A real face was sampled.** `under1` (entry 115) face 3 -> page 385 -> UVs
  `(64,64) (64,126) (126,64)` -> RGB8 `(49,49,49) (66,66,57) (49,49,49)`, all
  alpha bits 1.
- **`A` (face `+0x08`) is NOT a page id.** It is a signed-looking 16-bit value,
  symmetric around zero (corpus -6566..6565), and agrees with the record page
  in only 136 of 573,087 aligned faces — below chance. Its role is UNKNOWN; it
  is not needed to texture a room.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| G1 | Room record `+0x34` is a data-base-relative start offset and `+0x38` the end offset of one array; `(end - start)` is a multiple of 12 in **372/372** rooms | CONFIRMED |
| G2 | Face stride is 12: `faceOffset + faceCount*12 == vertexOffset` in **372/372** rooms | CONFIRMED |
| G3 | Vertex stride is 12: `vertexOffset + vertexCount*12 == arrayC offset` in **372/372** rooms | CONFIRMED |
| G4 | Face `+0x0A` is a byte offset into the `+0x34` array: it is `< (end-start)` and a multiple of 12 for **573,084/574,458** faces (99.76%); the maximum is `(end-start)-12` in the sampled rooms | CONFIRMED |
| G5 | The `+0x34` record is a `TextureFT3`-lite: `+0 u8 u0`, `+1 u8 v0`, `+2 u16 textureWord`, `+4 u8 u1`, `+5 u8 v1`, `+6 u16 attributesA`, `+8 u8 u2`, `+9 u8 v2`, `+10 u16 attributes`; **no colour word** | CONFIRMED (corpus + sample) |
| G6 | UV bytes never exceed 254 across 86,222 records (same bound as object T4) | CONFIRMED |
| G7 | `attributesA` takes the same value family as object `TextureFT3` (`0x0108..0x010B`, `0x0128`, `0x012A`, `0x012B`, `0x0169..0x016B`) | CONFIRMED (family) / UNKNOWN (bit meaning) |
| G8 | `page = textureWord & 0x07FF`; 6,717 of 86,222 words have high bits set, and no page falls outside 379..1286 | CONFIRMED |
| G9 | Room pages are 379..1286 (888 distinct); object pages are 2..378; the bank header says 1287 pages | CONFIRMED (rooms by corpus scan; objects by object-textures T5/T12) |
| G10 | Every room with UV records uses a contiguous page range; ranges are non-decreasing in archive entry order: 371/371 contiguous, **0 inversions / 370** | CONFIRMED |
| G11 | Face `+0x08` (`A`) is not a page: as `u16` it reaches 65519; as `i16` it is symmetric (-6566..6565) and `(A & 0x7FF)` equals the record page in only 136/573,087 faces | REFUTED as page / UNKNOWN role |
| G12 | Sampled texels are coherent and opaque on the addressed page (`under1` face 3, page 385) | CONFIRMED (measurement) |
| G13 | UV orientation is direct (`v` top-down), not flipped: `city2` transparent-texel hits are 1.1% direct vs 13.9% flipped | CONFIRMED (measurement) / INFERRED (convention) |
| G14 | 214 faces carry `B = 0xFFFF`; 1,160 faces carry a non-12-aligned `B`, and 1,157 of those sit in the last 10% of their room's face list | CONFIRMED (counts) / UNKNOWN (meaning) |
| G15 | The trailing 4 bytes of a face are **not** a `u32` offset: the value would be far past the entry size (e.g. 1,572,867 for a 159,936-byte entry) | REFUTED as u32 |

## 1. The face record truth

`level-geometry.md` already established a 12-byte face array (`+0x14` count,
`+0x20` offset) whose first three `u16` index the vertex array. This pass adds
two independent, corpus-scale checks:

- `faceOffset + faceCount*12 == vertexOffset` in **372/372** rooms.
- `vertexOffset + vertexCount*12 == arrayC offset` in **372/372** rooms.

So the face stride is **12**, and the mesh arrays are laid out back-to-back:
`[room record][face array][vertex array][array C]`.

Each 12-byte face is:

| Offset | Size | Field | Class |
|---:|---:|---|---|
| `+0` | 2 | `u16` vertex index 0 | CONFIRMED |
| `+2` | 2 | `u16` vertex index 1 | CONFIRMED |
| `+4` | 2 | `u16` vertex index 2 | CONFIRMED |
| `+6` | 1 | byte observed as a normal id (0, 2, 16, 18, 68, 104, ...) | INFERRED |
| `+7` | 1 | flag byte (mostly 0; 0x0A, 0x1E, 0x3F, 0xFE seen) | UNKNOWN |
| `+8` | 2 | `A` — signed-looking attribute, **not a page** | UNKNOWN |
| `+10` | 2 | `B` — **byte offset into the `+0x34` record array** | CONFIRMED |

The tail is two `u16`, not one `u32`. Reading `+8..+11` as a little-endian
`u32` gives values like 1,572,867 in an entry of 159,936 bytes, so a 32-bit
offset is refuted independently of the two-`u16` split.

### 1.1 The `0xFFFF` and non-aligned tails

Of 574,458 faces:

| Case | Count | Share |
|---|---:|---:|
| `B` is 12-aligned and inside the array | 573,084 | 99.76% |
| `B = 0xFFFF` | 214 | 0.04% |
| `B` is inside the array but not 12-aligned | 1,160 | 0.20% |

1,157 of the 1,160 non-aligned values sit in the last 10% of that room's face
list (1,158 in the last 50%). They look like a **tail sub-record type**: the
face still has valid vertex indices and a normal id, but the last two words do
not address a texture record. In `under2` the last two faces of the room point
at a mid-record byte and decode to a degenerate UV line on a page the room does
not otherwise use. This is bounded and excluded, never clamped.

## 2. `B` is an offset into the room's UV/texture array

The target array is described by **start `+0x34` / end `+0x38`**, both
data-base-relative. It is not a count/offset pair: both values are large
offsets and the second is greater than the first in every room.

- `(end - start)` is a multiple of 12 in 372/372 rooms.
- `B` is relative to **`+0x34`**, not to the data base: a `B` of 8,916 in
  `under1` is far below the array's absolute 147,656.
- `B` is 12-aligned for 99.76% of faces, which is what addressing whole
  12-byte records looks like.
- The maximum `B` in a room is the last record (`under1`: maximum 8,916 with a
  8,928-byte span).

When a face is textured, the record at `start + B` is read as 12 bytes.

## 3. The 12-byte record is `TextureFT3` minus the colour word

| Offset | Size | Field | Evidence |
|---:|---:|---|---|
| `+0` | 1 | `u0` | G5, G6 |
| `+1` | 1 | `v0` | G5, G6 |
| `+2` | 2 | `textureWord` (low 11 bits = page, high 5 = flags) | G5, G8 |
| `+4` | 1 | `u1` | G5, G6 |
| `+5` | 1 | `v1` | G5, G6 |
| `+6` | 2 | `attributesA` | G5, G7 |
| `+8` | 1 | `u2` | G5, G6 |
| `+9` | 1 | `v2` | G5, G6 |
| `+10` | 2 | `attributes` (0 in the sampled records) | G5 |

Two other splits were tested and rejected:

1. **One packed `u16` UV per vertex + `u16` attribute** (six `u16` fields) is
   wrong: the middle `u16` of a record takes values like 264–267, not a UV or a
   page, and would make one triangle's three "attribute" words differ when the
   page should be per record.
2. **Three separate UV pairs of `u16`** is wrong: the same words would then be
   three 16-bit coordinates in the hundreds/thousands, while decoding them as
   `u8 u, u8 v` reproduces exact complementary right triangles (for example
   `(64,64) (64,126) (126,64)` and `(126,126) (126,64) (64,126)` for the two
   triangles of the same quad).

The field order is the object `TextureFT3` order (object-textures T2) with the
trailing `u32` colour removed, so the record is 12 bytes instead of 16. The
room record array is therefore a **per-room face → UV/page table**, not a
per-face material array: records are shared between faces (`under1`: 2,922
faces, 744 records).

## 4. Page mapping and the bank

- Page = `textureWord & 0x07FF` (same mask as objects). The high 5 bits are
  flags: 6,717 of 86,222 words have at least one of them set, and masking them
  out never yields a page outside the room range.
- Pages are read from `1\TEXTURES.BIG`: 4,096-byte prefix, 256x256
  little-endian A1R5G5B5, 131,072 bytes per page. Page `p` starts at
  `4096 + p*131072`; texel `(u,v)` is at `2*(u + 256*v)` from the page start.
  This is unchanged from object-textures T6.
- **Ranges:** room pages are **379..1286** (888 distinct across 86,222
  records). Object pages are 2..378. The header's page count is 1287, so the
  room range reaches the last page and does not collide with objects.

### 4.1 Allocation order

- 371/371 rooms with UV records use a **contiguous** page range
  (`max - min + 1 == distinct count`).
- Ranges increase with archive entry order: **0 inversions in 370
  comparisons**. Examples:

| Entry | Unit | Pages |
|---:|---|---|
| 115 | `under1` | 379..385 |
| 120 | `push1` | 386 |
| 133 | `under2` | 387..388 |
| 134 | `under4` | 389 |
| 135 | `under3` | 391..393 |
| 1313 | `tower11` | 1280..1281 |
| 1316 | `chrono2` | 1282..1283 |
| 1317 | `oracle22` | 1284..1286 |

This is a sequential bank allocation per level, not a pointer table and not a
per-face texture list. It is the room-side analogue of the object allocation
(object-textures §2.3), with objects taking the head of the bank and rooms the
tail.

## 5. Proof on a real face

`under1` = archive entry 115, room vertices 1,468, faces 2,922, face array
absolute 38,768, UV record array absolute 147,656..156,584.

Face 3 (0-based, the fourth face) has vertex indices `(9, 10, 11)`, `A = 4`,
`B = 36`. The record at `147,656 + 36` decodes to:

| Property | Value |
|---|---|
| `textureWord` | `0x0181`; page = `0x0181 & 0x07FF` = **385** |
| UVs | `(64,64)`, `(64,126)`, `(126,64)` |
| `attributesA` | `0x0109` (265) |
| `attributes` | `0x0000` |

Page 385 starts at `4096 + 385*131072 = 50,466,816`. Sampling texel
`2*(u + 256*v)`:

| Vertex | UV | Raw A1R5G5B5 | RGB8 | Alpha bit |
|---|---|---|---:|---:|
| v0 | (64,64) | `0x98C6` | (49,49,49) | 1 |
| v1 | (64,126) | `0xA107` | (66,66,57) | 1 |
| v2 | (126,64) | `0x98C6` | (49,49,49) | 1 |

The two triangles of the same quad are complementary in UV space and sample the
same grey stone region, which is what a correct mapping should produce. Other
sampled faces:

| Unit / face | Page | UVs | RGB8 | Alpha |
|---|---:|---|---|---|
| `under1` face 2 | 385 | (64,190) (126,128) (64,128) | (41,33,24) (90,82,82) (123,115,107) | 1,1,1 |
| `under4` face 100 | 389 | (126,62) (126,0) (64,0) | (74,74,74) (41,41,41) (49,49,41) | 1,1,1 |
| `city1` face 2 | 856 | (192,190) (254,190) (192,128) | (66,41,33) (66,49,33) (90,57,49) | 1,1,1 |

### 5.1 UV orientation

The same test used for objects was repeated on six rooms: count sampled texels
whose alpha bit is 0 with direct `v` versus `255 - v`.

| Unit | Samples | Transparent direct | Transparent flipped |
|---|---:|---:|---:|
| `under1` | 8,766 | 1.4% | 1.4% |
| `under2` | 1,410 | 0.0% | 0.0% |
| `add1` | 10,128 | 0.4% | 0.4% |
| `city2` | 25,413 | **1.1%** | **13.9%** |
| `tower1` | 20,535 | 0.0% | 0.0% |
| `stone1` | 12,075 | 0.0% | 0.0% |

`city2` separates the two orientations; the others are fully opaque. Direct
`v` is the working convention, matching object-textures T9.

### 5.2 Model-free units

The mapping does not depend on the placed-object array. Units with geometry
and zero objects behave identically:

| Unit | Objects | Vertices | Faces | UV records | Pages | Non-aligned `B` |
|---|---:|---:|---:|---:|---|---:|
| `aluka11` | 0 | 1,108 | 1,701 | 110 | 1110 | 2 |
| `aluka9` | 0 | 1,157 | 1,750 | 105 | 1111 | 2 |
| `out8` | 0 | 161 | 290 | 45 | 527 | 2 |

## 6. Why `A` is not the page

The first faces of `under1` make `A` look like a page id (`0xFFFF`, `0xFFFE`,
3, 4, 4). That reading does not survive the corpus:

- as `u16`, `A` reaches 65,519, well past the 1287-page bank;
- as `i16`, `A` is symmetric around zero and reaches ±6,566;
- `(A & 0x07FF)` equals the record's page in only **136 of 573,087** aligned
  faces (chance for a 2048-value mask is about 280, so this is at or below
  chance);
- `A` does not match any vertex component, the face centroid, or a triangle
  area proxy in the sampled rooms.

So `A` is a per-face signed attribute (candidate: winding/light/group), not the
texture. Its role is UNKNOWN and the texture path does not read it.

## 7. Reader consequence

A room reader needs no new container knowledge:

```text
slot 0x00 -> room record
  +0x14 count / +0x20 offset          -> face array (stride 12)
  +0x34 start / +0x38 end             -> UV/texture records (stride 12)
face +0x0A -> byte offset B into the record array
record +0x02 -> textureWord; page = word & 0x07FF
record +0/+1,+4/+5,+8/+9 -> three u8 UV pairs
```

`B` is the only new indirection. It is relative to the record array, not the
data base, and it is not a face index.

### 7.1 Reader mapping (implemented)

| Fact | Code |
|---|---|
| `+0x34`/`+0x38` array, 12-byte records, `page = word & 0x07FF`, three `u8` UV pairs | `src/level.rs` `room_texture_faces` |
| Face `+0x0A` byte offset into that array | `src/level.rs` `room_texture_faces` |
| Per-face vertex expansion (three unique vertices, material 1 = flat) | `src/level.rs` `textured_room_mesh` |
| Same bank decode, page bounds and loud `SlotIndexOutOfRange` as objects | `src/object_texture.rs` `PageBank` / `scene` |
| Textured room, optional textured objects, flat markers | `src/main.rs` `--level-render ... --textures` |

`0xFFFF` and non-12-aligned `+0x0A` tails are counted as unresolved faces and
drawn flat; they are never dropped or clamped. A missing array or a record
array outside the data region is a distinct loud error. Without `--textures`,
the mesh path is unchanged.

## 8. UNKNOWN (explicit)

1. Role of face `+0x08` (`A`). Refuted as page/texture id; signed, symmetric,
   uncorrelated with geometry fields. Candidate: winding/light/group attribute.
2. Meaning of the `+0x06` byte (normal id) and `+0x07` flag byte in the room
   face. The room flags do not follow the object `0x02` texture-used rule.
3. The 214 faces with `B = 0xFFFF` and the 1,160 non-aligned tail records. They
   are a sub-1% tail; their record type and purpose are UNKNOWN.
4. Meaning of the room record's `+0x28` field, which equals the `+0x38` end
   offset in every sampled room. It may be a duplicate end pointer.
5. Meaning of the room `+0x30` offset. It points at a small table of
   `"<level>,<index>"` names (observed `under2,2`, `add2,28`, `add4,03`), not at
   geometry; the table's record format is not decoded here.
6. Why the room record omits the `TextureFT3` colour word (16 -> 12 bytes). The
   `+0x0A` attribute word is 0 in the sampled records; blend/colour behaviour is
   UNKNOWN.
7. Meaning of `textureWord` bits 11..15 in rooms (flags, same as objects; which
   flag is which is not established).
8. Whether the remaster repacked the bank or preserved the legacy pages at
   their original indices. The format and ordering agree with the legacy bank;
   pixel identity against the original PC release was not tested.
9. The renderer submission that turns these records into GPU buffers; this pass
   proves the data mapping only, not the draw call.

## 9. Next probes (ordered by cost)

1. **Room renderer submission.** Find the consumer of room `+0x34`/`+0x38` and
   of face `+0x0A`; confirm the UV record is uploaded as-is and identify how
   `A` and the flag byte are used (culling, sorting, light).
2. **Tail sub-records.** Decode the last few faces of `under2`/`under3`/`under4`
   and of `oracle15` (66 anomalous faces). If they are a distinct record type,
   the face count may need a split (mesh faces vs auxiliary faces).
3. **`A` correlation.** Test `A` against the 0x40-byte BSP leaf records and the
   room-local 36-byte group table (`level-geometry.md` G7). A per-face group id
   is the cheapest hypothesis to falsify.
4. **Colour/blend.** If `attributes`/`attributesA` carry blend state, find the
   consumer that applies it; compare with object `TextureFT3` attributes.
5. **Page identity.** Optionally compare a few room pages against the loose
   `TEX\*.DDS` family, as was done for objects; the expectation is noise, but
   it would settle whether rooms were re-authored too.

## 10. Legal

Format and behaviour facts only: field offsets, strides, counts, index
arithmetic, level/entry names and a few sampled numeric texel values. No game
bytes, no decoded imagery, no payload dumps, no code listings, no absolute
install paths. The game tree was opened read-only and nothing was copied into
the repository. Scratch lived in `%TEMP%`.

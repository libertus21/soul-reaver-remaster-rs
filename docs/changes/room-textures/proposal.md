# Proposal: room-textures (outcome: mapping located; decode slice proposed)

## Intent

Room geometry renders as an untextured triangle soup because the room face
tail and the per-room texture/UV table were never decoded. This pass asked
where room UV and texture data live, whether the face tail is two `u16` or one
`u32`, what the offset field addresses, and whether a real room face can be
sampled from the texture bank. Probe-first; a documented no-go was acceptable.

## Outcome

**GO. The mapping is CONFIRMED on one room, several rooms, and the corpus.**

The room record (`header slot 0x00`) describes a per-room **12-byte
UV/texture record array**:

```text
room +0x34 = data-base-relative start offset
room +0x38 = data-base-relative end offset
stride     = 12
```

Each record is the object `TextureFT3` order with the trailing colour word
removed:

```text
+0  u8  u0          +4  u8  u1          +8  u8  u2
+1  u8  v0          +5  u8  v1          +9  u8  v2
+2  u16 textureWord (low 11 bits = page, high 5 bits = flags)
+6  u16 attributesA                    +10 u16 attributes
```

The face record (`room +0x14` count / `+0x20` offset, stride 12) ends with two
`u16` fields:

```text
+8  u16 A          signed-looking per-face attribute; NOT the page
+10 u16 B          byte offset into the +0x34 record array
```

**`B` is relative to the `+0x34` array**, not the data base and not a face
index. It is 12-aligned for 573,084/574,458 faces (99.76%); the remaining 214
are `0xFFFF` and 1,160 are non-aligned tail records (1,157 in the last 10% of a
room's face list).

**Page = `textureWord & 0x07FF`, same bank as objects.** Rooms occupy
`TEXTURES.BIG` pages **379..1286**, objects occupy 2..378, and the header
declares 1287 pages. 371/371 rooms with UV records use a contiguous page
range; ranges are non-decreasing in archive entry order with **0 inversions in
370 comparisons**.

**A concrete face was sampled.** `under1` (entry 115), room face 3, `B = 36`,
page 385, UVs `(64,64) (64,126) (126,64)`, RGB8 `(49,49,49) (66,66,57)
(49,49,49)`, alpha 1 at all three vertices. Full numbers and the evidence
ledger are in `docs/notes/room-textures.md`.

**`A` is not the texture.** As `u16` it exceeds the bank; as `i16` it is
symmetric around zero (corpus ±6,566); `(A & 0x07FF)` matches the record page
in only 136/573,087 faces. Its role stays UNKNOWN and the texture path does not
read it.

## What changed versus prior assumptions

- The face tail is **two** fields, not a `u32` material offset. A 32-bit read
  yields values past the entry size, so the two-`u16` split is a positive result,
  not a convention.
- Room textures are not a per-face material array. They are a **shared
  per-room UV/page table** addressed by a byte offset; `under1` has 2,922 faces
  and 744 records.
- Room textures do not use pages 2..378. They use the **tail** of the same
  bank, 379..1286, allocated per level.
- The room record does not carry the `TextureFT3` colour word; the record is
  12 bytes instead of 16.

## Proposed slice (do not build in this pass)

Three small, verifiable steps:

1. Extend the level reader with a typed room texture record: three UV byte
   pairs, raw word + masked page, `attributesA`, `attributes`, and the
   `B`-relative record lookup. `--level-geometry` gains a `pages=<a,b,...>`
   field only (no texels).
2. Add a typed room sample arm: one line per textured face with page, three UVs
   and three RGB values, using the existing frame/page bank reader. No window,
   no imagery written.
3. Feed the sampled page and the per-face UVs into the existing textured mesh
   pipeline so `--level-render --textures <TEXTURES.BIG>` draws the room with
   its real texture; without `--textures` the arm is byte-for-byte unchanged.

## No-go condition (explicit)

Refuse loud and stop the slice if, at apply time, `room +0x38 <= room +0x34`,
`(end-start) % 12 != 0`, `B >= end-start`, `B % 12 != 0`, the page index is
outside the `TEXTURES.BIG` header count, the addressed slot is `empty`/`zero`,
or a UV byte addresses a texel outside the page. Never clamp, never skip a bad
face silently, never substitute a loose DDS.

## Forecast split

| Part | Lines |
|---|---:|
| Production (record view + bounds + sample/render arms) | ~260 |
| Tests (synthetic record table, synthetic bank; bounds, mask, refusal cases) x1.5 | ~390 |
| Docs (note reader section, README line, change docs) | ~70 |
| **TOTAL** | **~720** |

## PR plan

Kept under the 400-line single-PR budget by splitting at the data boundary.

1. **PR 1 - room record decode + `--level-geometry pages=`**
   (`feat/room-texture-record`): typed record view, `+0x34/+0x38` bounds,
   `B` lookup, page mask, `pages=` summary, synthetic tests.
   Production ~130, tests ~195, docs ~30, TOTAL ~355.
2. **PR 2 - room sample + textured render** (`feat/room-textured-render`):
   `--level-sample` one-line-per-face sampler and `--level-render --textures`
   bridge into the existing textured pipeline, with flat fallback and manual
   playtest. Production ~130, tests ~195, docs ~40, TOTAL ~365.

This pass adds no code; it adds the note and this proposal/spec/design/tasks
set.

# Proposal: object-textures (outcome: source located; decode slice proposed)

## Intent

The archived object models render as untextured triangles because the 16-byte
per-face record was never decoded. This pass asked what that record contains,
where the pixels live, and whether a legacy face can be sampled with real UVs.
Probe-first; a documented no-go was acceptable.

## Outcome

**The record is CONFIRMED as the PC `TextureFT3`.** One record per textured
face:

```text
+0  u8  u0          +4  u8  u1          +8  u8  u2          +12 u32 colour
+1  u8  v0          +5  u8  v1          +9  u8  v2
+2  u16 textureWord (low 11 bits = page, high 5 bits = flags)
+6  u16 attributesA                    +10 u16 attributes
```

**The pixels are NOT in the object container.** The tail is the auxiliary
table. Capacity proves it: `morlock` uses two pages (262,144 bytes) but its
whole payload is 56,684 bytes.

**The pixels are in `1\TEXTURES.BIG`, page = `textureWord & 0x07FF`.** The
container is the legacy PC monolithic texture bank: 4,096-byte prefix, then
256x256 little-endian A1R5G5B5 pages at 131,072 bytes each. No palette, no
index plane, no per-slot header.

**The index order is falsifiable and passes.** Across 394 containers and 32,370
textured faces, page indices are dense 2..378 with **0 inversions** against
archive entry order and three gaps explained by untextured models. Six
multi-page models use consecutive pages.

**A concrete face was sampled.** `raziel` (entry 66) polygon 0 -> page 4 ->
UVs (40,134), (22,134), (41,128) -> RGB8 (123,90,66), (90,57,33), (33,24,16).
Full numbers and the evidence ledger are in `docs/notes/object-textures.md`.

**The loose `TEX\*.DDS` family is NOT the source.** Correlation against the
legacy page is at noise level, so the remaster's named textures are a parallel,
re-authored family.

## What changed versus prior assumptions

- The 16-byte record is not opaque: the fields are known and testable.
- `materialOffset` is a per-face record, not a shared material table; two
  identically-textured faces still get two records.
- The texture reference is an 11-bit page index into a monolithic bank, not a
  pointer, not a palette index, and not a loose DDS name.
- The legacy object textures exist in this install after all; they are shipped
  as the `TEXTURES.BIG` bank, not as `<object>_<id>.DDS`.

## Proposed slice (do not build in this pass)

Three small, verifiable steps that turn the note into a textured view:

1. Extend the object reader with a typed `TextureFT3` view: UV triples,
   page index, attribute words and colour. `--object-info` gains a
   `pages=<a,b,...>` field only (no texels).
2. Add a pure texture-bank reader for `TEXTURES.BIG`: page bounds, A1R5G5B5 to
   RGBA8, and a `--object-sample --bigfile ... --entry ... --textures ...`
   arm that prints one line per face: page, three UVs and three RGB values. No
   window, no imagery written.
3. Feed the sampled RGBA8 page and the UVs into the existing textured mesh
   pipeline so `--object-mesh --textures <TEXTURES.BIG>` draws the archived
   object with its real texture.

## No-go condition (explicit)

Refuse loud and stop the slice if, at apply time, the page index is outside the
`TEXTURES.BIG` header count, the addressed slot is `empty`/`zero`, or the
sampled UV falls outside 0..255. Never clamp silently and never substitute a
loose DDS.

## Forecast split

| Part | Lines |
|---|---:|
| Production (record view + bank reader + sample/render arms) | ~300 |
| Tests (synthetic records and synthetic bank; bounds, mask, refusal cases) x1.5 | ~450 |
| Docs (note reader section, README line, change docs) | ~80 |
| **TOTAL** | **~830** |

## PR plan

Kept under the 400-line single-PR budget by splitting at the data boundary.

1. **PR 1 - record decode + `--object-info` pages** (`feat/object-texture-record`):
   typed `TextureFT3`, page mask, `pages=` summary, synthetic tests.
   Production ~120, tests ~180, docs ~30, TOTAL ~330.
2. **PR 2 - bank reader + `--object-sample`** (`feat/object-texture-bank`):
   `TEXTURES.BIG` page bounds and A1R5G5B5 decode, one-line-per-face sampler,
   synthetic bank tests. Production ~120, tests ~180, docs ~30, TOTAL ~330.
3. **PR 3 - textured render bridge** (`feat/object-textured-render`): sampled
   page plus UVs into the existing mesh path, manual playtest. Production ~60,
   tests ~90, docs ~20, TOTAL ~170.

This pass adds no code; it adds the note and this proposal/spec/design/tasks
set.

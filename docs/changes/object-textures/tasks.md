# Tasks: object-textures (record decode, bank sampler, textured render)

## T1 - `TextureRef` view in the object parser

- Add a typed view: three UV byte pairs, raw texture word + masked page,
  `attributesA`, `attributes`, colour.
- Store it per polygon; leave the existing material-offset validation intact.
- Acceptance: synthetic records decode to the documented fields; a page with
  bit 11..15 set still yields the low 11-bit index.

## T2 - Pure sampler module

- `page = word & 0x07FF`, texel address `2*(u + 256*v)`, A1R5G5B5 -> RGBA8 by
  bit replication; alpha from bit 15.
- Refuse an address past the page, an empty/zero page, or a page index past the
  header count.
- Acceptance: synthetic bank of 3 pages returns exact RGBA8 for known bytes;
  page 3 in a 3-page bank fails; the last texel of the last page is valid.

## T3 - `--object-info pages=`

- Append `pages=<sorted distinct>` (or `-`) to the existing line.
- Acceptance: entry 66 -> `pages=4`; entry 252 -> `pages=52,53`.

## T4 - `--object-sample`

- New arm: `--bigfile <DAT> --entry <n> --textures <BIG>`.
- One line per textured face with page, three UVs and three RGB triples; no
  window and no files written.
- Acceptance: synthetic fixture prints exactly the line in `spec.md`.

## T5 - `--object-mesh --textures`

- Without `--textures`: unchanged untextured mesh.
- With `--textures`: upload one RGBA8 page per referenced page and draw the
  existing triangles with the decoded UVs.
- Acceptance: entry 66 opens a textured mesh; an out-of-range page exits before
  a window opens.

## T6 - Tests and regression

- Tests use synthetic bytes only: record fields, mask, UV bounds, page bounds,
  empty/zero page, usage errors, archive-entry out of range.
- Run `cargo test`; confirm the level, mesh, texture and audio arms are
  unchanged; `git status` only `*.rs`/`*.md`.
- Acceptance: suite green, no game data staged.

## Forecast

| Part | Lines |
|---|---:|
| Production (T1-T5) | ~300 |
| Tests (T6) x1.5 | ~450 |
| Docs (note reader section, README, change docs) | ~80 |
| **TOTAL** | **~830** |

## PR plan

1. **PR 1** (`feat/object-texture-record`): T1 + T3. Production ~120, tests
   ~180, docs ~30, TOTAL ~330.
2. **PR 2** (`feat/object-texture-bank`): T2 + T4. Production ~120, tests ~180,
   docs ~30, TOTAL ~330.
3. **PR 3** (`feat/object-textured-render`): T5 + T6 render regression.
   Production ~60, tests ~90, docs ~20, TOTAL ~170.

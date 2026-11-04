# Tasks: room-textures (record decode, page summary, textured room)

## T1 - Room texture table and record view

- Add a typed view for room `+0x34`/`+0x38`: start, end, span, record count.
- Add a 12-byte record decode: three UV byte pairs, raw word + masked page,
  `attributesA`, `attributes`.
- Add the face `+0x0A` lookup (`start + B`) and classify `B == 0xFFFF` and
  `B % 12 != 0` as `tail`.
- Keep the existing vertex/face/object readers byte-for-byte unchanged.
- Acceptance: synthetic room decodes to the documented fields; a span of 1,919
  or `B` of 13 is a loud error; `B = 0xFFFF` is `tail`, not an error.

## T2 - Page summary and tail count

- `--level-geometry` appends `pages=<sorted distinct>` (or `-`) and
  `tail=<n>` to the existing line.
- Acceptance: entry 115 -> `pages=379,380,381,382,383,384,385`, `tail=2`.

## T3 - `--level-sample`

- New arm: `--bigfile <DAT> --entry <n> --textures <BIG>`.
- One line per face in face-array order with page, three UVs and three RGB
  triples; `status=tail` for the tail cases; no window and no files written.
- Acceptance: synthetic fixture prints exactly the line in `spec.md`.

## T4 - `--level-render --textures`

- Without `--textures`: unchanged untextured room.
- With `--textures`: upload one RGBA8 page per referenced page and draw the
  existing room triangles with the decoded UVs; report `tail=<n>`.
- Acceptance: entry 115 opens a textured room; an out-of-range page exits
  before a window opens.

## T5 - Models-free and page-range checks

- Cover a unit with zero objects (`aluka11`) and a unit whose page range is a
  single page (`under4`).
- Acceptance: same face/page/tail counts as the note; the object array is never
  read on this path.

## T6 - Tests and regression

- Tests use synthetic bytes only: record fields, `+0x34/+0x38` bounds, `B`
  alignment and range, `0xFFFF`, page mask, page bounds, empty/zero page, usage
  errors, archive-entry out of range.
- Run `cargo test`; confirm the level, mesh, texture and audio arms are
  unchanged; `git status` only `*.rs`/`*.md`.
- Acceptance: suite green, no game data staged.

## Forecast

| Part | Lines |
|---|---:|
| Production (T1-T4) | ~260 |
| Tests (T5-T6) x1.5 | ~390 |
| Docs (note reader section, README, change docs) | ~70 |
| **TOTAL** | **~720** |

## PR plan

1. **PR 1** (`feat/room-texture-record`): T1 + T2. Production ~130, tests ~195,
   docs ~30, TOTAL ~355.
2. **PR 2** (`feat/room-textured-render`): T3 + T4 + T5 + T6. Production ~130,
   tests ~195, docs ~40, TOTAL ~365.

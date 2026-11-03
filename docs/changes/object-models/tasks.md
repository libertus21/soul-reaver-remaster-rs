# Tasks: object-models (counts reader, then mesh bridge)

## T1 - Object container parser

- Add `src/object.rs`: read `N`, `data_base`, object header, both 12-byte names,
  the `+0x0A` count, the `+0x64` table, and the model header at `table[0]`.
- Bounds-check every field against `[data_base, entry_end)`; `table[0] == 0`
  returns the distinct multi-part refusal.
- Acceptance: a synthetic single-model container yields the header fields; a
  synthetic `table[0] == 0` returns the multi-part error.

## T2 - Vertex, polygon and material views

- Expose positions (8-byte records) and triangles (12-byte records) and the
  material offset set.
- Validate `vertexStart + V*8 == polygonStart`,
  `polygonStart + P*12 == endOfPolygons`, and every index `< V`.
- Acceptance: valid synthetic arrays pass; one index `== V` fails; a bad
  `endOfPolygons` fails.

## T3 - `--object-info`

- Add the CLI arm with `--bigfile <path> --entry <n>`; print exactly the line in
  `spec.md`; counts only.
- Refuse multi-part containers with their own message.
- Acceptance: entry 66 -> 299/640/66/0; entry 252 -> 178/334/37/0; entry 119 ->
  3/1/1/0.

## T4 - `--object-mesh`

- Feed parsed positions and triangle indices into the existing mesh pipeline.
- No textures, bones, posing or world transform.
- Acceptance: entry 66 opens a non-empty window mesh; a refused entry exits
  without a window.

## T5 - Tests (synthetic bytes only)

- Valid object; zero polygons; index `== V`; truncated vertex array; bad
  `endOfPolygons`; multi-part refusal; usage error; archive-entry out of range.
- Assert exit codes and the exact line shape; keep the suite green.
- Acceptance: new tests pass; no test reads a game file.

## T6 - Docs and regression

- Add the reader section to `docs/notes/object-models.md`, the README line, and
  an archive entry for this change.
- Run `cargo test`; confirm the existing level, mesh, texture and audio arms are
  unchanged; `git status` only `*.rs`/`*.md`.
- Acceptance: suite green, no game data staged.

## Forecast

| Part | Lines |
|---|---:|
| Production (T1-T4) | ~240 |
| Tests (T5) x1.5 | ~360 |
| Docs (T6) | ~40 |
| **TOTAL** | **~640** |

## PR plan

1. **PR 1** (`feat/object-model-parser`): T1-T3 + synthetic tests. ~375 lines.
2. **PR 2** (`feat/object-mesh-render`): T4-T6 + tests/docs. ~270 lines.

Chained so each PR stays under the 400-line review budget.

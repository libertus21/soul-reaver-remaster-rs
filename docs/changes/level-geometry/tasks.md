# Tasks: level-geometry (counts-only reader)

## T1 - Parse confirmed room geometry fields

- Add a `RoomGeometry` view in `src/level.rs`: room offset from header slot
  `0x00`; counts `+0x08/+0x10/+0x14/+0x18`; offsets `+0x0C/+0x1C/+0x20/+0x24`;
  level-wide 36-byte count at header slot `0x3A`.
- Bounds-check the room record and every array against the data region.
- Acceptance: a synthetic header with known counts yields those counts.

## T2 - Validate the face index array

- Read `V` and `F`; walk `F` 12-byte face records; read the three `u16` at
  `+0/+2/+4`.
- Any index `>= V` is a loud error naming the unit and the counts; exit 2.
- Acceptance: valid synthetic array returns min/max; index `== V` fails.

## T3 - CLI arm and path resolution

- Add `--level-geometry <path>` to the `main.rs` dispatcher, reusing the
  existing unit-or-room-directory resolution and name-sorted iteration.
- Print exactly the line in `spec.md`; no coordinates, names or hex.
- Acceptance: `LAIR5` -> `81/130/80/1/0`, `UNDRCT1` -> `3689/6765/268/30/0`.

## T4 - Tests (synthetic bytes only)

- Valid unit; zero faces; zero vertices; out-of-range index; room outside data;
  truncated face array; directory ordering; usage error.
- Assert exit codes 0/1/2/3 and the exact line shape.
- Acceptance: new tests pass; total suite grows past the 260 baseline.

## T5 - Docs and regression

- Update the `level-geometry` note's reader section and the README line.
- Run `cargo test`; confirm the ten existing level commands are unchanged.
- Acceptance: suite green, `git status` only `*.rs`/`*.md`, no game data.

## Forecast

| Part | Lines |
|---|---:|
| Production (T1-T3) | ~130 |
| Tests (T4) x1.5 | ~195 |
| Docs (T5) | ~35 |
| **TOTAL** | **~360** |

Single PR, under the 400-line slice budget.

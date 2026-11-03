# Spec: object-models (counts-only reader + mesh bridge, proposed)

`--object-info --bigfile <BIGFILE.DAT> --entry <INDEX>` reads one object
container and prints CONFIRMED counts and validation. `--object-mesh` feeds the
same positions and indices into the existing mesh viewer. Neither arm reads
level data, `campath`/`marker`/`splob`, bones or textures. Malformed input fails
LOUD; every existing CLI form keeps its prior behavior.

This spec is for the follow-up slice; no code lands in the docs pass.

## Output contract (`--object-info`)

One line, no coordinates and no bytes:

```text
object <name> vertices=<V> polygons=<P> bones=<B> index_min=<m> index_max=<x> materials=<M> status=ok
```

- `<name>` = the first 12-byte header name, `_` padding stripped.
- `V/P/B` = model header `+0x00/+0x10/+0x18`.
- `index_min/index_max` = min/max of all polygon indices; `0` when `P == 0`.
- `M` = number of 16-byte material records that textured polygons reference.
- A validation failure prints no `object` line and exits 2.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shape: `--object-info` and `--object-mesh` each take exactly `--bigfile <path> --entry <n>`. Missing/extra operands are usage errors + exit 1. Existing modes unchanged |
| R2 | Outer container: `N` at entry offset 0, `data_base = 4*(((N+512)&~511))`; object header and every array MUST stay inside `[data_base, entry_end)`. Out-of-range is exit 2 |
| R3 | Object header: names at `+0x4C/+0x58`, table count from `u16 +0x0A + 1`, table at `+0x64`; `table[0]` is the model header for single-model containers |
| R4 | Model header: `vertexCount/vertexStart` at `+0x00/+0x04`, `polygonCount/polygonStart` at `+0x10/+0x14`, `boneCount` at `+0x18`, `materialStart` at `+0x30` |
| R5 | Validation: `vertexStart + vertexCount*8 == polygonStart`, `polygonStart + polygonCount*12 == endOfPolygons` (`+0x0C`), and every polygon index `< vertexCount`. Any violation is a loud error + exit 2 naming the entry and counts |
| R6 | Multi-part containers (`table[0] == 0`, e.g. `human`, `vlgr`, `paths`) are refused with a distinct `UNKNOWN multi-part object` error, not guessed |
| R7 | `--object-mesh` reuses the existing mesh vertex/index pipeline; it does not add textures, bones, posing or an object transform |
| R8 | Tests use SYNTHETIC byte buffers only; no test reads a game file; no game bytes, absolute paths, coordinates or dumps are committed |
| R9 | No regression: every existing CLI arm keeps its prior messages and exit codes; `cargo test` stays green |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Object parsed and printed / mesh opened |
| 1 | Usage error: wrong argument shape, unknown flag |
| 2 | Malformed container, bad bounds, failed boundary identity, out-of-range index, refused multi-part container |
| 3 | Refused path kind / unreadable input |

## Scenarios

- Entry 66 -> `raziel` 299 vertices / 640 polygons / 66 bones / 0 bad indices.
- Entry 252 -> `morlock` 178 / 334 / 37 / 0.
- Entry 119 -> `undhost` 3 / 1 / 1 / 0.
- Entry 140 -> `warpg` 3 / 1 / 1 / 0.
- A synthetic polygon index equal to `V` -> exit 2, no `object` line.
- A synthetic `endOfPolygons != polygonStart + P*12` -> exit 2.
- `--entry` for `human`/`vlgr`/`paths` -> distinct multi-part refusal.

## Evidence basis

All fields used by this reader are CONFIRMED in
`docs/notes/object-models.md` (O2-O10). The reader does not interpret the
auxiliary table, the multi-part containers, the bone layout for the 168
non-conforming objects, texture/UV fields, or the world transform.

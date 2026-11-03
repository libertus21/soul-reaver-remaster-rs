# Spec: level-geometry (counts-only reader)

`cargo run -- --level-geometry <PATH>` reads one SR1-remaster `.PCM` unit, or a
room directory containing units, and prints CONFIRMED geometry and placement
**counts**. It does not print coordinates, bytes, names, offsets or the trailing
vertex attribute, and it never opens a window. Malformed input fails LOUD with
exit 2; a refused path exits 3; a bad invocation exits 1. Every existing CLI
form keeps its prior behavior.

## Output contract

One line per unit, sorted by unit name:

```text
geometry <unit> vertices=<V> faces=<F> c=<C> objects=<O> index_min=<m> index_max=<x> table36=<T>
```

- `V/F/C` = room source counts at `+0x10/+0x14/+0x18`.
- `O` = room object count at `+0x08`.
- `index_min/index_max` = min/max of the first three `u16` of every 12-byte face
  record; `0` when `F == 0`.
- `T` = level-wide 36-byte table count (header slot `0x3A`), 0 for most units.
- No positions, no object names, no strides in the line (strides are fixed:
  `0x0C` vertex/face, `u16` indices).

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shape: exactly `--level-geometry <path>`. Zero/extra args or an unknown flag is a usage error + exit 1. Existing modes unchanged |
| R2 | Path kinds: a `.PCM` unit or a directory (resolve `BIN` when present, else the directory itself), matching the existing `--level-structure` policy. A refused path exits 3; a missing/unreadable path exits 2 |
| R3 | Container: `N` at file offset 0, `data_base = 4 * (((N + 512) & ~511))`, room offset at header slot `0x00`. `data_base` and the room record MUST be inside the file; a count that would leave the data region is an error + exit 2 |
| R4 | Face validation: every one of the three `u16` of every face record MUST be `< V`. The first violation is a loud error + exit 2 naming the unit and counts; no partial line is printed |
| R5 | Output: exactly the contract above. Directory input prints one line per unit sorted by name. Counts only; no coordinates, names, hex, ids, offsets or bytes |
| R6 | Non-goals: no mesh export, rendering, collision, orientation, array C semantics, trailing vertex attribute, object spawning. No module outside `src/level.rs` and the `main.rs` arm may change |
| R7 | Tests use SYNTHETIC byte buffers only; no test reads a game file; no game bytes, absolute paths, coordinates or dumps are committed; the whitelist `.gitignore` stays |
| R8 | No regression: `--mesh`, `--pose`, `--bigfile`, `--textures`, `--audio`, `--level-info`, `--level-records`, `--level-structure`, `--level-links` keep their prior messages and exit codes |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | At least one unit parsed and printed |
| 1 | Usage error: wrong argument shape, unknown flag |
| 2 | Malformed container/I/O: bad `N`, room outside data, truncated arrays, face index out of range, no units |
| 3 | Refused path kind |

## Scenarios

- `LAIR5` -> `vertices=81 faces=130 c=80 objects=1 table36=0`.
- `LAIR1` -> `vertices=2829 faces=5571 c=938 objects=23 table36=6`.
- `UNDRCT1` -> `vertices=3689 faces=6765 c=268 objects=30 table36=0`.
- A synthetic face index equal to `V` -> exit 2, no `geometry` line.
- A room directory -> lines sorted by unit name.
- `--level-structure` and `--level-links` output unchanged.

## Evidence basis

All fields used by this reader are CONFIRMED in
`docs/notes/level-geometry.md` (G1-G3, G8, P1-P3, B2). The reader does not
interpret the UNKNOWN fields.

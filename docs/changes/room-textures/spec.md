# Spec: room-textures (record decode, page summary, room sampling; proposed)

`--level-geometry` gains a page summary. A new `--level-sample` prints measured
per-face texel values for one room. `--level-render --textures <TEXTURES.BIG>`
draws the room with its bank pages. No arm reads a loose `.DDS`. Malformed
input fails LOUD; every existing CLI form keeps its prior behaviour.

This spec is for the follow-up slice; no code lands in the docs pass.

## Output contracts

`--level-geometry` appends one field to the existing summary:

```text
level <name> vertices=<V> faces=<F> ... pages=<a,b,...> tail=<n> status=ok
```

- `pages` = sorted distinct `textureWord & 0x07FF` over records reachable from
  aligned faces; `-` when there are none.
- `tail` = number of faces whose `B` is `0xFFFF` or not 12-aligned; always
  printed, never folded into `faces`.

`--level-sample --bigfile <DAT> --entry <n> --textures <BIG>` prints one line
per face in face-array order, with no coordinates:

```text
face <i> page <p> uv (<u0>,<v0>) (<u1>,<v1>) (<u2>,<v2>) rgb (<r0>,<g0>,<b0>) (<r1>,<g1>,<b1>) (<r2>,<g2>,<b2>) alpha <a0><a1><a2>
```

- RGB is 8-bit after bit-replication expansion; alpha is the 1-bit value
  `0`/`1` per vertex.
- A tail face prints `face <i> status=tail` and is not sampled; exit stays 0.
  This is explicit, not a silent skip.
- `--group <PREFIX>` may be accepted for the same line-per-unit form; the
  first unit only is out of scope here.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shape: `--level-sample` takes exactly `--bigfile <path> --entry <n> --textures <path>`; missing/extra operands are usage errors + exit 1. Existing modes unchanged |
| R2 | Room record: `header slot 0x00`; `+0x14` face count / `+0x20` face offset, stride 12; `+0x34` record start / `+0x38` record end, both data-base-relative, stride 12. A record or array outside the data region is a distinct error |
| R3 | Face `+0x0A` is a byte offset into the `+0x34` array, **not** a face index and **not** data-base-relative. `B < end-start` AND `B % 12 == 0` selects the record at `start+B`. `B == 0xFFFF` or a non-aligned `B` is a **tail** face (counted, not sampled) |
| R4 | Record fields exactly as `docs/notes/room-textures.md` G5: UV bytes `+0/+1`, `+4/+5`, `+8/+9`; `textureWord` `+2`; `attributesA` `+6`; `attributes` `+10`. Face `+0x08` (`A`) MUST NOT be read as the page |
| R5 | Page index = `textureWord & 0x07FF`. Bits 11..15 MUST NOT be interpreted as part of the index |
| R6 | UVs are bytes; any value 0..255 is accepted as a byte. A texel address outside the 256x256 page is a loud error, never clamped |
| R7 | Bank: `u16 512` + `u16 count`, data at 4,096, stride 131,072, 256x256 A1R5G5B5. A page index `>= count` is a loud error naming both numbers |
| R8 | Empty/zero slots are refused, never rendered as black |
| R9 | A1R5G5B5 expansion is bit replication `(v << 3) \| (v >> 2)`; alpha is the top bit only |
| R10 | `--level-render --textures <BIG>` samples per aligned face and feeds one RGBA8 page per referenced page plus the UVs into the existing textured pipeline; tail faces are skipped and reported as `tail=<n>`. Without `--textures` the room arm is byte-for-byte unchanged |
| R11 | Tests use SYNTHETIC records and a SYNTHETIC bank only; no test reads a game file; no game bytes, absolute paths or dumps are committed |
| R12 | No regression: every existing CLI arm keeps its prior messages and exit codes; `cargo test` stays green |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Info printed / sample printed / textured room opened |
| 1 | Usage error: wrong argument shape, unknown flag |
| 2 | Malformed container, bad record/array bounds, bad UV, page out of range, empty/zero page |
| 3 | Refused path kind / unreadable input |

## Scenarios

- `--level-geometry` entry 115 -> `pages=379,380,381,382,383,384,385`,
  `tail=2`.
- `--level-sample` entry 115 face 3 -> `page 385`, UVs
  `(64,64) (64,126) (126,64)`, RGB `(49,49,49) (66,66,57) (49,49,49)`,
  alpha `111` (manual cross-check, not a unit test).
- `--level-sample` entry 133 (`under2`) last two faces -> `status=tail`
  (their `B` is 40, not a multiple of 12).
- A synthetic record with page 9999 in a 3-page bank -> exit 2.
- A synthetic `+0x38 <= +0x34` -> exit 2.
- A synthetic `B = 12` with a 12-byte span (out of range) -> exit 2.
- `--level-render` without `--textures` -> unchanged untextured room.
- A model-free unit (`aluka11`, objects 0) -> same page and face counts as any
  other room.

## Evidence basis

The face tail split, the `B` indirection, the record layout, the page mask,
the bank geometry and the room/object page partition are CONFIRMED in
`docs/notes/room-textures.md` (G1-G15). The reader does not interpret face
`A`, the flag byte, the record `attributes` meaning, the tail sub-records, or
the `"<level>,<index>"` table behind room `+0x30`.

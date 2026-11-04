# Spec: object-textures (record decode, bank sampling, textured render; proposed)

`--object-info` gains a page summary. A new `--object-sample` prints measured
per-face texel values. `--object-mesh --textures <TEXTURES.BIG>` draws the
archived object with its bank page. No arm reads a loose `.DDS`. Malformed
input fails LOUD; every existing CLI form keeps its prior behaviour.

This spec is for the follow-up slice; no code lands in the docs pass.

## Output contracts

`--object-info` appends one field to the existing line:

```text
object <name> vertices=<V> polygons=<P> materials=<M> bones=<B> pages=<a,b,...> status=ok
```

- `pages` = sorted distinct `textureWord & 0x07FF` values across textured faces;
  `-` when there are none.

`--object-sample --bigfile <DAT> --entry <n> --textures <BIG>` prints one line
per textured face, no coordinates:

```text
face <i> page <p> uv (<u0>,<v0>) (<u1>,<v1>) (<u2>,<v2>) rgb (<r0>,<g0>,<b0>) (<r1>,<g1>,<b1>) (<r2>,<g2>,<b2>) alpha <a0><a1><a2>
```

- RGB is 8-bit after bit-replication expansion; alpha is the 1-bit value
  `0`/`1` per vertex.
- Non-textured faces are skipped.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shape: `--object-sample` takes exactly `--bigfile <path> --entry <n> --textures <path>`; missing/extra operands are usage errors + exit 1. Existing modes unchanged |
| R2 | Record: stride 16, fields exactly as `docs/notes/object-textures.md` T2; every textured polygon's `materialOffset` MUST land on a 16-byte record inside the payload (already enforced) |
| R3 | Page index = `textureWord & 0x07FF`. The reader MUST NOT interpret bits 11..15 as part of the index |
| R4 | UVs are bytes `+0/+1`, `+4/+5`, `+8/+9`; any value > 254 is accepted as a byte but a texel index outside the page is a loud error |
| R5 | Bank: `u16 512` + `u16 count`, data at 4,096, stride 131,072, 256x256 A1R5G5B5. A page index `>= count` is a loud error naming both numbers |
| R6 | Empty/zero slots are refused, never rendered as black |
| R7 | A1R5G5B5 expansion is bit replication `(v << 3) \| (v >> 2)`; alpha is the top bit only |
| R8 | `--object-mesh --textures <BIG>` samples per face and feeds a single RGBA8 page + UVs into the existing textured pipeline; without `--textures` the mesh arm is byte-for-byte unchanged |
| R9 | Tests use SYNTHETIC records and a SYNTHETIC bank only; no test reads a game file; no game bytes, absolute paths or dumps are committed |
| R10 | No regression: every existing CLI arm keeps its prior messages and exit codes; `cargo test` stays green |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Info printed / sample printed / textured mesh opened |
| 1 | Usage error: wrong argument shape, unknown flag |
| 2 | Malformed container, bad record bounds, bad UV, page out of range, empty/zero page |
| 3 | Refused path kind / unreadable input |

## Scenarios

- `--object-info` entry 66 -> `pages=4`; entry 252 -> `pages=52,53`.
- `--object-sample` entry 66 face 0 -> `page 4`, UVs `(40,134) (22,134) (41,128)`,
  RGB `(123,90,66) (90,57,33) (33,24,16)` (manual cross-check, not a unit test).
- A synthetic record with page 9999 in a 3-page bank -> exit 2.
- A synthetic UV 255 into a 256-wide page is valid; UV indexing is checked in
  arithmetic, not by clamping.
- A synthetic empty slot at the referenced page -> exit 2.
- `--object-mesh` without `--textures` -> unchanged untextured mesh.

## Evidence basis

Page semantics, the mask, the bank geometry and the one-record-per-face rule
are CONFIRMED in `docs/notes/object-textures.md` (T1-T12). The reader does not
interpret texture-word flag bits, the two attribute words, the colour blend, or
the auxiliary table.

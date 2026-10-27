# `.SRA` probe - decode ledger (2026-10-27)

Planning note for the `sra-animation` change. Read-only probe of the SR1
remaster game data (`1\OBJECT\`); nothing under the install was written, no
game bytes were copied into the repo, and scratch lived only in the session
temp directory. Aggregates and shapes only: no file listings, no raw
coordinates, no dumps, no absolute install paths.

## Box (what was and was not done)

Three passes, no Ghidra, no disassembly:

1. Census, size classes, naming, sibling types.
2. Header model and payload classification on every file.
3. Community tools and sibling formats.

Bounded to roughly 90 minutes of probing. Anything not answered here is
listed as UNKNOWN, not guessed.

## Census

- 227 `.SRA` files, **all** under `OBJECT`, alongside 349 `.SRM`, 60 `.SRL`
  and 29 `.PCM`. No `.SRA` anywhere else in the SR1 tree.
- Total 3,828,348 bytes. Min 2,084; p25 2,308; median 3,236; p75 12,432;
  p90 41,176; p99 125,448; max 533,444.
- 134 distinct SHA-256 contents; 93 files are byte-identical duplicates of
  another file, in clusters of up to 36 and 30. A large part of the set is
  repeated placeholder/static content.
- Basenames are 4-8 uppercase alphanumerics. Only 1/227 basenames also
  exist as an `.SRM`; 13/227 match any other file (12 `.PCM`, 1 `.SRM`).

Evidence class: observed.

## Header model

| Field | Shape | Evidence class |
|---|---|---|
| Word 0 | declaration count `N`, observed 2..1054 | Observed |
| Word 1 | `0` in 227/227 | Observed |
| Word 2 | `4` (144 files), `28` (79), or one of 36/40/68/140 | Observed |
| Table | `N` ascending offsets from word 2, then a `0` terminator in 227/227 | Observed |
| Header length | `max(2048, 8 + 4*(N+1))` | Derived (strong) |
| Payload | `[header_len, EOF)` | Derived |

The 2 KiB floor is the load-bearing part: in 225/227 files the first
non-zero byte after the table lands exactly at
`max(2048, 8 + 4*(N+1))`. The two exceptions are the single largest table
(whose data is pushed to the next 2 KiB boundary) and one file with a
one-byte skew. In the largest table, 822 of 1,053 gaps are exactly 4 bytes.

No text magic exists in any file. The format is identified by structure
only.

Evidence class: derived (strong) for the floor and the length rule.

## Payload

The payload is mixed, not a single homogeneous array:

- **Packed integer records.** A small count, an `0xFFFFFFFF` marker, a
  `0x0064xxxx` triple repeated three times, and a following small value.
  The `0x64` (100) constant recurs across unrelated files, which suggests a
  fixed-point scale. Meaning UNKNOWN.
- **Smooth float runs.** Strictly increasing, bounded to `0..360`, with
  steps well below 30. A 0.5 -> 70.4 ramp with roughly 6.7 per sample was
  reproduced on a small prop. 156/227 files contain at least one such run;
  count per file ranges 0..323 (median 1); 71 files have none.

Negative tests (all ran clean-room on the census):

| Test | Result | Consequence |
|---|---|---|
| Quaternion array (4-float window, norm in `0.9..=1.1`) | 3.2% of windows - near chance | no contiguous quaternion track |
| Parent array (`u16` in `1..=127`) | longest run = 8 | no 128-slot hierarchy |
| Index array (`u32` in `1..=255`) | longest run = 15 | no 128-entry index table |
| Name table (printable runs) | 107 files show runs, all float-bit noise on inspection | no names |

Evidence class: observed for the tests and run maxima; derived (strong) for
the curve-run shape; inferred (not proven) that the runs are rotation
angles in degrees.

## Verdict

- **Hierarchy: NO.** No parent indices, no child lists, no bone names, no
  contiguous small-index array anywhere in 227 payloads.
- **Clips: NO catalog.** No clip name, count, duration, or frame-rate field
  survives a cross-file check. Curves exist, but with no clip boundary and
  no time base they are an undifferentiated stream.
- **Binding to `.SRM`: NO.** `.SRA` names do not track `.SRM` names
  (1/227) and no bone-index field was found, so a curve cannot be attached
  to a joint slot without inventing the mapping.

**Consequence.** One-bone posing cannot be generalized to real animation
from these files. Any claim that a `.SRA` curve rotates bone `k` would be
fabricated. This is a documented no-go.

## What stays UNKNOWN

1. Channel-to-joint binding.
2. Time base, frame rate, and the meaning of `N` (frames, blocks, clips).
3. Clip boundaries and any per-clip record.
4. The packed header fields (small count, `0xFFFFFFFF`, `0x0064xxxx`).
5. Why the largest file's table overflows the 2 KiB floor and pads to the
   next boundary.
6. `.SRL` record semantics (see below).

## Sibling formats (census only)

- **`.SRL` (60 files, 2.8 KiB - 1.7 MiB).** ASCII magic `SRL.`, a second
  word of `1`, a count word, and a fixed record stride: size is
  approximately `count * 1200` (1,418 records -> 1,701,620 bytes). No name
  overlap with `.SRA` or `.SRM`; no readable strings. Consistent with a
  placement/record array, not a skeleton. Out of scope.
- **`.PCM` (29 files).** Not probed. Community tools read the original-game
  `.pcm`, not this remaster variant.
- **`.SRM` (349).** Already decoded: flat 128-slot joint pool, no
  hierarchy, no keyframes (see `srm-bone-block.md`).

## Community tools (exact URLs + licenses)

| Tool | URL | License | Reads `.SRA`? | Fields documented |
|---|---|---|---|---|
| io_scene_xrm | https://github.com/TheDodylectableX/io_scene_xrm | No LICENSE file (raw fetch 404; repo metadata reports none) | No | `.SRM`/`.CHR`/`.TMT`/`.TRM` import. Source contains zero mentions of SRA, animation, parent, hierarchy, or keyframe; it builds unparented bones from the 48-byte records |
| ModelEx | https://github.com/TheSerioliOfNosgoth/ModelEx | LICENSE present, composite (GitHub classifies NOASSERTION) | No | Original `.drm`/`.pcm` only; README has zero SRA/animation mentions |
| Recombobulator | https://github.com/TheSerioliOfNosgoth/Recombobulator | GPL-3.0 | No | Original `.pcm`/room files; no animation structs for this remaster |
| AsunderNoMore | https://github.com/TheSerioliOfNosgoth/AsunderNoMore | MIT | No | Bigfile packer; no model or animation |

No documented remaster `.SRA` parser or animation importer was found. The
session's web-search provider returned HTTP 403, so the check above was run
against GitHub raw files and repository trees directly (reproducible from
the URLs). Absence in those four tools is evidence of absence in the
public tool set as of this probe, not proof that no private tool exists.

## Next candidates

1. **`.SRL` scoping** - magic, count, ~1200-byte records. If it is a
   placement/scene format it unblocks scene assembly, not animation. Cheap
   to scope.
2. **Ghidra on the loader** - the only route left for the `.SRA` header
   semantics and channel binding. Deferred by the repo rule until
   community formats are exhausted; this probe exhausts them.
3. **SR2 parity** - the SR2 install may ship the same `.SRA` family with a
   documented difference; not probed here.
4. **`.PCM`** - 29 files, undecoded; low animation value.

## Legal

Counts, field shapes, run lengths, and aggregate statistics only. No game
bytes, no absolute install paths, no per-file coordinates, no hashes, no
dumps, no listings. Game data was read in memory and never written.

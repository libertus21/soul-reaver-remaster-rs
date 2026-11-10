# aim-stream-bounds - the aim sub-slices are delimited, the 16-byte header is a
# per-slot track header, and the aim short is Q12 fixed point (2026-11-09)

Read-only extension of `docs/notes/slot-descriptor-parse.md`. The previous slice
left two questions open: the per-slot aim record length and the meaning of the
16-byte structure after the descriptor array. This slice delimits the aim stream,
decodes that structure, and pins the fixed-point unit. Nothing was launched,
attached or instrumented for this slice. No engine behaviour changed, no golden
moved, and no game bytes, addresses, file offsets or install paths are recorded
here.

Evidence classes per claim: **CONFIRMED** (observed bytes with a falsifiable
shape), **PARTLY** (structure observed, one side open), **UNKNOWN** (not
established).

## Headline

1. **Stream bounds.** An active slot's aim sub-slice is
   `[aim_offset, aim_next)` for every slot except the last. The last sub-slice is
   `[aim_offset, aim_offset + 24 + frame_count * 20)`, where `frame_count` is read
   from the slot's track header. A sub-slice is a 24-byte header plus
   `frame_count` 20-byte frames.
2. **Table verdict.** The structure after the descriptor array is **not** a
   fixed-stride 16-byte array. It is one 16-byte track header per active aim
   slot, packed at the start of each slot's record block; the distance between
   headers is the slot's aim gap, not 16. Only the count and the destination
   offset are evidence-backed; the remaining 8 bytes stay UNKNOWN.
3. **Unit verdict.** The best-supported aim scale is **Q12 (`4096 == 1.0`,
   `>>12`)**, because that is the remaster's only documented animation
   fixed-point shift. The aim-specific magnitude is small and the absolute
   direction-versus-angle meaning stays PARTLY.

| Question | Verdict | Class |
|---|---|---|
| Sub-slice bounds for non-last slots | `[offset, next larger offset)` | CONFIRMED |
| What terminates the last sub-slice | Its track header's `frame_count` (`24 + count * 20`), not a sentinel | CONFIRMED for 167/168 aim models |
| Are the offsets monotonic per model? | Yes, in descriptor order, 0 violations across the corpus | CONFIRMED |
| Are sub-slice lengths whole 3-i16 triples? | No; none is divisible by 6 | CONFIRMED |
| Smallest divisor of every observed gap | 4 bytes (also 2) | CONFIRMED |
| Is the 16-byte structure a fixed-stride table? | No; headers are at variable positions | CONFIRMED |
| Entry count per model | Number of slots with a non-zero aim offset | CONFIRMED |
| Track-table hypothesis | Supported in shape (count + destination + divisor-like fields), refuted as a flat 16-byte array | PARTLY |
| Aim fixed-point unit | Q12 (`4096 == 1.0`), the documented animation scale | PARTLY |

## A. Delimiting the stream

The descriptor's `+20` word is a base-relative offset into one shared aim region.
For the canonical body model (66 slots, three active) the three sub-slice lengths
are **64, 44 and 44 bytes**. The first two come from the next offset; the last is
derived in section A.2.

### A.1 Corpus distribution and divisibility

The corpus scan covered **394 valid object containers**; **168** carry at least
one aim offset and **40** carry at least two, giving **65 finite gaps**.

| Gap length | Count |
|---:|---:|
| 44 | 59 |
| 64 | 5 |
| 52 | 1 |

- **No gap is divisible by 6.** Every gap is `2` or `4` modulo 6, so a sub-slice
  is **not** a whole number of 3-i16 triples. The triple reading in the earlier
  note sampled the first words of the 24-byte header; it is not the element
  structure.
- **The smallest common divisor is 4 bytes** (2 also divides every gap). That is
  the raw structural floor, not the record model.
- **The record model is `24 + frame_count * 20`.** For **64 of 65** finite gaps,
  `gap - 24` is an exact multiple of the track header's `frame_count`, and the
  quotient is always **20**. The single exception is `bellow` (gap 52; only one
  of its two headers resolves, so the second offset cannot be reconciled). It is
  reported as a violation, not smoothed over.
- **Monotonicity:** taking the non-zero aim offsets in descriptor order, **0
  models violate ascending order** across the corpus.

### A.2 What terminates the last sub-slice

There is no sentinel: the values continue in the same small signed band past the
last slot. The termination is the **frame count** in the slot's track header.
Applying the same `24 + count * 20` rule to the last slot gives its end:

| Slot | Header count | Length | Termination |
|---:|---:|---:|---|
| 1 | 2 | 64 | next offset |
| 31 | 1 | 44 | next offset |
| 41 | 1 | 44 | frame count |

The rule is falsifiable: a last slot with no resolvable header falls back to the
24-byte header only, and the code marks that case explicitly (`HeaderOnly`)
instead of reading to the payload end.

## B. The 16-byte track header

The count-like dword and the offset-like dword are real, but the surrounding
structure is not a flat table.

### B.1 Stride and entry count

- The header **is 16 bytes**: `frame_count` (`u32`), `frame_offset` (`u32`), and
  an 8-byte tail.
- The **stride is not 16**. Headers sit at the start of each active slot's record
  block, so consecutive headers are separated by that slot's aim gap (64, 44, …).
- The **entry count equals the number of slots with a non-zero aim offset**. The
  canonical body model has three entries for its three active slots. Across the
  corpus, every active aim slot has exactly one header.
- The header block begins after the descriptor array plus a **0, 8 or 16-byte
  preamble** (76 / 73 / 19 models respectively). The parser scans that bounded
  window for the first header instead of assuming a fixed preamble.

### B.2 Fields

| Field | Evidence | Verdict |
|---|---|---|
| `frame_count` | `gap == 24 + frame_count * 20` for 64/65 finite gaps; observed counts 1, 2, 4, 6, 8, 12, 18, 20, 24, 32, 36, 38, 62 | CONFIRMED |
| `frame_offset` | Always equals `aim_offset + 24`, i.e. the first frame after the 24-byte header | CONFIRMED |
| 8-byte tail | Zero on some entries, non-zero on others; no stable role | UNKNOWN |

`frame_offset` resolves inside the model payload. It is the start of the frame
region, not a pointer to a new structure.

### B.3 Track-table hypothesis

**Supports:** the record is 16 bytes; it carries a count and a destination offset;
the destination resolves to the frame region; and the shape matches the
`destination + frame count + divisor` family named by
`docs/notes/animation-loader-trace.md`.

**Refutes:** the entries are **not** a fixed 16-byte-stride array (the stride is
the aim gap); the remaining 8 bytes are zero on many entries, so a non-zero
divisor is **not** observed; and the runtime memory-form track entry seen for a
track-bearing class uses an 8-byte destination pointer with a different field
order. The file-form header is therefore **track-like**, not proven to be the
exact runtime `TrackEntry`.

## C. Fixed-point unit

Observed aim shorts (body models) stay in a small signed band: gaps sampled from
the canonical head run roughly **-50..+225**. Candidates:

| Unit | Evidence for | Evidence against | Status |
|---|---|---|---|
| **Q12 (`4096 == 1.0`, `>>12`)** | The remaster documents `>>12` as its animation fixed-point conversion (`docs/notes/spline-animation.md` section 2.1); the trace calls the aim shorts fixed-point | The aim-specific shift is not directly observed; raw magnitudes are ~0.005–0.055 of 1.0 | **Best supported** |
| Q8 (`256 == 1.0`) | Would make observed values ~±0.9, plausible direction cosines | No engine documentation | Alternative |
| Unit 1 (raw degrees) | Values read directly as plausible angles (max ~225°) | Contradicts the tri-axial-vector reading; no engine documentation | Rejected |
| `angle * 2^n` for n > 0 | — | Observed values are not the scaled angles a `×2^n` unit would produce | Refuted |

**Verdict:** Q12 is the best-supported **arithmetic** scale because it is the
only documented shift in the remaster's animation path. The small magnitude means
the triples behave like small correction components (or are normalized before
use), not like absolute unit vectors. The absolute
direction-versus-angle meaning and the exact aim-specific shift stay **PARTLY**.

## D. Implementation

`src/slot_descriptor.rs` (pure, not wired into the viewer):

- `AimSlice` / `AimTermination` and `aim_slices(descriptors, payload)` - real
  bounds with `NextOffset`, `FrameCount(u32)` and `HeaderOnly` terminations.
- `TrackEntry` / `track_entries(descriptors, payload)` - typed 16-byte headers
  with `frame_count`, `frame_offset` and a raw UNKNOWN 8-byte tail.
- `FixedPoint` / `AIM_UNIT` - Q12 arithmetic (`4096 == 1.0`, `>>12`).
- `aim_record` is now a thin wrapper over `aim_slices`.
- 18 unit tests: next-offset bounds, the frame-count last bound, the header-only
  fallback, all three preambles, count/offset/tail extraction, the malformed
  header stop, and the Q12 arithmetic. Synthetic fixtures only; no game bytes.

`src/main.rs` keeps the module behind `#[allow(dead_code)]`; it is not wired into
any CLI arm, so headless digests are unchanged.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| A sub-slice is `24 + count * 20` bytes | a gap whose `gap - 24` is not `count * 20` | Tested: 64/65 agree; `bellow` is the outlier |
| The last sub-slice ends at its frame count | a last slot with no resolvable header, or bytes at the derived end that belong to the next aim record | Tested on the canonical model; fallback is explicit |
| Sub-slices are whole 3-i16 triples | a gap divisible by 6 | Refuted: 0/65 divisible by 6 |
| The 16-byte structure is a flat table | headers at a constant 16-byte stride | Refuted: headers follow the aim gaps |
| The header is a runtime track entry | a non-zero divisor in the tail, or the exact runtime field order | Not observed; the tail stays UNKNOWN |
| The aim short is Q12 | an engine aim conversion that shifts by another amount | Not directly observed; the documented animation shift is `>>12` |

## Limits

- The 20-byte frame and the 24-byte header are structural; their per-frame field
  meanings are not decoded here.
- The last-slot bound depends on a resolvable track header. `bellow` violates the
  `24 + count * 20` rule and is reported, not repaired.
- The aim-specific fixed-point shift is inferred from the documented animation
  shift; the observed magnitudes are small.
- This slice parses file-form records only; it does not decode the object
  container, relocate pointers, or compose rotations.

## Legal

Read-only. No game process was launched or instrumented. No game file was
written, no game bytes were copied into the repository, and no disassembly,
address, file offset or install path appears here. Only structure, field roles,
format constants and corpus counts are recorded.

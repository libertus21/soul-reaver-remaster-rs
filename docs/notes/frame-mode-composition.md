# frame-mode-composition - the aim frame is ten signed shorts, the mode bits
# stay UNKNOWN, and the composition ships only its confirmed identity path
# (2026-11-10)

Read-only extension of `docs/notes/aim-stream-bounds.md`. That note delimited
each per-slot aim sub-slice (a 24-byte header plus `frame_count` 20-byte frames)
and pinned the arithmetic unit as Q12 (`4096 == 1.0`, `>>12`), but left the
20-byte frame, the descriptor mode bits and the rotation composition open. This
slice decodes the frame as far as the bytes support, resolves the four named
descriptor words only to a documented UNKNOWN, and ships the confirmed part of
the composition as pure code. Nothing was launched, attached or instrumented. No
engine behaviour changed, no golden moved, and no game bytes, addresses, file
offsets or install paths are recorded here.

Evidence classes per claim: **CONFIRMED** (observed bytes with a falsifiable
shape), **INFERRED** (one side observed, the other reasoned), **UNKNOWN** (not
established).

## Headline

1. **Frame verdict.** A 20-byte aim frame is **ten signed 16-bit words**
   (`20 / 2`). All ten positions carry the same small bidirectional
   fixed-point-like band (`|word| <= 459` across the clean set, zero on 9-26% of
   samples), so there is no padding slot and no `u32` component reading. The
   internal grouping of the ten words (for example three axis triples plus a
   scalar) stays **UNKNOWN**.
2. **Mode-bit verdict.** None of the four named descriptor words `+4`, `+8`,
   `+12`, `+16` carries a resolvable mode bit in the file form. `+4` is constant
   zero; `+8`, `+12` and `+16` are broad per-bone fields with 48 / 22 / 64
   distinct values on active aim slots and 306 / 206 / 351 on inactive slots,
   and no low-bit value partitions the two classes. The mode-bit location is
   **UNKNOWN**.
3. **Composition verdict.** Only the empty path is confirmed: a `Static` slot,
   or a procedural slot with an all-zero aim triplet, has no aim to rotate and
   yields the identity. The object transform is the best-supported rotation base
   for a procedural slot (**INFERRED**). The local aim-to-rotation term and the
   world-target term are **UNKNOWN** and are deliberately not invented.

| Question | Verdict | Class |
|---|---|---|
| Element size of a 20-byte frame | Ten `i16` words (`20 / 2`) | CONFIRMED |
| Are five `u32` a component reading? | No; the high half becomes `0xFFFF....` | CONFIRMED negative |
| Is the frame "three triples + 2 padding bytes"? | No; the tenth word is active (nonzero 74% of clean frames) | CONFIRMED negative |
| Internal grouping of the ten words | Not established | UNKNOWN |
| Are the words Q12 fixed point? | Consistent (`|word| <= 459 << 4096`), same band as the aim shorts | INFERRED |
| Does any word position carry a time, index or weight? | No monotone lane, no bounded weight lane observed | UNKNOWN |
| Do a slot's frames form a smooth temporal sequence? | No; all ten words change between frames | Observed (small sample) |
| Do `+4/+8/+12/+16` carry procedural-orientation mode bits? | No resolvable partition | UNKNOWN |
| Static or zero-aim rotation | Identity | CONFIRMED |
| Procedural rotation base | The object transform | INFERRED |
| Local aim term and world target | Exact formula not established | UNKNOWN |

## A. Decoding the 20-byte frame

### A.1 Corpus and the active-aim rule

The scan covered the **394 named single-model containers** in the SR1 archive
(the same corpus as `aim-stream-bounds.md`). A slot is an active aim slot when
its `+20` word is non-zero, lies at or after the end of the descriptor array,
and resolves inside the payload. Under that rule the corpus gives **167 models**
with at least one active aim slot and **40 models** with at least two, producing
**65 finite gaps**: **59 x 44**, **5 x 64**, **1 x 52**. That reproduces the
prior note's gap histogram exactly; its model count was reported as 168, and the
one-model difference is a boundary case, not a change to the gap rule.

### A.2 Partition candidates and the chosen reading

Gap-derived frames (a non-last slot's length is exactly `24 + count * 20`) give
**69 candidate frames from 39 models**. **58 frames from 35 models** keep every
one of their ten words inside `|word| <= 512` and match the aim band; **11
frames from 4 models** contain at least one word outside that band (up to several
times Q12) and are treated as non-aim outliers, not component values.

| Candidate partition | Evidence | Verdict |
|---|---|---|
| **10 x `i16`** | Every position is a small bidirectional signed value; zero fraction 0.09-0.26; per-position range about `-262..459` | **Best supported** |
| 5 x `u32` | The high half of many words is negative, so a `u32` reading yields values like `0xFFFF....`; not a component | Refuted |
| 3 triples + 2 padding bytes | The tenth word is nonzero in 74% of clean frames and has the same range as the rest | Refuted |
| 3 triples + 1 scalar | Numerically possible but no grouping test separates it | UNKNOWN |

The clean per-position profile (58 frames):

| Position | Range | Nonzero |
|---|---|---:|
| 0 | -221 .. 392 | 48 / 58 |
| 1 | -226 .. 240 | 48 / 58 |
| 2 | -142 .. 300 | 53 / 58 |
| 3 | -247 .. 230 | 46 / 58 |
| 4 | -221 .. 393 | 50 / 58 |
| 5 | -262 .. 243 | 46 / 58 |
| 6 | -117 .. 323 | 51 / 58 |
| 7 | -228 .. 242 | 45 / 58 |
| 8 | -218 .. 459 | 48 / 58 |
| 9 | -154 .. 215 | 43 / 58 |

All ten positions behave alike: none is a monotone counter, none is a bounded
weight, and none is a clean sentinel. The grouping therefore stays UNKNOWN even
though the word width is settled.

### A.3 Consistency with the Q12 aim shorts

The established aim shorts sit in a small signed band and the documented
animation fixed point is Q12 (`4096 == 1.0`, `>>12`). The clean frame words top
out at `|word| = 459`, well under `4096`, i.e. under `0.12` of one. That is the
same order as the header-adjacent aim components reported earlier (roughly
`-50..225`). The frame values are therefore **consistent with Q12 aim
components**; the unit is INFERRED, not directly observed.

### A.4 Coherence of a slot's frames

Five clean slots carry two frames (the corpus has five `64`-byte gaps, i.e.
`24 + 2 * 20`). In all five frame pairs **every one of the ten words changed**,
with per-word deltas reaching roughly `1000` Q12 units. The player model's only
two-frame slot behaves the same way: every word moves. The four player frames
across three slots are not small variations of one another, and the only
same-slot pair is not smooth. The frames therefore read as **independent
per-state samples, not a temporal clip**. The sample is only five pairs, so this
is an observation, not a decoded timing rule.

## B. The descriptor `+4`, `+8`, `+12`, `+16` words

The 394 named containers hold **232 active aim slots** and **1584 inactive
slots** under the rule in A.1. The four words named by the task, read as `u16`:

| Word | Active distinct | Active zero | Inactive distinct | Inactive zero |
|---|---:|---:|---:|---:|
| `+4` | 1 (always 0) | 232 / 232 | 1 (always 0) | 1584 / 1584 |
| `+8` | 48 | 177 / 232 | 306 | 656 / 1584 |
| `+12` | 22 | 206 / 232 | 206 | 1029 / 1584 |
| `+16` | 64 | 144 / 232 | 351 | 441 / 1584 |

**Low-bit partitions.** Cross-tabulating each word's low bits against the
active/inactive split shows no separating value. For example the bit-0
zero-share is `+8` 89% active versus 69% inactive, `+12` 93% versus 82%, and
`+16` 84% versus 65%. That is a weak enrichment (active aim slots are a little
more often zero) but not a bimodal mode bit: roughly a quarter to two-fifths of
active slots still carry a non-zero low bit in `+8`/`+16`.

**`u32` readings.** Reading `+4`/`+8`/`+12`/`+16` as aligned `u32` only folds in
the next `u16`; the `+4` `u32` is dominated by its `+6` half, so it is not a
clean mode word either.

**Models with more active aim slots.** Models with two or more active aim slots
do not share a constant value in any of the four words, and the player's three
active slots disagree on `+8`/`+12`/`+16`. There is no per-model mode signature.

**Verdict: UNKNOWN.** In the file form, the procedural-orientation mode bit is
**not** one of `+4`, `+8`, `+12`, `+16`. It may live in the expanded in-memory
record, in a word this scan did not name, or be derived rather than stored; none
of those is established here.

## C. Composition rule

Three candidates were weighed against the evidence:

| Candidate | Evidence | Status |
|---|---|---|
| **Empty path:** `Static`, or procedural with a zero aim triplet, gives identity | A slot with no aim data has no authored aim vector to rotate; the trace ties procedural orientation to the aim vector | **CONFIRMED** |
| **Object-frame base:** a procedural slot's rotation is expressed in the object frame, so the object transform is the base | The animation trace names the object transform as a contributor to the runtime orientation | **INFERRED** |
| **Full local algebra:** object transform, then a local aim-to-rotation term, then a world target | The trace names all three contributors but does not pin the algebra; no capture in this slice isolates one slot's rotation against its aim, transform and target | **UNKNOWN** |
| Aim words read directly as yaw/pitch angles | No angular unit is observed; the words are small fixed-point components | Rejected as a direct reading |

The exact runtime rule stays UNKNOWN, so the shipped function implements only
what is supported:

- `Static`, or a procedural slot with an all-zero aim triplet -> identity.
- A procedural slot with a non-zero aim triplet -> the object transform, used as
  the `INFERRED` rotation base. The local aim term is **omitted** (treated as
  identity) rather than invented, and the world-target term is absent.

Callers must treat the procedural result as a lower bound, not the runtime pose.

## D. Implementation

`src/slot_descriptor.rs` (pure, not wired into any CLI arm):

- `Frame` - the 20-byte record as ten signed words; `parse`, the Q12 `units`
  conversion, and a labelled `leading_triplet` candidate accessor.
- `frames(descriptors, payload, slot)` - parse a slot's whole frames from its
  bounded sub-slice; an incomplete trailing frame is ignored, never padded.
- **5 new frame tests**: the ten-word parse and Q12 units, the length gate, the
  two-frame parse, the absent-slot empty case, and the incomplete-tail case.

`src/pose_slot.rs` (new pure module, also unwired):

- `OrientationMode`, `Rotation`, `identity_rotation` and `slot_rotation` - the
  composition with each evidence class stated inline.
- **3 new rotation tests**: static identity, procedural zero-aim identity, and
  the procedural object-transform base.

All tests are synthetic; no game bytes. The suite is **580 passed**. `src/main.rs`
registers both modules behind `#[allow(dead_code)]`; neither is wired into any
CLI arm, so headless digests are unchanged.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| A 20-byte frame is ten `i16` | a `u32` reading that stays in a component band, or a zero padding lane | Not observed: `u32` folds a negative high half; no padding lane |
| The ten words group into named fields | one position that is monotone, bounded as a weight, or constant | Not observed |
| The aim frame unit is Q12 | any clean word at or above `4096`, or an engine shift other than 12 | Not observed: max clean word 459 |
| A slot's frames are a temporal sequence | two consecutive frames that differ by only small deltas | Refuted on the five pairs: every word changes |
| `+4/+8/+12/+16` carry a mode bit | a low-bit value that cleanly splits active from inactive | Not observed: no separating value |
| Static or zero-aim is identity | a static slot with an authored aim rotation | Not observed; no aim data exists there |
| The full composition is a simple product | a captured slot rotation equal to object x aim x target | Not tested here; the algebra stays UNKNOWN |

## Limits

- The clean frame set is small (58 frames, 5 same-slot pairs); the coherence
  result is an observation, not a timing model.
- The frame grouping beyond "ten signed words" is not decoded.
- The mode-bit location is not found in the file-form words named here.
- The composition is implemented only where it is confirmed or best-supported;
  the local aim and world-target terms are omitted, not approximated.
- This slice parses file-form records only; it does not decompress the object
  container, relocate pointers, or replay the runtime pose.

## Legal

Read-only. No game process was launched or instrumented. No game file was
written, no game bytes were copied into the repository, and no disassembly,
address, file offset or install path appears here. Only structure, field roles,
format constants and aggregate corpus counts are recorded.

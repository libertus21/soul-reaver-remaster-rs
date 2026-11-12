# i16-animation-scan - the element-type rescan is negative (2026-11-11)

Read-only re-scan of the 227 loose `.SRA` files and the 349 loose `.SRM` files in
the SR1 remaster `OBJECT` folder (plus the disc-2 `OBJECT` folder as an
independent cross-check). Every earlier animation scan tested **`f32`** windows;
the decoded runtime aim frame is **ten `i16` in Q12** (`4096 == 1.0`). An `i16`
fixed-point matrix would have been invisible to those scans, so this pass repeats
the orthonormality sweep with the element type the engine actually uses. Nothing
under the install was written; no game bytes were copied into the repository; no
addresses, per-file offsets, disassembly listings or absolute install paths
appear here.

Evidence classes per claim: **CONFIRMED** (bytes plus a falsifiable test),
**PARTLY** (one side observed), **UNKNOWN**.

## Verdict

| # | Question | Answer | Class |
|---|---|---|---|
| 1 | Does any `.SRA` hold an `i16` Q12 3x3 pose block? | **NO.** 1 isolated window below 0.05 in **3,824,489**; no run of 2 at any tested stride. | CONFIRMED negative |
| 2 | Does any `.SRM` hold an `i16` Q12 3x3 pose block? | **NO.** **0** of **26,653,885** windows below 0.05; minimum error 0.136164. | CONFIRMED negative |
| 3 | Does any `.SRA`/`.SRM` hold an `i16` Q12 3x4 block? | **NO.** **0** of **30,474,918** combined windows; minima 0.269699 / 0.108800. | CONFIRMED negative |
| 4 | Does the 20-byte frame's leading nine words form an orthonormal matrix? | **NO.** 1 isolated hit total (the same `.SRA` window); **0** in `.SRM`. | CONFIRMED negative |
| 5 | Do hits form runs that beat a matched-histogram baseline? | **NO.** Longest run is **1** everywhere; the `.SRM` Q15 corpus has **fewer** hits than its shuffled control (55 vs 1,231). | CONFIRMED negative |
| 6 | Does any candidate respect the bone/frame gate? | **NO.** No run equals 66/35/37/24/3/2 and no run falls in 7..=360. | CONFIRMED negative |
| 7 | Is a wrong scale hiding the data? | **NO.** Q15 hits are isolated and below the matched-histogram baseline; raw produces none. | CONFIRMED negative |
| 8 | Ship a decoder? | **NO.** Phase 2 is not justified. | CONFIRMED negative |

The decisive rows are 1, 2 and 5. Across both loose families there is **one**
sub-0.05 Q12 3x3 window in **30,478,374** positions, it has no neighbour at any
tested stride, and the Q15 control is *worse* than chance. There is no candidate
block to decode.

## Why this is new, stated honestly

The earlier scans were not wrong about what they measured; they measured the
wrong **element type**:

- `.SRA`: **0** of 3,744,114 `f32` 3x3 windows below 0.05 (minimum 0.329).
- `.SRM`: **0** of 26,647,603 `f32` 3x3 windows below 0.05 (minimum 0.509).

Those scans prove "no `f32` matrix". They say nothing about "no `i16` matrix",
and the runtime frame that was later decoded is ten `i16` Q12 words. This pass
tests the actual element type. It is a methodological correction, not a repeat.

One nuance must be stated up front: the decoded 20-byte frame's words are
**small** (`|word| <= 459`, i.e. at most `0.112` of one). A Q12 rotation basis
needs entries near `+-4096`. The frame is therefore a **correction vector, not a
rotation matrix** even before any scan. The i16 scan was still worth running
because a *separate* packed i16 matrix block could exist elsewhere in the asset;
it does not.

## Corpus

| Corpus | Files | Bytes | Windows (3x3, Q12) |
|---|---:|---:|---:|
| Loose `.SRA` (disc 1) | 227 | 3,828,348 | 3,824,489 |
| Loose `.SRM` (disc 1) | 349 | 26,659,818 | 26,653,885 |
| Loose `.SRM` (disc 2, cross-check) | 208 | 28,924,136 | 28,920,600 |

The disc-1 `.SRM` size reproduces the prior scan's 25.42 MiB / 26,659,818 bytes
exactly; the disc-2 size reproduces its 28,924,136 bytes. The corpora are the
same ones the float scans used.

## Method

- **Windows.** `3x3` = nine consecutive `i16` (18 bytes). `3x4` = twelve `i16`
  (24 bytes), using the three basis rows and skipping the fourth column, exactly
  as the float 3x4 scan did. `frame20` = the decoded 20-byte frame, using its
  leading nine words as the matrix (the tenth word has no place in a 3x3 basis).
- **Offsets.** Every byte offset, both parities. A hit at an odd offset is
  counted and reported separately, because an odd-only hit would indicate
  misalignment.
- **Metric.** `max |R^T R - I|` on the three basis rows, computed in exact
  integer arithmetic (`i64` dot products) and scaled once. Zero is a perfect
  rotation. Thresholds: 0.05, 0.02, 0.01.
- **Scales.** Q12 (`1/4096`, the documented engine unit), Q15 (`1/32768`), and
  raw (`1.0`) as controls. Q15 cannot represent exactly 1.0 in `i16`; its best
  identity is `32767` (error ~6.1e-5).
- **Runs.** A hit is evidence only when consecutive windows at a fixed stride
  chain. Runs are measured at stride 18 (packed 3x3), 20 (frame cadence, for
  both the 18-byte and 20-byte window), and 24 (packed 3x4). Because every
  offset is tested, a chain may start at any phase.
- **Baselines.** (a) the same corpus with its byte order shuffled (deterministic
  Fisher-Yates, same byte histogram, structure destroyed), and (b) uniform
  random `i16` of the same per-file lengths. Both use a fixed xorshift64 seed,
  so every number below is reproducible.

## Main sweep - Q12 (the hypothesis)

### `.SRA` (227 files)

| Window / stride | Windows | Min error | `<0.05` | `<0.02` | `<0.01` | Longest run | Files with run >=2 |
|---|---:|---:|---:|---:|---:|---:|---:|
| 3x3 / 18 | 3,824,489 | 0.041436 | 1 | 0 | 0 | 1 | 0 |
| 3x3 / 20 | 3,824,489 | 0.041436 | 1 | 0 | 0 | 1 | 0 |
| 3x4 / 24 | 3,823,127 | 0.269699 | 0 | 0 | 0 | 0 | 0 |
| frame20 / 20 | 3,824,035 | 0.041436 | 1 | 0 | 0 | 1 | 0 |

Per-file minimum distribution (3x3/18): 1 file `<0.05`, 2 in `0.05..<0.1`,
8 in `0.1..<0.3`, 2 in `0.3..<0.5`, 78 in `0.5..<0.8`, 136 in `0.8..<1.0`.
The single hit sits in one file, at an even offset, 0.83x the gate, and is a
near-diagonal read of three separate scalar fields (about 2.4% off unity), not a
packed matrix. No neighbour exists at 18 or 20 bytes.

### `.SRM` (349 files)

| Window / stride | Windows | Min error | `<0.05` | `<0.02` | `<0.01` | Longest run | Files with run >=2 |
|---|---:|---:|---:|---:|---:|---:|---:|
| 3x3 / 18 | 26,653,885 | 0.136164 | 0 | 0 | 0 | 0 | 0 |
| 3x3 / 20 | 26,653,885 | 0.136164 | 0 | 0 | 0 | 0 | 0 |
| 3x4 / 24 | 26,651,791 | 0.108800 | 0 | 0 | 0 | 0 | 0 |
| frame20 / 20 | 26,653,187 | 0.136164 | 0 | 0 | 0 | 0 | 0 |

Per-file minimum distribution (3x3/18): 17 files in `0.1..<0.3`, 60 in
`0.3..<0.5`, 240 in `0.5..<0.8`, 32 in `0.8..<1.0`. **No file is below 0.1.**
The corpus floor is 2.7x the gate.

### Combined Q12

| Window | Windows | Hits `<0.05` | Longest run |
|---|---:|---:|---:|
| 3x3 | 30,478,374 | 1 | 1 |
| 3x4 | 30,474,918 | 0 | 0 |
| frame20 | 30,477,222 | 1 | 1 |

Because the whole Q12 corpus contains at most **one** hit, no packed array of
Q12 orthonormal matrices can exist at **any** stride: a block would need two or
more hits, and a run of two is impossible with a single hit. This argument does
not depend on the tested strides. It closes the 3x3 and 3x4 Q12 hypotheses
without needing to enumerate every possible record layout.

## Baselines

| Corpus | Baseline | 3x3 Q12 min | 3x3 Q12 `<0.05` | 3x4 Q12 min | 3x4 Q12 `<0.05` |
|---|---|---:|---:|---:|---:|
| `.SRA` | shuffled | 0.527439 | 0 | 0.447510 | 0 |
| `.SRA` | uniform random | 1.817511 | 0 | 1.929363 | 0 |
| `.SRM` | shuffled | 0.143648 | 0 | 0.121976 | 0 |
| `.SRM` | uniform random | 1.684274 | 0 | 1.557663 | 0 |

The real `.SRA` corpus dips below its shuffled floor (0.041 vs 0.527) because
structured fixed-point data can sit closer to a basis than noise; the dip is one
isolated window and nowhere near a run. The real `.SRM` corpus (0 hits) and both
random controls (0 hits) are indistinguishable. There is no hit rate to claim
against chance at Q12: one in 30.5 million, with no cluster.

## Cluster analysis and parity

- **Longest run is 1 in every family, width, stride and scale.** Zero files have
  a run of 2 or more at Q12. No candidate block exists to test against a bone or
  frame count.
- **Parity.** The single Q12 hit is at an **even** offset. There is no
  odd-offset-only Q12 hit, so there is no misalignment red flag. At Q15 the
  `.SRM` split is 45 even / 10 odd, while its matched shuffled control is 616
  even / 615 odd; the control is symmetric, so the real odd hits are not a
  hidden phase.
- **Same-file spacing.** The four `.SRA` Q15 hits lie in two files and are
  separated by 131,376 and 64,816 bytes, not by any matrix, bone or frame
  stride. The `.SRM` Q15 same-file spacings are 32/64/96/128/192 and
  224/2,240/2,784/3,104/8,832 - none is a plausible pose record stride.

## Q15 and raw controls

Q15 exists to catch a wrong scale choice. It does not rescue the hypothesis; it
refutes itself:

| Corpus | Q15 3x3 real | Q15 3x3 shuffled | Q15 3x3 random | Q15 3x4 real | Q15 3x4 shuffled |
|---|---:|---:|---:|---:|---:|
| `.SRA` | 4 hits, min 0.013868 | 0, min 0.061505 | 0, min 0.072438 | 0, min 0.094866 | 0, min 0.071287 |
| `.SRM` | 55 hits, min 0.011719 | **1,231**, min 0.000000 | 1, min 0.041965 | 61 hits, min 0.007782 | **1,093**, min 0.000000 |

The `.SRM` real corpus has **4.5% of** the shuffled corpus's hit count at Q15
(55 vs 1,231) - i.e. the real data is far *less* hit-dense than a matched byte
shuffle. Those Q15 "hits" are token accidents in data that already contains
`i16` unit-range values (the shuffled histogram has the same tokens). The `.SRA`
Q15 count (4) is small but isolated: no run, no stride, and the best error is
230x the Q15 representation floor (6.1e-5), so it is not a clean authored
rotation. The raw scale produces **0** hits in both families; its minimum is
exactly 1.0 (all-zero windows), as expected.

The disc-2 `.SRM` cross-check reproduces the pattern: Q12 3x3 **0** of
28,920,600, minimum 0.216070; Q12 3x4 **0** of 28,919,352, minimum 0.191188;
Q15 real 82 hits against a shuffled control of 156.

## Bone-count and frame-count gate

The gate is: any candidate block's element count must relate to a bone count
(66, 35, 37, 24, 3, 2) or a plausible frame count (7..360).

- **No run length equals 66, 35, 37, 24, 3 or 2** in any family, width, stride
  or scale. With a maximum run of 1, the gate has no candidate.
- **No 20-byte-cadence run falls in 7..360.** The only runs are isolated
  singleton hits.

## Structural note on the 20-byte frame

A 20-byte frame holds ten `i16`. A 3x3 rotation uses nine words, so a 20-byte
frame can hold a rotation plus one spare word, but **cannot** hold a rotation
plus a translation (that needs twelve words). The earlier frame decode already
found the ten words in a small band (`|word| <= 459`, at most `0.112` of one),
which is incompatible with Q12 basis entries near `+-4096`. The frame is an
aim/correction vector, not a matrix. Testing its leading nine words is therefore
a control, and it returns no run in `.SRM` and one isolated window in `.SRA`.

## What this scan does NOT cover (explicit)

- **4x4 `i16` matrices** (32 bytes, 16 words). Not part of the requested widths.
- **Quaternion or Euler encodings.** Only orthogonal 3x3/3x4 bases are tested.
- **Non-orthonormal local transforms** (scale, shear, translation-only records).
  A pose block that is deliberately not a rotation would not be found by this
  metric, by design.
- **In-memory runtime records.** This reads file bytes, not the engine's built
  structures. It cannot prove the engine never assembles a matrix at runtime
  from non-matrix data.
- **The `.PCM` archive object containers and `.SRL` clips.** They are outside
  this pass; `.SRL` is already documented as facial/HD 3x4 tracks.
- **Any file family other than loose `.SRA` and `.SRM`.**

## Method provenance and reproduction

As with the two prior scans, the scanner is a scratch tool outside the
repository. It records counts only and does not copy game bytes. To reproduce a
run, apply the metric and offsets in the Method section to every `*.SRA` and
`*.SRM` in a flat `OBJECT` folder:

1. For each config (3x3/18, 3x3/20, 3x4/24, frame20/20) and scale (Q12, Q15,
   raw), evaluate every byte offset and record the minimum, the counts below
   0.05 / 0.02 / 0.01, and each hit's byte parity.
2. Build the runs at the config stride; a run is two or more hits exactly
   `stride` bytes apart. Report the longest run per file and per corpus.
3. Repeat on (a) each file's bytes passed through a deterministic Fisher-Yates
   shuffle and (b) uniform random `i16` of the same length, with fixed seeds.
4. Evaluate the bone gate (66/35/37/24/3/2) against every maximal run length and
   the frame gate (7..360) against 20-byte-cadence runs.

No production code changed, no engine behaviour changed, no golden moved, and
the suite remains at its pre-existing **603 passed** with a warning-free
`cargo build`. The engine loop, headless digests and goldens are untouched.

## Falsification ledger

| Claim | Observation that would refute it | Tested? |
|---|---|---|
| `.SRA` carries i16 Q12 3x3 pose matrices | a run of two sub-0.05 windows at any stride | Tested: 1 hit in 3.82M, longest run 1 |
| `.SRM` carries i16 Q12 3x3 pose matrices | one sub-0.05 window outside the known float HD block | Tested: 0 in 26.65M, floor 0.136 |
| `.SRA`/`.SRM` carry i16 Q12 3x4 matrices | one sub-0.05 3x4 window | Tested: 0 in 30.47M, floors 0.270 / 0.109 |
| A coherent block beats chance | a run whose hit density exceeds the shuffled control | Tested: no run; Q15 real is below shuffled |
| The block respects a bone count | a run of 66/35/37/24/3/2 | Tested: none |
| The block is a clip | a 7..360 run at a 20-byte cadence | Tested: none |
| Q15 is the hidden scale | Q15 real hits exceeding the Q15 shuffled control with runs | Refuted: 55 vs 1,231; no runs |
| A wrong scale caused the old negative | any scale producing a coherent run | Refuted: Q12, Q15 and raw all produce no run |
| Odd offsets hide the data | odd-offset-only hits at the hypothesis scale | Not observed: the only Q12 hit is even |
| The 20-byte frame is a Q12 matrix | frame words near `+-4096` | Refuted: max clean word 459, i.e. 0.112 of one |

## Legal

Read-only. No game process was launched or instrumented. No game file was
written, no game bytes were copied into the repository, and no disassembly,
address, per-file offset or install path appears here. Only structure, format
constants and aggregate corpus counts are recorded.

# srm-animation-scan - the `.SRM` mesh does not carry the slot-pose or track table (2026-11-09)

Read-only orthonormality scan of the 349 loose `.SRM` files in the SR1 remaster
`OBJECT` folder (25.42 MiB), replicating the `.SRA` method exactly: every byte
offset, a 3x3 (9 `f32`, 36 B) and a 3x4 (12 `f32`, 48 B) window, the same
orthonormality error, an explicit chance baseline, and a clusters-not-isolated-hits
criterion. Nothing under the install was written; no game bytes were copied into
the repository; scratch lived only in the session temp directory. This note
records counts, sizes, strides, section byte totals and format constants only: no
addresses, no per-file offsets, no disassembly, no raw game bytes, no absolute
install paths.

Date: 2026-11-09. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable test), **INFERRED** (one side only), **UNKNOWN**.

## Question

`docs/changes/remaster-animation/animation-source.md` pins the runtime chain
(per-slot 13-float pose, 32-byte descriptor, 4-float track table) but leaves the
**file carrier** UNKNOWN. `.SRA` (3.74M / 3.72M windows, min error 0.329 / 0.370)
and the `.PCM` object container (no descriptor array, no track table) are already
eliminated. `.SRM` is the last loose asset family. This pass asks one question:
**does the `.SRM` mesh carry a materialised pose array or a 4-float track table,
or is the animation built elsewhere?**

## Verdict

| # | Question | Answer | Class |
|---|---|---|---|
| 1 | Does any `.SRM` hold a 13-float (3x3) pose block? | **NO.** 0 of 26,647,603 3x3 windows fall below 0.05 / 0.02 / 0.01. Minimum error is 0.509084. | CONFIRMED negative |
| 2 | Does any `.SRM` hold a 3x4 pose/palette block outside the known HD matrices? | **NO.** 147 windows are below 0.05 and all 147 sit in the documented `extra > 0` HD-matrix section. Outside it the minimum error is 0.222478, with 0 below any threshold. | CONFIRMED negative |
| 3 | Are the hits clusters or isolated coincidences? | **Isolated / structural.** Zero stride-52 runs in 349/349 files. Every stride-48 run is the HD-matrix block and its length equals the file's `extra` field exactly. | CONFIRMED negative |
| 4 | Is there a 4-float quaternion track table? | **NO.** The longest unit-quaternion run is 8 frames (one file) and every run of 3+ frames lies in the `u16` face-index section. Zero framed `frame_count`/`divisor` blocks. | CONFIRMED negative |
| 5 | Does any candidate block agree with the model bone count? | **NO.** No run length equals 66, 24, 35, 37, 3 or 2. The only counts found are 1/7/17/25/26, which are the HD-matrix counts and match the `.SRL` track counts, not bone counts. | CONFIRMED negative |
| 6 | Ship a decoder? | **NO.** No coherent cluster beats the baseline and the bone-count gate has no candidate. Phase 2 is not justified. | CONFIRMED negative |

The decisive rows are 1 and 2. The only orthonormal payload anywhere in 26.6 MB
is the 6,144-byte `extra > 0` HD-matrix block already decoded by `src/srm.rs`;
outside it there is not one window below 0.05 in 26.6 million positions.

## 1. Corpus and method

- **349/349** loose `.SRM` files parse to exact EOF under the `src/srm.rs` layout;
  **26,659,818** bytes scanned at **every** byte offset.
- **Metric (same as the `.SRA` scan):** for a 9-float window, rows `r0, r1, r2`,
  error = `max(|r0.r0-1|, |r1.r1-1|, |r2.r2-1|, |r0.r1|, |r0.r2|, |r1.r2|)`; 0 is
  a perfect rotation. The 12-float window uses the same three basis rows and
  skips the fourth column. Non-finite floats score infinity.
- **Thresholds:** 0.05, 0.02, 0.01.
- **Baselines:** (a) the same corpus with its byte order shuffled (deterministic
  Fisher-Yates, same byte histogram, structure destroyed); (b) 4 MiB of uniform
  random bytes.
- **Rule:** a real table makes runs of consecutive windows at a fixed record
  stride, not isolated hits. Runs are measured at stride 52 (13-float pose) and
  48 (3x4), and at 16 for 4-float quaternion frames.

## 2. Orthonormality sweep

| Window | Windows tested | Min error (corpus) | Min error outside HD block | `<0.05` | `<0.02` | `<0.01` |
|---|---:|---:|---:|---:|---:|---:|
| 3x3 (9 f32) | 26,647,603 | 0.509084 | 0.509084 | **0** | **0** | **0** |
| 3x4 (12 f32) | 26,643,415 | 0.000000 | 0.222478 | **147** | **139** | **138** |

All 147 sub-0.05 3x4 windows are in the bone-extra section; the counts in shader,
material, joint, flags, vertex and face sections are **0**. The single 0.000000
minimum is the known HD-matrix class, not animation.

Per-file minimum-error distribution (349 files, outside the HD block):

| Min error | 3x3 files | 3x4 files |
|---|---:|---:|
| `<0.30` | 0 | 2 |
| `0.30..<0.50` | 0 | 1 |
| `0.50..<0.70` | 20 | 9 |
| `0.70..<0.90` | 10 | 28 |
| `>=0.90` | 319 | 309 |

The floor is 0.509 (3x3) and 0.222 (3x4). Both are far above the 0.05 gate; the
3x4 floor is 4.4x the gate.

### 2.1 Baseline control

| Corpus | 3x3 windows | 3x3 min | 3x4 windows | 3x4 min | `<0.05` |
|---|---:|---:|---:|---:|---:|
| real `.SRM` | 26,647,603 | 0.509084 | 26,643,415 | 0.222478* | 0 |
| byte-shuffled `.SRM` | 26,647,603 | 0.749958 | 26,643,415 | 0.749649 | 0 |
| uniform random (4 MiB) | 4,194,269 | 0.917803 | 4,194,257 | 0.944549 | 0 |

*outside the HD block; whole-corpus minimum is 0.000000 from that block.

The real corpus dips below the shuffled baseline because structured `f32` runs
(vertex positions, joint translations) can sit closer to an orthonormal basis than
uniform noise. That dip is real but nowhere near the gate: 0 windows below 0.05 in
26.6M structured positions, versus 0 in the two baselines. A hit rate cannot be
claimed against chance because there is no hit rate.

## 3. Cluster analysis

`run52` = longest consecutive chain of 3x3 hits at 52-byte stride, over all 52
phase alignments; `run48` = same for 3x4 at 48-byte stride.

| Longest run | Files with `run52` | Files with `run48` |
|---|---:|---:|
| 0 | **349** | 342 |
| 1 | 0 | 1 |
| 5..7 | 0 | 1 |
| 16..31 | 0 | 5 |

- **`run52`: 0 in every file.** No file has even two consecutive 13-float pose
  records.
- **`run48`:** the seven non-zero files are exactly the documented `extra > 0`
  HD-matrix files, and their run lengths equal `extra` exactly
  (**1, 7, 17, 25, 26, 26, 26**). Outside that 6,144-byte block there is no
  stride-48 run. All 147 sub-0.05 3x4 windows sit inside the 6,144-byte section
  that holds the 128 HD-matrix records; none lies outside it.

The corpus therefore contains **no animation-like cluster**: no run exists outside
a structure already decoded and named as HD matrices.

## 4. Track-table search

A 4-float frame is a unit quaternion at 16-byte stride (`|q| = 1 +- 0.05`); a
coherent run also requires consecutive frames to vary smoothly
(`|q_i . q_(i+1)| >= 0.9`). A framed table is a `frame_count` (4..1024) and
`divisor` (1..100000) pair followed by `frame_count` unit quaternions.

| Longest run (frames) | Files (`unit`) | Files (`smooth`) |
|---|---:|---:|
| 0 | 178 | 178 |
| 1 | 159 | 166 |
| 2 | 7 | 0 |
| 3 | 1 | 1 |
| 4 | 2 | 2 |
| 5..7 | 1 | 1 |
| 8..15 | 1 | 1 |
| `>=16` | **0** | **0** |

- The maximum is **8 frames**, in a single file. Every run of 3+ frames sits in
  the `u16` face-index section, so the "quaternions" are triangle indices seen as
  floats, not frames. A clip is roughly 7-360 frames at 24-60 fps, so an isolated
  8-frame fragment in mesh indices is coincidence, not a track.
- **Framed `frame_count`/`divisor` blocks: 0 in 349.** No header bounds any run.

For completeness, the disc-2 `OBJECT` folder (208 files, 28,924,136 bytes) was
scanned as an independent cross-check: 3x3 min 0.494455, 3x4 min 0.413134, **0**
below 0.05, zero stride runs, zero framed blocks, and a longest quaternion run of
9 frames in its face-index section. Same result.

## 5. Bone-count gate

The gate models present as loose `.SRM` are the four body models; `eggfx` (3) and
`soul` (2) have no loose `.SRM` in this folder, so the gate is evaluated on the
four that are present.

Minima below are outside the HD block (whole-file `run48` is 0 for all except
`MORLOCK_3`, whose 26 is its own HD block).

| File | Runtime slots | Min 3x3 | Min 3x4 | `run52` | `run48` | Agreement |
|---|---:|---:|---:|---:|---:|---|
| `RAZIEL_1.SRM` | 66 | 0.521019 | 0.413134 | 0 | 0 | none |
| `SLUAGH_1.SRM` | 24 | 0.977074 | 0.976891 | 0 | 0 | none |
| `SKINNER_1.SRM` | 35 | 0.961704 | 0.941220 | 0 | 0 | none |
| `SKINNER_2.SRM` | 35 | 0.749756 | 0.975572 | 0 | 0 | none |
| `MORLOCK_1.SRM` | 37 | 0.974590 | 0.983849 | 0 | 0 | none |
| `MORLOCK_2.SRM` | 37 | 0.979644 | 0.982458 | 0 | 0 | none |
| `MORLOCK_3.SRM` | 37 | 0.647236 | 0.653701 | 0 | 26 | none |
| `MORLOCK_4.SRM` | 37 | 0.647236 | 0.653701 | 0 | 0 | none |

`soul` and `eggfx` are absent from the 349-file corpus. No run length equals 66,
24, 35, 37, 3 or 2. The only structured counts in the corpus are 1/7/17/25/26 â€”
the HD-matrix counts, already count-matched to `.SRL` track counts (the
facial/HD class), not to bone counts. The gate has no candidate to agree with.

## 6. Chunk structure and placement

`.SRM` has **no tag table**. It is a fixed section sequence:

```text
magic + shader count + shader records (44 B each)
material count + material records (32 B each)
extra-bone count + HD matrices (48 B each)
32 joint records (48 B each) = 128 joint slots
[optional sentinel] + 128 activation flags
vertex count + face count
32-byte vertices
u16 face indices
```

Aggregate section bytes over the 349 files:

| Section | Bytes | Share |
|---|---:|---:|
| shader | 15,356 | 0.06% |
| material | 31,328 | 0.12% |
| bone-extra (HD matrices) | 6,144 | **0.023%** |
| bone-joint (128 slots) | 536,064 | 2.01% |
| flags | 44,672 | 0.17% |
| vertex | 20,494,400 | 76.87% |
| face | 5,523,450 | 20.72% |
| magic / count words / sentinel | 8,404 | 0.03% |

Vertex and face arrays are **97.6%** of the corpus. The only orthonormal payload
is the **6,144-byte** HD-matrix block (0.023%): 147 of 147 sub-0.05 3x4 windows
land there, and **zero** land in the vertex, face, joint, shader, material or flag
sections. There is no separate animation chunk, tagged or otherwise; animation and
mesh are not merely "separated" â€” the mesh file carries no animation block at all.

## 7. Falsification ledger

| Claim | Observation that would refute it | Tested? |
|---|---|---|
| `.SRM` carries the 13-float per-slot pose | one stride-52 run of 3x3 orthonormal windows | Tested: 0/349 files, any length |
| `.SRM` carries a 3x4 pose/palette table outside HD matrices | a 3x4 window below 0.05 outside the HD block | Tested: 147 hits, 147 inside the block, 0 outside; floor 0.222 |
| A real table beats chance | a cluster whose hit rate exceeds the shuffled/random baseline | Tested: 0 clusters, 0 hits in the real corpus outside the known block |
| The animation is a 4-float quaternion track | a smooth unit-quaternion run of plausible clip length, or a framed block | Tested: max 8 frames, in mesh indices; 0 framed blocks |
| Any candidate respects the bone-count gate | a block with 66/24/35/37/3/2 elements | Tested: no such run length exists |
| The `.SRM` stores animation in a separate chunk | a section other than the HD block containing the hits | Tested: all hits in bone-extra; every other section 0 |

## 8. Why no decoder shipped

The pass/fail gate is: coherent clusters that beat baseline and a block whose
element count matches the bone count or a plausible frame count. The `.SRM` has
neither. A count-equal or rotation-like window would be trivial to force by
choosing an offset; here a full 26.6 MB sweep finds none outside a structure that
`src/srm.rs` already decodes as HD 3x4 matrices. Per the hard rule, a documented
negative is the success condition; inventing a `.SRM` animation chunk that
"looks plausible" would be fabrication. No CLI, parser, struct or test was added,
the engine loop and digest are untouched, and no golden moved.

## 9. What this changes

- `.SRM` is the **mesh + 128-slot joint pool + optional HD 3x4 matrix block**. It
  does **not** carry a 13-float pose array, a 3x4 animation array, or a 4-float
  track table.
- This closes the last loose asset family and agrees with
  `animation-source.md`: the slot pose is built at runtime from an in-memory
  animation record whose file carrier is **not** `.SRA`, **not** the `.PCM`
  object container, and **not** `.SRM`.
- The honest next route is the one already named in the authority note: **pin
  the loader in the shipped module** (trace the routine that allocates the
  `bone_count`-sized descriptor array and fills the mode bits) and read the data
  source it consumes. That is a binary-side slice, not another mesh decode.

## Method and legal

- Read-only over the user's own remaster install; the install was never written.
- Corpus: 349 loose `.SRM` (25.42 MiB), every byte offset, both window shapes; a
  byte-shuffled control of the same corpus; and a 4 MiB uniform-random control.
  A 208-file disc-2 folder was scanned as an independent cross-check.
- The scanner is a scratch tool outside the repository. It records counts, sizes,
  strides, section totals and format constants only. No addresses, no per-file
  offsets, no disassembly, no raw game bytes, no absolute install paths.
- No production code changed; no golden moved; no CLI added. The negative is the
  deliverable.

## Appendix - per-file numbers (generated)

Every byte offset was evaluated; counts below are per file. `run52`/`run48` are the longest consecutive orthonormal-window runs at the pose-record stride (52 B) and 3x4-record stride (48 B). `quat` is the longest unit-quaternion run at 16 B stride. Minima are shown for the 3x3 (9-float) and 3x4 (12-float) windows.

| file | win 3x3 | min 3x3 | <.05 | <.02 | <.01 | win 3x4 | min 3x4 | <.05 | <.02 | <.01 | run52 | run48 | quat |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| ALKLENS_1.SRM | 4881 | 1 | 0 | 0 | 0 | 4869 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ALKWND_1.SRM | 7097 | 1 | 0 | 0 | 0 | 7085 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ALUKABSS.SRM | 598189 | 0.969016 | 0 | 0 | 0 | 598177 | 0.947726 | 0 | 0 | 0 | 0 | 0 | 1 |
| ALUKABSS_1.SRM | 599441 | 0.933112 | 0 | 0 | 0 | 599429 | 0 | 28 | 28 | 28 | 0 | 26 | 2 |
| ALUKABSS_2.SRM | 598189 | 0.969016 | 0 | 0 | 0 | 598177 | 0.947726 | 0 | 0 | 0 | 0 | 0 | 1 |
| ALUKA_1.SRM | 646729 | 0.523658 | 0 | 0 | 0 | 646717 | 0.295985 | 0 | 0 | 0 | 0 | 0 | 8 |
| ALUKA_2.SRM | 441857 | 0.625598 | 0 | 0 | 0 | 441845 | 0.97507 | 0 | 0 | 0 | 0 | 0 | 1 |
| ANVIL_1.SRM | 8469 | 0.999017 | 0 | 0 | 0 | 8457 | 0.999017 | 0 | 0 | 0 | 0 | 0 | 0 |
| ARIEL_1.SRM | 463793 | 0.555159 | 0 | 0 | 0 | 463781 | 0 | 17 | 17 | 17 | 0 | 17 | 2 |
| ASTAT_1.SRM | 235241 | 0.983579 | 0 | 0 | 0 | 235229 | 0.996016 | 0 | 0 | 0 | 0 | 0 | 1 |
| AVDOOR_1.SRM | 96161 | 0.967677 | 0 | 0 | 0 | 96149 | 0.860825 | 0 | 0 | 0 | 0 | 0 | 1 |
| BABAR_1.SRM | 208017 | 0.96839 | 0 | 0 | 0 | 208005 | 0.968387 | 0 | 0 | 0 | 0 | 0 | 1 |
| BANNER_1.SRM | 2173 | 1 | 0 | 0 | 0 | 2161 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANNER_2.SRM | 2173 | 1 | 0 | 0 | 0 | 2161 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANNER_3.SRM | 2173 | 1 | 0 | 0 | 0 | 2161 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANNER_4.SRM | 2173 | 1 | 0 | 0 | 0 | 2161 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANNER_5.SRM | 2173 | 1 | 0 | 0 | 0 | 2161 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANNER_6.SRM | 2173 | 1 | 0 | 0 | 0 | 2161 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANRALU_1.SRM | 3145 | 1 | 0 | 0 | 0 | 3133 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANRCTY_1.SRM | 3145 | 1 | 0 | 0 | 0 | 3133 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANRRAZ_1.SRM | 3145 | 1 | 0 | 0 | 0 | 3133 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANRWAL_1.SRM | 3145 | 1 | 0 | 0 | 0 | 3133 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BANVIL_1.SRM | 14925 | 0.999996 | 0 | 0 | 0 | 14913 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| BELLCHN_1.SRM | 48481 | 0.998984 | 0 | 0 | 0 | 48469 | 0.962915 | 0 | 0 | 0 | 0 | 0 | 1 |
| BELLCLP_1.SRM | 9689 | 0.999023 | 0 | 0 | 0 | 9677 | 0.999023 | 0 | 0 | 0 | 0 | 0 | 1 |
| BELLOW_1.SRM | 22341 | 0.983954 | 0 | 0 | 0 | 22329 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 0 |
| BELLSC_1.SRM | 26681 | 0.984338 | 0 | 0 | 0 | 26669 | 0.984338 | 0 | 0 | 0 | 0 | 0 | 1 |
| BELL_1.SRM | 31633 | 1 | 0 | 0 | 0 | 31621 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| BGEAR_1.SRM | 27129 | 0.998992 | 0 | 0 | 0 | 27117 | 0.998992 | 0 | 0 | 0 | 0 | 0 | 0 |
| BGFAN_1.SRM | 21145 | 0.983336 | 0 | 0 | 0 | 21133 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| BHAND_1.SRM | 19369 | 0.999992 | 0 | 0 | 0 | 19357 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 1 |
| BKALPH_1.SRM | 5481 | 1 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| BLUDPUD_1.SRM | 2065 | 1 | 0 | 0 | 0 | 2053 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BNFIRE_1.SRM | 40629 | 0.983596 | 0 | 0 | 0 | 40617 | 0.983596 | 0 | 0 | 0 | 0 | 0 | 1 |
| BOLDASC_1.SRM | 11405 | 0.984319 | 0 | 0 | 0 | 11393 | 0.984319 | 0 | 0 | 0 | 0 | 0 | 1 |
| BOLDASD_1.SRM | 13205 | 0.999935 | 0 | 0 | 0 | 13193 | 0.996956 | 0 | 0 | 0 | 0 | 0 | 0 |
| BOLDASE_1.SRM | 13397 | 0.999878 | 0 | 0 | 0 | 13385 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 0 |
| BOLDASF_1.SRM | 12369 | 0.9842 | 0 | 0 | 0 | 12357 | 0.984321 | 0 | 0 | 0 | 0 | 0 | 0 |
| BONDOOR_1.SRM | 3433 | 1 | 0 | 0 | 0 | 3421 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| BOUNCE_1.SRM | 17237 | 1 | 0 | 0 | 0 | 17225 | 0.999963 | 0 | 0 | 0 | 0 | 0 | 0 |
| BRGEAR_1.SRM | 22573 | 1 | 0 | 0 | 0 | 22561 | 0.749878 | 0 | 0 | 0 | 0 | 0 | 1 |
| BRIDGE_1.SRM | 12353 | 1 | 0 | 0 | 0 | 12341 | 0.999939 | 0 | 0 | 0 | 0 | 0 | 1 |
| CATDOOR_1.SRM | 7473 | 1 | 0 | 0 | 0 | 7461 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| CATDORA_1.SRM | 146921 | 0.999996 | 0 | 0 | 0 | 146909 | 0.999142 | 0 | 0 | 0 | 0 | 0 | 1 |
| CATPIPE_1.SRM | 7773 | 0.99902 | 0 | 0 | 0 | 7761 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| CCOVER_1.SRM | 6353 | 0.937953 | 0 | 0 | 0 | 6341 | 0.937953 | 0 | 0 | 0 | 0 | 0 | 0 |
| CFAKEY_1.SRM | 57649 | 0.998893 | 0 | 0 | 0 | 57637 | 0.998954 | 0 | 0 | 0 | 0 | 0 | 0 |
| CHPBAR_1.SRM | 9393 | 1 | 0 | 0 | 0 | 9381 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| CHPBLK_1.SRM | 5677 | 1 | 0 | 0 | 0 | 5665 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| CHPDRL_1.SRM | 4241 | 1 | 0 | 0 | 0 | 4229 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| CHPDRR_1.SRM | 4241 | 1 | 0 | 0 | 0 | 4229 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| CHRNBL_1.SRM | 63169 | 0.986181 | 0 | 0 | 0 | 63157 | 0.859895 | 0 | 0 | 0 | 0 | 0 | 1 |
| CNTABLK_1.SRM | 2601 | 1 | 0 | 0 | 0 | 2589 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| CRANK_1.SRM | 12625 | 1 | 0 | 0 | 0 | 12613 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| CRNMCH_1.SRM | 163009 | 0.998871 | 0 | 0 | 0 | 162997 | 0.998871 | 0 | 0 | 0 | 0 | 0 | 1 |
| CROW_1.SRM | 58437 | 0.982811 | 0 | 0 | 0 | 58425 | 0.983944 | 0 | 0 | 0 | 0 | 0 | 1 |
| CSTAFF_1.SRM | 28061 | 0.999996 | 0 | 0 | 0 | 28049 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| C_DIALA_1.SRM | 16139 | 0.999005 | 0 | 0 | 0 | 16127 | 0.999006 | 0 | 0 | 0 | 0 | 0 | 1 |
| C_DIALA_2.SRM | 16139 | 0.999005 | 0 | 0 | 0 | 16127 | 0.999006 | 0 | 0 | 0 | 0 | 0 | 0 |
| C_DIALA_3.SRM | 16139 | 0.999005 | 0 | 0 | 0 | 16127 | 0.999006 | 0 | 0 | 0 | 0 | 0 | 0 |
| C_DIALB_1.SRM | 16139 | 0.999005 | 0 | 0 | 0 | 16127 | 0.999006 | 0 | 0 | 0 | 0 | 0 | 0 |
| C_DIALC_1.SRM | 16139 | 0.999005 | 0 | 0 | 0 | 16127 | 0.999006 | 0 | 0 | 0 | 0 | 0 | 0 |
| DBLGATE_1.SRM | 9745 | 1 | 0 | 0 | 0 | 9733 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| DBRIDGE_1.SRM | 17281 | 0.999006 | 0 | 0 | 0 | 17269 | 0.998011 | 0 | 0 | 0 | 0 | 0 | 0 |
| DEDFIRA_1.SRM | 312503 | 0.672004 | 0 | 0 | 0 | 312491 | 0.672004 | 0 | 0 | 0 | 0 | 0 | 1 |
| DEDFIRB_1.SRM | 94313 | 0.982819 | 0 | 0 | 0 | 94301 | 0.982819 | 0 | 0 | 0 | 0 | 0 | 1 |
| DIALA_1.SRM | 616897 | 0.518833 | 0 | 0 | 0 | 616885 | 0.75 | 0 | 0 | 0 | 0 | 0 | 1 |
| DIALC_1.SRM | 15921 | 1 | 0 | 0 | 0 | 15909 | 0.989817 | 0 | 0 | 0 | 0 | 0 | 0 |
| DIAL_1.SRM | 37073 | 0.983658 | 0 | 0 | 0 | 37061 | 0.997964 | 0 | 0 | 0 | 0 | 0 | 1 |
| DOORH_1.SRM | 5073 | 1 | 0 | 0 | 0 | 5061 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| DPLUG_1.SRM | 16721 | 1 | 0 | 0 | 0 | 16709 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| DRONB_1.SRM | 239969 | 0.669236 | 0 | 0 | 0 | 239957 | 0.669412 | 0 | 0 | 0 | 0 | 0 | 1 |
| DRON_1.SRM | 584791 | 0.516398 | 0 | 0 | 0 | 584779 | 0.525144 | 0 | 0 | 0 | 0 | 0 | 3 |
| DUMYTUB_1.SRM | 17301 | 0.984318 | 0 | 0 | 0 | 17289 | 0.998046 | 0 | 0 | 0 | 0 | 0 | 1 |
| DVILLA_1.SRM | 53429 | 0.999799 | 0 | 0 | 0 | 53417 | 0.999799 | 0 | 0 | 0 | 0 | 0 | 1 |
| DVILLB_1.SRM | 73285 | 0.98407 | 0 | 0 | 0 | 73273 | 0.999393 | 0 | 0 | 0 | 0 | 0 | 1 |
| DVILLC_1.SRM | 201989 | 0.980043 | 0 | 0 | 0 | 201977 | 0.761668 | 0 | 0 | 0 | 0 | 0 | 1 |
| DVILLHB_1.SRM | 57909 | 0.999932 | 0 | 0 | 0 | 57897 | 0.999869 | 0 | 0 | 0 | 0 | 0 | 1 |
| DVILLHC_1.SRM | 238815 | 0.959772 | 0 | 0 | 0 | 238803 | 0.959767 | 0 | 0 | 0 | 0 | 0 | 1 |
| EAGGOTR_1.SRM | 2065 | 1 | 0 | 0 | 0 | 2053 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| EAGGOTU_1.SRM | 2097 | 1 | 0 | 0 | 0 | 2085 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| EGGSAC_1.SRM | 196889 | 0.69105 | 0 | 0 | 0 | 196877 | 0.708686 | 0 | 0 | 0 | 0 | 0 | 1 |
| EYEF_1.SRM | 157801 | 0.998751 | 0 | 0 | 0 | 157789 | 0.99876 | 0 | 0 | 0 | 0 | 0 | 1 |
| EYEGLO_1.SRM | 2441 | 1 | 0 | 0 | 0 | 2429 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| EYELID_1.SRM | 2493 | 1 | 0 | 0 | 0 | 2481 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| EYE_1.SRM | 34545 | 0.999996 | 0 | 0 | 0 | 34533 | 0.998263 | 0 | 0 | 0 | 0 | 0 | 1 |
| FALLT_1.SRM | 34421 | 1 | 0 | 0 | 0 | 34409 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 1 |
| FBLOCK_1.SRM | 5449 | 0.999016 | 0 | 0 | 0 | 5437 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| FENCEA_1.SRM | 204201 | 0.999918 | 0 | 0 | 0 | 204189 | 0.986795 | 0 | 0 | 0 | 0 | 0 | 1 |
| FGLASS_1.SRM | 283969 | 0.998447 | 0 | 0 | 0 | 283957 | 0.998823 | 0 | 0 | 0 | 0 | 0 | 1 |
| FIRBLK_1.SRM | 8581 | 0.999016 | 0 | 0 | 0 | 8569 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALL_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALL_2.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALL_3.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALL_4.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALL_5.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALL_6.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGALU_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGCTY_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGKAI_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGRAZ_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGRON_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGSKN_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLAGWAL_1.SRM | 3781 | 1 | 0 | 0 | 0 | 3769 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| FLRSWCI_1.SRM | 18201 | 0.999938 | 0 | 0 | 0 | 18189 | 0.999992 | 0 | 0 | 0 | 0 | 0 | 0 |
| FRCBLK_1.SRM | 2601 | 1 | 0 | 0 | 0 | 2589 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| FRCGATE_1.SRM | 81497 | 0.983058 | 0 | 0 | 0 | 81485 | 0.997851 | 0 | 0 | 0 | 0 | 0 | 1 |
| FTHROW_1.SRM | 64421 | 0.999949 | 0 | 0 | 0 | 64409 | 0.999949 | 0 | 0 | 0 | 0 | 0 | 1 |
| GASBLK_1.SRM | 5481 | 0.999016 | 0 | 0 | 0 | 5469 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| GASWEEL_1.SRM | 24769 | 0.984194 | 0 | 0 | 0 | 24757 | 0.999003 | 0 | 0 | 0 | 0 | 0 | 0 |
| GCRANK_1.SRM | 11749 | 0.999939 | 0 | 0 | 0 | 11737 | 0.999939 | 0 | 0 | 0 | 0 | 0 | 0 |
| GEAR_1.SRM | 14445 | 0.999021 | 0 | 0 | 0 | 14433 | 0.999939 | 0 | 0 | 0 | 0 | 0 | 0 |
| GENVA_1.SRM | 210733 | 0.982256 | 0 | 0 | 0 | 210721 | 0.775589 | 0 | 0 | 0 | 0 | 0 | 1 |
| GFIRE_1.SRM | 8897 | 0.999996 | 0 | 0 | 0 | 8885 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| GFIRE_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GFORCE_1.SRM | 8897 | 0.999996 | 0 | 0 | 0 | 8885 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| GFORCE_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GFORCE_3.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GONG_1.SRM | 11449 | 0.999022 | 0 | 0 | 0 | 11437 | 0.999877 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSNLGHT_1.SRM | 8897 | 0.999996 | 0 | 0 | 0 | 8885 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSNLGHT_2.SRM | 2009 | 1 | 0 | 0 | 0 | 1997 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSOUND_1.SRM | 8897 | 0.999996 | 0 | 0 | 0 | 8885 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSOUND_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSOUND_3.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSTONE_1.SRM | 8897 | 0.999996 | 0 | 0 | 0 | 8885 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSTONE_2.SRM | 2009 | 1 | 0 | 0 | 0 | 1997 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSTONE_3.SRM | 2009 | 1 | 0 | 0 | 0 | 1997 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GSTONE_4.SRM | 2009 | 1 | 0 | 0 | 0 | 1997 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| GWATER_1.SRM | 8897 | 0.999996 | 0 | 0 | 0 | 8885 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| GWATER_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| HANDA_1.SRM | 18177 | 0.968695 | 0 | 0 | 0 | 18165 | 0.984354 | 0 | 0 | 0 | 0 | 0 | 1 |
| HANDB_1.SRM | 21833 | 0.999161 | 0 | 0 | 0 | 21821 | 0.999161 | 0 | 0 | 0 | 0 | 0 | 0 |
| HANDC_1.SRM | 8673 | 0.999996 | 0 | 0 | 0 | 8661 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| HATCH_1.SRM | 9601 | 0.999938 | 0 | 0 | 0 | 9589 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 1 |
| HEALTHR_1.SRM | 2065 | 1 | 0 | 0 | 0 | 2053 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| HEALTHS_1.SRM | 2141 | 1 | 0 | 0 | 0 | 2129 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| HEALTHU_1.SRM | 3569 | 1 | 0 | 0 | 0 | 3557 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| HLAMP_1.SRM | 59425 | 0.999996 | 0 | 0 | 0 | 59413 | 0.999982 | 0 | 0 | 0 | 0 | 0 | 1 |
| HNDTRCH_1.SRM | 7829 | 0.998703 | 0 | 0 | 0 | 7817 | 0.998703 | 0 | 0 | 0 | 0 | 0 | 1 |
| HORZSHF_1.SRM | 21357 | 1 | 0 | 0 | 0 | 21345 | 0.999998 | 0 | 0 | 0 | 0 | 0 | 0 |
| HUBSTAFF_1.SRM | 26661 | 1 | 0 | 0 | 0 | 26649 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| HUNTER_1.SRM | 294353 | 0.725289 | 0 | 0 | 0 | 294341 | 0.733485 | 0 | 0 | 0 | 0 | 0 | 1 |
| HUNTER_2.SRM | 306233 | 0.978242 | 0 | 0 | 0 | 306221 | 0.980734 | 0 | 0 | 0 | 0 | 0 | 1 |
| INGEAR_1.SRM | 32721 | 1 | 0 | 0 | 0 | 32709 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| KAIN_1.SRM | 728477 | 0.51712 | 0 | 0 | 0 | 728465 | 0 | 33 | 29 | 29 | 0 | 25 | 4 |
| KNIFE_1.SRM | 6931 | 0.999996 | 0 | 0 | 0 | 6919 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| LDOOR_1.SRM | 28329 | 0.999996 | 0 | 0 | 0 | 28317 | 0.999268 | 0 | 0 | 0 | 0 | 0 | 0 |
| LENSRCK_1.SRM | 28901 | 0.999023 | 0 | 0 | 0 | 28889 | 0.999023 | 0 | 0 | 0 | 0 | 0 | 1 |
| LENSRCK_2.SRM | 28901 | 0.999023 | 0 | 0 | 0 | 28889 | 0.999023 | 0 | 0 | 0 | 0 | 0 | 1 |
| LENSRCK_3.SRM | 28901 | 0.999023 | 0 | 0 | 0 | 28889 | 0.999023 | 0 | 0 | 0 | 0 | 0 | 1 |
| LENSRCK_4.SRM | 28901 | 0.999023 | 0 | 0 | 0 | 28889 | 0.999023 | 0 | 0 | 0 | 0 | 0 | 1 |
| LEVER_1.SRM | 7281 | 1 | 0 | 0 | 0 | 7269 | 0.999995 | 0 | 0 | 0 | 0 | 0 | 0 |
| LIFT_1.SRM | 89821 | 0.999996 | 0 | 0 | 0 | 89809 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| LNGSWCH_1.SRM | 25033 | 0.999938 | 0 | 0 | 0 | 25021 | 0.999878 | 0 | 0 | 0 | 0 | 0 | 1 |
| LSTAFF_1.SRM | 29065 | 0.998922 | 0 | 0 | 0 | 29053 | 0.999937 | 0 | 0 | 0 | 0 | 0 | 1 |
| LUKPOON_1.SRM | 12421 | 0.999996 | 0 | 0 | 0 | 12409 | 0.999871 | 0 | 0 | 0 | 0 | 0 | 1 |
| MASHER_1.SRM | 132417 | 0.979177 | 0 | 0 | 0 | 132405 | 0.997869 | 0 | 0 | 0 | 0 | 0 | 1 |
| MASHER_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| MATSREVR_1.SRM | 124015 | 0.983843 | 0 | 0 | 0 | 124003 | 0.729607 | 0 | 0 | 0 | 0 | 0 | 1 |
| MEALTHR_1.SRM | 2065 | 1 | 0 | 0 | 0 | 2053 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| MMCRANK_1.SRM | 11749 | 0.999939 | 0 | 0 | 0 | 11737 | 0.999939 | 0 | 0 | 0 | 0 | 0 | 0 |
| MOEBIUS_1.SRM | 562541 | 0.938714 | 0 | 0 | 0 | 562529 | 0 | 30 | 28 | 28 | 0 | 26 | 2 |
| MORBOSS_1.SRM | 478217 | 0.749191 | 0 | 0 | 0 | 478205 | 0.749191 | 0 | 0 | 0 | 0 | 0 | 1 |
| MOREGG_1.SRM | 95461 | 0.9979 | 0 | 0 | 0 | 95449 | 0.998902 | 0 | 0 | 0 | 0 | 0 | 1 |
| MORLOCK_1.SRM | 427021 | 0.97459 | 0 | 0 | 0 | 427009 | 0.983849 | 0 | 0 | 0 | 0 | 0 | 1 |
| MORLOCK_2.SRM | 369041 | 0.979644 | 0 | 0 | 0 | 369029 | 0.982458 | 0 | 0 | 0 | 0 | 0 | 1 |
| MORLOCK_3.SRM | 367613 | 0.647236 | 0 | 0 | 0 | 367601 | 0 | 30 | 28 | 28 | 0 | 26 | 2 |
| MORLOCK_4.SRM | 366361 | 0.647236 | 0 | 0 | 0 | 366349 | 0.653701 | 0 | 0 | 0 | 0 | 0 | 1 |
| MOUND_1.SRM | 4201 | 1 | 0 | 0 | 0 | 4189 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| MRDIAL_1.SRM | 11573 | 0.999008 | 0 | 0 | 0 | 11561 | 0.999934 | 0 | 0 | 0 | 0 | 0 | 0 |
| MRDOORA_1.SRM | 27009 | 0.999007 | 0 | 0 | 0 | 26997 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| MRDOORB_1.SRM | 27009 | 0.999007 | 0 | 0 | 0 | 26997 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| MSTAT_1.SRM | 561993 | 0.649034 | 0 | 0 | 0 | 561981 | 0.969415 | 0 | 0 | 0 | 0 | 0 | 1 |
| NABLK_1.SRM | 7201 | 1 | 0 | 0 | 0 | 7189 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| NBBLK_1.SRM | 5513 | 1 | 0 | 0 | 0 | 5501 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| NBDOOR_1.SRM | 166673 | 0.980699 | 0 | 0 | 0 | 166661 | 0.543712 | 0 | 0 | 0 | 0 | 0 | 1 |
| NDOORA_1.SRM | 18537 | 0.998889 | 0 | 0 | 0 | 18525 | 0.998947 | 0 | 0 | 0 | 0 | 0 | 0 |
| NDOOR_1.SRM | 18537 | 0.999012 | 0 | 0 | 0 | 18525 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| NIGBLK_1.SRM | 5449 | 0.99902 | 0 | 0 | 0 | 5437 | 0.999021 | 0 | 0 | 0 | 0 | 0 | 0 |
| NITDBLD_1.SRM | 10865 | 1 | 0 | 0 | 0 | 10853 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| NITGEAR_1.SRM | 22681 | 0.999016 | 0 | 0 | 0 | 22669 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| NITVALV_1.SRM | 9697 | 1 | 0 | 0 | 0 | 9685 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| NODOORA_1.SRM | 6905 | 1 | 0 | 0 | 0 | 6893 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| NODOORB_1.SRM | 6905 | 1 | 0 | 0 | 0 | 6893 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OABLK_1.SRM | 5481 | 0.99902 | 0 | 0 | 0 | 5469 | 0.99902 | 0 | 0 | 0 | 0 | 0 | 1 |
| OBELISK_1.SRM | 13405 | 1 | 0 | 0 | 0 | 13393 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| OCDOORA_1.SRM | 17065 | 1 | 0 | 0 | 0 | 17053 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| OCDOORB_1.SRM | 6977 | 1 | 0 | 0 | 0 | 6965 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OCDOORC_1.SRM | 14433 | 0.999939 | 0 | 0 | 0 | 14421 | 0.999999 | 0 | 0 | 0 | 0 | 0 | 0 |
| OCDOORD_1.SRM | 14433 | 0.999939 | 0 | 0 | 0 | 14421 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OCDOORE_1.SRM | 6977 | 1 | 0 | 0 | 0 | 6965 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OCDOORF_1.SRM | 15553 | 1 | 0 | 0 | 0 | 15541 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OCDOOR_1.SRM | 32761 | 0.999996 | 0 | 0 | 0 | 32749 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OCPILLA_1.SRM | 28113 | 0.998009 | 0 | 0 | 0 | 28101 | 0.998943 | 0 | 0 | 0 | 0 | 0 | 1 |
| OCPILL_1.SRM | 40561 | 0.997947 | 0 | 0 | 0 | 40549 | 0.99891 | 0 | 0 | 0 | 0 | 0 | 1 |
| OCVWALL_1.SRM | 26777 | 0.999996 | 0 | 0 | 0 | 26765 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| OPSHBLK_1.SRM | 5449 | 0.99902 | 0 | 0 | 0 | 5437 | 0.999021 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORADOOR_1.SRM | 17961 | 0.745124 | 0 | 0 | 0 | 17949 | 0.999877 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORADORA_1.SRM | 24325 | 0.999999 | 0 | 0 | 0 | 24313 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 1 |
| ORBLKA_1.SRM | 5257 | 0.999996 | 0 | 0 | 0 | 5245 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| ORBLKB_1.SRM | 5257 | 1 | 0 | 0 | 0 | 5245 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| ORBLKC_1.SRM | 5257 | 1 | 0 | 0 | 0 | 5245 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| ORBLKD_1.SRM | 5257 | 1 | 0 | 0 | 0 | 5245 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| ORDIALA_1.SRM | 7593 | 0.999022 | 0 | 0 | 0 | 7581 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORDIALB_1.SRM | 7593 | 0.999022 | 0 | 0 | 0 | 7581 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORDIALC_1.SRM | 7593 | 0.999022 | 0 | 0 | 0 | 7581 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORFACE_1.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORFACE_2.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORFACE_3.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| ORPLAT_1.SRM | 35421 | 0.999933 | 0 | 0 | 0 | 35409 | 0.999937 | 0 | 0 | 0 | 0 | 0 | 0 |
| OTORCH_1.SRM | 84391 | 0.977085 | 0 | 0 | 0 | 84379 | 0.998902 | 0 | 0 | 0 | 0 | 0 | 1 |
| OUTGEAR_1.SRM | 36277 | 0.999937 | 0 | 0 | 0 | 36265 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| PENDULM_1.SRM | 22881 | 0.983967 | 0 | 0 | 0 | 22869 | 0.998994 | 0 | 0 | 0 | 0 | 0 | 1 |
| PIPESTK_1.SRM | 7773 | 0.99902 | 0 | 0 | 0 | 7761 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| PLSWTCH_1.SRM | 20393 | 0.997995 | 0 | 0 | 0 | 20381 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| PLUGBLK_1.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PLUGZ_1.SRM | 11137 | 1 | 0 | 0 | 0 | 11125 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PRIESTS_1.SRM | 392851 | 0.98152 | 0 | 0 | 0 | 392839 | 0.952144 | 0 | 0 | 0 | 0 | 0 | 1 |
| PRONG_1.SRM | 25809 | 0.999877 | 0 | 0 | 0 | 25797 | 0.993935 | 0 | 0 | 0 | 0 | 0 | 1 |
| PSHBLKA_1.SRM | 6505 | 1 | 0 | 0 | 0 | 6493 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHBLKZ_1.SRM | 9865 | 1 | 0 | 0 | 0 | 9853 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHBLK_1.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHPL_1.SRM | 9833 | 1 | 0 | 0 | 0 | 9821 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHSCA_1.SRM | 5481 | 0.999016 | 0 | 0 | 0 | 5469 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| PSHSCB_1.SRM | 5481 | 0.999016 | 0 | 0 | 0 | 5469 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| PSHSCC_1.SRM | 5481 | 0.999016 | 0 | 0 | 0 | 5469 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| PSHSCD_1.SRM | 5481 | 0.999017 | 0 | 0 | 0 | 5469 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| PSHSCE_1.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHSCF_1.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHSCG_1.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| PSHSCZ_1.SRM | 11785 | 0.99896 | 0 | 0 | 0 | 11773 | 0.998981 | 0 | 0 | 0 | 0 | 0 | 1 |
| PSHUBA_1.SRM | 9793 | 0.999939 | 0 | 0 | 0 | 9781 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 1 |
| PWEEL_1.SRM | 38421 | 1 | 0 | 0 | 0 | 38409 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 0 |
| RAVEN_1.SRM | 63393 | 0.98308 | 0 | 0 | 0 | 63381 | 0.983455 | 0 | 0 | 0 | 0 | 0 | 1 |
| RAZIEL_1.SRM | 865729 | 0.521019 | 0 | 0 | 0 | 865717 | 0.413134 | 0 | 0 | 0 | 0 | 0 | 4 |
| RDOOR_1.SRM | 28393 | 0.999933 | 0 | 0 | 0 | 28381 | 0.999997 | 0 | 0 | 0 | 0 | 0 | 0 |
| REAVPRT_1.SRM | 9169 | 0.999996 | 0 | 0 | 0 | 9157 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| RIBROT_1.SRM | 116197 | 0.998959 | 0 | 0 | 0 | 116185 | 0.999932 | 0 | 0 | 0 | 0 | 0 | 1 |
| RONINBSS_1.SRM | 597781 | 0.641747 | 0 | 0 | 0 | 597769 | 1E-06 | 8 | 8 | 7 | 0 | 7 | 2 |
| RONINBSS_2.SRM | 597441 | 0.641747 | 0 | 0 | 0 | 597429 | 0.7472 | 0 | 0 | 0 | 0 | 0 | 1 |
| RONIN_1.SRM | 423565 | 0.590867 | 0 | 0 | 0 | 423553 | 0.719311 | 0 | 0 | 0 | 0 | 0 | 1 |
| RONIN_2.SRM | 398713 | 0.682923 | 0 | 0 | 0 | 398701 | 0.952108 | 0 | 0 | 0 | 0 | 0 | 1 |
| ROOFLAP_1.SRM | 2345 | 1 | 0 | 0 | 0 | 2333 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| RUBLEA_1.SRM | 4989 | 1 | 0 | 0 | 0 | 4977 | 0.847127 | 0 | 0 | 0 | 0 | 0 | 1 |
| RUBLEB_1.SRM | 4481 | 0.999016 | 0 | 0 | 0 | 4469 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| RVRSKL_1.SRM | 98187 | 0.787975 | 0 | 0 | 0 | 98175 | 0.787975 | 0 | 0 | 0 | 0 | 0 | 1 |
| SCBLCK_1.SRM | 4513 | 1 | 0 | 0 | 0 | 4501 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SCFAN_1.SRM | 27913 | 1 | 0 | 0 | 0 | 27901 | 0.996734 | 0 | 0 | 0 | 0 | 0 | 1 |
| SCHOOK_1.SRM | 7333 | 1 | 0 | 0 | 0 | 7321 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SCSLBDR_1.SRM | 10465 | 1 | 0 | 0 | 0 | 10453 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| SCWINDW_1.SRM | 13681 | 1 | 0 | 0 | 0 | 13669 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SGBASE_1.SRM | 16449 | 0.984088 | 0 | 0 | 0 | 16437 | 0.999001 | 0 | 0 | 0 | 0 | 0 | 0 |
| SGBELLA_1.SRM | 33457 | 0.999996 | 0 | 0 | 0 | 33445 | 0.999987 | 0 | 0 | 0 | 0 | 0 | 0 |
| SGBELLB_1.SRM | 33397 | 0.998871 | 0 | 0 | 0 | 33385 | 0.982989 | 0 | 0 | 0 | 0 | 0 | 0 |
| SGTOPHT_1.SRM | 7365 | 1 | 0 | 0 | 0 | 7353 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SGWINDW_1.SRM | 4033 | 1 | 0 | 0 | 0 | 4021 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SHACKLE_1.SRM | 11529 | 0.999758 | 0 | 0 | 0 | 11517 | 0.999758 | 0 | 0 | 0 | 0 | 0 | 1 |
| SHACKLE_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SHAFTA_1.SRM | 65409 | 0.999992 | 0 | 0 | 0 | 65397 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| SHAND_1.SRM | 21125 | 0.984318 | 0 | 0 | 0 | 21113 | 0.984127 | 0 | 0 | 0 | 0 | 0 | 1 |
| SKBBLK_1.SRM | 5449 | 0.999016 | 0 | 0 | 0 | 5437 | 0.999016 | 0 | 0 | 0 | 0 | 0 | 1 |
| SKDOOR_1.SRM | 11481 | 0.999996 | 0 | 0 | 0 | 11469 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| SKGATE_1.SRM | 11881 | 0.998025 | 0 | 0 | 0 | 11869 | 0.999012 | 0 | 0 | 0 | 0 | 0 | 1 |
| SKINBOS_1.SRM | 518925 | 0.702843 | 0 | 0 | 0 | 518913 | 0 | 1 | 1 | 1 | 0 | 1 | 2 |
| SKINBOS_2.SRM | 518873 | 0.702843 | 0 | 0 | 0 | 518861 | 0.721138 | 0 | 0 | 0 | 0 | 0 | 1 |
| SKINNER_1.SRM | 342479 | 0.961704 | 0 | 0 | 0 | 342467 | 0.94122 | 0 | 0 | 0 | 0 | 0 | 1 |
| SKINNER_2.SRM | 365381 | 0.749756 | 0 | 0 | 0 | 365369 | 0.975572 | 0 | 0 | 0 | 0 | 0 | 1 |
| SKPLANK_1.SRM | 3561 | 1 | 0 | 0 | 0 | 3549 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SKPLANK_2.SRM | 1869 | 1 | 0 | 0 | 0 | 1857 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SKULL_1.SRM | 18349 | 0.999938 | 0 | 0 | 0 | 18337 | 0.999013 | 0 | 0 | 0 | 0 | 0 | 1 |
| SLUAGH_1.SRM | 336033 | 0.977074 | 0 | 0 | 0 | 336021 | 0.976891 | 0 | 0 | 0 | 0 | 0 | 1 |
| SNDPTSA_1.SRM | 2797 | 1 | 0 | 0 | 0 | 2785 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| SNDPTSC_1.SRM | 5113 | 1 | 0 | 0 | 0 | 5101 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SNDPTSD_1.SRM | 8377 | 1 | 0 | 0 | 0 | 8365 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| SNDPTSE_1.SRM | 3353 | 1 | 0 | 0 | 0 | 3341 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| SPECRVR_1.SRM | 124047 | 0.983843 | 0 | 0 | 0 | 124035 | 0.729607 | 0 | 0 | 0 | 0 | 0 | 1 |
| SPHINCA_1.SRM | 12353 | 1 | 0 | 0 | 0 | 12341 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SQUOSH_1.SRM | 5761 | 1 | 0 | 0 | 0 | 5749 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STAFF_1.SRM | 24573 | 0.999992 | 0 | 0 | 0 | 24561 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 1 |
| STAKE_F_1.SRM | 6917 | 0.984363 | 0 | 0 | 0 | 6905 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| STAKE_L_1.SRM | 6917 | 0.984363 | 0 | 0 | 0 | 6905 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| STAKE_R_1.SRM | 6917 | 0.984363 | 0 | 0 | 0 | 6905 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| STDALUK_1.SRM | 10561 | 0.999016 | 0 | 0 | 0 | 10549 | 0.999938 | 0 | 0 | 0 | 0 | 0 | 1 |
| STDCATH_1.SRM | 35453 | 0.982552 | 0 | 0 | 0 | 35441 | 0.998973 | 0 | 0 | 0 | 0 | 0 | 1 |
| STDORAC_1.SRM | 21613 | 0.99802 | 0 | 0 | 0 | 21601 | 0.98403 | 0 | 0 | 0 | 0 | 0 | 1 |
| STDSKIN_1.SRM | 38017 | 0.983718 | 0 | 0 | 0 | 38005 | 0.984067 | 0 | 0 | 0 | 0 | 0 | 1 |
| STDTOMB_1.SRM | 6841 | 1 | 0 | 0 | 0 | 6829 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| STICK_1.SRM | 9213 | 1 | 0 | 0 | 0 | 9201 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNAFF_1.SRM | 80417 | 0.999948 | 0 | 0 | 0 | 80405 | 0.983636 | 0 | 0 | 0 | 0 | 0 | 1 |
| STNBLK_1.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNBLK_2.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNBLK_3.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNBLK_4.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNBLK_5.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNBLK_6.SRM | 5481 | 0.999996 | 0 | 0 | 0 | 5469 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNBLK_7.SRM | 5449 | 0.999996 | 0 | 0 | 0 | 5437 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLPH_1.SRM | 119481 | 0.998828 | 0 | 0 | 0 | 119469 | 0.997994 | 0 | 0 | 0 | 0 | 0 | 1 |
| STNGLSA_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSB_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSC_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSD_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSE_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSF_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSG_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLSH_1.SRM | 2569 | 1 | 0 | 0 | 0 | 2557 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| STNGLS_1.SRM | 4329 | 1 | 0 | 0 | 0 | 4317 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SUDOOR_1.SRM | 7473 | 1 | 0 | 0 | 0 | 7461 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| SUGATEA_1.SRM | 222805 | 0.983202 | 0 | 0 | 0 | 222793 | 0.998827 | 0 | 0 | 0 | 0 | 0 | 1 |
| SUGATE_1.SRM | 21785 | 0.999992 | 0 | 0 | 0 | 21773 | 0.999933 | 0 | 0 | 0 | 0 | 0 | 1 |
| SWING_1.SRM | 164529 | 0.996094 | 0 | 0 | 0 | 164517 | 0.996094 | 0 | 0 | 0 | 0 | 0 | 2 |
| TRNBLK_1.SRM | 5449 | 1 | 0 | 0 | 0 | 5437 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| TUBBLKA_1.SRM | 7673 | 0.999016 | 0 | 0 | 0 | 7661 | 0.99275 | 0 | 0 | 0 | 0 | 0 | 1 |
| TUBBLKB_1.SRM | 8661 | 1 | 0 | 0 | 0 | 8649 | 0.999989 | 0 | 0 | 0 | 0 | 0 | 0 |
| TUBSCA_1.SRM | 7673 | 0.999992 | 0 | 0 | 0 | 7661 | 0.749996 | 0 | 0 | 0 | 0 | 0 | 1 |
| TUBSCB_1.SRM | 8813 | 1 | 0 | 0 | 0 | 8801 | 0.75012 | 0 | 0 | 0 | 0 | 0 | 1 |
| TUBSCC_1.SRM | 9133 | 1 | 0 | 0 | 0 | 9121 | 0.750238 | 0 | 0 | 0 | 0 | 0 | 1 |
| URNX_1.SRM | 14805 | 1 | 0 | 0 | 0 | 14793 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| URNY_1.SRM | 9401 | 0.999996 | 0 | 0 | 0 | 9389 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 1 |
| URNZ_1.SRM | 16041 | 0.984255 | 0 | 0 | 0 | 16029 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| VALVA_1.SRM | 9833 | 0.999996 | 0 | 0 | 0 | 9821 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| VALVE_1.SRM | 9833 | 0.999996 | 0 | 0 | 0 | 9821 | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| VENTSC_1.SRM | 5793 | 1 | 0 | 0 | 0 | 5781 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| VERTSHF_1.SRM | 28671 | 1 | 0 | 0 | 0 | 28659 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| VLGRA_1.SRM | 190925 | 0.713288 | 0 | 0 | 0 | 190913 | 0.999515 | 0 | 0 | 0 | 0 | 0 | 1 |
| VLGRA_2.SRM | 164601 | 0.998879 | 0 | 0 | 0 | 164589 | 0.997757 | 0 | 0 | 0 | 0 | 0 | 1 |
| VLGRB_1.SRM | 118981 | 0.998794 | 0 | 0 | 0 | 118969 | 0.998865 | 0 | 0 | 0 | 0 | 0 | 1 |
| VLGRB_2.SRM | 166329 | 0.739905 | 0 | 0 | 0 | 166317 | 0.941834 | 0 | 0 | 0 | 0 | 0 | 1 |
| VLGRC_1.SRM | 221707 | 0.998871 | 0 | 0 | 0 | 221695 | 0.996954 | 0 | 0 | 0 | 0 | 0 | 1 |
| VWRAITH_1.SRM | 181205 | 0.962761 | 0 | 0 | 0 | 181193 | 0.997095 | 0 | 0 | 0 | 0 | 0 | 1 |
| WALBOSB_1.SRM | 99305 | 0.997888 | 0 | 0 | 0 | 99293 | 0.998899 | 0 | 0 | 0 | 0 | 0 | 1 |
| WALBOSC_1.SRM | 13009 | 0.999938 | 0 | 0 | 0 | 12997 | 0.999939 | 0 | 0 | 0 | 0 | 0 | 0 |
| WALBOSE_1.SRM | 22017 | 0.999938 | 0 | 0 | 0 | 22005 | 0.999996 | 0 | 0 | 0 | 0 | 0 | 0 |
| WALBOSS_1.SRM | 1244157 | 0.509084 | 0 | 0 | 0 | 1244145 | 0.222478 | 0 | 0 | 0 | 0 | 0 | 6 |
| WALFLAP_1.SRM | 2217 | 1 | 0 | 0 | 0 | 2205 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| WALLCR_1.SRM | 387233 | 0.683181 | 0 | 0 | 0 | 387221 | 0.683163 | 0 | 0 | 0 | 0 | 0 | 1 |
| WALLCR_2.SRM | 389101 | 0.737566 | 0 | 0 | 0 | 389089 | 0.73719 | 0 | 0 | 0 | 0 | 0 | 1 |
| WAPIPE_1.SRM | 6785 | 0.998959 | 0 | 0 | 0 | 6773 | 0.99902 | 0 | 0 | 0 | 0 | 0 | 1 |
| WBPIPE_1.SRM | 6785 | 0.99902 | 0 | 0 | 0 | 6773 | 0.99902 | 0 | 0 | 0 | 0 | 0 | 1 |
| WCEGG_1.SRM | 114657 | 0.982029 | 0 | 0 | 0 | 114645 | 0.98109 | 0 | 0 | 0 | 0 | 0 | 1 |
| WCPIPE_1.SRM | 6241 | 0.999935 | 0 | 0 | 0 | 6229 | 0.999935 | 0 | 0 | 0 | 0 | 0 | 1 |
| WINDER_1.SRM | 20017 | 0.99801 | 0 | 0 | 0 | 20005 | 0.999934 | 0 | 0 | 0 | 0 | 0 | 0 |
| WINDER_2.SRM | 20017 | 0.968631 | 0 | 0 | 0 | 20005 | 0.749985 | 0 | 0 | 0 | 0 | 0 | 0 |
| WINDER_3.SRM | 20017 | 0.968631 | 0 | 0 | 0 | 20005 | 0.749985 | 0 | 0 | 0 | 0 | 0 | 0 |
| WRPDOOR_1.SRM | 18713 | 0.98296 | 0 | 0 | 0 | 18701 | 0.998997 | 0 | 0 | 0 | 0 | 0 | 1 |
| WRSHP_1.SRM | 242465 | 0.699801 | 0 | 0 | 0 | 242453 | 0.700209 | 0 | 0 | 0 | 0 | 0 | 1 |
| WRSHP_2.SRM | 250845 | 0.979234 | 0 | 0 | 0 | 250833 | 0.965892 | 0 | 0 | 0 | 0 | 0 | 1 |
| WTRWEEL_1.SRM | 48497 | 0.999996 | 0 | 0 | 0 | 48485 | 0.937953 | 0 | 0 | 0 | 0 | 0 | 1 |
| XBOW_1.SRM | 71657 | 0.999929 | 0 | 0 | 0 | 71645 | 0.858142 | 0 | 0 | 0 | 0 | 0 | 1 |
| XBOW_2.SRM | 4405 | 0.999996 | 0 | 0 | 0 | 4393 | 0.820341 | 0 | 0 | 0 | 0 | 0 | 0 |

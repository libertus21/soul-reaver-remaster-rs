# pcm-animation-record - no materialised descriptor array or track table in the archived object container (2026-11-09)

Read-only byte probe of the single-model object containers inside the shipped
SR1 `BIGFILE.DAT` (`1\BIGFILE.DAT`), plus the engine's own `--object-info`
reader as the bone-count oracle. Nothing under the install was written; no game
bytes were copied into the repository; scratch lived only in the session temp
directory. This note records counts, sizes, record counts and format constants
only: no addresses, no file/record offsets, no disassembly, no raw game bytes, no
absolute install paths.

Date: 2026-11-09. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable test), **INFERRED** (one side only), **UNKNOWN**.

## Question

`docs/changes/remaster-animation/animation-source.md` names a per-model
**animation record** made of (a) a **32-byte per-slot descriptor array** (an
active marker plus mode bits) sized to the model's bone count, and (b) a separate
**track table** (`frame count` / `divisor` plus frames of **4 floats**). It lists
the exact file field that populates that record as UNKNOWN. This pass asks one
question: **is that descriptor array or that track table actually carried inside
the archived object container (`.PCM`)?** The pass/fail gate is the model's bone
count: a per-slot descriptor array must contain exactly `bone_count` records.

## Verdict

| # | Question | Answer | Class |
|---|---|---|---|
| 1 | Do the runtime slot counts still equal the model bone counts? | **YES, 9/10** as already recorded (`raziel 66`, `sluagh 24`, `eggfx 3`, `soul 2`, singletons `1`; `hint` is `6` bones vs `1` slot). Re-read here with the engine's own parser. | CONFIRMED |
| 2 | Does the container hold a 32-byte-stride array of exactly `bone_count` records that behaves like a descriptor array? | **NO.** A count-equal window is trivial (every container larger than `bone_count * 32` has one), so that alone cannot validate. A structural test (a marker byte column whose set/clear pattern equals the model's vertex-bearing-bone pattern) fires in **135 / 394** containers, but **109 of those (81%)** land inside the object header/directory, the bone table, the vertex array or the polygon array, i.e. they are `0`/`1` bytes of known arrays. The remaining **26** are auxiliary/bytecode bytes that do not reproduce. `raziel` (66) and `soul` (2) have **none**. | CONFIRMED negative for the structural gate |
| 3 | Does the container hold a track table of 4-float frames? | **NO.** Unit-quaternion `f32` runs never exceed **4** frames anywhere (only 2 containers reach 4; 0 reach 8). Fixed-point `i16`/4096 quaternion runs exist but are coincidental: the longest (87) is in `undhost`, a **one-bone** model that cannot carry 87 body frames. No frame-count/divisor header bounds any run. | CONFIRMED negative |
| 4 | Does the container leave room for a separate materialised descriptor array? | **NO for the decidable small models.** `eggfx` (3 bones, needs 96 bytes) and `hint` (6 bones, needs 192 bytes) are fully byte-accounted: header + directory + model record (header, bone table, vertices, polygons, materials) + one type-1 colour/index record. There is no distinct `bone_count * 32` block. `hint` is the very model the authority note explains with "only one active descriptor"; six records would need 192 bytes and none exist. | CONFIRMED negative |
| 5 | Where do the large model records' tails go? | Into the actor **behaviour / parameter program** (small opcode-like words), the same family already recorded for actor blocks. Not animation. | INFERRED (strong) |
| 6 | Ship a decoder? | **NO.** The bone-count gate cannot be satisfied by a real descriptor array, so phase 2 is not justified. | CONFIRMED negative |

The decisive rows are 2 and 4. A materialised per-slot descriptor array must sit
somewhere as `bone_count` contiguous 32-byte records; in the two smallest
decidable containers there is no such block, and in the largest (`raziel`, the
66-bone gate model) no window carries the descriptor signature at all.

## 1. Bone-count oracle (engine, read-only)

The engine's own parser was used as the oracle, so the gate values are not
invented:

| Asset | Runtime slots | `--object-info` bones | Ranged bones | Match |
|---|---:|---:|---:|:---:|
| `raziel` | 66 | 66 | 58 | yes |
| `sluagh` | 24 | 24 | 23 | yes |
| `skinner` | 35 | 35 | 32 | yes |
| `eggfx` | 3 | 3 | 1 | yes |
| `soul` | 2 | 2 | 1 | yes |
| `morlock` | 37 | 37 | 34 | yes |
| `hint` | 1 | 6 | 2 | no (documented outlier) |

"Ranged bones" are the bones whose vertex range is set (`first/last` not the
`0xFFFF` sentinel and `first <= last`). They are the only plausible on-disk
correlate of an "active marker", so they are used as the structural target below.

## 2. Descriptor-array search

### 2.1 Corpus

- **394** single-model object containers parse with the confirmed boundary
  identities and a non-zero bone count — the same count as
  `docs/notes/object-models.md`.
- **~2,042,764** payload bytes were scanned at **every** byte offset (~2.0 MB).
- The count-equal check alone is vacuous: every container larger than
  `bone_count * 32` trivially contains a window of `bone_count` 32-byte records.
  Count equality is therefore necessary but not sufficient, and cannot be used as
  the discriminating gate by itself.

### 2.2 Structural test

For a window to be a descriptor array it must have a marker column whose
set/clear pattern is the model's ranged-bone pattern (active slots align to
vertex-bearing bones). Scan: a `32`-byte stride, exactly `bone_count` records,
over all four byte columns of every offset.

| Location of the matching window | Containers |
|---|---:|
| object header / directory | 62 |
| bone table | 29 |
| vertex array | 10 |
| polygon array | 8 |
| auxiliary / bytecode tail | 26 |
| **total** | **135 / 394** |

The 109 non-tail hits are `0`/`1` bytes of arrays the mesh parser already
confirms, so they are **not** descriptor arrays. Of the 26 tail hits, none is
corroborated by a companion track table or a stable per-model record shape, and
the two structurally decisive models are absent:

- `raziel` (66 bones): **no** offset anywhere in 202,640 payload bytes yields an
  exact match.
- `soul` (2 bones): **no** match.

So the structural agreement rate is **0 / 2** on the primary gate models and the
corpus "hits" are false-positive-dominated. A layout adopted on this evidence
would be the "looks plausible, disagrees with the gate" failure the task forbids.

## 3. Track-table search

The documented track frame is `16` bytes (four `f32`); the selector copies the
selected frame into the head slot, indexed by `(time / divisor) % frame_count`.

- **`f32` unit-quaternion runs (four floats, |q| = 1 ± 0.05):** only **2** of 394
  containers contain a run as long as **4** frames; **0** contain a run of 8 or
  more. A clip is 0.3-6 s at 24-60 fps, i.e. roughly 7-360 frames, so a 4-frame
  maximum refutes a float-quaternion track table.
- **Fixed-point `i16`/4096 quaternion runs:** present, but the longest (**87**
  frames) is in `undhost`, a **one-bone** model with one polygon — impossible as
  a body clip and therefore coincidence (`i16` data in colour/bytecode regions
  readily satisfies a unit-norm window). 43 containers reach 8 frames and 16
  reach 16, none bounded by a frame-count/divisor header.
- No container shows a `frame_count` / `divisor` header followed by a matching
  `frame_count`-long frame block at `16`-byte stride.

The only shipped structure with the documented `frames`/`fps` + `3x4` shape is
the separate `.SRL` clip class (`src/srl.rs`), which the authority note already
binds to the facial/HD branch, not to body bones.

## 4. Full byte-accounting of the decidable containers

The smallest containers are fully explained, which removes any "the array is
just hidden in the tail" escape:

- **`eggfx`** (3 bones; a descriptor array needs `3 * 32 = 96` bytes). Payload is
  `432` bytes: object header + two-entry directory + a `200`-byte model record
  (model header, three-bone table, three vertices, one polygon, one 16-byte
  material) + a single `76`-byte type-1 colour/index record. There is no
  `96`-byte block outside the mesh bytes.
- **`hint`** (6 bones; needs `6 * 32 = 192` bytes). Payload is `696` bytes:
  object header + two-entry directory + a `444`-byte model record (header, six
  bones, twelve vertices, nine polygons, one material) + a single `112`-byte
  type-1 colour/ramp record. There is no `192`-byte block. `hint` is exactly the
  model whose `6 bones vs 1 slot` the authority note explains as "one active
  descriptor"; if such an array existed it would be six records here, and it is
  not present.
- **`soul`** (2 bones; needs `64` bytes). Payload is `920` bytes: model record
  (`208`) + one `548`-byte type-4 nested sub-resource container (pointer-linked
  child records), not a flat 64-byte descriptor array.

The large records (`raziel`, `sluagh`, `skinner`, `ronin`, `morlock`, `kain`)
carry a long tail of small opcode-like words — the actor behaviour/parameter
program — not animation frames.

## 5. Falsification ledger

| Claim | Observation that would refute it | Tested? |
|---|---|---|
| The container carries a `bone_count`-record 32-byte descriptor array | a 32-byte window of `bone_count` records whose marker tracks the vertex-bearing bones, distinct from mesh bytes | Tested: 0/2 on the gate models (`raziel`, `soul`); corpus hits are header/table/mesh bytes |
| The descriptor array has a stable structural signature (active + mode) | any container where the candidate block survives mesh-byte exclusion and repeats across models | Tested: 26 tail candidates, none repeat; no companion track table |
| The container carries a 4-float track table | a run of ≥7 unit-quaternion float frames, or a framed `frame_count`/`divisor` block | Tested: max float run is 4; no framed block |
| The container leaves room for the array | a `bone_count * 32` block outside the mesh/colour/bytecode regions | Tested: `eggfx` and `hint` fully mapped, no such block |
| The animation record is a `.PCM`-resident materialised array | the array or track table found in any single-model container | Tested negative over 394 containers / ~2.0 MB |

## 6. Why no decoder shipped

The task's hard gate is: the descriptor array's element count must equal the
model bone count, and the track table's frames must decode coherently. A count-
equal window exists trivially in every container, so it cannot carry the gate; a
**structural** count-equal window (marker tied to bone semantics) does not exist
on the gate models, and no 4-float track table exists anywhere. Per the hard
rule, a documented negative is the success condition, and inventing a 32-byte
layout that "looks plausible" but disagrees with the bone-count gate would be
fabrication. No `--pcm-anim` command, parser, struct or test was added, the
engine loop and digest are untouched, and no golden moved.

## 7. What this changes

- The `.PCM` object container is the **legacy low-poly model + actor program**.
  Its bytes are the object header, relocation directory, model header, 24-byte
  bone table, vertex/polygon arrays, 16-byte materials, type-tagged colour/param
  records and the behaviour/parameter bytecode. It does **not** carry a
  materialised per-slot descriptor array or a 4-float track table.
- This is consistent with (and tightens) `animation-source.md`: the in-memory
  animation record's **slot count is the model bone count by construction**, and
  the descriptor/track values are built at load; the exact file carrier remains
  **UNKNOWN** and is not the model container.
- The honest next route is unchanged from the authority note: **pin the loader
  in the shipped module** (trace the routine that allocates the descriptor array
  of size `bone_count` and fills the mode bits), then look at what data source
  feeds it. That is a binary-side slice, not a `.PCM` decode.

## Method and legal

- Read-only: the engine's `--object-info --bigfile <DAT> --entry <N>` on the
  user's own SR1 archive (`1\BIGFILE.DAT`), plus a scratch byte scanner over the
  same read-only archive. Scratch lived in the session temp directory.
- Corpus: 394 single-model containers, ~2.0 MB payload, every byte offset for the
  descriptor column test, every 2-byte and 4-byte alignment for the quaternion
  run tests.
- No production code changed; no golden moved; no CLI added. Counts, sizes,
  strides and format constants only. No addresses, file/record offsets,
  disassembly, raw game bytes or absolute install paths are recorded here.

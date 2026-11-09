# sra-key-search - a layout-free search for the confirmed spline keys

Read-only probe of the SR1 remaster `OBJECT` folder (227 `.SRA` files) plus a
read-only calibration on the `font` object container. Nothing under the install
was written; no game bytes were copied into the repository; scratch lived only in
the session temp directory. This note records counts, container names, tag
families and format constants only: no addresses, no file/record offsets, no
disassembly, no raw game bytes, no install paths.

This note answers a narrower question than `docs/notes/sra-animation.md`. That
pass assumed a clip table and compared record counts. This pass searches the
payload directly for the **key shapes** confirmed in
`docs/notes/spline-animation.md`, which had never been done.

## Verdict

| Question | Answer | Class |
|---|---|---|
| 1. Does the confirmed detector re-find the `font` channel? | **YES**: one 3-key rotation run, deltas `75,75,0`, span `150`, unit quaternions. | CONFIRMED (calibration) |
| 2. Do `.SRA` payloads contain coherent rotation-key runs? | **NO**: 7 runs in 4/227 files; 6 runs are 2 keys and the single 9-key run is a periodic byte artefact. | CONFIRMED negative |
| 3. Do `.SRA` payloads contain position-key runs? | Raw runs are abundant (790 in 61 files) but the position shape is **not discriminative**; the same detector fires on mesh data. Not evidence of animation. | CONFIRMED non-specific |
| 4. Do detected runs relate to the 8x3 trio clip ids (24 clips)? | **NO**: 2/21 actor files carry any rotation run, none has more than 2 keys, and no file has 24 channels. | CONFIRMED negative |
| 5. Is body animation present in `.SRA` in the confirmed key format? | **NO**. | CONFIRMED negative |

The decisive result is #5: searching for the confirmed key shapes does not
recover an animation channel in `.SRA`, and the one shape that *does* validate
(rotation) only produces chance fragments plus a periodic artefact.

## 1. Detector and calibration

The detector implements the file-side records from `spline-animation.md`
section 2:

| Channel | Stride | Fields | Bounds |
|---|---:|---|---|
| rotation (file) | `0x0a` | `i16` delta + 4 x `i16` quaternion | delta `0..=4096`; quaternion components `<= 8192`; length after `>>12` within `0.10` of `1.0` |
| rotation (runtime) | `0x1c` | the same fields plus 4 x `f32` mirror | mirror must equal `component / 4096` within `0.02` |
| position (file) | `0x20` | `i16` delta + 3 x `i16` position + 6 x `i32` tangents | delta `0..=4096`; tangents `<= 1_000_000` |
| position (runtime) | `0x30` | file fields plus 3 x `f32` position mirror | mirror must equal `position / 4096` within `0.02` |

A run is a maximal sequence of consecutive keys at the fixed stride with total
span `> 0`. Frames are accumulated from the deltas, matching the runtime sampler.

**Calibration:** the `font` object container carries the only end-to-end
validated channel in the corpus (`spline-animation.md` section 5.1). The
detector finds exactly that channel: three rotation keys, deltas `75,75,0`,
span `150`, quaternions `(0,0,0,-4096)`, `(0,0,-4096,0)`, `(0,0,0,4096)`. The
runtime-stride variant correctly finds nothing there, because `font` stores the
compact file form. The in-repo detector is exercised by
`src/sra.rs` unit tests against this exact channel and against deterministic
noise.

## 2. `.SRA` corpus scan

The container parser reproduces the census in `sra-animation.md` exactly:

- 227/227 files parse, 1798 records total, 0 structural failures;
- record-count distribution `1` record in 83 files, `2` in 35, `3` in 20,
  `4-5` in 17, `6-10` in 9, `11-20` in 48, `21-30` in 8, `31-60` in 6, `>60` in 1.

All scans below are over **payload records only** (the byte spans named by the
`0xFFFFFFFF`-closed payload directory). A whole-file scan additionally sees the
preamble/directory bytes; those extra hits are index bytes, not payload.

### 2.1 Rotation keys

| Scan | Runs | Files | Run lengths |
|---|---:|---:|---|
| file stride `0x0a`, payload records | 7 | 4 / 227 | 6 x 2 keys, 1 x 9 keys |
| runtime stride `0x1c`, payload records | 0 | 0 / 227 | - |

Per-file detail (record tag family in brackets):

| File | Actor? | Runs | Lengths | Record tag |
|---|---|:---:|---|---|
| `raziel` | player | 2 | 9, 2 | `0x64646403`, `0x4b4b4b03` |
| `ronin` | actor | 1 | 2 | `0x64646402` |
| `spout` | prop/effect | 1 | 2 | `0x64646401` |
| `walboss` | actor | 3 | 2, 2, 2 | `0x64646401` |

The single 9-key run is the only candidate longer than two keys. It is a
**periodic byte pattern, not a key array**:

- its deltas are `0, 961, 2744, 961, 2744, ...`, i.e. the "delta" word is just
  the next word of the same repeating sequence;
- only two distinct quaternions alternate (`[2744,961,2744,961]` and
  `[961,2744,961,2744]`);
- the same repeating 20-byte signature occurs 17 times in that file.

A real channel authors deltas and quaternions independently; here the delta is
literally part of the quaternion cycle. It is coincidence.

The six 2-key runs are accidental adjacency; two-key "runs" are below any
plausible channel length.

### 2.2 Position keys

| Scan | Runs | Files |
|---|---:|---:|
| file stride `0x20`, payload records | 790 | 61 / 227 |
| with `>= 3` keys and max delta `<= 512` | 383 | 59 / 227 |

The position shape is **not a usable detector**. It has no validated positive
(the `font` container has no position channel; the `particle` fragment in
`actor-spline-census.md` is explicitly marginal) and the same predicate matches
ordinary mesh data:

- 721 position-shaped runs across all 349 `.SRM` meshes;
- on `font`, the first "64-key position run" is the relocation directory itself;
- the `.SRA` position runs have no stable count per file and no relation to a
  clip list.

A position-shaped run therefore cannot distinguish animation from vertices,
matrices or directory words. These counts are reported for completeness only;
they are **not** evidence of animation.

### 2.3 Specificity baseline

| Corpus | Rotation-shaped runs | Files |
|---|---:|---:|
| `.SRA` (whole file) | 27 | 27 / 227 |
| `.SRA` (payload records) | 7 | 4 / 227 |
| `.SRM` (mesh) | 3344 | 175 / 349 |
| deterministic random buffers | 0 in 40 trials | - |

The rotation shape is rare in random data but common in mesh vertex data. It is
not by itself an animation signature, which is why the coherence step above
matters: only a multi-key run with independent, plausible deltas would count.

## 3. Trio correlation

The actor block stores 8 x 3 one-byte clip-set ids (24 values). If `.SRA` held
the clip table, an actor file would expose a channel structure on the order of
those 24 ids.

- Actor `.SRA` files with any rotation run: **2 of 21** (`ronin`, one 2-key
  fragment; `walboss`, three 2-key fragments).
- Actor `.SRA` files with a rotation run of 3 or more keys: **0 of 21**.
- Actor `.SRA` files with 24 detected channels: **0 of 21**.
- Position-shaped runs per actor range from 0 to 43 with no cluster at 24, but
  they are non-specific and cannot support the relation anyway.

This re-refutes the clip-table hypothesis from the key-pattern side, matching
the count-based falsification in `sra-animation.md` section 3.

## 4. What this settles and what it does not

Settled:

- The confirmed key shapes are **detectable** and the detector re-finds the one
  known positive (`font`).
- `.SRA` payload records do **not** contain a coherent rotation channel, and the
  runtime `0x1c` rotation form is absent entirely.
- Position-shaped runs are not animation evidence because the predicate matches
  mesh and directory data just as well.
- The trio ids still do not index per-model `.SRA` channels.

Still UNKNOWN (unchanged):

- The exact meaning of the dense payload in `0x6464640n` records.
- The actor spline-set carrier and the trio-to-spline resolution
  (`spline-animation.md`, `actor-spline-census.md`).
- Whether any other archive family carries the actor channel set.

## 5. Falsification ledger

| Claim | Observation that would refute it | Tested? |
|---|---|---|
| The detector re-finds the known positive | failure to find the `font` 3-key rotation channel | Found exactly, span 150 |
| No coherent rotation channel in `.SRA` | one payload record with >= 3 rotation keys and independent, plausible deltas | 227-file scan: only 2-key fragments and one periodic artefact |
| Runtime rotation keys are absent | one `0x1c`-shaped run with matching float mirrors | 0 hits |
| Position runs are not evidence | a position-only corpus where the predicate fires on animation and not on mesh | Fires on 349/349 meshes and on directory bytes |
| Trio ids do not index `.SRA` channels | an actor file with ~24 detected channels | 0 of 21 |

## 6. Method and legal

- Read-only probe of the 227 loose `OBJECT/*.SRA` files and of the `font`
  object container inside the shipped archive; the install was never written.
- The detector and container parser live in `src/sra.rs` as a read-only module,
  exercised by synthetic fixtures; no engine path consumes them.
- Counts, container names, tag families and format constants only. No addresses,
  no file/record offsets, no disassembly, no raw game bytes, no install paths.
- No production behaviour changed, no golden moved, no CLI added: the verdict is
  negative, and the task conditions the CLI on a coherent positive.

# `.SRL` decode ledger - voice-line animation, not the behaviour clip table (2026-11-09)

Read-only probe of the SR1 remaster `1\OBJECT` folder (60 `.SRL`), cross-checked
against the localized voice tracks (`1\TRACKS\<lang>\*.OGG`) and subtitle files
(`1\TEXT\<lang>\*.TXT`). Nothing under the install was written, no game bytes
were copied into this repository, and scratch lived only in the session temp
directory. This note records counts, structural shapes and small format
constants only: no addresses, no raw offsets, no dumps, no install paths.

## Verdict

The lead hypothesis - *the trailing `.SRL` number is the clip id referenced by
the actor behaviour trios* - is **FALSIFIED**. `.SRL` is a **per-voice-line
animation container**: each file is the animation for one spoken dialogue line,
named with the same identifier as that line's voice `.OGG` and subtitle `.TXT`.
The container is fully self-describing and parses on all 60 files; the pose
question splits into "the transform stream - YES" and "binding it to a
character's bones - NO".

| Question | Answer | Confidence |
|---|---|---|
| 1. Naming | `<3-char target><3-char context><2-digit line>`; shared with voice/subtitle assets, **not** render classes or behaviour families | CONFIRMED shared scheme / INFERRED token roles |
| 2. Trio cross-check | **Fails / invalid**: `.SRL` numbers are local dialogue-line counters, the trios are a global `0..66` action space; overlap is coincidental | CONFIRMED |
| 3. Where other clips live | Not in `.SRL` (and not `.SRM`/`.SRA`): the trio ids index a separate global action list, consistent with the spline system in `anim-binding.md` | INFERRED / partly UNKNOWN |
| 4. Structure | 20-byte header + `tracks * frames` 48-byte cells, frame-major; `len = 20 + tracks*frames*48` on 60/60 | CONFIRMED |
| 5. Bone binding | No name table, no index array; track order is the only signal and does not match model bone counts | CONFIRMED absence / UNKNOWN binding |
| 6. Keyframe encoding | Baked per-frame 48-byte 3x4 affine (twelve `f32`); fps 24/25/30; duration `frames/fps`; no interpolation stored | CONFIRMED shape / UNKNOWN outlier decomposition |
| 7. Pose from files alone | **YES** for the per-track transform stream; **NO** for binding it to a character mesh bone list | CONFIRMED |

## Quick path

1. Read section 2 for the decisive falsification (voice/subtitle/duration).
2. Read sections 4-6 for the confirmed container and record shape.
3. Read section 7 for the YES/NO split and the minimal algorithm.
4. Treat everything under UNKNOWN as open; do not fill it by inference.

## 1. Naming convention

Every `.SRL` name is eight characters plus the extension: `FFFTTTNN.SRL`.

- `FFF` - a 3-letter **animation target / speaker** token. Observed values:
  `ACL, ARI, GRD, KAN, MOB, RCL, SCL`.
- `TTT` - a 3-letter **context / scene** token. Observed values:
  `ALU, HNT, KRT, PIL, TMB, ORC, MSC, ASH, SKN`.
- `NN` - a two-digit **line index**, local to the `FFF`+`TTT` pair.

This is the same identifier scheme used by the localized voice and subtitle
assets. Evidence:

- **60/60** `.SRL` base names also exist as a voice `.OGG` under
  `1\TRACKS\<lang>`; **54/60** also have an English subtitle `.TXT` under
  `1\TEXT\EN`.
- Across the 333 English subtitle names of length 8+, the first token is one of
  18 values and the second one of 26; `.SRL` uses a strict subset of both sets.
- The `.SRL` header's track count is **constant per first token** (table below),
  which identifies the first token as the *speaker/rig*, not the scene.
- Subtitle content is consistent per first-token group: the `KAN` group is one
  speaker (Kain), the `ARI` group is Ariel's narration, `MOB` is the Elder God,
  and the `SCL`/`RCL` groups are two of Raziel's vampire brothers.

| Target token | Tracks (channels) | Files | Speaker evidence (INFERRED) |
|---|---:|---:|---|
| `SCL` | 1 | 6 | a brother, single jaw-like channel |
| `RCL` | 7 | 6 | another brother |
| `ARI` | 17 | 9 | Ariel (narration lines) |
| `KAN` | 25 | 16 | Kain |
| `ACL` | 26 | 8 | scripted boss (one of the brothers) |
| `GRD` | 26 | 2 | a guard encounter |
| `MOB` | 26 | 1 | the Elder God |

The constant count per target is the strongest internal signal that `FFF` is the
animated entity. What the two tokens name *exactly* (speaker vs scene) is
INFERRED; the shared voice/subtitle scheme is CONFIRMED.

This is **not** the six render classes and **not** the behaviour-set families
(`hunter`, `vlgr*`, `skinner`, ...): only three second tokens (`ALU`, `HNT`,
`SKN`) coincide with an actor model name at all.

## 2. Decisive correlation with the behaviour trios

The hypothesis predicts: if `.SRL` files are the clip table the 8x3 trios index,
their trailing numbers must live in the actor's trio id space. Two independent
observations reject it.

**(a) The id spaces differ in kind.** `.SRL` numbers restart per `FFF`+`TTT`
pair and are sparse local line indices (e.g. `ACLALU` uses 01,03,04,05,12,20,21,
23), while the trios are a sparse, shared, global `0..66` action space. A local
per-dialogue counter is not a global clip index.

**(b) The numbers are dialogue line ids, proven by the voice/subtitle assets.**
All 60 `.SRL` have a matching `.OGG`, and the clip duration (`frames/fps`)
matches the Ogg stream length within a fraction of a second:

| Clip | `.SRL` duration | Voice `.OGG` duration | Delta |
|---|---:|---:|---:|
| `ACLALU01` | 2.000 s | 1.918 s | +0.082 |
| `ARIHNT02` | 6.560 s | 6.499 s | +0.061 |
| `KANKRT01` | 12.250 s | 12.199 s | +0.051 |
| `KANORC03` | 16.208 s | 16.170 s | +0.038 |
| `KANPIL03` | 36.042 s | 35.653 s | +0.389 |
| `SCLSKN26` | 2.458 s | 2.425 s | +0.033 |
| `RCLASH01` | 6.500 s | 6.474 s | +0.026 |
| `GRDTMB01` | 3.875 s | 3.814 s | +0.061 |
| `MOBMSC02` | 19.400 s | 19.340 s | +0.060 |
| `ARIPIL02` | 27.000 s | 26.963 s | +0.037 |
| `KANKRT03` | 15.417 s | 15.374 s | +0.043 |
| `KANORC01` | 59.083 s | 58.996 s | +0.087 |

For reference, the per-pair clip presence versus available subtitle lines and
the trio set of the only defensibly mapped actors:

| Pair | `.SRL` ids present | Lines available | Mapped actor | Trio ids | Ids inside trio space |
|---|---|---|---|---:|---|
| `ACLALU` | 01,03,04,05,12,20,21,23 | 13 | `aluka` | 19 | 6/8 (04,23 outside) |
| `ARIALU` | 20 | 1 | `aluka` | 19 | 1/1 |
| `ARIHNT` | 02,03 | 3 | `hunter` | 20 | 2/2 |
| `ARIKRT` | 01,02,20 | 5 | none | - | n/a |
| `ARIPIL` | 02,03,04,20,21 | 6 | none | - | n/a |
| `GRDTMB` | 01,02 | 2 | none | - | n/a |
| `KANKRT` | 01-05,21-23 | 8 | `kain` (via `KAN`) | 16 | 5/8 |
| `KANORC` | 01-08 | 8 | `kain` (via `KAN`) | 16 | 6/8 |
| `KANPIL` | 01-10 | 7 | `kain` (via `KAN`) | 16 | 8/10 |
| `MOBMSC` | 02 | 3 | none | - | n/a |
| `RCLASH` | 01-04,09,10 | 6 | none | - | n/a |
| `SCLSKN` | 01-04,26,27 | 7 | `skinner` | 18 | 4/6 (04,26 outside) |

The mixed "inside/outside" pattern is exactly what coincidental overlap between
two unrelated small-integer spaces produces. Combined with (a) and (b), the
clip-id reading is rejected. **Documented negative result, not a not-found-yet.**

## 3. Where the remaining clips live

The premise of the coverage question assumed `.SRL` was the trio table; it is
not, so there is no `.SRL` "coverage gap". The 60 `.SRL` files cover 60 dialogue
lines. The behaviour trios (`0..66` global) index a separate action/animation
list, and the only animation runtime the binary work found is a per-object
**spline** system: per-key frame deltas, 12-bit fixed-point samples, and two
key-record sizes (`docs/notes/anim-binding.md` A9). That representation does not
match `.SRL`'s dense `f32` 3x4 cells, and `.SRM` carries no clips
(`docs/notes/srm-bone-block.md`). So:

- **Not `.SRL`** (this note).
- **Not `.SRM`** (flat 128-slot joint pool, no key table).
- **Not `.SRA`** (record count fails the trio test; `sra-animation.md`).
- **Not obviously the object container** (meshes plus the actor block only).

The trio action list itself, and where the spline key tables are written from,
remain **UNKNOWN** - the same open question `anim-binding.md` left, now with one
more candidate excluded.

## 4. Container structure

```text
[ header: 5 little-endian u32 words ]
  magic        "SRL\x02"
  track count  (channels; 1, 7, 17, 25 or 26 in the corpus)
  version      (1 in all 60 files)
  frame count  (2,852 .. 1,701,620-byte files carry 28 .. 1,418 frames)
  frames/sec   (24, 25 or 30)
[ frame-major cells ]
  tracks * frames cells, each 48 bytes, last cell runs to EOF
```

| Part | Shape | Confidence |
|---|---|---|
| Magic | `SRL\x02` | CONFIRMED (60/60) |
| Header | exactly five `u32` words | CONFIRMED (60/60) |
| Length identity | `len == 20 + tracks*frames*48` | CONFIRMED (60/60) |
| Cell order | frame-major: one frame's `tracks` cells are contiguous | CONFIRMED (section 5) |
| Terminator | none; the last frame runs to EOF | CONFIRMED (arithmetic exact) |

The observed file-size range is **2,852 .. 1,701,620 bytes**. The `59,924 ..
268,340` figure in the task brief is a subset of the corpus, not the range.

## 5. Record shape

Each 48-byte cell is **twelve little-endian `f32` values**, read row-major as a
**3x4 affine**: a 3x3 linear block and a translation in the fourth column.

| Property | Observation | Confidence |
|---|---|---|
| Cell size | 48 bytes = 12 `f32` | CONFIRMED |
| Layout | 3x4 affine, row-major (3 rows of 3, then translation) | CONFIRMED |
| Ordering | frame-major: same track in the next frame is far closer than the adjacent cell | CONFIRMED |
| Linear block | rotation-like; `RCL`/`SCL` are exactly orthonormal, most other cells within `1e-2` | CONFIRMED for RCL/SCL / mostly for others |
| Translation | fourth column, magnitude ~1 .. 158 units (head/face scale, not body scale) | observed |
| Outliers | ~19% of cells exceed `5e-2` orthonormality error; rows stay near-orthogonal but with unequal lengths (scale/shear) | observed / decomposition UNKNOWN |
| Finite | zero NaN/Inf across all 290,064 cells | CONFIRMED |

Frame-major was proven by distance: for a sample, the mean 12-float distance
between a cell and the same track one frame later is `0.011 .. 1.011`, while the
distance to the adjacent cell in the same frame is `9.96 .. 99.6`. Mirror-paired
transforms on adjacent tracks (equal magnitude, opposite X) are the left/right
face or limb pairs within one frame.

The `RCL` and `SCL` clips are 100% exactly rigid 3x4 matrices (error `0.0` on
every cell); the larger rigs mix rigid and mildly scaled cells. Whether the
scaled cells are authored squash/stretch or a second encoding is **UNKNOWN**;
the cell can still be applied as a plain 3x4 affine without decomposing it.

## 6. Bone binding

There is no binding table in `.SRL`:

- The header ends after five words; there is no name list, no index array, and no
  per-cell joint id.
- Track identity is therefore positional: cell `i` within a frame belongs to
  channel `i`.
- The channel count is constant per `FFF` token (1, 7, 17, 25, 26) and does not
  equal any inspected model's bone count: loose `.SRM` humanoids have ~60-177
  plausible joint slots, archived `.PCM` actors ~24-66 bones.
- The binary string table contains an `OBJECT\%s.SRL` loader template next to
  the `.SRM` loader, so the engine does consume `.SRL` by asset name. The
  consumer and the mesh side it drives were **not traced** (out of this box).

So the file gives an ordered set of per-channel transforms, but not *which* bone
or landmark each channel is. Binding is **UNKNOWN**.

## 7. Keyframe encoding and time base

| Property | Observation | Confidence |
|---|---|---|
| Sample density | one cell per track per frame (baked, no key reduction) | CONFIRMED |
| Time base | frame index; sample rate = the header fps | CONFIRMED |
| Duration | `frames / fps` seconds | CONFIRMED (matches voice length) |
| Rotation | 3x4 affine linear block (not quaternion, not Euler) | CONFIRMED |
| Translation | fourth affine column | CONFIRMED |
| Interpolation | none stored; a consumer may blend adjacent frames | CONFIRMED absence |
| Frame-0 phase | assumed t=0 at frame 0; not independently pinned | UNKNOWN |

## 8. Can a pose be reconstructed from files alone?

**Split verdict.**

**YES - the per-track transform stream.** Every channel's 3x4 transform at any
sampled time is fully file-derived:

1. Parse the five-word header; assert `len == 20 + tracks*frames*48`.
2. `f = clamp(floor(t * fps), 0, frames-1)`; optional blend with `f+1` by the
   fractional part (linear is a viewer choice, not stored).
3. For channel `i`, read cell `(f * tracks + i)` as twelve `f32` = one 3x4
   affine.
4. Apply each affine to whatever the consumer knows channel `i` drives.

This yields the animated transform set for a dialogue line with no guessing.

**NO - a specific character mesh pose.** The minimal mesh algorithm needs
`track -> bone` correspondence plus a bind/origin convention, and neither is in
`.SRL`, `.SRM`, or the object container. The channel count is a speaker rig
size, not a model bone count, so the object's bone list cannot resolve it.
Missing exactly: **the channel-to-bone map (and whether channels are bones,
facial landmarks, or morph controls)**.

## Reader

`--srl-info <FILE.SRL>` prints `version`, `tracks`, `frames`, `fps`, the derived
duration and byte count, and restates the confirmed length identity. It
deliberately prints no bone names and claims no pose. Unit tests use synthetic
bytes only.

## What stays UNKNOWN

1. Exact meaning of the two 3-letter naming tokens (target vs context) beyond
   the INFERRED first-token-is-speaker reading.
2. Whether each cell is a local or global transform, and its bind/origin
   convention.
3. The cause of the non-rigid (scaled/sheared) minority of cells.
4. Which channels are bones, facial landmarks, or morph controls.
5. The engine consumer of `OBJECT\%s.SRL` and how it binds channels.
6. The global action list the behaviour trios index, and where spline keys are
   written from.
7. Frame-0 timing phase and the intended interpolation.

## Falsification notes

This verdict would be overturned by evidence of:

1. A per-file table (names, indices, or joint ids) mapping channels to bones.
2. A `.SRL` whose channel count equals a specific model's bone count and whose
   playback is authored as that model's skeletal animation.
3. The behaviour-trio ids resolving directly to `.SRL` file numbers across the
   corpus (they do not; the numbers are local dialogue-line counters).

The refuted claim from the task brief is the clip-id reading; the corrected
result is that `.SRL` is dialogue-line animation, and the trio action source
stays open.

## Method and legal

- Read-only passes: header census over all 60 files; full cell scan
  (290,064 cells) for finiteness and orthonormality; frame-major ordering test;
  duration cross-check against voice `.OGG` lengths and subtitle cues; token
  census over the English subtitle corpus.
- Format constants only (`SRL\x02`, 20-byte header, 48-byte cell, fps values);
  no addresses, no raw offsets, no game bytes, no install paths.
- The install was read-only; scratch was in the session temp directory and was
  not committed. No engine behaviour is asserted.

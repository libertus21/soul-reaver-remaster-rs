# animation-conclusion - body motion is procedural, and no authored clip exists (updated 2026-11-12)

This is the definitive answer to the body-animation question, written so a future
reader does not repeat the search. Two findings now sit together:

1. The remaster **does not author a per-bone keyframe clip** to decode. That
   negative stands on explicit carrier counts.
2. The body **does** move, and the motion is **computed at runtime**. A probe has
   now measured the per-slot rotation block changing while an actor walks, and
   the slot format is confirmed at **13 floats**.

The producer is named: the two sibling **orientation constructors** that write
each slot's rotation block. The arithmetic from their three input floats to the
rotation block is now also solved (`docs/notes/rotation-model.md`). What is still
unknown is the **source** of those three angle values. This note records the
correction instead of deleting the older claims silently.

Every number below is either reproduced by a shipped command (and listed with it)
or cited from the evidence note that established it. Nothing was written to the
game install; no game bytes, decompiled listings, file-position offsets or
absolute install paths appear here.

Evidence classes: **CONFIRMED** (observed bytes, a shipped-command witness, or a
runtime measurement), **PARTLY** (structure observed, one side open), **UNKNOWN**
(not established).

## Superseded claims (2026-11-12)

The earlier version of this note said the orientation story was the whole story.
Parts of that are now corrected by runtime evidence. The superseded statements
are kept here so the history is visible.

| Superseded claim (before 2026-11-12) | Corrected statement | Evidence |
|---|---|---|
| "The remaster does not author skeletal body animation" was read as "there is no body motion to explain" | **No authored clip exists, but body motion exists and is computed.** The distinction is data versus production. | Runtime dump: `VERDICT ANGLES_ANIMATE`, `changed_slots 3756` |
| "Bodies are oriented procedurally" as a vague hand-wave | The producer is **named**: the two sibling **orientation constructors**, called per active slot per frame from the pose builders. | `docs/notes/pose-writer-identity.md` |
| The pose slot format was a static inference | The runtime slot is **confirmed**: 13 `f32`, stride 52 bytes, row-major 3x3 at indices 0..8, scale field at 9, translation at 10..12. | `docs/notes/pose-format-confirmed.md` |
| "Body motion is root/actor motion plus procedural limb orientation" | The **per-slot rotation block itself animates**. A same-slot walking series sweeps a Z-axis rotation while standing still stays identity. | Four-sample series in `pose-format-confirmed.md` |
| The constructor inputs are radians consumed directly | **Corrected twice.** They are not radians, but they **are angles**: `4096` units per turn, composed `Rz(c)·Ry(b)·Rx(a)`. The "position or look-target" reading is also superseded. | `docs/notes/rotation-model.md` (336/336 rows) |

Everything below that describes carrier elimination still stands unchanged.

## The conclusion

> The remaster does not author skeletal body animation: there is no per-bone
> keyframe clip to decode. Body motion exists and is **procedural**: per active
> slot, per frame, the two named orientation constructors compute a rotation from
> three scalar inputs into a **confirmed 13-float slot**. The slot's rotation
> block animates on a walking actor. The remaining UNKNOWN is the **source** of
> the constructor's three angle values, not their arithmetic.
>
> Decoding further frames cannot produce walking legs, because the authored
> per-bone data does not exist. Reproducing the motion is a runtime/procedural
> problem, not a file-format hunt.

## Quick path (what to run to see the parts that are provable)

| Witness | Command | What it proves |
|---|---|---|
| Skeleton tree | `--bone-hierarchy --bigfile <DAT> --entry 66` | `raziel` has one root, depth 9, acyclic. |
| Corpus census | `--bone-hierarchy --bigfile <DAT>` | 199/394 (50.5%) validate under the strict `-1` root rule. |
| Hierarchical skinning | `--object-mesh --bigfile <DAT> --entry 66 --pose <FILE> --hierarchy --bounds` | A root rotation carries the whole model; a leaf rotation stays local. |
| Determinism intact | `--play --headless --bigfile <DAT> --unit under1 --ticks 60` | The body/skinning work changed no simulation output. |

All four are read-only and reproduce the numbers in this note.

## What was searched

The chain that a body pose would need, end to end:

1. a **skeletal bone table** with a parent relationship (so a pose can compose);
2. a **per-slot pose** (rotation plus translation) that the runtime could read;
3. an **authored animation clip** - per-bone keyframes over time - in some file
   family.

Steps 1 and 2 are found and verified. Step 3 does not exist. The runtime now also
confirms what step 2 looks like in memory (13 floats), which is the point the
static pass could not settle.

## What was found

### The descriptor record is the model's own 24-byte bone table

The per-slot descriptor array is the model's **24-byte bone table**, located
generically through the bone-table pointer in the model header: **394/394**
single-model object containers. Each record carries a vertex range, a local
translation and a parent word. See `docs/notes/slot-descriptor-parse.md` and
`docs/notes/aim-stream-bounds.md`.

### `+18` is a signed parent index - a real hierarchy exists

The `+18` word is a **signed `i16` parent bone index** (`-1` = root). `raziel`
(66 bones) forms **one root, depth 9, 0 topological violations, 0 cycles**:

```text
hierarchy object=raziel entry=66 bones=66 roots=1 root=0 depth=9 topological=true valid
```

**This corrects the older claim that archived object models have no bone
hierarchy.** That claim is true only for loose `.SRM`, whose joint pool is a flat
128 slots; the archived `.PCM` object models carry a real tree. See
`docs/notes/bone-hierarchy.md`.

Corpus census over the 394 skeletal containers:

```text
hierarchy census objects=394 located=394 unlocated=0 valid=199 invalid=195 agreement=50.5% empty=0 no_root=194 multiple_roots=1 parent_out_of_range=0 cycles=0
```

- **199/394 (50.5%)** validate under the strict `-1` root rule.
- **194** fail as `no_root`: one-bone props store a **self-parent `0`** instead of
  `-1`. That is a second convention and stays **UNKNOWN**.
- **1** genuine multi-root model: `matsrevr` (7 bones).
- **0** out-of-range parents and **0** cycles anywhere.

### Composition and skinning work on real data

The hierarchy composes as `world[child] = world[parent] * local[child]`,
root-first and cycle-safe. Skinning applies the composed world matrix to each
bone's vertex range. A 30-degree Z rotation on the root versus a leaf, with every
other bone at rest, gives a falsifiable witness:

| Model | Bones | Rest bbox `min` / `max` | Root rotation | Leaf rotation |
|---|---|---:|---|---|
| `raziel` | 66 | `(-90,-86,3)` / `(90,284,509)` | moves **299/299** vertices | bone 9 moves **8** |
| `skinner` | 35 | `(-98,-79,1)` / `(100,35,569)` | moves **173/173** vertices | bone 7 moves **4** |

A root rotation carries every descendant frame and moves the whole model; a leaf
rotation moves only its own small range. This is the whole skeleton/skinning
achievement, and it is verified, not inferred.

### The runtime slot is 13 floats, and it animates

The runtime probe hooked the two orientation constructors and dumped complete
slots. The layout is confirmed:

```text
[0..8]   3x3 rotation, row-major
[9]      scale field (observed 0.0 throughout)
[10..12] translation, world units
```

with a measured **stride of 52 bytes** (exactly 13 `f32`), exact for **99.26%**
of writes. On the same slot, while walking, the rotation's upper-left 2x2 block
sweeps `(cos, sin; -sin, cos)` about the Z axis over four consecutive samples,
and the translation traverses the level. Standing still gives the identity
rotation and a constant translation. The engine's `PoseMatrix` already models
this exact layout, so the runtime **confirms** the engine rather than correcting
it. Full numbers: `docs/notes/pose-format-confirmed.md`.

Because the observed scale field is `0.0` throughout, it cannot be a
multiplicative scale; the engine's decision to keep it as an inspection marker
and apply unit scale is validated.

### The producer is named

Both writers reached by the live write-watch are the **two sibling orientation
constructors**: each writes one slot's nine-float rotation block as an identity
followed by three in-place axis rotations, and each derives every value from
three scalar inputs. They compute rather than copy. They are called from the
per-object pose builders, per active slot, per frame, into the object's own
per-frame pose array. See `docs/notes/pose-writer-identity.md`.

### The rotation construction is solved

An atomic capture paired each call's three input floats with the 3x3 block that
same call returned, then fitted a closed family of 192 rotation models. The fit
is `UNIQUE` on **336/336** usable rows:

```text
M = Rz(c · K) · Ry(b · K) · Rx(a · K)      K = 2π / 4096
```

The three floats are **angles**, `4096` units per turn, composed **X first,
then Y, then Z** in row-major form. The earlier negative-sign reading was an
extraction artifact and is marked superseded; the correct pairing gives a
positive sign. Evidence, the equivalence class and the correction are in
`docs/notes/rotation-model.md`.

The producer chain is therefore complete **up to the angle source**: the call,
the unit, the order and the output slot are known. How the three angles are
derived (aim/face mode, object transform, world target) is still partly open.

### The authored data is tiny and aim-only

The static animation data chain was parsed end to end:

- The descriptor `+20` word is a `u16` offset into one shared aim region. An
  active slot's sub-slice is `[offset, offset + 24 + frame_count * 20)`: a 24-byte
  header plus `frame_count` 20-byte frames. **64 of 65** finite gaps fit that rule
  exactly (`bellow` is the one reported outlier).
- A frame is **ten `i16` words**, Q12 fixed point (`4096 == 1.0`); observed
  magnitudes are **0.005-0.055** of one. The ten words behave alike: no time lane,
  no bounded weight lane.
- On `raziel`, only **3 slots** carry any data, with frame counts **2/1/1**;
  **63 of 66 slots have none**. Two same-slot frames change every word, so they
  read as independent state samples, not a temporal clip.

That is a set of small aim-correction poses, not a body animation. It stands:
**the walking motion is not in these frames; it is computed by the procedural
path.**

## What was eliminated (with the numbers, so nobody re-does it)

Every loose asset family and the archive object containers were swept. Each row
is a hard negative with its count:

| Carrier | Scope | Result |
|---|---|---|
| `.SRA` | 227 files | **0** orthonormal `f32` 3x3 or 3x4 windows in **3,744,114** / **3,717,786** positions. The 52-byte slot-pose layout is falsified **as an authored-file layout**; slot counts are model bone counts, not `.SRA` fields. |
| `.SRA` + `.SRM` (i16/Q12) | 227 + 349 files | **1** isolated Q12 3x3 hit in **30,478,374** windows and **0** runs; **0** Q12 3x4 hits; Q15 hits are below their matched baseline. The i16 element-type blind spot is closed (`i16-animation-scan.md`). |
| `.PCM` object containers | 394 | **No** flat `bones x 32` descriptor array and no 4-float track table. The two smallest decidable models (`eggfx`, `hint`) are fully byte-accounted with no such block. |
| `.SRM` | 349 files, 25.4 MB | **26,647,603** 3x3 windows with **0** below 0.05; **26,643,415** 3x4 windows with **147** below 0.05, all inside the known HD-matrix block; `run52 = 0` in **349/349** files. It is mesh + 128-slot pool + optional HD matrices only. |
| `.SRL` | 60 files | Facial/HD only - 3x4 `f32` tracks whose track counts are `{1,7,17,25,26}`. No channel-to-bone table. |
| Animation extension | whole tree | **None exists.** No animation file extension is shipped. |

The static loader trace independently agrees: the animation record's data is a
procedural-orientation problem, not a hidden keyframe clip
(`docs/notes/animation-loader-trace.md`).

Note the one correction of wording: the 52-byte layout is now **confirmed as the
runtime slot**, so the `.SRA` negative means it is not an authored *file* layout,
not that the layout is wrong. The runtime measurement and the file negative are
consistent.

## What the finding means

- The observable "body motion" is **procedural per-slot rotation** plus root and
  actor motion, not a skin clip.
- The producer of that rotation is named: the two orientation constructors, and
  their angle-to-matrix construction is solved (`docs/notes/rotation-model.md`).
- A faithful body replay needs the **source of the three angles**, not more frame
  decoding. The authored data stops at a few aim corrections.
- Do **not** start another keyframe hunt in `.SRA`, `.SRM`, `.PCM` or `.SRL`.
  Those are closed with the counts above.

## What remains UNKNOWN (explicit)

1. **The source of the constructor's three angles.** The arithmetic is solved:
   the three floats are angles in `4096`-units-per-turn form, composed
   `Rz(c)·Ry(b)·Rx(a)` (`docs/notes/rotation-model.md`). What produced those
   three angles is only partly established.
2. **How the three angles are derived.** The caller-side conversion between the
   object aim triple, the descriptor triple, the sampled channel frames and the
   constructor angles is not fully decoded. The earlier probe measurements
   (`abs_max = 5951`, `min_step = 0.0224`, `quant_frac = 0`) remain valid; the
   "position/look-target" reading is superseded.
3. **The mode bits.** The four named descriptor words `+4`, `+8`, `+12`, `+16`
   could not be partitioned into an authored mode bit; `+4` is always zero and the
   others are broad per-bone fields. The mode that selects procedural aim/face
   orientation is not located in the file form.
4. **The record length of the remaining 8 header bytes.** The 16-byte track
   header has a confirmed `frame_count`, a `frame_offset` (always `aim_offset +
   24`) and an 8-byte tail whose role is **UNKNOWN**.
5. **The one-bone `parent = 0` convention.** 194 one-bone props store a
   self-parent `0` rather than `-1`; whether that is a distinct encoding or a
   defect is **UNKNOWN**.
6. **`matsrevr`'s second root.** The only multi-bone model with two `-1` roots;
   the second root's meaning is **UNKNOWN** (it is reported, never repaired).

## Why a future reader should stop searching files

The body-animation question is answered at two levels. The **file side** is
closed: no authored clip exists, the carriers are eliminated with counts, and the
skeleton, hierarchy and skinning are real and verified. The **runtime side** is
characterised: the slot format is confirmed (13 floats), the producer is named
(orientation constructors), the rotation construction is solved
(`docs/notes/rotation-model.md`), and the rotation block demonstrably animates.
What is left is **procedural**: the source of the three angles. That is a
runtime/math problem, not a file-format hunt.

Do not re-open `.SRA`, `.SRM`, `.PCM` or `.SRL` looking for a walking clip.

## Related notes

- `docs/notes/rotation-model.md` - the solved angle-to-matrix construction, its
  336/336 evidence and the corrected sign.
- `docs/notes/pose-format-confirmed.md` - the 13-float layout and the walking
  series.
- `docs/notes/pose-writer-identity.md` - the two orientation constructors and
  their callers.
- `docs/changes/runtime-capture/angle-input-plan.md` - the probe design.
- `docs/notes/bone-hierarchy.md` - the `+18` tree, composition and skinning.
- `docs/notes/slot-descriptor-parse.md` - the descriptor array is the 24-byte bone
  table.
- `docs/notes/aim-stream-bounds.md` - the aim sub-slice bounds and the Q12 unit.
- `docs/notes/frame-mode-composition.md` - the ten-word frame, the unresolved mode
  bits and the confirmed identity composition.
- `docs/notes/animation-loader-trace.md` - the static carrier verdict.
- `docs/notes/sra-slot-pose.md`, `docs/notes/pcm-animation-record.md`,
  `docs/notes/procedural-slot-pose-gate.md`, `docs/notes/srm-animation-scan.md`,
  `docs/notes/srl-animation.md` - the eliminated carriers in detail.
- `docs/notes/i16-animation-scan.md` - the i16/Q12 element-type rescan and its
  matched-histogram baselines.

## Legal

Read-only. No game file was written, no game bytes were copied into the
repository, and no disassembly, address, file-position offset or install path
appears here. Only structure, field roles, format constants and corpus counts are
recorded.

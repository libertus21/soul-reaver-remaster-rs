# animation-conclusion - the remaster does not author skeletal body animation (2026-11-10)

This is the definitive answer to the body-animation question, written so a future
reader does not repeat the search. The remaster **does not author skeletal body
animation**: there is no per-bone keyframe clip to decode. Bodies are oriented
procedurally, and the only authored pose data is a handful of aim corrections.
Every number below is either reproduced by a shipped command (and listed with it)
or cited from the evidence note that established it. Nothing was written to the
game install; no game bytes, decompiled listings, file-position offsets or
absolute install paths appear here.

Evidence classes per claim: **CONFIRMED** (observed bytes or a shipped-command
witness), **PARTLY** (structure observed, one side open), **UNKNOWN** (not
established).

## The conclusion

> The remaster does not author skeletal body animation. Bodies are oriented
> procedurally (mode bits + authored aim corrections + object transform + world
> target). The authored aim data is 1-2 correction poses on 3 of 66 slots.
> Decoding further frames cannot produce walking legs, because the data does not
> exist.

That sentence is the payoff of the whole investigation. The sections below show
how it was reached, what was ruled out with counts, and what genuinely remains
open.

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

Steps 1 and 2 were found and verified. Step 3 does not exist.

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
|---|---:|---|---|---|
| `raziel` | 66 | `(-90,-86,3)` / `(90,284,509)` | moves **299/299** vertices | bone 9 moves **8** |
| `skinner` | 35 | `(-98,-79,1)` / `(100,35,569)` | moves **173/173** vertices | bone 7 moves **4** |

A root rotation carries every descendant frame and moves the whole model; a leaf
rotation moves only its own small range. This is the whole skeleton/skinning
achievement, and it is verified, not inferred.

### The authored animation data is tiny and aim-only

The animation data chain was parsed end to end:

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

That is a set of small aim-correction poses, not a body animation.

## What was eliminated (with the numbers, so nobody re-does it)

Every loose asset family and the archive object containers were swept. Each row
is a hard negative with its count:

| Carrier | Scope | Result |
|---|---|---|
| `.SRA` | 227 files | **0** orthonormal 3x3 or 3x4 windows in **3,744,114** / **3,717,786** positions. The 52-byte slot-pose layout is falsified; slot counts are model bone counts, not `.SRA` fields. |
| `.PCM` object containers | 394 | **No** flat `bones x 32` descriptor array and no 4-float track table. The two smallest decidable models (`eggfx`, `hint`) are fully byte-accounted with no such block. |
| `.SRM` | 349 files, 25.4 MB | **26,647,603** 3x3 windows with **0** below 0.05; **26,643,415** 3x4 windows with **147** below 0.05, all inside the known HD-matrix block; `run52 = 0` in **349/349** files. It is mesh + 128-slot pool + optional HD matrices only. |
| `.SRL` | 60 files | Facial/HD only - 3x4 `f32` tracks whose track counts are `{1,7,17,25,26}`. No channel-to-bone table. |
| Animation extension | whole tree | **None exists.** No animation file extension is shipped. |

The static loader trace independently agrees: the animation record's data is a
procedural-orientation problem, not a hidden keyframe clip
(`docs/notes/animation-loader-trace.md`).

## What the finding means

- The observable "body motion" in play is **root/actor motion plus procedural
  limb orientation**, not a skin clip.
- A faithful body replay needs the **mode bits, the object transform and the
  world target** - not more frame decoding. The authored data simply stops at a
  few aim corrections.
- Do **not** start another keyframe hunt in `.SRA`, `.SRM`, `.PCM` or `.SRL`.
  Those are closed with the counts above.

## What remains UNKNOWN (explicit)

1. **The mode bits.** The four named descriptor words `+4`, `+8`, `+12`, `+16`
   could not be partitioned into an authored mode bit; `+4` is always zero and the
   others are broad per-bone fields with no separating low-bit value. The mode
   that selects procedural aim/face orientation is not located in the file form.
2. **The aim-to-rotation composition.** The identity path is confirmed (static, or
   a zero aim triplet, yields identity). The object transform is the best-supported
   rotation base (**INFERRED**); the local aim term and the world-target term are
   the **UNKNOWN** procedure, deliberately not invented.
3. **The record length of the remaining 8 header bytes.** The 16-byte track header
   has a confirmed `frame_count`, a `frame_offset` (always `aim_offset + 24`) and
   an 8-byte tail whose role is **UNKNOWN**.
4. **The one-bone `parent = 0` convention.** 194 one-bone props store a self-parent
   `0` rather than `-1`; whether that is a distinct encoding or a defect is
   **UNKNOWN**.
5. **`matsrevr`'s second root.** The only multi-bone model with two `-1` roots; the
   second root's meaning is **UNKNOWN** (it is reported, never repaired).

## Why a future reader should stop searching

The body-animation question is answered: the data does not exist to decode. The
skeleton, the hierarchy and the skinning are real and verified; the authored
animation is three aim poses on `raziel` and nothing else on the corpus. The only
remaining work is procedural (mode bits and composition), and that is a
runtime/math problem, not a file-format hunt.

## Related notes

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

## Legal

Read-only. No game process was launched or instrumented for this consolidation.
No game file was written, no game bytes were copied into the repository, and no
disassembly, address, file-position offset or install path appears here. Only
structure, field roles, format constants and corpus counts are recorded.

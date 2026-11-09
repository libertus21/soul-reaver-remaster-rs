# spline-animation - where the spline key tables come from

Read-only static pass on the shipped SR1 module plus a read-only census of the
shipped object containers. Format and control-flow facts only: field roles,
record shapes, stride rules, small counts and short asset names. No binary
addresses, no file offsets, no disassembly listings, no raw game bytes, no
absolute install paths. Scratch work lived in the session temp directory and was
removed; the module and the archive were opened read-only.

This note closes the open question left by `docs/notes/anim-binding.md`
("where the spline keys come from") and continues `sra-animation.md`,
`srl-animation.md` and `actor-transition-table.md`. It does not change production
code and does not move a golden.

## Verdict first

| # | Question | Answer | Class |
|---|---|---|---|
| 1 | Who populates the spline key tables? | A load-time builder allocates the spline set and expands **file-side channel descriptors** into runtime key arrays. The source is the SR1 archived **object container** (a `BIGFILE.DAT` object entry), not `.SRA`, not `.SRL`, not `.SRM`, and not a static table in the DLL. | CONFIRMED (code + data) |
| 1b | Which object field carries the actor spline set? | Structurally identified in the actor-block animation records, but **not pinned to one named file field** in this pass. The sub-model carrier (`+0x2C`) is non-zero for only 1 of 397 containers. | PARTLY / UNKNOWN |
| 2 | What are the `0x1c` and `0x30` strides? | `0x1c` = rotation keys (quaternion, spherical interpolation); `0x30` = position keys (Hermite with in/out tangents). The per-spline flag byte at `+0x0A` selects them: `1` -> `0x1c`, otherwise `0x30`. | CONFIRMED |
| 3 | Which instance fields does the sampler write? | Three object transform groups: position, rotation and scale, each as three shorts plus three float mirrors. Object transform only; no joint or skin deformation. | CONFIRMED |
| 4 | Do the behaviour trios index this spline space? | The trios expand into animation-set records read per channel, but **no `(name, id) -> spline` resolution** was found. | UNKNOWN |
| 5 | Can a body pose be reconstructed from files? | **Object transform: YES in principle** (minimal algorithm below), conditional on the spline descriptor being present. **Skeletal body pose: NO** - the spline system has no joint binding. | CONFIRMED / conditional |

The decisive correction to the prior notes is Q1: the keys are **converted at
load** from a compact file representation, so looking for a runtime `0x1c/0x30`
key array in the files was the wrong search. The file stores channel descriptors
and compact source keys; the engine expands them.

## Quick path

1. Read section 1 for the population path (builder + source).
2. Read section 2 for the exact key record shape and the stride rule.
3. Read section 3 for what the sampler writes.
4. Read section 4 for the clip-identity status.
5. Read section 5 for the YES/NO verdict and the minimal algorithm.
6. Treat section 6 as the authoritative UNKNOWN list.

## 1. Population path

### 1.1 The runtime spline set

The spline runtime is a **four-slot set** of spline-definition pointers:

```text
spline set (runtime, 0x80 bytes)
  +0x00  pointer to spline def 0   (position)
  +0x08  pointer to spline def 1   (rotation)
  +0x10  pointer to spline def 2   (scale)
  +0x18  pointer to spline def 3   (raw / auxiliary)
  +0x20 .. +0x78                   twelve further qwords copied from the file descriptor
```

Each spline definition (runtime, 0x10 bytes) has:

| Field | Role |
|---|---|
| `+0x00` | pointer to the runtime key array |
| `+0x08` | `i16` key count (`numkeys`) |
| `+0x0A` | stride flag: `1` selects `0x1c`, otherwise `0x30` |
| `+0x0B` | behaviour flags (wrap / clamp / loop bits) |
| `+0x20` | runtime cursor state (current key, accumulator) |

`anim-binding.md` A9 already established this shape from the reader side. This
pass locates the writer.

### 1.2 The builder

One load-time function builds the whole set. It allocates the 0x80-byte set,
then for each of the four channels:

1. Reads a **channel descriptor** from the loaded container.
2. Allocates a 0x10-byte spline definition and copies `numkeys`, the stride flag
   and the behaviour flags from the descriptor.
3. Allocates a key array sized `numkeys * stride` and expands the compact source
   keys into the runtime key records.
4. Stores the definition pointer in the set.

The channel descriptor (file side) is:

| Field | Role |
|---|---|
| `+0x00` | offset to the source key array (container-relative) |
| `+0x04` | `u16` key count |
| `+0x06` | `u8` stride flag (`1` = rotation, `0` = position) |
| `+0x07` | `u8` behaviour flags |

Channels 0 and 2 always expand as position keys; channel 1 always expands as
rotation keys; channel 3 is copied raw as a `0x30` key array. The descriptor is
reached from an **object animation record**, not from a global table.

### 1.3 The asset family

The source is the **archived SR1 object container**: the `BIGFILE.DAT` object
entry whose payload carries the relocation directory, object header, model
array and animation records. Two file-side carriers were located:

- **Sub-model record `+0x2C`** -> spline-set descriptor (position, rotation,
  scale, raw). This is a confirmed carrier: one shipped container (`font`)
  carries a non-zero descriptor with a three-key rotation spline.
- **Actor-block animation records** -> spline set, via the actor-block pointer
  table. The runtime structure and the builder are confirmed; the exact file
  field that survives to the actor spawn record was not pinned in this pass.

What is **not** the source (re-confirmed, not re-derived):

- `.SRA`: the `0x32` post-processor builds a resource table and hands the
  result to the object-slot callback, which is an empty `RET 0`. The table is
  discarded. This matches `anim-binding.md` A6/A11.
- `.SRL`: dialogue/facial 3x4 `f32` cells, no spline keys (`srl-animation.md`).
- `.SRM`: mesh + joint pool + weights only; no key table (`srm.rs`,
  `srm-bone-block.md`).
- **Static image**: a scan of the initialized `.rdata`/`.data` for a four-pointer
  set whose targets carry `numkeys`/stride-flag/key-pointer found no such set.
  The spline sets are built at load, not compiled in.

## 2. Key record shape and the stride rule

### 2.1 Source (file) keys

Position channel, source stride `0x20` bytes:

| Order | Width | Meaning |
|---|---|---|
| 1 | `i16` | frame delta |
| 2 | 3 x `i16` | position x, y, z (12-bit fixed point) |
| 3 | 3 x `i32` | outgoing tangent x, y, z |
| 4 | 3 x `i32` | incoming tangent x, y, z |

Rotation channel, source stride `0x0A` bytes:

| Order | Width | Meaning |
|---|---|---|
| 1 | `i16` | frame delta |
| 2 | 4 x `i16` | quaternion x, y, z, w (12-bit fixed point) |

Raw channel, source stride `0x30` bytes: copied verbatim.

The fixed-point scale is **4096 = 1.0**; the reader shifts right by 12
(`>>12`). A quaternion component of `4096` is `1.0` and `-4096` is `-1.0`.

### 2.2 Runtime keys

Position channel, runtime stride `0x30` bytes:

| Order | Width | Meaning |
|---|---|---|
| 1 | `i16` | frame delta |
| 2 | 2 bytes | padding |
| 3 | 3 x `i16` | position x, y, z |
| 4 | 3 x `f32` | float mirror of the position |
| 5 | 3 x `i32` | outgoing tangent |
| 6 | 3 x `i32` | incoming tangent |

Rotation channel, runtime stride `0x1c` bytes:

| Order | Width | Meaning |
|---|---|---|
| 1 | `i16` | frame delta |
| 2 | 4 x `i16` | quaternion x, y, z, w |
| 3 | 4 x `f32` | float mirror of the quaternion |

### 2.3 The stride rule

The per-spline flag byte at definition `+0x0A` selects the runtime record:

- flag `1` -> stride `0x1c`, quaternion keys, spherical interpolation.
- flag `0` -> stride `0x30`, position keys, cubic Hermite with stored tangents.

The flag is copied verbatim from the file channel descriptor `+0x06`, so the
file already decides which channel is position and which is rotation. In the
sampled corpus the rotation channel carries flag `1` and the position channels
carry flag `0`.

### 2.4 Sampling and interpolation

The sampler walks keys by **accumulating frame deltas**, not by absolute frame
numbers:

1. Read the current key's delta.
2. While the time accumulator (in 1/4096 frame units) is at least the delta,
   advance the cursor and subtract the delta.
3. At the end of the array, either clamp to the last key or wrap to the first,
   depending on the behaviour flag.

Interpolation is chosen by the same stride flag: Hermite for `0x30` position
keys, quaternion spherical interpolation for `0x1c` rotation keys. The
interpolation is in 12-bit fixed point throughout.

## 3. Scope of the transform

The evaluator writes three transform groups into the object instance:

| Group | Short fields | Float mirrors | Role |
|---|---|---|---|
| 0 | three shorts | three floats | position |
| 1 | three shorts | three floats | rotation (quaternion / matrix) |
| 2 | three shorts | three floats | scale (initialised to 4096 = 1.0) |

The same pass mirrors two of the groups into a "previous sample" set used by the
render/update path. This is **object transform animation**: the sampler writes
the instance's own transform, not the 128-slot joint pool and not per-vertex
skin deformation. This confirms `anim-binding.md` A10 from the writer side.

Consequence: the spline system can move, turn and scale an object. It cannot
pose a character's bones.

## 4. Clip identity

The actor block's 8x3 trios are expanded at load into animation-set records and
read one clip byte per animation channel (`actor-transition-table.md` sections 2
and 5). The spline set is attached to the object/animation record, not indexed
by the trio ids.

No `(name, id) -> spline` resolution was found: the spline set is selected by
the object/animation record that owns it, and the trios select clip bytes inside
the animation-set records. Whether a trio clip byte eventually selects one of
the four spline slots is **UNKNOWN**. The two lookups are structurally different
and no join between them was observed.

## 5. Practical verdict

### 5.1 Object transform at time t - YES in principle

If the object's animation record carries a spline-set descriptor, the transform
is fully file-derived:

1. Read the four channel descriptors (key-array offset, `numkeys`, stride flag).
2. For each channel, read the compact source keys:
   - rotation: `i16` delta + 4 x `i16` quaternion;
   - position: `i16` delta + 3 x `i16` position + 3 x `i32` out-tangent +
     3 x `i32` in-tangent.
3. `t` in frames; accumulate deltas until the cursor is the key whose interval
   contains `t`.
4. Evaluate:
   - position: cubic Hermite with the stored tangents;
   - rotation: spherical interpolation of the two quaternions.
5. Divide by 4096 for the fixed-point scale and write the object transform
   groups (position, rotation, scale).

This is a real algorithm with no guessing, and it is validated end-to-end on
one shipped container (`font`, three rotation keys, deltas 75/75/0, quaternions
`(0,0,0,-4096)`, `(0,0,-4096,0)`, `(0,0,0,4096)`).

**Caveat:** the confirmed file carrier (`sub-model +0x2C`) is populated for only
1 of 397 object containers. For actors, the spline set is reached through the
actor-block animation records, whose exact file field was not pinned here.
Until that field is pinned, "YES in principle" is not the same as "YES for every
actor".

### 5.2 Skeletal body pose - NO

The spline system has no joint or bone reference:

- It writes three object transform groups, not 128 joint transforms.
- `.SRM` supplies a flat 128-slot joint pool with no hierarchy and no clip
  table.
- `.SRL` supplies per-channel 3x4 `f32` transforms for dialogue, with no
  channel-to-bone map.

A full-body pose needs a channel-to-joint binding that does not exist on any
confirmed path. This is a documented no-go, not a not-found-yet.

## 6. Explicit UNKNOWN list

1. The exact file field that carries the **actor** spline-set descriptor. The
   sub-model `+0x2C` carrier is confirmed but rare; the actor-block animation
   record path is structurally identified but not mapped to one named field.
2. Whether the trio clip bytes select a spline slot, and how a `(name, id)` pair
   resolves to a spline set.
3. The meaning of the twelve further qwords copied into the spline set at
   `+0x20..+0x78` (behaviour parameters vs animation parameters).
4. The behaviour-flag bit semantics at definition `+0x0B` (wrap vs clamp vs
   loop) beyond the observed "end of array" branch.
5. The intended interpolation of the scale channel (the evaluator writes it, but
   the scale-key source was not sampled).
6. Whether the runtime cursor state lives in the definition (`+0x20`) or in the
   instance (`+0x2F8`); the code has both paths and the selection condition is
   not fully traced.
7. Whether any non-actor object family (doors, platforms) carries actor-style
   spline records; the corpus census only proved the sub-model carrier for one
   container.

## 7. Falsification notes

| Claim | What would refute it | Tested? |
|---|---|---|
| The spline set is built at load from file channel descriptors | a four-pointer static set in `.rdata`/`.data`, or a builder that synthesises keys with no file input | Static-image scan found none; builder reads file offsets -> CONFIRMED |
| Source position keys are `0x20` bytes | a source position array that only fits `0x30` or another stride | Builder arithmetic and one rotation sample -> CONFIRMED for rotation; position inferred from the same builder |
| Source rotation keys are `0x0A` bytes | a rotation key array that does not fit `numkeys * 0x0A` | `font` rotation channel fits exactly -> CONFIRMED |
| `.SRA` is not the source | a reader of the `.SRA`-derived resource table | The `0x32` builder result is handed to `RET 0` -> CONFIRMED |
| The spline writes object transforms only | a joint or bone index in the sampler write set | Sampler writes three transform groups, no joint index -> CONFIRMED |
| The trios index splines | a trio id resolving directly to a spline set | Not observed -> UNKNOWN, not refuted |

## 8. What would unblock this

Ordered by cost:

1. **Static data-flow of the actor-block animation record.** Follow the
   actor-block pointer-table record through the runtime block to the spawn
   record `+0x40`, and name the exact file field. This is the shortest route to
   "YES for every actor".
2. **Corpus census of actor-block animation records.** Count non-empty spline
   descriptors across the 21 actor containers; if none are non-empty, the actor
   spline set may be built from a global table instead.
3. **Controlled runtime capture (read-only).** With one moving actor, break on
   the spline-set builder and record the source container offset, `numkeys` and
   stride flag. This settles both the actor field and the clip identity.
4. **Scale-channel sampling.** Capture one object whose spline set has a
   non-empty slot 2 and confirm the scale key encoding.

## 9. Method and legal

- Static pass on the shipped SR1 module with the existing headless Ghidra
  workflow; the binary was opened read-only and the project lived outside every
  repo and game directory.
- Read-only byte probe of the `BIGFILE.DAT` index and the object containers:
  the sub-model table, the `+0x2C` spline-set descriptor, the four channel
  descriptors, and the source key arrays.
- One static-image scan for a compiled-in four-pointer spline set.
- Format constants only (field widths, strides, fixed-point scale 4096, tag
  values already documented elsewhere); no binary addresses, no file offsets,
  no disassembly, no raw game bytes, no install paths.
- No game process was launched or instrumented. No production code changed; no
  golden moved.

# pose-format-confirmed - the runtime slot is 13 floats, and the rotation block animates (2026-11-12)

A read-only Frida probe hooked the two orientation constructors and dumped full
runtime pose slots. The dump **confirms the 13-float slot format** the engine
already models and shows that the **rotation block itself changes while an actor
walks**. This note is the evidence-first record of that confirmation. It does not
claim the animation system is fully solved: the producer is named and the output
format is confirmed, while the constructor's three input floats were an explicit
UNKNOWN *at this slice*. A later atomic fit solved their construction
(`docs/notes/rotation-model.md`); the source of the angles remains partly open.

Everything here is behaviour and format evidence. No addresses, module-relative
numbers, disassembly listings, raw game bytes or absolute install paths appear.

Evidence class per claim: **CONFIRMED** (runtime measurement, or two independent
observations agree), **PARTLY KNOWN** (structure pinned on one side), **UNKNOWN**
(explicitly not established).

## Quick path

1. Read "The confirmed layout" - the 13-float table and the stride-52 evidence.
2. Read "The animation proof" - the four-sample walking series.
3. Read "The engine already matches" - the struct the runtime now confirms.
4. Read "The constructor's three input floats" - measured first, then solved in
   construction by `docs/notes/rotation-model.md`.
5. Read "Falsification ledger" before quoting anything.

## The confirmed layout

One runtime slot is **13 consecutive `f32` values, a 52-byte stride**.

| Float index | Bytes in slot | Role | Evidence |
|---|---|---|---|
| 0, 1, 2 | 0, 4, 8 | rotation row 0 | CONFIRMED (4/4 readable rotations) |
| 3, 4, 5 | 12, 16, 20 | rotation row 1 | CONFIRMED |
| 6, 7, 8 | 24, 28, 32 | rotation row 2 | CONFIRMED |
| 9 | 36 | scale field | CONFIRMED position; meaning UNKNOWN |
| 10, 11, 12 | 40, 44, 48 | translation, world units | CONFIRMED |

The rotation block is **row-major 3x3 at float indices 0 through 8**.

Stride evidence from the probe:

```text
dest_stride              52
dest_stride_exact_frac   0.9926
```

Fifty-two bytes is exactly thirteen `f32` values. The probe measured the
destination stride directly and found it exact for **99.26%** of writes. This is
the decisive change from the earlier static guesses: the stride is now measured,
not inferred, and it matches the 13-float layout the engine had already chosen.

The probe also read four complete rotations without a single failure:

```text
rot_ok            4/4
rot_read_fails    0
calib             38627/38627   rate=1.00
```

`calib` means the probe's address arithmetic reproduced its own calibration
target on every one of its 38,627 samples, so the measured stride and offsets are
trustworthy rather than a lucky read.

## The animation proof

The probe held the **same slot** across a walking sequence and dumped four
consecutive samples. The 2x2 upper-left block of the rotation reads:

| Sample | rotation upper-left (cos, sin; -sin, cos) | translation (x, y, z) |
|---|---|---|
| n = 37 | `(0.9279, 0.3727, -0.3727, 0.9279)` | `(-226.1, -1397.2, -4828)` |
| n = 38 | `(0.1072, 0.9942, -0.9942, 0.1072)` | `(-445.0, -1569.3, -4828)` |
| n = 39 | `(-0.6006, 0.7995, -0.7995, -0.6006)` | `(-816.9, -1376.0, -4828)` |
| n = 40 | `(-0.9519, 0.3064, -0.3064, -0.9519)` | `(-979.4, -1061.9, -4828)` |

Each block is a clean `(cos, sin; -sin, cos)` pair: this is a **rotation about
the Z axis**. The angle advances smoothly and monotonically between samples
(order of half a radian per sample at roughly 4 Hz). The translation traverses
the level in `x` and `y` while `z` stays constant at `-4828`, consistent with a
Z-up world.

The control is the earlier standing-still dump: the rotation there is the
**identity** and the translation is **constant**. Same slot, same probe, same
layout - so the change is body motion, not measurement noise.

Corpus-level corroboration from the same probe:

```text
VERDICT        ANGLES_ANIMATE
changed_slots  3756
slots=136  dumps=79  distinct_dump_hashes=75
key dest_pointer stable_pairs=74 unstable_pairs=4 sxp=95%
```

Seventy-five of seventy-nine dumps hash differently, and 3,756 slots changed
across the session. The pose is not a static rest transform.

## The engine already matches - now confirmed

`src/pose.rs` already models exactly this format. The alias and struct are:

```rust
pub type SlotPose = [f32; 13];

pub struct PoseMatrix {
    pub rotation: Mat3,       // [[f32; 3]; 3], row-major
    pub scale: f32,           // always 1.0
    pub translation: [f32; 3],
    pub scale_marker: f32,
}
```

The doc comment on `SlotPose` says: "row-major 3x3 rotation at indices `0..=8`,
one scale marker at index `9`, and a translation at indices `10..=12`."
`slot_pose_matrix` copies floats 0 through 8 into the row-major rotation, floats
10 through 12 into the translation, and float 9 into `scale_marker`.

The runtime observation **confirms this layout**:

| Engine field | Float index | Runtime status |
|---|---|---|
| `rotation` (row-major `Mat3`) | 0..8 | CONFIRMED |
| `scale_marker` | 9 | CONFIRMED position; observed `0.0` throughout |
| `translation` | 10..12 | CONFIRMED |
| `scale` (applied value) | n/a | Engine keeps `1.0`; see below |

The engine's choice to preserve index 9 as an inspection-only marker and apply
`scale = 1.0` is now **validated by measurement**: the probe observed the scale
field as `0.0` in every dumped slot. A literal multiplicative scale of `0.0`
would collapse the model to a point, so the field cannot be a scale. The engine
was right not to apply it.

No engine code is changed by this note. The model was already correct; the
runtime caught up with proof.

## The constructor's three input floats: measured, then solved

The output slot is confirmed. The **three floats passed into the orientation
constructor** are not. The probe sampled them and reported:

```text
abs_max       5951
min_step      0.0224
shape         neither      (for 37722 of 38627 samples)
quant_frac    0
```

Two readings are ruled out by those numbers:

- **Not radians.** Radians used directly for a full body rotation stay within a
  small bounded range; an absolute maximum of `5951` is world-scale, not angle
  scale.
- **Not Q12 fixed point.** `quant_frac = 0` means the values are not multiples of
  a `1/4096` step; a fixed-point angle would show exactly that quantization.

**SUPERSEDED 2026-11-12.** The readings above are still correct about what the
values are *not* (radians, or Q12 of `1.0`), but the interpretation "position or
look-target, not angles" no longer holds. The three floats **are angles in a
turn-based binary unit**: `4096` units is one full turn. `abs_max = 5951` is
about `1.45` turns, and `quant_frac = 0` only tested the `1.0 == 4096`
fraction, which is a different unit. See "The construction is solved" below and
`docs/notes/rotation-model.md`.

Honest statement of the boundary at this slice:

- The **slot** is confirmed: 13 floats, stride 52, rows 0..8, translation 10..12.
- The **producer** is named: the two sibling orientation constructors.
- The **rotation animates**: measured on the same slot while walking.
- The **inputs** were unknown here: this note established that they are not
  radians and not Q12 of `1.0`, but not what they are. The next slice solved the
  construction; the **source** of the angles stays partly open.

## The construction is solved (2026-11-12)

A later atomic capture paired each call's input triplet with the slot returned by
that same call, then fitted a closed family of 192 rotation models. The result is
`UNIQUE` on **336/336** usable rows:

```text
M = Rz(c · K) · Ry(b · K) · Rx(a · K)      K = 2π / 4096
```

Row-major 3x3, X innermost/first-applied and Z outermost. The unit is `4096`
units per turn; the sign is positive. The two tying parameterizations are
algebraically identical (`R(-θ) = R(θ)ᵀ`), so the verdict is one model class, not
two candidates.

What this closes: the arithmetic from the three floats to the rotation block.
What it does **not** close: how the engine derives the three angles (aim/face
mode, object transform, world target), and the meaning of the scale field at
index `9`. The producer chain is now complete **up to the angle source**. Full
evidence and the corrected sign: `docs/notes/rotation-model.md`.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The runtime slot stride is 52 bytes (13 floats) | a measured stride other than 52 | Holds: `dest_stride = 52`, exact for 99.26% of writes |
| The rotation is float indices 0..8, row-major | a rotation that does not read as a 3x3 block there | Holds: 4/4 rotations read cleanly, 0 failures |
| The slot has one scale field at index 9 | a different field position or a varying scale | Holds: index 9 constant `0.0` in every dump |
| The translation is float indices 10..12 | a translation outside those floats | Holds: walking series moves exactly those floats |
| The rotation block animates | an identical rotation on the same slot while the actor moves | Holds: the four-sample series rotates; standing still stays identity |
| The scale field is a multiplicative scale | a value that, applied, preserves the model | Refuted: it is `0.0`, which would collapse the model; engine applies `1.0` |
| The constructor's three inputs are radians | bounded, radian-scale values | Refuted by measurement: `abs_max = 5951`, `quant_frac = 0` |
| Authored per-slot keyframes exist in `.SRA`/`.PCM`/`.SRM`/`.SRL` | any coherent authored rotation channel with counts | Holds as negative: each carrier eliminated with explicit counts (`animation-conclusion.md`) |

## What this note does not claim

- It does not identify the three constructor input floats as angles, positions or
  targets. That stays UNKNOWN.
- It does not re-derive the caller-side conversion between descriptor data and
  the constructor inputs.
- It does not change engine behaviour, code or any golden.
- It does not claim the animation system is fully solved.

The correct summary is: **named producer, confirmed output format, known-unknown
inputs.**

## Related notes

- `docs/notes/rotation-model.md` - the solved construction: formula, unit, order,
  336/336 evidence and the corrected sign.
- `docs/notes/pose-writer-identity.md` - the two orientation constructors and
  their callers.
- `docs/changes/runtime-capture/angle-input-plan.md` - the probe design that
  produced this evidence.
- `docs/notes/object-models.md` - the slot layout and the skinning pipeline.
- `docs/notes/animation-conclusion.md` - the carrier elimination and the
  superseded "no body motion" reading.
- `docs/notes/bone-hierarchy.md` - the descriptor parent tree and composition.

## Legal

Read-only runtime observation. No game process was modified, no game file was
written, and no game byte was copied into the repository. This note records
behaviour, field roles, offsets, strides and format constants only. It contains
no addresses, no module-relative numbers, no disassembly listings, no raw game
bytes and no absolute install paths.

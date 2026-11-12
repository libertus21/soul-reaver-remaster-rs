# rotation-model - the runtime pose rotation is `Rz(c)·Ry(b)·Rx(a)` in 4096-unit turns (2026-11-12)

The body-rotation question is closed at the construction step. An atomic runtime
capture paired each orientation call's three input floats with the 3x3 block that
same call produced, and a closed 192-model search returned one model class on
every usable row. The three floats **are angles**, the unit is **4096 units per
turn**, and the composition is **X first (innermost), then Y, then Z**.

This note records the model, the evidence, the one-model verdict, and the
correction of an earlier sign error. It also states what is still not known: the
**source** of the three angles, not their arithmetic.

## Quick path

1. Read "The model" for the formula, the unit and the order.
2. Read "Evidence: 336 of 336 rows" for the counts and error magnitudes.
3. Read "Verdict: UNIQUE, one equivalence class of two" before quoting a model
   name.
4. Read "Superseded claim: the negative sign" before reusing any older note.
5. Read "What remains UNKNOWN" before claiming the body pipeline is complete.

## The model

For one runtime orientation call, the three input floats `(a, b, c)` and the
produced row-major 3x3 block `M` satisfy:

```text
K = 2π / 4096

M = Rz(c·K) · Ry(b·K) · Rx(a·K)

Rz(t) = [[cos t, -sin t, 0], [ sin t, cos t, 0], [0, 0, 1]]
Ry(t) = [[cos t,  0, sin t], [0, 1, 0], [-sin t, 0, cos t]]
Rx(t) = [[1, 0, 0], [0, cos t, -sin t], [0, sin t, cos t]]
```

- **Unit:** `4096` units is one full turn (`360` degrees), so one unit is
  `2π / 4096` radians. The values are continuous floats in that unit, not
  integers and not a `1.0 == 4096` fraction.
- **Order:** XYZ with X **innermost/first-applied** and Z **outermost**. Written
  as a matrix product, `Rz` is leftmost.
- **Layout:** row-major 3x3. Slot indices `0..8` at runtime; translation at
  `10..12` and the scale field at `9` are unchanged from
  `docs/notes/pose-format-confirmed.md`.
- **Code:** `pose::BINARY_ANGLE_UNITS_PER_TURN`,
  `pose::binary_angle_to_radians` and `pose::rotation_from_binary_angles` in
  `src/pose.rs` implement exactly this. Pose files still copy their stored 3x3
  through `slot_pose_matrix`; the engine additionally **produces** a root
  rotation from a movement direction (see "Now consumed by" below).

## Now consumed by

The formula is no longer test-only. The engine produces a pose instead of only
copying a stored matrix:

| Step | Producer | What it does |
|---|---|---|
| 1 | `engine::frame` stage 9 | Takes the **committed** horizontal movement delta (after the collision sweep), so a blocked player keeps its facing and a slide faces along the wall. Zero delta keeps the previous yaw. |
| 2 | `pose::yaw_rotation_from_direction` | Turns `(dx, dy)` into `Rz(atan2(dy, dx))` through `rotation_from_binary_angles(0, 0, yaw)`. Convention: world is Z-up, `Rz` is the yaw, local `+X` rotates onto the direction. |
| 3 | `pose::pose_from_yaw` | Builds one slot pose per bone: slot 0 carries the produced yaw, every other slot is identity. |
| 4 | `pose::skin_positions_hierarchical` | Composes the bone tree and skins the vertices, the same path `--object-mesh --hierarchy` uses. `object::ModelResolver::posed_textured` expands the result to the per-face rendered mesh. |

`engine::scene` applies step 4 only to the live player object. Level-placed
objects stay at identity: their authored record carries no rotation field, so
giving them one would invent data the level does not hold.

`--facing --bigfile <DAT> --entry <INDEX> --direction <DX>:<DY>` is the headless
witness: it prints the produced binary yaw, the yaw in degrees, how many
vertices moved and the first three posed vertices. It is deterministic (integer
direction in, fixed formatting out) and opens no window.

Facing is presentation state: it is not part of the canonical determinism fold,
and the world-state, frame-state and headless digests are unchanged by this
integration.

## Evidence: 336 of 336 rows

The capture ran as one atomic probe: the input triplet and the returned slot
were read in the **same call**, so a row can never mix two frames.

```text
pairs captured        4096
three-axis rows        437
captured_in_leave        1
leave_read_fails         0
```

The fit tool enumerates 192 closed models (6 orders x 8 sign sets x 2
composition directions x 2 layouts) and scores every row element by element
(tolerance `1e-3`). Rows where all three inputs exceed magnitude `1.0` are the
only rows that can separate orders; single-axis rows are counted separately.

| Row class | Hits | Median abs error | Max abs error |
|---|---|---|---|
| Three-axis | 224 / 224 | `4.67e-07` | `5.95e-07` |
| Two-axis | 83 / 83 | `0.00e+00` | `0.00e+00` |
| Z-only | 27 / 27 | `3.84e-07` | — |
| X-only | 2 / 2 | — | — |
| **Total** | **336 / 336** | | |

Three-axis rows are element-by-element identical at capture precision. The
two-axis rows match exactly. No usable row is unexplained.

## Verdict: UNIQUE, one equivalence class of two

The search reports `UNIQUE`: every selected row is explained by exactly one
equivalence class. Two parameterizations tie on hits:

- `XYZ | + + + | reverse | no-transpose`
- `XYZ | - - - | forward | transpose`

They are **the same map**, not two explanations. For any rotation,
`R(-θ) = R(θ)ᵀ`, so transposing a product reverses the factor order and negates
every angle:

```text
(Rz(c)Ry(b)Rx(a))ᵀ = Rx(-a)Ry(-b)Rz(-c)
```

The tool classifies the pair as one class with two members. `UNIQUE` therefore
means "one algebraic model", not "one source statement". Which literal
multiplication the engine wrote is not observable from the produced matrix, and
the two spellings cannot be separated by any dataset.

## Superseded claim: the negative sign

**SUPERSEDED — do not reuse.** An earlier note stated that the angle was
`-value · K`. That reading came from an extraction artifact, not from the data:

```text
atan2(m[1], m[0])  ->  -θ     (old, wrong pairing)
atan2(m[3], m[0])  ->  +θ     (correct pairing)
```

With the row-major `Rz` above, `m[1] = -sin θ` and `m[3] = +sin θ`, so pairing
`m[1]` with `m[0]` flips the sign. The sign is **POSITIVE**: `value · K` is the
angle, and the fitted model with `+ + +` is the one that matches.

Any older note or comment that carries the negative sign must be read through
this correction. The claim is kept here rather than deleted so the history stays
auditable.

## What remains UNKNOWN

1. **The source of the three angles.** The arithmetic from `(a, b, c)` to the
   matrix is solved. How the engine derives those three angles - aim/face mode,
   the object transform, the world target, and their blend - is only partially
   established. The producer chain is complete **up to the angle source**.
2. **The scale field at `[9]`.** It is `0.0` in every observation. A literal
   `0.0` scale would collapse the model, so it is not a multiplicative scale, but
   its real meaning is unproven.
3. **Coverage.** One shipped build, one level, one actor. The fit is unique on
   that corpus; other actors, builds or mode combinations are unobserved.
4. **Sign versus transpose for consumers.** It does not matter for the stored
   matrix: both spellings produce the same 3x3. It **does** matter for any code
   that decomposes the matrix back into angles, because the two spellings assign
   the sign to different sides of the convention.

## How this is checked in the repo

`cargo test pose::tests` includes:

- the identity at `(0, 0, 0)`;
- each single-axis closed form;
- monotonic advance of each axis over a quarter turn;
- a golden test over three real observed triples and their six-decimal runtime
  matrices, at `1e-5` tolerance.

The golden values are observations, not generated output. The model agrees with
the unrounded matrices to about `5e-7`.

## Related notes

- `docs/notes/pose-format-confirmed.md` - the 13-float slot and its stride.
- `docs/notes/animation-conclusion.md` - the carrier elimination and the
  procedural-orientation conclusion.
- `docs/notes/pose-writer-identity.md` - the two orientation constructors.
- `docs/changes/runtime-capture/axis-order-plan.md` - the atomic capture and the
  192-model search design.

## Legal

Read-only runtime observation. No game file was written, no game byte was copied
into the repository, and no address, module-relative number, disassembly
listing or absolute install path appears here. Only formulas, field roles, unit
constants and corpus counts are recorded.

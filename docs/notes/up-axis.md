# up-axis - the world's vertical axis is Z

Date: 2026-11-09. Read-only corpus probe plus the engine fix. Behaviour and
counts only: no game bytes, no decompiled text and no install paths live in this
repository. The probe ran in memory against the player's own copy and was
removed before commit.

## Verdict

**CONFIRMED: the shipped room and object data is Z-up. The vertical component is
index 2 and `+Z` is up.** The horizontal plane is world X (index 0) / world Y
(index 1).

Before this change the engine treated index 1 (`Y`) as up. That inverted gravity
and the vertical collision surface, put the walkable floor on a horizontal axis,
and rolled the player camera 90 degrees (the reported "character horizontal,
floor vertical" screenshot). The engine is now Z-up end to end; no level data is
rotated at load.

## Method

A temporary `#[ignore]` test decoded every archive entry through the existing
`level::read_entry_world` parser, reverted before commit. It covered **372
units, 573,739 faces and 6,142 placed objects** and computed five independent
checks.

## Evidence

### 1. Face-normal dominant axis

For every non-degenerate face, the axis with the largest absolute normal
component is tallied.

| Axis | Faces whose normal is dominated by it |
|---|---:|
| X | 170,647 |
| Y | 192,799 |
| **Z** | **210,293** |

Z dominates the most faces.

### 2. Near-horizontal set and its sign asymmetry

A face is near-horizontal about axis `A` when `n_A^2 >= 3 * (other two squares)`.
This is the engine's own floor/ceiling test, applied to each candidate axis.
Signs split walkable (`+`) from ceiling-facing (`-`).

| Candidate up | Near-horizontal faces | Positive normal | Negative normal | Positive:negative |
|---|---:|---:|---:|---:|
| X | 125,978 | 63,141 | 62,837 | 1.005:1 |
| Y | 134,502 | 67,173 | 67,329 | 0.998:1 |
| **Z** | **182,740** | **113,973** | **68,767** | **1.66:1** |

The true up axis is the only one whose near-horizontal set is **sign
asymmetric**: walkable floor outnumbers ceiling. The two horizontal axes are
symmetric to within ~0.5%, exactly what two opposite wall facings produce.

### 3. Extent shape across units

The axis with the largest bounding-box span, per unit (rooms are usually wider
than they are tall):

| Largest-span axis | Units (of 372) |
|---|---:|
| X | 192 |
| Y | 143 |
| **Z** | **37** |

Z is the largest span in only 37 units; the horizontal axes dominate. The
vertical axis is usually the short one, as a floor plan implies.

### 4. Placed-object vertical clustering

Each placed object is normalised inside its unit's bounding box per axis. Count
inside the lowest 15% and highest 15% of each candidate axis:

| Candidate up | Objects in lowest 15% | Objects in highest 15% |
|---|---:|---:|
| X | 643 | 567 |
| Y | 532 | 603 |
| **Z** | **2,413** | **165** |

Of 6,142 objects, **39.3% sit in the lowest 15% of Z**, only 2.7% at the top.
Objects rest on the floor, and the floor is low Z. X and Y are flat (~10% each
end).

### 5. Ground-at-spawn coherence

For each unit that places a `raziel` spawn, the highest near-horizontal surface
at the spawn's horizontal point was solved for each candidate up axis, and the
absolute error between that surface and the spawn's coordinate was measured.

| Candidate up | Units with a surface | Mean absolute error (world units) |
|---|---:|---:|
| X | 197 | 2,248 |
| Y | 198 | 2,031 |
| **Z** | **291** | **71** |

Under Z, 291 units have a floor at the spawn and the mean error is 71 units.
Under X or Y the "floor" is ~2,000 units away, because those are horizontal axes
whose extremes are not a floor plane. The pre-fix gravity slice reported a
`y = -3357` ground for `under1`, whose Y range is `-5110..4991`; that value is
deep inside a horizontal spread, not a floor.

## Fix

- Added `src/engine/axis.rs`: `UP = 2`, `HORIZONTAL_A = 0`, `HORIZONTAL_B = 1`,
  `UP_SIGN = 1`, `WorldAxis`, and `up_of`/`horizontal_of`/`from_horizontal`/
  `with_up`. These are the single source of truth for "which way is up".
- `engine/collision.rs` now reads those constants everywhere: face
  classification, floor/ceiling height, the horizontal sweep plane (X/Y), and
  the signed vertical step (`VerticalStep::vertical`). The internal math is
  unchanged; only the axis it reads and writes changed.
- `engine/frame.rs` stage 9 maps `Forward`/`Back` to world `+Y`/`-Y` and
  `Left`/`Right` to `-X`/`+X`, and writes the resolved height to `position[UP]`.
- `engine/input.rs::move_axis` documents forward as `+Y` (the return shape is
  unchanged).
- The engine cameras use a Z-up basis. `FlyCamera`/`OrbitCamera` gained an
  opt-in Z-up basis, and the `--camera player` mode is a third-person follow
  camera behind (`-Y`) and above (`+Z`) the player. The camera reads simulation
  state but never writes it, so determinism is unaffected.
- The **legacy level viewers** now opt into Z-up too: `--level-render` (flat and
  textured) renders with `render::SceneUp::world()`. The object-model viewers
  (`--object-mesh`, `--mesh`, `--mesh-textured`, `--mesh-all`) also use Z-up:
  `docs/notes/object-up-axis.md` later confirmed the object-model local vertical
  is Z (index 2, `+Z`), and `render::SceneUp::object_local()` derives its basis
  from `engine::axis::OBJECT_UP`. See
  `docs/notes/legacy-viewers-up-axis.md` for the per-path table.

No level, object or texture data is rotated at load.

## Determinism

The fold and its field set did not change. The **real no-input digests moved**
because the old axis made gravity act in the wrong direction; that is a
deliberate simulation correction, not a fold change.

| Unit | Before (Y-up) | After (Z-up) | Lands at |
|---|---|---|---|
| `under1` 60-tick | `def409846ff3a2cf` | `ede39c540a10ee39` | `z = 0`, grounded on tick 1 |
| `skinnr1` 60-tick | `131ce9555e923980` | `398d1870a463fd7a` | `z = -636`, grounded on tick 59 |

`under1`'s spawn already rests on the Z floor, so its 60-tick digest collapses
back to the horizontal-only digest `ede39c540a10ee39` logged by the collision
slice. `skinnr1`'s spawn is airborne and now lands on a real Z floor instead of
on the bottom of a horizontal axis.

Synthetic goldens with no floor triangles do not move. One synthetic golden did
move: `GOLDEN_FRAME_STATE_HASH` (`0xd1f3_6008_a152_04a2` ->
`0x9c68_230a_4907_fbe2`) because its scripted tick run moves the player and the
movement plane changed from the wrong X/Z to the correct X/Y. The options,
error, tracker, classification, actor-runtime, world-state and headless goldens
are untouched (their fixtures carry no floor and no movement through this axis).
See `docs/notes/determinism.md`.

## Tests

- `axis::tests::the_world_vertical_axis_is_z_up` and the round-trip accessor
  test pin the convention once.
- Collision fixtures are Z-up, and `faces_are_classified_as_wall_floor_ceiling_or_other`
  proves the classification agrees with `UP`.
- A player spawned above a Z floor falls and lands on it (collision and frame
  levels); walking off a ledge falls to the lower floor.
- `forward_input_moves_the_player_by_the_expected_integer_delta` now also
  asserts the vertical coordinate is unchanged by horizontal walking.
- The third-person camera tests assert the eye is behind (`-Y`) and above
  (`+Z`) and that world `+Z` maps to screen up.
- The same input twice yields the same positions and digest (existing gravity,
  jump and walled-script replays).

## UNKNOWN

- The winding sign is the only normal-direction source, so floor/ceiling is a
  diagnostic split; ground uses every near-horizontal surface.
- The object-model local up axis was not established here. It was proven
  afterwards to be Z-up (`docs/notes/object-up-axis.md`), so the object viewers
  now use the same Z-up basis as the world. See
  `docs/notes/legacy-viewers-up-axis.md`.
- Whether the original applied a load-time axis conversion is not claimed; this
  engine makes the code axis-correct instead.

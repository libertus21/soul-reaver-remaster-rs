# engine-collision - solid walls, gravity, ground, ceilings and jump

Date: 2026-11-09. Slices: `feat/engine-collision` (base `main` at `2f1f0c9`),
`feat/engine-gravity` (base `main` at `5fd5d93`) and `feat/engine-jump` (base
`main` at `4d42001`). All numbers below are behaviour and counts; no game bytes,
no decompiled text and no install paths live in this repository.

## Verdict

The player can no longer walk through walls, falls onto the level floor instead
of hovering at the spawn point, stops under a ceiling when rising, and can jump
from the ground. The room triangles the parser already produces are turned into
a small integer collision mesh with a deterministic uniform grid; stage 9
resolves each movement delta as a swept circle in the `X`/`Z` plane that stops
before a wall and slides along it, then resolves one signed gravity tick in `Y`
against the near-horizontal surfaces. A jump is a deterministic input edge, not
a timer: it sets an upward velocity only while the player is grounded, and the
ceiling resolver clamps the head at the lowest surface above the feet.

This is a **correct, deterministic sweep-and-slide plus a gravity/ground/ceiling
resolver and a synthetic jump rule**. It is **not** the original's four-step
probe/resolve/commit solver, whose order is UNKNOWN and is not claimed. Stage 8
remains a named no-op for that reason.

## Geometry model

- The parser behind header slot `0x00` already yields world-space integer
  triangles (`level::RoomTriangles`, positions `i32`, faces `[u32; 3]`).
  `room_mesh` now widens that same integer result to `f32` instead of reading
  the arrays twice, so the renderer and the collision mesh share one parse.
- `World::load` reads the archive entry once (`level::read_entry_world`) and
  builds both the placed-object database and `CollisionMesh` from it.
- Every face is classified once from the direction of its geometric normal:
  - **wall**: within about 30 degrees of horizontal
    (`nx^2 + nz^2 >= 3 * ny^2`); the existing horizontal barrier;
  - **floor-facing**: near-vertical normal pointing up
    (`ny > 0 && ny^2 >= 3 * (nx^2 + nz^2)`);
  - **ceiling-facing**: the same test with the normal pointing down (`ny < 0`);
  - **other**: steeper slopes and degenerate faces.
- The winding sign is the only normal-direction source and a plane supports a
  body from either side, so ground detection uses **every** near-horizontal
  triangle. The up/down split is a diagnostic count, not a filter: a
  ceiling-facing triangle above the player is never chosen because the ground
  query only looks at or below the feet.
- A wall's three edges are projected to `XZ`. A vertical wall projects to the
  line segment that is its footprint: exactly the barrier the player must not
  cross. Duplicate projected edges are harmless; the narrow phase takes the
  earliest contact.

On the shipped `under1` unit, 1365 of 2922 room faces are wall-like barriers and
464 are near-horizontal surfaces (235 floor-facing, 229 ceiling-facing); the
remaining 1093 are steeper slopes or degenerate (read-only runtime probe;
nothing is committed from the game). `skinnr1` reports 4427 walls and 1876
surfaces (941 floor-facing, 935 ceiling-facing).

## Broad phase

Wall triangles are inserted, in ascending source-face order, into a
`BTreeMap`-backed uniform grid of 512-world-unit cells. A query walks the cell
rectangle in a fixed `(x, z)` order, collects wall indices, then sorts and
deduplicates them. Candidate order is therefore independent of allocation and
hash order. There is no `HashMap` or `HashSet` on this path; 512 units is about
1/20 of the `under1` span, so a player sweep touches one or two cells.

## Sweep and slide

`CollisionMesh::sweep(from, delta, radius)`:

1. A zero `X`/`Z` delta returns `from + delta` immediately. No query and no
   write happen, so a no-input tick is byte-identical.
2. The moving player is a circle of `PLAYER_RADIUS_UNITS` (16 world units;
   the original radius is UNKNOWN) swept from `from` to `from + delta`.
3. For every candidate wall edge, the earliest contact is the minimum of:
   - the infinite line test `|signed distance| = radius`, restricted to the
     segment extent;
   - the two capsule end caps (circle versus point).
4. The sweep advances by truncated integer progress, so the center stops
   strictly before the contact. The leftover delta is projected onto the
   contact tangent and swept again, up to four times. A diagonal push
   therefore slides along the wall instead of sticking or stopping.
5. `Y` is not touched by this horizontal sweep. The vertical resolver runs
   afterwards, at the resolved `X`/`Z`.

## Vertical resolution (gravity, ground, ceilings, landing and jump)

`CollisionMesh::vertical_step(from, velocity)` is a separate, integer-only step
called by stage 9 after the horizontal sweep. The velocity is signed: positive
is rising (a jump), negative is falling, zero is grounded or at the apex.

1. Gravity is `GRAVITY_SUBUNITS_PER_TICK2` (one world unit per tick squared) and
   is subtracted every tick, for both rising and falling motion. Downward speed
   is capped at `MAX_FALL_SUBUNITS_PER_TICK` (16 world units per tick). Both are
   documented synthetic values; the original values are UNKNOWN.
2. The candidate height is `from.y + velocity` after gravity.
3. **Rising**: `ceiling_height(x, z, from.y)` returns the lowest near-horizontal
   surface strictly above the feet. The feet may rise until the head
   (`feet + PLAYER_HEIGHT_UNITS`, a synthetic 48 units) reaches that surface.
   At contact the feet are clamped below the ceiling, the velocity resets to
   zero and the next tick's gravity pulls the player back down. Hitting a
   ceiling is not landing, so `grounded` stays `false`. If the room is shorter
   than the player, the feet never move below their current height.
4. **Falling or resting**: `ground_height(x, z, max_y)` returns the highest
   near-horizontal surface at `(x, z)` that is at or below `from.y +
   MAX_STEP_UP_UNITS` (4 units). Every surface triangle is tested with an integer
   point-in-triangle test in `XZ`; the surface height comes from the triangle
   plane with integer division. When the candidate reaches that surface, the
   player lands exactly on it, velocity resets to zero and `grounded` is true.
5. Stepping off a ledge is just a `(x, z)` where no surface is at the feet; the
   next tick falls. The step-up tolerance keeps a walkable slope from reading as
   a ledge, so walking up and down gentle slopes does not flicker between
   grounded and falling.
6. A mesh with no near-horizontal triangles leaves `Y` untouched and reports
   `grounded = false`. This is deliberate: synthetic fixtures and geometry-free
   worlds have no ground plane, so gravity and the jump impulse do not invent
   one, and the synthetic determinism goldens stay stable.

Ground and ceiling share the same near-horizontal surface set and differ only in
direction: ground is the highest surface at or below the feet, ceiling is the
lowest surface strictly above them. Because a plane supports a body from either
side and the winding sign is the only normal-direction source, a floor of a
higher storey acts as a ceiling from below. This uses the classification the
gravity slice already produced; nothing is re-parsed or re-classified.

## Jump rule

`InputButton::Jump` already existed in the stable input enum (code 5); the jump
rule consumes its **press edge**, so the input script is data and never a
clock. In stage 9, after the horizontal sweep:

- a jump starts only when `player_grounded` is true, i.e. the player ended the
  previous tick resting on a floor;
- the press edge replaces the signed vertical velocity with
  `JUMP_IMPULSE_SUBUNITS_PER_TICK` (a documented synthetic 10 world units per
  tick), then the vertical step applies gravity;
- a held button is one edge, and a press while airborne is ignored because
  `player_grounded` is false; there is no double jump;
- landing sets `grounded` again, which re-enables the next press edge.

The discrete arc peaks 45 world units above the launch point under one unit per
tick squared. The original impulse and the original player height are UNKNOWN
and are not claimed.

Because the player's `Y` is an integer world unit and landing snaps exactly to
the surface, a grounded player rests at the same height every tick (no jitter,
no sinking), and repeated jumps from the same ground produce the same arc.

## Determinism and the no-input digest

The math is fixed-point integer: 1024 subunits per world unit, all products
widened to `i128`, and an integer square root rounded up for the sweep. The
vertical path uses the same fixed-point discipline: velocity is an integer in
subunits per tick, the ground point query is integer, and surface heights use
integer division. Chosen positions are converted back to world units with
explicit rounding. There is no `f32`/`f64` on this path, and determinism beats
geometric fidelity.

Sources on this path contain no hash-ordered container, `Instant`, `SystemTime`,
`rand` or thread RNG. `determinism::collision_and_movement_path_never_names_hash_clock_or_random_sources`
scans `collision.rs` and `frame.rs` for those tokens, and `collision.rs` now
participates in the existing wall-clock source scan.

Gravity moves the player, so the no-input digest moves **when the authored spawn
is above the floor**. That is correct simulation change, not a regression; the
fold shape and field set are unchanged.

The shipped spawns are airborne on both tested units (read-only runtime probe):

| unit | spawn | ground under spawn | 60-tick before | 60-tick after |
|---|---|---|---|---|
| `under1` | `[0, 0, 0]` | `-3357` | `ede39c540a10ee39` | `def409846ff3a2cf` |
| `skinnr1` | `[-505, 270, 173]` | `-959` | `d93bcb5cd913f812` | `131ce9555e923980` |

Both 60-tick runs are byte-identical across two consecutive executions, and both
settle: `under1` lands at `y = -3357` between ticks 200 and 250, `skinnr1` lands
at `y = -959` before tick 120, and neither moves in `Y` again through tick 300.
The 300-tick digests are deterministic too: `adbfd4d88af3f093` (`under1`) and
`b4bda7e380c3157b` (`skinnr1`).

The unit test `zero_input_with_a_wall_matches_the_collision_free_digest` still
proves that no floor geometry means no vertical movement: a walled synthetic
world and a collision-free synthetic world fold the same digest over 40
zero-input ticks. All synthetic goldens (`options`, `error`, `world-state`,
`tracker`, `classification`, `actor-runtime`, `frame-state`, `headless`) are
untouched, because their fixtures carry no floor triangles.

The jump slice does **not** touch a no-input run: nothing sets a positive
velocity without a `Jump` press edge, so both real 60-tick digests are
byte-identical to the gravity slice. Two consecutive runs per unit print:

| unit | 60-tick digest (twice) |
|---|---|
| `under1` | `def409846ff3a2cf` |
| `skinnr1` | `131ce9555e923980` |

`player_vertical_velocity` and `player_grounded` remain diagnostics and are not
folded. The canonical fold still covers only tick, class, integer position and
`visible`; a jump changes the digest only because the player's `Y` is part of
the existing position fold, not because a new field was added.

## Tests

Synthetic only; no game file is read by the suite.

- wall/floor/ceiling classification, a too-steep slope kept out, degenerate
  faces;
- broad phase never misses a brute-force AABB match, and chooses the same
  earliest contact as a brute-force narrow pass;
- a single sweep stops at the radius; 100 forward ticks never cross the wall;
- a diagonal push slides and keeps making progress;
- a floor-only or empty mesh never blocks horizontally;
- zero input with a wall keeps the collision-free digest;
- the same walled input script replays identical positions and digests;
- a player spawned above a floor falls and lands on it without passing through;
- a grounded player rests at the same height for 300 ticks with zero velocity;
- walking off a ledge starts a fall and landing on the lower floor stops it;
- a mesh without surfaces leaves `Y` untouched;
- the same vertical script and the same frame-level gravity run replay the same
  positions and digest;
- a jump rises, peaks at the documented 45 units and lands exactly on the
  ground; two consecutive jumps produce the same arc with no drift;
- a jump under a low ceiling stops at the ceiling clearance and falls back;
- `ceiling_height` picks the lowest surface strictly above the feet;
- an upward step with no surfaces does nothing;
- the same jump script replays identical observations;
- a mid-air jump press does nothing, and landing re-enables the jump;
- a held jump button produces exactly one jump;
- a combined forward/right/jump script into a wall corner never tunnels;
- zero input never moves the player upward (the no-input regression);
- the same frame-level jump input script replays the same positions and digest.

## UNKNOWN / limits

- The original solver order, its exact probe extents, its gravity, its jump
  impulse, its player height and its ground-snap tolerance are UNKNOWN. The
  constants here are documented synthetic values.
- The player radius and the step-up tolerance are documented synthetic values.
- The winding sign of a room face is the only normal-direction source; ground
  and ceiling detection therefore treat both floor- and ceiling-facing
  horizontal triangles as planar surfaces.
- A ceiling is resolved only above the feet. A room shorter than the player
  (ceiling below `feet + PLAYER_HEIGHT_UNITS` at the start of the tick) clamps
  the feet at their current height instead of pushing them down; it does not
  model a crouch or a squeeze.
- Slopes steeper than about 60 degrees from horizontal are not barriers, and
  slopes steeper than about 30 degrees from horizontal are not ground.

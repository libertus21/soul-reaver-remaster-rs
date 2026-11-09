# engine-collision - solid walls, gravity and ground

Date: 2026-11-09. Slices: `feat/engine-collision` (base `main` at `2f1f0c9`) and
`feat/engine-gravity` (base `main` at `5fd5d93`). All numbers below are
behaviour and counts; no game bytes, no decompiled text and no install paths live
in this repository.

## Verdict

The player can no longer walk through walls, and now falls onto the level floor
instead of hovering at the spawn point. The room triangles the parser already
produces are turned into a small integer collision mesh with a deterministic
uniform grid; stage 9 resolves each movement delta as a swept circle in the
`X`/`Z` plane that stops before a wall and slides along it, then resolves one
gravity tick in `Y` against the near-horizontal surfaces.

This is a **correct, deterministic sweep-and-slide plus a gravity/ground
resolver**. It is **not** the original's four-step probe/resolve/commit solver,
whose order is UNKNOWN and is not claimed. Stage 8 remains a named no-op for
that reason.

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

## Vertical resolution (gravity, ground and landing)

`CollisionMesh::vertical_step(from, velocity)` is a separate, integer-only step
called by stage 9 after the horizontal sweep:

1. Downward velocity is negative and accumulates
   `GRAVITY_SUBUNITS_PER_TICK2` (one world unit per tick squared) each tick,
   capped at `MAX_FALL_SUBUNITS_PER_TICK` (16 world units per tick). Both are
   documented synthetic values; the original values are UNKNOWN.
2. The candidate height is `from.y + velocity`.
3. `ground_height(x, z, max_y)` returns the highest near-horizontal surface at
   `(x, z)` that is at or below `from.y + MAX_STEP_UP_UNITS` (4 units). Every
   surface triangle is tested with an integer point-in-triangle test in `XZ`;
   the surface height comes from the triangle plane with integer division.
4. When the candidate reaches that surface, the player lands exactly on it,
   velocity resets to zero and `grounded` is true. Otherwise the candidate
   stands and the fall continues.
5. Stepping off a ledge is just a `(x, z)` where no surface is at the feet; the
   next tick falls. The step-up tolerance keeps a walkable slope from reading as
   a ledge, so walking up and down gentle slopes does not flicker between
   grounded and falling.
6. A mesh with no near-horizontal triangles leaves `Y` untouched and reports
   `grounded = false`. This is deliberate: synthetic fixtures and geometry-free
   worlds have no ground plane, so gravity does not invent one, and the
   synthetic determinism goldens stay stable.

Because the player's `Y` is an integer world unit and landing snaps exactly to
the surface, a grounded player rests at the same height every tick (no jitter,
no sinking). Ceilings are classified and counted but resolve nothing in this
slice: the only vertical producer is gravity, so the player never moves upward.
A future jump/upward slice fills the ceiling path without a second parse.

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
  positions and digest.

## UNKNOWN / limits

- The original solver order, its exact probe extents, its gravity and its
  ground-snap tolerance are UNKNOWN. The constants here are documented synthetic
  values.
- The player radius and the step-up tolerance are documented synthetic values.
- The winding sign of a room face is the only normal-direction source; ground
  detection therefore treats both floor- and ceiling-facing horizontal
  triangles as support surfaces.
- Ceilings are classified but block nothing yet, because nothing moves the
  player upward in this slice.
- Slopes steeper than about 60 degrees from horizontal are not barriers, and
  slopes steeper than about 30 degrees from horizontal are not ground.

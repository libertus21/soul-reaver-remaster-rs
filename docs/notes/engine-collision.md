# engine-collision - solid walls for the player

Date: 2026-11-09. Slice: `feat/engine-collision`, base `main` at `2f1f0c9`.
All numbers below are behaviour and counts; no game bytes, no decompiled text
and no install paths live in this repository.

## Verdict

The player can no longer walk through walls. The room triangles the parser
already produces are turned into a small integer collision mesh with a
deterministic uniform grid, and stage 9 resolves each movement delta as a swept
circle in the `X`/`Z` plane that stops before a wall and slides along it.

This is a **correct, deterministic sweep-and-slide**. It is **not** the
original's four-step probe/resolve/commit solver, whose order is UNKNOWN and is
not claimed. Stage 8 remains a named no-op for that reason.

## Geometry model

- The parser behind header slot `0x00` already yields world-space integer
  triangles (`level::RoomTriangles`, positions `i32`, faces `[u32; 3]`).
  `room_mesh` now widens that same integer result to `f32` instead of reading
  the arrays twice, so the renderer and the collision mesh share one parse.
- `World::load` reads the archive entry once (`level::read_entry_world`) and
  builds both the placed-object database and `CollisionMesh` from it.
- A face is a **wall** when its 3D normal is close to horizontal:
  `nx^2 + nz^2 >= 3 * ny^2` (within about 30 degrees of vertical). Floors,
  ceilings and gentler slopes are skipped because this milestone has no
  vertical movement; the player must cross them, not collide with their
  triangle edges.
- A wall's three edges are projected to `XZ`. A vertical wall projects to the
  line segment that is its footprint: exactly the barrier the player must not
  cross. Duplicate projected edges are harmless; the narrow phase takes the
  earliest contact.

On the shipped `under1` unit, 1365 of 2922 room faces are wall-like barriers
(read-only runtime probe; nothing is committed from the game).

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
5. `Y` is translated directly; there is no vertical movement in this
   milestone.

## Numbers and determinism

The math is fixed-point integer: 1024 subunits per world unit, all products
widened to `i128`, and an integer square root rounded up. Rounding up makes
every computed contact slightly early, which keeps the result on the near side.
Chosen positions are converted back to world units with explicit rounding.
There is no `f32`/`f64` here, and determinism beats geometric fidelity.

Sources on this path contain no hash-ordered container, `Instant`, `SystemTime`,
`rand` or thread RNG. `determinism::collision_and_movement_path_never_names_hash_clock_or_random_sources`
scans `collision.rs` and `frame.rs` for those tokens, and `collision.rs` now
participates in the existing wall-clock source scan.

## No-input digest proof

`--play --headless` with no input must not move the player and must not collide.
The 60-tick digests are unchanged from `main`:

| unit | before | after |
|---|---|---|
| `under1` | `ede39c540a10ee39` | `ede39c540a10ee39` |
| `skinnr1` | `d93bcb5cd913f812` | `d93bcb5cd913f812` |

The unit test `zero_input_with_a_wall_matches_the_collision_free_digest` also
proves that a walled world and a collision-free world fold the same digest over
40 zero-input ticks.

## Tests

Synthetic only; no game file is read by the suite.

- wall/floor classification and degenerate faces;
- broad phase never misses a brute-force AABB match, and chooses the same
  earliest contact as a brute-force narrow pass;
- a single sweep stops at the radius; 100 forward ticks never cross the wall;
- a diagonal push slides and keeps making progress;
- a floor-only or empty mesh never blocks;
- zero input with a wall keeps the collision-free digest;
- the same walled input script replays identical positions and digests.

## UNKNOWN / limits

- The original solver order and its exact probe extents are UNKNOWN.
- The player radius is a documented synthetic value.
- Collision is horizontal only: a future vertical-movement slice must add the
  `Y` axis rather than reusing this sweep unchanged.
- Slopes gentler than about 60 degrees from horizontal are not barriers in this
  slice.

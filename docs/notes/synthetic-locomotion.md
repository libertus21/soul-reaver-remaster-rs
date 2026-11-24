# Synthetic NPC locomotion

**Status:** implemented in `src/engine/synthetic_locomotion.rs`, called from the
stage-14 body in `src/engine/frame.rs`.

**This is not original behaviour.** The decoded data gave **no NPC pathing**.
The `campath` position spline is not an NPC route (`campath-route-test.md`), and
the original actor AI was never decoded. Retail actor movement is **UNKNOWN**.
The patrol described here is our own gameplay layer. It exists for one reason:
every NPC used to play `idle` because its position never changed, so
`ActorAnimation::observe` always passed `moving = false`. A real position delta
is what makes the already-labelled `walk` clip observable.

## What it does

Every live non-player `monster_` actor owns a `SyntheticPatrol`:

- an integer heading into a fixed table of eight directions;
- a deterministic duty cycle (walk, then dwell) whose phase is offset by the
  live id, so the actors do not move in lockstep and **both** the `walk` and the
  `idle` clip are visible;
- a four-world-unit horizontal step per walking tick;
- a bounce on contact: the step is committed only when the existing
  `CollisionMesh` narrow phase reports no contact; otherwise the heading turns
  90 degrees and the actor tries again next tick;
- a room fence: the step is also refused when the body-radius footprint leaves
  the geometry bounds, so an actor cannot walk out of its room.

The player is skipped explicitly. Stage 9, the input path and the facing
pipeline are untouched.

## Constants

| Constant | Value | Meaning |
|---|---|---|
| `SYNTHETIC_STEP_UNITS_PER_TICK` | `4` | Per-axis step scale, same as the player's direct translation. |
| `SYNTHETIC_DIRECTIONS` | 8 entries | `(4,0)`, `(3,3)`, `(0,4)`, ... in horizontal `[A, B]` order. |
| `SYNTHETIC_ACTOR_RADIUS_UNITS` | `16` | Collision radius, the player's stand-off. |
| `SYNTHETIC_WALK_TICKS` | `72` | Walking part of one cycle. |
| `SYNTHETIC_DWELL_TICKS` | `24` | Standing part of one cycle. |
| `SYNTHETIC_CYCLE_TICKS` | `96` | Walk plus dwell. |

All are integer world units or tick counts. No `f32`/`f64` enters the position
update.

## Why bounce plus wall clamp

An NPC route needs a destination or a navigation graph. The decoded data has
neither: the spline test refuted the only candidate, so inventing a waypoint
list would be a bigger lie than an honest local patrol. A bounce needs only the
geometry the engine already trusts:

- the wall narrow phase is the same `first_contact`/`BodySpan` code the player
  sweep uses, reached through `CollisionMesh::blocked`; it is not duplicated;
- the room fence uses `CollisionMesh::bounds`, computed from every referenced
  room vertex, so a floor or ceiling also bounds the walk area;
- a blocked step commits nothing. The actor never ends the tick inside a wall
  or outside the room; it just turns and waits for the next tick.

The bounce also produces a bounded, repetitive path, which is exactly what makes
a duty cycle meaningful: the actor walks, hits something, turns, and later
stands still, so the labelled clips alternate.

## Determinism

The patrol state is seeded from `(live id, authored start position)` with integer
arithmetic. Each tick's update is a pure function of that state, the tick index,
the actor class and the fixed collision mesh/bounds. There is no float, no
wall-clock time, no hash-ordered container and no allocation-dependent order:
the live list is walked in spawn order and patrols are keyed by `LiveId`. The
same argv, archive and unit therefore produce the same positions, and the same
canonical digest, on every run and platform.

Patrol state is **not** part of the canonical fold. Only the resulting
positions are, so the digest still describes the world rather than our
bookkeeping.

## Which actors move

Only `RenderClass::Monster` live objects, excluding the player. The engine
already distinguishes the classes: `raziel` is the player, `sreavr` is the
cutscene model, `glphicon` is a glyph icon, `physical` is a static prop,
`particle` is an effect and `Unclassified` is an unknown placement. Markers and
inert names never become live objects at all. Moving any of those would invent
motion for things that are not actors.

Because the actor's animation state is derived from the real position delta,
a `monster_` whose `.SRA` resolves now selects its `walk` clip while the patrol
walks and its `idle` clip while it dwells.

## Switch and goldens

`--no-npc-motion` turns the stage off (default is on). With it off, every actor
stays where the level placed it, which reproduces the pre-locomotion digests
exactly. The determinism gate keeps both values:

- `GOLDEN_WORLD_STATE_HASH` moved from `0x043f_f933_4dfe_3b58` to
  `0x09ab_2f05_ecb6_7cd8` because the fixture's monster now moves;
- `disabling_synthetic_motion_reproduces_the_previous_world_digest` pins the old
  constant with the stage off;
- the capsule acceptance fixture has no eligible monster, so its headless
  digest is unchanged, and a dedicated monster fixture pins the stage-off
  digest.

## Non-goals

- No pathfinding, patrol routes, waypoints, schedules or AI decisions.
- No animation state machine changes and no clip-label store writes.
- No player movement changes.
- No claim about what the original does. The original is UNKNOWN.

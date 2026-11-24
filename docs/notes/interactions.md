# Door interactions: open/closed state, collision, rendering and digest

**Status:** implemented. State, the `Blocking`/`Passive` classification and the
trigger live in `src/engine/interaction.rs`, the collision overlay in
`src/engine/collision.rs` (`union_walls`), the frame wiring in
`src/engine/frame.rs` (stages 1, 6, 9), the mesh build in
`src/engine/scene.rs` (`interaction_collision`) and the CLI in
`src/engine/play.rs`.

**This is not original door behaviour.** The authored door **motion** (the
closed-to-open pose/animation) was never decoded and is **UNKNOWN**. What is
implemented here is a synthetic, deterministic interaction layer: open means the
object is hidden and, for a `Blocking` record, stops being a barrier; closed
means the authored placement is drawn and, for `Blocking`, blocks. `Passive`
records never block in either state. The retail door animation is deliberately
not reproduced.

## Phase 1 gate: the four answers, with measurements

### 1. Do door objects affect collision today? No.

The collision mesh is built **only** from the room's integer triangles:

- `src/engine/world.rs:71-75` reads `level::read_entry_world`, which returns the
  object report **and** `RoomTriangles`, then runs
  `CollisionMesh::from_room_triangles(&triangles.vertices, &triangles.faces)`.
- Placed objects are a separate stream (`PlacedDb::from_records`) and never
  reach `CollisionMesh`.
- `src/engine/frame.rs` stage 9 sweeps the player against `self.world.collision`
  only (before this change). No placed-name geometry was ever queried.

### 2. The problem is "doors never blocked", not "doors block while closed".

There was no door state at all and no placed object in the collision mesh, so a
door was purely decorative geometry.

### 3. Is a door's own geometry available separately? Yes.

`object::ModelResolver` resolves a placed name to its own archived container
(`model`/`textured`/`mesh`), and the scene builder already draws it as its own
`DrawItem` at `object.position` (`SceneAssets::build_with_poses`). The placed
record carries **no rotation** field, so world space is `model + position`. A
door can therefore be meshed on its own exactly like an actor model, and its
world-space triangles can be added to or removed from a collision set.

### 4. Per-name counts across the archive

Measured on the shipped archive `1/BIGFILE.DAT` (372 `.PCM` units, 0
unparseable). Instances / units that place it:

| name | instances | units |
|---|---:|---:|
| `sudoor` | 29 | 27 |
| `catdoor` | 2 | 2 |
| `wrpdoor` | 11 | 11 |
| `hatch` | 9 | 3 |
| `valve` | 6 | 6 |
| `plswtch` | 11 | 8 |
| `bellsc` | 3 | 3 |
| `dblgate` | 5 | 5 |
| `catdora` | 1 | 1 |
| `skgate` | 3 | 1 |
| `sugate` | 8 | 8 |
| **total** | **88** | |

All eleven names resolve to a model. Rest-pose triangles, and how many classify
as near-vertical walls under the same rule the room uses:

| name | vertices | triangles | walls | horizontal | other |
|---|---:|---:|---:|---:|---:|
| `sudoor` | 28 | 44 | 32 | 12 | 0 |
| `catdoor` | 28 | 44 | 32 | 12 | 0 |
| `wrpdoor` | 28 | 44 | 32 | 12 | 0 |
| `hatch` | 19 | 32 | 16 | 16 | 0 |
| `valve` | 24 | 34 | 20 | 6 | 8 |
| `plswtch` | 92 | 158 | 110 | 40 | 8 |
| `bellsc` | 60 | 99 | 36 | 28 | 35 |
| `dblgate` | 40 | 72 | 52 | 20 | 0 |
| `catdora` | 48 | 84 | 76 | 0 | 8 |
| `skgate` | 93 | 144 | 137 | 5 | 2 |
| `sugate` | 4 | 14 | 8 | 0 | 6 |

## Classification: `Blocking` vs `Passive`

**This classification is our interpretation, not decoded behaviour.** The
archive gives the authored name, placement and rest geometry; it does not say
which records the player may open or what their authored state is. We assign a
kind from each name's physical role so the collision rule is explicit instead of
implicit in the name list.

| name | kind | instances | units | reasoning |
|---|---|---:|---:|---|
| `sudoor` | Blocking | 29 | 27 | door standing across a passage |
| `catdoor` | Blocking | 2 | 2 | door |
| `catdora` | Blocking | 1 | 1 | door (variant spelling of `catdoor`) |
| `wrpdoor` | Blocking | 11 | 11 | warp door; a closed barrier that opens |
| `hatch` | Blocking | 9 | 3 | floor/ceiling hatch; a closed barrier |
| `dblgate` | Blocking | 5 | 5 | gate |
| `skgate` | Blocking | 3 | 1 | gate |
| `sugate` | Blocking | 8 | 8 | gate |
| `valve` | Passive | 6 | 6 | wall-mounted control; activated, never traversed |
| `plswtch` | Passive | 11 | 8 | wall switch; activated, never traversed |
| `bellsc` | Passive | 3 | 3 | bell pull/switch; activated, never traversed |

`Blocking` names contribute walls **only while closed**. `Passive` names
contribute no walls in either state, but they keep the activation state, the
`E`/`--interact-at` trigger, the console line and the digest fold.

### `doorh`: measured, then deliberately excluded

`doorh` is outside the surveyed set. A full `--level-objects` scan of the shipped
archive found **1 instance in 1 unit** (`aluka20`, position
`10399,0,-2560`), and its model is a flat vertical plane with bounds
`(-639,0,0)..(640,0,1290)` (entry 788), i.e. door-shaped.

It is still **excluded** and stays non-interactive and non-colliding, as before:

- This slice is a correction of the previous registry's side effect, not an
  expansion of it. Wiring `doorh` as `Blocking` would newly make a previously
  passable placement in `aluka20` a closed barrier.
- The archive does not say whether that placement is opened, decorative or a
  static gate, so that new barrier would be unmeasured behaviour change.

If `doorh` is promoted later, it should be its own deliberate change with its
own before/after movement evidence, not a silent addition to the table above.

## State model

The registry entry is created **per live record** (`LiveId`), not per name, so
two doors in one room open independently. The state is a single `open: bool`
that defaults to `false` (closed). Registration happens when a live object whose
authored name is in `INTERACTABLES` first appears (stage 1, before the stages
read the live set). The player is never an interactable.

## Trigger and radius

- **Key `E`**: windowed player camera mode activates the nearest interactable
  within `INTERACT_RADIUS_UNITS = 128` world units of the player. In fly/orbit
  mode `E` stays the fly-up key; the interaction binding is player-mode only.
- **Scripted**: `--interact-at <TICK>:<TARGET>` (headless only), applied at the
  start of that tick, exactly like `--warp-at`. `TARGET` is `nearest` or a
  live slot `<slot>` / `<slot>:<generation>`.
- The radius is a single documented constant, not per-object magic.

One console line per activation:

```
interact object=<name> pos=<X,Y,Z> state=<open|closed> trigger=<key|script|proximity>
```

`trigger` is `key` for `E`, `script` for an explicit slot and `proximity` for a
`nearest` target resolved by distance. `state` is the state **after** the toggle.

## Collision decision and its limit

Because a door's geometry is separable (§1.3), collision is implemented, not
faked:

- `SceneAssets::interaction_collision` builds one `CollisionMesh` per placed
  **blocking** interactable from that model's rest-pose triangles in world space
  (model + placement, rounded to integer units), classified by the same
  `CollisionMesh::from_room_triangles` the room uses. `Passive` names get no mesh
  at all.
- The simulation attaches the matching mesh to each live **blocking** record and,
  while the door is **closed**, unions its walls ahead of the room walls for the
  horizontal player sweep (`CollisionMesh::union_walls`). When **open**, the
  walls are omitted.
- Vertical (gravity/ceiling) resolution stays on the room mesh only, so a door's
  horizontal faces can never become a floor.

Limits, stated plainly:

- The barrier is the **authored rest pose**, i.e. the closed door as placed. The
  real open pose is UNKNOWN; we model open by removing the barrier, not by
  moving the mesh.
- A named blocking door whose model fails to resolve still interacts and hides,
  but adds no barrier. (In the shipped archive all eleven names resolve.)
- Only `Blocking` records contribute. `Passive` records (`valve`, `plswtch`,
  `bellsc`) register, toggle, log and fold into the digest, but they add no wall
  in either state; their meshes are not even built for collision. The
  per-registration kind check in `InteractionLayer::register_live` enforces this
  even if a passive mesh were installed by mistake.
- NPC synthetic locomotion still uses the room mesh (`world.collision`), so
  closed doors block the player, not the NPC patrol. That is a deliberate scope
  limit of this layer.

Mechanism proof:

- `src/engine/frame.rs::a_closed_door_blocks_the_player_and_an_open_one_lets_it_through`
  builds a wall across a corridor, blocks the player, opens the door, and shows
  the player passing.
- `src/engine/interaction.rs::the_classification_is_pinned_to_exact_name_sets`
  pins the exact `Blocking` and `Passive` sets, so a future edit is deliberate.
- `src/engine/interaction.rs::a_passive_object_adds_no_wall_closed_or_open`
  installs a wall mesh under a passive name on purpose and proves the layer
  still adds no wall, closed or open.
- `src/engine/frame.rs::a_passive_toggle_still_folds_into_the_digest` proves a
  passive toggle still moves the production digest.

## Rendering placeholder

`SYNTHETIC_OPEN_REPRESENTATION = "hidden"`. An open door is hidden (its
`visible` flag is forced false in stage 6); a closed door draws exactly as
before. The quantity and shape of the authored door motion are UNKNOWN and are
not implemented. This is the named synthetic placeholder, not retail behaviour.

## Digest treatment

The canonical world fold is unchanged. The production digest adds an interaction
segment **only when something is open**:
`world_state_hash_with_interactions` folds the world, then (only for a non-empty
open set) appends a marker byte, the count and each open interactable's live-list
ordinal. Therefore:

- with no open door, and always with `--no-interactions`, the digest is
  byte-identical to the previous `world_state_hash` of the same world;
- opening or closing **any** interactable — blocking or passive — moves the
  digest; closing it back restores it. A passive toggle is observable state even
  though it adds no collision.

`--no-interactions` also disables the door collision overlay and every
activation, so it reproduces the pre-interaction digests exactly.

Measured digests (`--headless`, idle unless noted):

| run | digest |
|---|---|
| `under1` 60 ticks, `--no-interactions` | `ede39c540a10ee39` |
| `under1` 60 ticks, interactions on | `ede39c540a10ee39` |
| `cathy28` 60 ticks, `--no-interactions` | `3e8293c0e38afac1` |
| `cathy28` 60 ticks, interactions on | `3e8293c0e38afac1` |
| `aluka15` 12 ticks, at `sudoor`, no activation | `b0ab26619d5c3e0b` |
| `aluka15` 12 ticks, `--interact-at 5:nearest` | `2805d8f368187acc` |
| `aluka15` 12 ticks, `--no-interactions` + schedule | `b0ab26619d5c3e0b` |

`under1` and `cathy28` are byte-for-byte unchanged with the flag, and unchanged
with interactions on because neither idle run opens a door.

## Real headless evidence (`aluka15`, which places `sudoor`)

Scripted activation at tick 5 with the player at the door:

```
$ --play --bigfile 1/BIGFILE.DAT --unit aluka15 --headless --ticks 12 \
    --player-pos -1693,5438,1479 --interact-at 5:nearest
interact object=sudoor pos=-1693,5438,1479 state=open trigger=proximity
engine unit=aluka15 rooms=1 placed=16 admitted=8 tracked=6 skipped_markers=8 inert=0 unresolved=0 live=8 ticks=12 digest=2805d8f368187acc
```

Collision, pushing forward into the same door from `-1693,5200,1479` for 120
ticks:

```
--no-interactions   end=-1693,5664,1480 blocked_ticks=4    (passes the doorway)
interactions closed end=-1693,5385,1480 blocked_ticks=73   (stops at the door)
open at tick 40     interact object=sudoor ... state=open trigger=proximity
                    end=-1693,5664,1480 blocked_ticks=4    (passes after opening)
```

The closed door stops the player at `y=5385`, the door's near face minus the
16-unit player radius; opening removes the barrier and the player reaches the
same `y=5664` as the no-interaction run. These numbers were re-measured after the
`Blocking`/`Passive` split and are unchanged.

## Real headless evidence (a passive switch: `skinnr10` places two `plswtch`)

`skinnr10` places `plswtch` at `-4159,-3,-3036`. Its rest panel occupies
`x -4287..-4029`, `y -69..-2`, `z -2827..-2484` (model bounds
`(-128,-66,209)..(130,1,552)`), so a rightward walk at feet `z=-2736` crosses the
panel footprint at body height. The switch is **closed** (no activation) in both
runs.

The regression, measured on the parent commit with a 60-tick probe:

```
--no-interactions   end=-4095,-35,-3576 dx=225 blocked_ticks=3
interactions on     end=-4130,-35,-3576 dx=190 blocked_ticks=12   (switch stopped the player)
```

After the split, the same 60-tick probe is identical with interactions on and
off:

```
$ --play ... --unit skinnr10 --headless --ticks 60 \
    --player-pos -4320,-35,-2736 --input-script right:60 [--no-interactions]
movement ticks=60 start=-4320,-35,-2736 end=-4095,-35,-3576 dx=225 dy=0 dz=-840 blocked_ticks=3 grounded=0 player_bound=1
```

A 120-tick run crosses the whole panel span `x -4287..-4029`; on and off are
again identical (`end=-3855,-35,-4536`, `dx=465`, `blocked_ticks=3`). The
`blocked_ticks=3` that remains is room geometry, present with and without the
layer.

The switch still toggles and still folds into the digest (it is closed by
default; this is the activation run):

```
$ --play ... --unit skinnr10 --headless --ticks 4 \
    --player-pos -4159,-3,-3036 --interact-at 1:nearest
interact object=plswtch pos=-4159,-3,-3036 state=open trigger=proximity
engine unit=skinnr10 rooms=1 placed=14 admitted=4 tracked=3 skipped_markers=10 inert=0 unresolved=0 live=4 ticks=4 digest=d975162e99886e8c
```

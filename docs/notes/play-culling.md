# `--play` distance culling follows the player

## The bug

Stage 6 (`ActivationCull`) marks a live object visible when it is within
`DEFAULT_CULL_RADIUS` (4096 world units) of `CullView::eye`:

```text
dx*dx + dy*dy + dz*dz <= radius*radius
```

`CullView::default()` has `eye = [0, 0, 0]`. Nothing in the play loop ever
replaced that view, so the cull was permanently centred on the **world origin**.
The level item is always `visible: true`, which is why room geometry drew
normally while every placed object disappeared in zones far from the origin.

`cathy28` is the proof case. Its `raziel` spawns at `-2849,5455,11510`, which is
`sqrt(2849^2 + 5455^2 + 11510^2) = 13051` units from the origin — more than three
times the 4096 radius. Its five enemies are 12,325–14,710 units from the origin,
so all of them were culled too. `under1` spawns `raziel` at `0,0,0`, which is
why the engine only ever appeared to work there.

## The fix

`FrameState::cull_to_player` points the stage-6 eye at the bound player and
keeps the documented 4096 radius. The play drivers call it **before each step**:

- `FrameState::frame` (the windowed clock driver), per due tick;
- `finish_headless` / `headless_run_with_actors`;
- `run_script_ticks` (`--input-script`).

With no player bound it does nothing, so a playerless world keeps the default
view. The third-person camera and the cull now agree: both are derived from the
player position, which is what the original engine does (cull around the
camera).

### Previous-tick choice

The eye is the player's position as of the **last completed tick**, captured
before the next tick's stages run. It is a pure function of the state the tick
starts from, so a replay is byte-identical; the player moves at most
`PLAYER_SPEED_UNITS_PER_TICK` (4) units per tick, far inside the radius, so the
body never culls itself.

### The rendered body tracks the player

`FrameState::player_position` reads the live player object, and stage 9 writes
the committed position back into that same live entry. `build_with_poses` copies
`LiveObject.position` into `DrawItem.world`, so the drawn body is the player
position by construction. A test
(`the_live_player_body_tracks_the_player_position_as_it_moves`) pins that the
stage-15 snapshot follows a committed move instead of staying at the spawn.

## Determinism guard

With the player at `(0, 0, 0)` the eye equals `CullView::default()`, so the
visibility bytes of `under1` are unchanged and its 60-tick digest stays
`ede39c540a10ee39`. Units whose spawn is offset legitimately change digest
because their actors are now drawn (`cathy28`: `475343b4676fc5cc` before,
`b8c30d28e0c67130` after).

## Measuring it

`--cull-report` (headless only) prints one extra read-only line after the
acceptance line, so the acceptance contract is unchanged:

```text
cull eye=<X>,<Y>,<Z> radius=<R> live=<L> visible=<V> activated=<A> culled=<C>
```

`visible` counts the live objects the last stage-6 pass left visible — exactly
the set the scene turns into draw items.

```text
# before
engine unit=cathy28 ... live=8 ticks=60 digest=475343b4676fc5cc
cull eye=0,0,0 radius=4096 live=8 visible=0 activated=0 culled=8

# after
engine unit=cathy28 ... live=8 ticks=60 digest=b8c30d28e0c67130
cull eye=-2849,5455,11505 radius=4096 live=8 visible=8 activated=8 culled=0
```

`under1` is byte-identical in both runs, eye included:
`digest=ede39c540a10ee39`, `cull eye=0,0,0 ... visible=2 ...`.

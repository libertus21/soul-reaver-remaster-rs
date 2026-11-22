# `--play --group <PREFIX>`

## What it loads

`--play` normally takes `--unit <NAME>` and loads exactly one archive unit.
Multi-room levels are stored as a family of units whose names are one prefix
plus ASCII digits (`under1`..`under4`, `cathy1`..`cathy76`). `--group` loads the
whole family.

Selection and ordering are the existing shared helpers
`level::discover_group` / `group_member` / `group_rank`, the same code behind
`--level-objects --group`, `--level-geometry --group` and
`--level-render --group`. There is no second discovery implementation. Matching
is ASCII case-insensitive, only members whose suffix is non-empty and all
digits are kept, and the family is ordered by numeric suffix (`under10` follows
`under9`).

`--unit` and `--group` are mutually exclusive: passing both is a typed
`PlayError::UnitAndGroup` (exit 1), never a silent override.

## What the world contains

- Every selected room's placed records enter one placed database, in family
  order. The spawn policy is unchanged, so markers stay untracked and unresolved
  models are counted, not spawned.
- Every selected room's room triangles are merged into one collision mesh with
  face indices rebased by the running vertex base. Walls, floors and ceilings
  from all rooms collide.
- The player binds the **first `raziel` placed record in family order**, which is
  the first room's record. This is `FrameState::new`'s existing rule; a group
  does not add a second binding path.
- Static room geometry (the windowed scene) is the first member's, addressed by
  `LevelData::entry`. The live world and collision are the whole family. Loading
  every room's render mesh is out of scope because family members may have no
  room geometry (for example a marker-only final room), which the merged-mesh
  reader rejects.

Per-actor `.SRA` resolution and the label store are unchanged: they iterate the
live list by name, so they cover every resolved actor from every room.

## Headless contract

The line format is unchanged and stays parseable:

```text
engine unit=<name> rooms=<R> placed=<P> admitted=<A> tracked=<T> skipped_markers=<M> inert=<I> unresolved=<U> live=<L> ticks=<N> digest=<16-lowercase-hex>
```

With `--group`, `unit` is `group-<PREFIX>` (one whitespace-free token, so the
line stays parser-safe), `rooms` is the number of loaded
family members, and the remaining counters describe the merged live world. The
digest folds the merged live list, so a family run is deterministic too. The
default single-unit digest is untouched: `under1` 60 ticks still prints
`ede39c540a10ee39`.

## Known limit: the fixed 48-slot tracker

The engine keeps a confirmed fixed **48-slot** name-keyed object tracker. It
tracks distinct live names, so a family with more than 48 distinct admitted
names cannot be live simultaneously and fails with the documented engine
invariant (`exit 4`):

```text
error: engine invariant failed: The Object tracker is full MAX_OBJECTS=48.
```

`cathy` is such a family: the archive holds `cathy1`..`cathy76` (61 members),
1019 placed records and 101 distinct object names. That is the original engine's
room-streaming model: a normal session never has a whole city loaded. The group
loader is still correct; the invariant is the confirmed cap, not a loader bug.
Use a family that fits, such as `under` (4 rooms), for a live multi-room run.

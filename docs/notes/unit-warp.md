# Unit warp: swapping the loaded room at runtime

This note records the world-swap machinery that level transitions will eventually
drive. The swap itself is built and verified; **the trigger that decides when to
swap is still UNKNOWN** and is deliberately not guessed.

## What exists

One shared seam performs every swap: the loaded unit's geometry, collision mesh,
placed objects, live actors and per-actor animations are rebuilt for a target
unit, and the player keeps its identity. Three entry points call that same seam:

| Flag | Scope | Behaviour |
|---|---|---|
| `--warp-to <UNIT>` | any | Starts in the normal unit, then swaps once after the first completed tick, so the swap path is exercised rather than bypassed. |
| `--warp-at <TICK>:<UNIT>` | headless | Applies at the start of the given tick. Repeatable (`--warp-at 60:cathy28 --warp-at 120:skinnr4`); entries are sorted and each fires once. |
| `--warp-cycle <U1,U2,...>` | window | `G` walks the list, one swap per press. This is the room-browser tool. |

An unknown unit name is a typed error that lists example units resolved from the
archive.

## Player placement rule

`warp::placement` resolves where the player lands, in this order:

1. the target unit's `raziel` placed record;
2. otherwise the first placed record of the target unit;
3. otherwise the origin.

The console line distinguishes the fallback case: `SpawnMissing` takes precedence
over the trigger token, so a swap that had to fall back never reads as a normal
portal-equivalent swap.

## Console contract

One line per swap:

```
warp from=<unit> to=<unit> player=<X,Y,Z> reason=<flag|key|script|spawn-missing>
```

In headless mode each unit segment is announced before its acceptance line, so a
multi-unit script is auditable:

```
warp segment=0 unit=under1
engine unit=under1 ... ticks=60 digest=ede39c540a10ee39
warp from=under1 to=cathy28 player=-2849,5455,11510 reason=script
warp segment=1 unit=cathy28
engine unit=cathy28 ... ticks=60 digest=d9355ca196d7a369
```

A swap is a pure function of the tick schedule: no wall-clock, no hash order, no
randomness. The same script produces the same segments and the same digests.

## Digest behaviour

Without any warp flag nothing changes. Verified on the current tree:

| Run | Digest |
|---|---|
| `under1` (60 ticks) | `ede39c540a10ee39` |
| `cathy28` (60 ticks) | `3e8293c0e38afac1` |

With a warp, each segment reports its own digest. Note that the second segment of
a script is **not** byte-equal to a fresh run of that unit, because the tick
counter continues across the swap and the animation and synthetic-patrol phases
are functions of the global tick. That is expected, not a defect: the pair
`(script, digest sequence)` is the reproducible artifact.

## What is still UNKNOWN

**No trigger is known.** The evidence so far:

- `portal` objects are **realm-shift** portals (material to spectral), not level
  warps. Their `type` field takes only four values (`0`, `3`, `6`, `12`), which is
  an enum of portal kinds, not a unit reference. Crossing one changes realm, not
  room.
- A unit link graph is decoded (855 edges, every one resolving to a real unit),
  but it has no per-object binding: only 15 of 136 units have a 1:1 portal-to-link
  cardinality, so ordering does not explain the mapping.
- The level-transition object appears to be `warpg`: exactly two per unit, present
  in only nine units (`cathy58`, `cathy62`, `under3`, `city6`, `oracle1`,
  `aluka43`, `out10`, `chrono17`, `stone15`). **None of those units is reachable
  from the game's debug level menu**, so the runtime trace of a `warpg` crossing
  cannot be performed without progressing the game first.

When that binding is found, the only change required is to call the existing swap
seam with the resolved target unit; placement, determinism, digest reporting and
the console contract are already in place.

## Limits

- The swap replaces the whole loaded unit. There is no streaming of neighbouring
  rooms; `--group` still loads a family eagerly.
- A swap does not carry sub-unit state (a door left open, an item taken); the
  target unit is rebuilt from its placed records.
- `--warp-cycle` is windowed only: it exists to browse, not to script.

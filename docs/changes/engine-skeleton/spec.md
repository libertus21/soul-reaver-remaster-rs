# Spec: engine-skeleton (first milestone acceptance)

The first milestone is a deterministic headless tick plus an optional textured
window. It proves the world model, the 48-slot tracker, the class hooks, the
fixed clock, the name/data seam and the renderer boundary. It does not prove
actor AI, animation or collision.

## CLI contract

```text
cargo run -- --play --bigfile <BIGFILE.DAT> [--textures <TEXTURES.BIG>]
                 [--unit <NAME>] [--ticks <N>] [--headless]
                 [--camera orbit|fly|player]
```

- `--unit` defaults to `under1`; the unit is resolved by its self-declared name,
  never by a hardcoded archive index.
- `--headless` runs the fixed clock without a window. `--ticks` defaults to 60
  in headless mode; in windowed mode it is an optional frame cap (0 = unlimited).
- `--camera` defaults to `fly`; `player` follows the `raziel` live object.
- Unknown flags, a missing `--bigfile`, or `--ticks` without a positive integer
  are usage errors (exit 1).
- `--play` must not change the output, exit codes or behaviour of any existing
  command.

## Headless output contract

Exactly one line on stdout, then exit 0:

```text
engine unit=<name> rooms=<R> placed=<P> admitted=<A> tracked=<T> skipped_markers=<M> inert=<I> unresolved=<U> live=<L> ticks=<N> digest=<16-lowercase-hex>
```

| Field | Meaning |
|---|---|
| `rooms` | Room/unit count merged into the world (1 for a single unit). |
| `placed` | Placed-object records read from the level. |
| `admitted` | Records whose name the spawn policy admitted to the tracker. |
| `tracked` | Tracker slots that reached `Ready` and spawned a live object. |
| `skipped_markers` | Names with no model container (`campath`, `marker`, `splob`) that were deliberately not tracked. |
| `inert` | Names the policy classified as inert; recorded, not admitted. |
| `unresolved` | Admitted names whose model could not be loaded; counted, never fatal. |
| `live` | Live objects after the final tick, including the player. |
| `ticks` | Fixed simulation ticks executed. |
| `digest` | FNV-1a 64 over the canonical state stream below. |

Canonical digest stream (little-endian, no padding):

1. `ticks` as `u64`.
2. For each live object in live-list order: `class` as `u8`,
   `position.x/y/z` as `i32`, `visible` as `u8`.

The digest depends only on the tick count, the loaded world and the input
sequence. It must not depend on wall-clock time, hash-map iteration order or
GPU state.

T10 makes this the production digest: `--play --headless` computes it with
`engine::determinism::world_state_hash` over the real `FrameState` live world
and prints it as the `digest` field. There is no second hashing scheme. The
tracker slot is deliberately **not** in this stream; the 48-slot tracker has
its own independent fold (`tracker_hash`): for each slot in index order, the
slot index as `u32`, the state as `u8` (`0` empty, `1` loading, `2` ready) and
the name as length-prefixed bytes (empty for an empty slot). It pins
acquisition, duplicate-name sharing and lowest-free reuse; it does not enter
the world-state digest.

## Acceptance scenarios

### S1 - Headless load of `under1`

Run `--play --headless --bigfile <BIGFILE.DAT> --textures <TEXTURES.BIG>
--unit under1 --ticks 120`.

- Exit 0; one line matching the contract.
- `rooms >= 1`, `placed > 0`, `tracked > 0`, `live >= tracked`.
- `placed == admitted + skipped_markers + inert` (the four counters partition
  the placed records; `tracked + unresolved <= admitted`).
- Two consecutive runs produce the same `digest`.
- No window is created, no file is written and no game byte is copied.

### S2 - Textured window

Run the same command without `--headless`, with `--textures`.

- Exit 0 on `Esc` or window close; window title contains `engine under1`.
- Room geometry and object models are textured from the page bank; untextured
  faces stay flat, never dropped.
- No panic and no partial frame on a missing texture page: a missing page is a
  loud error before the window opens (exit 2).
- A manual playtest note is added to the PR body.

### S3 - Tracker cap is preserved

A synthetic level admits 49 distinct names.

- The 49th `acquire` returns `TrackerError::Full`.
- The message is exactly `The Object tracker is full MAX_OBJECTS=48.`
- The CLI integration fixture exits 4 (engine invariant), never panics and
  prints no partial success line.

### S4 - Marker names never enter the tracker

A synthetic level places `campath`, `marker`, `splob` and one real model.

- `skipped_markers == 3`, `tracked == 1`.
- No tracker slot holds a marker name.
- The real model still spawns and ticks.

### S5 - Name-keyed spawning and data-keyed AI do not leak

Unit-level contract:

- `SpawnPolicy::admit` and `EngineNameTable` are the only code allowed to read
  the roster, boss/special and always-spawn lists.
- `ActorProfile` contains no name field; `ActorRuntime::new` takes only the
  profile and runtime state.
- Two models with byte-identical actor profiles produce equal
  `ActorRuntime::initial_state()` regardless of model name.
- Renaming a model does not change its profile or its initial actor state.

### S6 - Determinism

`--headless --ticks 120` twice yields identical output including `digest`.
`--ticks 121` changes `digest` (the tick count is part of the stream).
No input, no wall-clock reads inside the simulation.

### S7 - Fixed step is bounded

`SimClock::advance(10 s)` yields at most `MAX_CATCH_UP` simulation ticks
(default 5), keeps the remainder for the next frame, and never loops
unboundedly. A single tick is exactly `1/60 s` as a rational value.

### S8 - CLI coexistence

- `--help` lists the new `game play` capability and all 32 existing
  capabilities.
- Every existing arm keeps its exact prior output and exit codes.
- `cargo test` stays green (baseline 328 + new tests).
- `--play --bogus` exits 1 with a usage message.

### S9 - Class hooks match the six-class table

- `classify("raziel__")` -> `Raziel`; `classify("monster_...")` -> `Monster`;
  the four other prefixes map to their classes; an unknown prefix maps to
  `Unclassified`.
- `Raziel` and `Monster` have a non-null `tick`; `Sreavr`, `GlyphIcon`,
  `Physical` and `Particle` have `tick == None`.
- The event and query slots are present or absent exactly as the table says.
- A `None` tick is an opt-out, not an error.

### S10 - Player input

- With zero input, the `raziel` position is unchanged after 60 ticks.
- With `forward = 1` for 60 ticks, the position changes by the expected
  integer delta and the digest changes.
- Input is sampled at tick boundaries; a burst of events between ticks cannot
  change the number of ticks executed.
- Collision is absent: the player may pass through geometry in this milestone.

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success (headless completion or clean window close). |
| 1 | Usage / unknown flag / malformed option. |
| 2 | Asset, archive, parse or load error (loud, before any window). |
| 3 | Refused path (existing repository convention). |
| 4 | Engine invariant: tracker full, live-arena corruption, impossible state. |

## Non-goals asserted by this spec

No actor decision, no animation playback, no collision solver, no contact or
hit resolution, no trigger query, no script unit, no audio, no streaming, no
particles, no menu, no save, no warp, no cutscene, no orientation, no async
loading, no ECS.

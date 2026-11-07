# Determinism

Determinism is the premise of the engine, not an afterthought. The original
advances on a fixed 60 Hz rational tick (`num/den`, no float accumulation) over
plain structs, ordered arenas, a fixed 48-slot tracker and an explicit live-list
order. We reproduce that shape so the same inputs always produce the same
observable state.

A determinism regression fails silently: the game still runs, and the break is
only visible when a run is compared against the original or an earlier build.
By then several slices are built on top of it. That is why the gate exists from
the first slice with a tick loop.

## Rules the gate enforces

1. No `HashMap` iteration in tick paths. Iterate `BTreeMap`/sorted keys, or the
   ordered arena and live-list order. Lookup by key is fine; iterating is not.
2. No `f32`/`f64` where the original uses integers or rationals. Time is
   `ObjectTick { num, den }`; positions and counters stay integral.
3. No unordered parallelism in the tick. Systems run in the documented stage
   order, sequentially.
4. No `SystemTime`/`Instant` reads in the simulation, no unseeded RNG, and no
   `Vec` ordering that depends on allocation or hash order. The caller feeds the
   clock; the state stream is written in live-list order.

## Cross-platform path caveat

`write_path` hashes `Path::as_os_str().as_encoded_bytes()`, which is the
OS-native encoding: raw bytes on Unix (locale dependent) and WTF-8 on Windows.
FNV-1a itself is byte-exact on every platform, but the input bytes are not, so
the same non-ASCII path can hash differently across operating systems.

The determinism gate is therefore **same-platform, same-build**. Comparing
digests of non-ASCII paths across platforms is **out of scope**. That is
acceptable because the original is a Windows game and the gate runs against it
on Windows; ASCII paths, which the CLI contract uses, are byte-identical
everywhere. Normalizing the path encoding is a design decision for a later
slice, not part of this gate.

## Gate policy per slice

- T2 (first tick loop): **hard gate**. A test must run N ticks twice and assert
  an identical state hash. T2 must not merge without it.
- T3 (placed database + world construction): no tick loop and no
  `WorldState.live` write, so the canonical fold and
  `GOLDEN_WORLD_STATE_HASH` stay unchanged.
- T5 (live arena + class hooks): the arena is not wired into `WorldState.live`
  yet, so the canonical fold and `GOLDEN_WORLD_STATE_HASH` stay unchanged too.
  T5 adds a separate live-order replay (`src/engine/determinism.rs`) that pins
  slot index, generation and order across two identical spawn/remove runs; it
  is an independent observation, not part of the world golden.
- T6 (name-keyed spawn policy): no tick and no `WorldState` write, so the four
  existing goldens do not move. A fifth, independent gate,
  `GOLDEN_CLASSIFICATION_HASH`, pins every name in the five lists in documented
  order: list id, exact count, name bytes, admission, load role and asset tree.
  Renaming, removing, adding or reordering an entry breaks it.
- T7 (asset source + actor profile): no tick, no `WorldState` write and no
  frame pipeline, so all five existing goldens stay unchanged. A sixth,
  independent gate, `GOLDEN_ACTOR_RUNTIME_HASH`, pins the initial
  `ActorRuntime` derived from an `ActorProfile`: profile kind, the eight trios,
  the entry-array kind and the entry ids, then the numeric relation, state,
  target and flags. The profile has no name field, so renaming a model does not
  move the fold. It lives in `src/engine/determinism.rs` tests.
- T8 (18-stage frame pipeline): the frame driver consumes the existing fold and
  does not change it, and there is still no CLI digest, so **all six existing
  goldens are untouched**. T8 adds a seventh, independent gate: the 18 stage
  numbers and names are pinned by `GOLDEN_FRAME_STAGES`, and a scripted 40-tick
  run is replayed twice comparing the stage trace and the per-tick world hash;
  `GOLDEN_FRAME_STATE_HASH` pins the final state. It lives in
  `src/engine/determinism.rs` tests.
- T9 (render scene + backend): the scene is a CPU snapshot of the existing live
  order and does not write `WorldState`, so **all seven existing goldens are
  untouched**. A separate scene replay builds the same live state twice and
  compares the whole `RenderScene` (draw order, handles, meshes and material
  table); making the live walk unordered fails it. See
  `docs/notes/engine-render.md`.
- T10 (headless acceptance): the canonical fold becomes the REAL production
  digest. `--play --headless` prints one line over the real `FrameState` live
  world and `GOLDEN_WORLD_STATE_HASH` is re-pinned deliberately from the T2
  synthetic vector to an end-to-end real-world run. A second, CLI-path golden
  (`GOLDEN_HEADLESS_DIGEST`) pins a synthetic-archive acceptance run. The
  options, error, tracker, classification, actor-runtime, stage-order and
  frame-state goldens do not move. See "Headless acceptance (T10)" below.
- engine-actor-tick (actor tick plumbing): stage 12 now dispatches the class
  tick hook and a live object may carry an `ActorRuntime`. The canonical fold
  gains, per live object in live order, an actor-presence byte plus the actor
  relation and action state bytes. `GOLDEN_WORLD_STATE_HASH` moves from
  `0xb5bb_59fd_97b9_50b1` to `0x2c8e_61aa_2f02_31b0`;
  `GOLDEN_FRAME_STATE_HASH` moves with it (`0x7b7f_16bc_a6af_b9b2` to
  `0xa203_0add_093d_a5e6`) because it pins the same fold; and
  `GOLDEN_HEADLESS_DIGEST` moves from `0x8dd6_3315_4e26_f1c4` to
  `0x490c_8582_c297_df18`. The options, error, tracker, classification,
  actor-runtime and stage-order goldens do not move. See "Actor tick
  (engine-actor-tick)" below.
- Every later slice re-runs the determinism test unchanged. A slice that
  changes the canonical state stream updates `spec.md` and the test in the same
  PR.
- The digest is FNV-1a 64 over the canonical stream in
  `docs/changes/engine-skeleton/spec.md`; the harness lives in
  `src/engine/determinism.rs`.

## Golden model

The harness keeps independent folds, because "how the engine was launched",
"what the simulation is" and "how the tracker allocated slots" are different
questions:

| Golden | Fold | Guards |
|---|---|---|
| `options_hash` / `error_hash` | parsed `PlayOptions`, typed error text | **Regression gate**: argv parsing, fold order, field set, error tags. No simulation state. |
| `world_state_hash` | tick count, live-list order, per-entry class, position, visible, actor presence, actor relation and actor state | **Canonical state gate** and the production `--play --headless` digest: the simulation contract from `spec.md`. Independent of the options fold. |
| `tracker_hash` | each slot in index order: index, state code, name | **Tracker gate**: acquisition, duplicate-name sharing and lowest-free reuse. Independent of both folds above. |
| `classification_hash` | the five name lists in documented order: list id, exact count, then per name the name bytes, admission code, load-role code and asset-tree code | **Name-table gate**: the exact membership and roles of the spawn policy. Independent of every fold above; lives in `src/engine/spawn.rs` tests. |
| `actor_runtime_hash` | profile kind, the eight trios, entry-array kind, entry ids; then relation, state, target and flags | **Actor-runtime gate**: the data-keyed AI initial state. No name is folded. Independent of every fold above; lives in `src/engine/determinism.rs` tests. |

`tracker_hash` folds all 48 slots, empty ones included, so a release is
observable and a name's slot index is part of the contract, not just its
presence. `GOLDEN_TRACKER_HASH` pins that byte stream; it is a new constant and
does not move the three existing goldens.

T1 shipped a single `state_hash` that mixed the options fold with the notion of
state. That was a tripwire, not a gate: changing the argv could move the "state"
golden even though no state existed. T2 **retires** it instead of re-pinning it
to a new number.

The canonical world fold, in order: `tick` as `u64`, then for each live entry in
**live-list order** (spawn order, survivors keep relative order after removals)
`class` as `u8`, `position.x/y/z` as `i32`, `visible` as `u8`, the
actor-presence byte as `u8`, the actor `relation` as `u8` and the actor action
`state` as `u8`. Presence is `1` only when the live object carries an
`ActorRuntime`; when it is `0`, relation and state are the neutral `0` and the
presence byte already states that the fields are absent. `GOLDEN_WORLD_STATE_HASH`
pins that exact byte stream. T3+ extends the fold and the constant deliberately,
in the same PR that updates `spec.md`; it is never "fixed" to go green.

## Name-table classification (T6)

`EngineNameTable` owns the always-spawn (10), actor roster (10), boss/special
(8), E3 override (22) and marker-only (3) lists. Membership is ASCII
case-insensitive, matching the level name resolver. The four load-complete
literals (`MORLOCK`, `RONINBSS`, `ALUKABSS`, `SKINBOS`) are exact, including
case, because the evidence records `MORLOCK` as exact upper-case.
`SpawnPolicy` exposes only admission, load role and asset paths; it has no
actor method and loads nothing.

`GOLDEN_CLASSIFICATION_HASH` lives in `src/engine/spawn.rs` tests. It folds, for
each list in documented order, the list id and exact count, then per name the
name bytes, the `Admission` code, the `LoadRole` code and the asset-tree code.
It does not read the canonical world state and does not move any existing
golden.

## Live order (T5)

The live arena keeps three things separate: the generational slot array, the
free list, and an explicit `order: Vec<LiveId>`. Iteration follows `order`, so:

- the live order is spawn order, and removing an object never reorders the
  survivors;
- a reused slot bumps its generation, so an old [`LiveId`] is rejected as stale
  instead of silently pointing at the new occupant;
- a free-list reuse appends the new object at the end of the live order even
  when its slot index is in the middle of the array.

Replaying the same spawn/remove sequence twice therefore yields byte-identical
`(slot index, generation)` tuples in the same order. T5 does not feed this arena
into `WorldState.live`, so the canonical world golden does not move; the replay
is a separate gate in `src/engine/determinism.rs`.

## Actor runtime (T7)

The actor seam is data-keyed, not name-keyed. `ActorProfile` copies the kind,
the eight state trios, the behaviour-entry ids and the entry-array kind from the
shipped per-model block through the existing `actor::parse_actor_block`; it has
no name field. `ActorRuntime` embeds the profile and adds only numeric state:
relation and action state (raw bytes, no invented enum), an optional live target
and a flag word.

The initial state is derived from the profile alone, so two models with
byte-identical actor blocks start in equal runtime state even when their asset
names differ. `actor_runtime_hash` folds the profile data first, then the
numeric runtime fields, in a fixed order. `GOLDEN_ACTOR_RUNTIME_HASH` pins that
byte stream; it is a sixth, independent gate and moves none of the five existing
goldens. A deliberate change to the initial action state (0 -> 1) moves the hash
from `0xe169_695d_0eda_ec99` to `0x04d3_052a_20e9_2660`, which is the evidence
that the gate bites.

The asset source is synchronous in this milestone (`ImmediateSource` resolves a
name through the bigfile index with the existing container and actor parsers and
reuses the existing textured-mesh builder). `LoadQueue` is only a deterministic
FIFO shape for a later asynchronous source; it adds no thread, runtime or
dependency, and a missing model is `Unresolved`, never a panic.

## Actor tick (engine-actor-tick)

The actor carrier is now part of the frame loop, still without AI semantics.

- **Dispatch is the class table.** Stage 12 looks up the tick hook for the live
  object's render class. `raziel__` and `monster_` install one; `sreavr__`,
  `glphicon`, `physical` and `particle` leave it null. A null tick is the
  documented opt-out and makes zero calls.
- **Carriage is data-keyed.** A live object owns `Option<ActorRuntime>`, filled
  only from the loaded model's actor block (`ModelAsset::actor`). The bound
  player receives its profile in the shared boot pass. A model without an actor
  block stays a non-actor even when its class name would otherwise tick.
- **No semantics.** `ActorRuntime::advance` is the single transition entry
  point and returns `ActorTransition::NotInterpreted`. It changes no relation
  and selects no action id. The eight trios and the behaviour-entry ids remain
  profile data and are never consumed by ticking.
- **State is folded.** The canonical stream adds the presence byte and, when a
  carrier exists, the relation and state bytes. Non-actors fold `0/0/0`.
- **The gate bites.** Dropping `hasher.write_u8(entry.state)` moves the
  world-state gate to `0xccb5_59a4_77be_1118`, the frame-state gate to
  `0xd710_2563_f731_9382` and the headless gate to `0xf51c_9abc_d024_b374`. The
  perturbation was reverted before the commit.
- **Real data.** A 60-tick `--play --headless` run on the shipped `under1` unit
  (read-only, nothing committed) prints `digest=3ac33bf08aab697d` on two
  consecutive runs. The pre-slice digest was `fe951463efc0dbec`; the live set,
  counters and tick count are unchanged, so the move is the fold extension.
  The real run attaches one actor carrier (`raziel`); `under1` has no monster
  placement.

## Frame pipeline (T8)

The frame pipeline is where determinism actually breaks in practice: the same
input can look stable while the stage order, the input sampling point or the
double player update silently changed. `engine/frame.rs` therefore has one
ordered definition, `STAGE_ORDER`, and one driver, `FrameState::run_tick`.

- Stages 3, 4, 5, 7, 10, 11, 13, 14, 16 and 17 are named no-ops. Stage 8 is a
  named no-op too: the original four-step move/collide/commit solver is out of
  scope and direct input translation lives in stage 9.
- Input is sampled once per tick boundary, inside stage 1. The clock is fed by
  the caller and never reads input, so 1 and 100 queued events inside the same
  wall-time window execute the same number of ticks.
- The player is updated twice: directly by stage 9 and again as an ordinary
  live object in the stage 12 generic pass. This is the documented original
  double update and the gate must preserve it.

`GOLDEN_FRAME_STAGES` pins the 18 `(code, name)` pairs in order; swapping any
two stages fails `frame_stage_order_matches_the_golden_table`. The scripted
replay in `frame_pipeline_replays_stage_order_and_state_identically` runs the
same input script over the same tick sequence twice and compares, per tick, the
full stage trace and the canonical world hash, then pins the last hash with
`GOLDEN_FRAME_STATE_HASH`. `FrameState::world_state()` reuses
`world_state_hash` as a pure function; neither the fold nor any existing
constant moves.

## Clock: rational step, no drift

Time is a fixed rational step (`ObjectTick { num: 1, den: 60 }`), never `f32`.
`SimClock` accumulates elapsed time in thirds of a nanosecond so the `1/60 s`
step is an exact integer; no float touches time. One `advance` emits at most
`MAX_CATCH_UP` (5) ticks and keeps the unconsumed remainder in the accumulator.
A 10 s advance therefore emits 5 ticks and keeps `10 s - 5 * (1/60 s)`, and later
frames pay the rest back: the cap delays ticks, it never loses them. Ticks are
integer-indexed and monotonic.

## Headless acceptance and the production digest (T10)

T10 closes the milestone by making the canonical fold the **real end-to-end
gate**. `--play --headless --ticks N` prints exactly one line on stdout and
exits 0:

```text
engine unit=<name> rooms=<R> placed=<P> admitted=<A> tracked=<T> skipped_markers=<M> inert=<I> unresolved=<U> live=<L> ticks=<N> digest=<16-lowercase-hex>
```

`digest` is `world_state_hash(&frame.world_state())`: FNV-1a 64 over the tick
count and, in live-list order, each entry's class, integer position and visible
byte. There is no second hashing scheme. The tracker slot is deliberately not
part of this fold; `tracker_hash` owns slot allocation independently.

The boot pass walks the authored placements exactly once
(`PlacedDb::records_for_boot`, never from the tick loop), classifies them with
`EngineNameTable`, skips marker-only names, counts inert and unresolved names,
and defers resolved spawns to stage 18. Marker names never reach the tracker.
The 48-slot cap is therefore reachable end-to-end: a 49th distinct live name
returns `FrameError::Tracker`, which maps to exit code 4 and prints no partial
success line.

Goldens:

- `GOLDEN_WORLD_STATE_HASH` moved deliberately from the retired T2 synthetic
  vector `0x82fe_8a38_1b9d_2a6d` to the real `FrameState` end-to-end run
  `0xb5bb_59fd_97b9_50b1`. It now covers a real live world with a culled
  object, so dropping the `visible` byte fails the gate: the perturbed digest is
  `0xe34dc760aae69953`.
- `GOLDEN_HEADLESS_DIGEST` (`0x8dd6_3315_4e26_f1c4`) pins the full CLI-path
  run over a synthetic archive; dropping `visible` moves it to
  `0x6094208b15fb134e`.
- The options, error, tracker, classification, actor-runtime, stage-order and
  frame-state goldens did not move: none of their fold inputs changed.

Wall-clock independence is proved twice. The simulation sources (`clock`,
`frame`, `world`, `placed`, `live`, `class`, `input`, `tracker`, `spawn`,
`assets`, `actor`, `state`) must not name `Instant` or `SystemTime`;
`simulation_path_never_names_a_wall_clock` enforces that. The frame loop in
`play.rs` may read time only to decide how many ticks are owed, never to compute
state. `equal_tick_counts_ignore_the_wall_clock_schedule` reaches the same
120-tick state through 120 exact `1/60 s` frames and through one 2 s advance
plus capped drains.

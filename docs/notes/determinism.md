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
| `world_state_hash` | tick count, live-list order, per-entry class, position, visible | **Canonical state gate**: the simulation contract from `spec.md`. Independent of the options fold. |
| `tracker_hash` | each slot in index order: index, state code, name | **Tracker gate**: acquisition, duplicate-name sharing and lowest-free reuse. Independent of both folds above. |

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
`class` as `u8`, `position.x/y/z` as `i32`, `visible` as `u8`. `GOLDEN_WORLD_STATE_HASH`
pins that exact byte stream. T3+ extends the fold and the constant deliberately,
in the same PR that updates `spec.md`; it is never "fixed" to go green.

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

## Clock: rational step, no drift

Time is a fixed rational step (`ObjectTick { num: 1, den: 60 }`), never `f32`.
`SimClock` accumulates elapsed time in thirds of a nanosecond so the `1/60 s`
step is an exact integer; no float touches time. One `advance` emits at most
`MAX_CATCH_UP` (5) ticks and keeps the unconsumed remainder in the accumulator.
A 10 s advance therefore emits 5 ticks and keeps `10 s - 5 * (1/60 s)`, and later
frames pay the rest back: the cap delays ticks, it never loses them. Ticks are
integer-indexed and monotonic.

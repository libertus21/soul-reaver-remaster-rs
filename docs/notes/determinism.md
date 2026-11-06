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

## Gate policy per slice

- T2 (first tick loop): **hard gate**. A test must run N ticks twice and assert
  an identical state hash. T2 must not merge without it.
- Every later slice re-runs the determinism test unchanged. A slice that
  changes the canonical state stream updates `spec.md` and the test in the same
  PR.
- The digest is FNV-1a 64 over the canonical stream in
  `docs/changes/engine-skeleton/spec.md`; the harness lives in
  `src/engine/determinism.rs`.

T1 ships the harness and the pattern only: there is no world or tick loop yet,
so `state_hash` folds the parsed `PlayOptions` and typed error text, and the
tests prove equal input yields an equal hash while different input does not.

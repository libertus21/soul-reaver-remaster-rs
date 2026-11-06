# Tasks: engine-skeleton

Planning change only. No production code lands in `docs/engine-skeleton`.

## Forecast convention

`production` = authored Rust lines (non-test). `tests` = production x 1.5
(synthetic fixtures and assertions are heavier than the code they cover).
`docs` = authored Markdown lines in that PR. `TOTAL` = production + tests +
docs. Every production PR is forecast under 400 authored lines.

## Determinism gate (policy, not optional)

**A determinism test (N ticks -> identical state hash) is a HARD ACCEPTANCE GATE
for T2. T2 must NOT be merged without it, and every later slice re-runs it.**

Why: determinism is the premise of the design (fixed 60 Hz rational tick, plain
structs + ordered arenas, fixed 48-slot tracker, live-list order). It fails
silently — the game still runs — and is only visible when comparing against the
original. See `docs/notes/determinism.md` and the harness in
`src/engine/determinism.rs`.

## Ordered tasks

### T1 - Play-mode facade and catalog entry

- Add `src/engine/mod.rs` and `src/engine/play.rs` with a pure
  `PlayOptions::parse(&[String]) -> Result<PlayOptions, PlayError>`.
- Add `--play` to `main::dispatch` before the legacy chain; unknown flags are
  exit 1. `engine::play::run` returns a typed "not implemented" error until T9
  (window) and T10 (headless).
- Add group `game` / capability `play` to `cli_catalog`; `GROUPS` -> 6,
  `CAPABILITIES` -> 33, update `Capability::takes_value` for the new options,
  and update the catalog tests.
- Acceptance: `--play --bogus` exits 1; `--help` lists `game play` and all 32
  legacy capabilities; `cargo test` stays green.
- Files: `src/main.rs`, `src/cli_catalog.rs`, `src/engine/mod.rs`,
  `src/engine/play.rs`.
- PR: B.

### T2 - Fixed clock and game state machine

- `src/engine/clock.rs`: `TICK_HZ`, `MAX_CATCH_UP`, `ObjectTick { index, num,
  den }`, `SimClock::advance(Duration) -> TickBatch`.
- `src/engine/state.rs`: `GameState` with `Boot`, `Intro`, `KainIntro`, `Menu`,
  `Logos`, `Loading { unit }`, `InGame`, `Shutdown`; explicit transition
  function; unsupported states return `EngineError::UnsupportedState`.
- Acceptance: 1/60 rational step; a 10 s advance yields at most 5 ticks and
  keeps the remainder; `Boot -> Loading -> InGame` only when loads are done.
- Gate: the determinism test (N ticks -> identical state hash) passes; T2 does
  not merge without it.
- Files: `src/engine/clock.rs`, `src/engine/state.rs`.
- PR: C.

### T3 - World and placed-object database

- `src/engine/placed.rs`: `PlacedObject { name, position: [i16; 3],
  object_type, archetype, flags, instance_id }`, `PlacedDb` with name and id
  indexes; the database is queried, never iterated per frame.
- `src/engine/world.rs`: `World`, `LevelData`, construction from a loaded unit.
- Add `entry: usize` to `level::ArchivedUnit` and
  `level::resolve_unit_entry(dat, name)`; add `archetype`/`instance_id` to
  `level::ObjectRecord` without changing existing printed lines.
- Acceptance: `under1` resolves by name; duplicate names keep separate placed
  records; no per-frame placed iteration exists.
- Files: `src/engine/placed.rs`, `src/engine/world.rs`, `src/level.rs`.
- PR: D.

### T4 - 48-slot object tracker

- `src/engine/tracker.rs`: `MAX_OBJECTS = 48`, `SlotState { Empty, Loading,
  Ready }`, `TrackerSlot`, `ObjectTracker`.
- `acquire(name)` matches an existing name first, then takes the first empty
  slot; `complete`, `release`, `find`, `ready_slots`.
- `TrackerError::Full` carries the exact documented message.
- Acceptance: 49th distinct name is a typed error, never a panic; duplicate
  names share one slot; release reuses the lowest free slot; slot order is
  deterministic.
- Files: `src/engine/tracker.rs`.
- PR: E.

### T5 - Live arena, class table and hooks

- `src/engine/live.rs`: `LiveId { index, generation }`, `LiveObject`, ordered
  `LiveArena` with free list and explicit live order.
- `src/engine/class.rs`: `RenderClass`, `Hooks` (three `Option<fn>` slots),
  `CLASS_TABLE` for the six prefixes, `classify`.
- `raziel__` and `monster_` get a tick; the other four leave it `None`.
- Acceptance: live order is spawn order after removals; generation prevents
  stale ids; the class table test asserts every prefix and every null slot.
- Files: `src/engine/live.rs`, `src/engine/class.rs`.
- PR: F.

### T6 - Name-keyed spawn policy

- `src/engine/spawn.rs`: `SpawnPolicy` trait, `EngineNameTable` (always-spawn,
  actor roster, boss/special, E3 overrides, marker-only names), `Admission`,
  `LoadRole`, `AssetPaths`.
- The policy is the only name-list reader; it exposes no actor methods.
- Acceptance: the ten always-spawn names, ten roster names, eight boss/special
  names and 22 E3 names classify exactly; `campath`/`marker`/`splob` are
  `MarkerOnly`; an unknown name is `PlacedModel`.
- Files: `src/engine/spawn.rs`.
- PR: G.

### T7 - Asset source and actor-profile plumbing

- `src/engine/assets.rs`: `AssetSource` trait, `AssetStore`, `ModelAsset
  { model, textured, actor }`, `AssetRequest`/`AssetCompletion`; a synchronous
  `ImmediateSource` for the milestone and a `LoadQueue` shape for later async.
- `src/engine/actor.rs`: `ActorProfile` from `actor::parse_actor_block` and
  `ActorRuntime` with numeric relation/state, no name field.
- Acceptance: a model with an actor block yields a profile; a non-actor yields
  `None`; two models with identical profiles produce equal initial runtime
  state; a missing model is `unresolved`, not a panic.
- Files: `src/engine/assets.rs`, `src/engine/actor.rs`.
- PR: H.

### T8 - Ordered frame pipeline and player input

- `src/engine/frame.rs`: the 18 documented stages in order; stubs are named
  no-ops.
- `src/engine/input.rs`: `InputFrame` and an edge queue sampled at tick
  boundaries.
- Implement stages 1, 2, 6, 9, 12, 15, 18; stage 8 is direct input translation
  only (no four-step solver, no collision).
- Acceptance: the player is ticked directly and again in the generic pass;
  zero input leaves the position unchanged; the tick count is independent of
  event burst size.
- Files: `src/engine/frame.rs`, `src/engine/input.rs`.
- PR: I.

### T9 - Render scene, wgpu backend and windowed `--play`

- `src/engine/scene.rs`: `RenderScene`, `DrawItem`, `MeshHandle`.
- `src/engine/render_backend.rs`: `RenderBackend` and `NullBackend`.
- `src/engine/backend_wgpu.rs`: wgpu backend that rebuilds one merged CPU mesh
  per frame from the scene and reuses the existing shader/pipeline helpers.
- `src/engine/play.rs`: winit window loop, orbit/fly/player camera, `Esc`
  exits 0; headless uses `NullBackend`.
- Acceptance: `under1` renders textured; a missing page fails before the window
  (exit 2); no viewer path in `src/render.rs` changes.
- Files: `src/engine/scene.rs`, `src/engine/render_backend.rs`,
  `src/engine/backend_wgpu.rs`, `src/engine/play.rs`.
- PR: J.

### T10 - Headless acceptance and determinism

- Implement the canonical digest (FNV-1a 64 over tick count + live order +
  class + position + visible).
- `--headless --ticks N` prints the one-line contract and exits 0.
- Add integration tests with synthetic archives: counts, marker skip, tracker
  cap exit 4, determinism, fixed-step bound.
- Acceptance: the same run twice yields the same digest; `--ticks` changes it;
  no wall-clock read in the simulation.
- Files: `src/engine/play.rs`, tests.
- PR: K.

### T11 - README and milestone report

- Add a `--play` row to `README.md` "What works" and a short
  `docs/changes/engine-skeleton/milestone-report.md` (what was seen, what
  remained stubbed).
- Acceptance: README stays current per repo rule; the report contains no game
  bytes, no decompiled listings and no absolute install paths.
- Files: `README.md`, `docs/changes/engine-skeleton/milestone-report.md`.
- PR: L.

## Forecast split

| Task | PR | Production | Tests (x1.5) | Docs | TOTAL |
|---|---|---:|---:|---:|---:|
| T1 | B | 90 | 135 | 10 | 235 |
| T2 | C | 80 | 120 | 10 | 210 |
| T3 | D | 90 | 135 | 10 | 235 |
| T4 | E | 110 | 165 | 10 | 285 |
| T5 | F | 130 | 195 | 10 | 335 |
| T6 | G | 100 | 150 | 10 | 260 |
| T7 | H | 120 | 180 | 10 | 310 |
| T8 | I | 130 | 195 | 15 | 340 |
| T9 | J | 140 | 210 | 10 | 360 |
| T10 | K | 70 | 105 | 15 | 190 |
| T11 | L | 0 | 0 | 40 | 40 |
| **Total** | | **1,060** | **1,590** | **150** | **2,800** |

## PR chain plan

Stacked slices; each branch starts at the previous PR head and merges in order.
Every PR body states `cargo test` and, for rendering slices, the manual
playtest (file and what was seen).

| PR | Branch | Base | Content | Boundary |
|---|---|---|---|---|
| A | `docs/engine-skeleton` | `main` | proposal/spec/design/tasks | Docs only; no production code. This is the required planning PR. It exceeds 400 lines because it carries four artifacts; if the 400-line gate is applied to docs, split into A1 (proposal+spec) and A2 (design+tasks) before merge. |
| B | `feat/engine-cli-facade` | A | T1 | CLI + catalog + empty engine facade; no world, no renderer. |
| C | `feat/engine-clock-state` | B | T2 | Time and state only; no world. |
| D | `feat/engine-world-placed` | C | T3 | Placed database + level name resolution; no tracker. |
| E | `feat/engine-tracker` | D | T4 | Tracker only; no live objects. |
| F | `feat/engine-live-hooks` | E | T5 | Live arena + class hooks; no spawn policy. |
| G | `feat/engine-spawn-policy` | F | T6 | Name table + policy; no asset loading. |
| H | `feat/engine-assets-actor` | G | T7 | Asset source + actor profile; no frame loop. |
| I | `feat/engine-frame-pipeline` | H | T8 | 18 stages + player input; no renderer. |
| J | `feat/engine-render-backend` | I | T9 | Scene + wgpu backend + window; no headless digest. |
| K | `feat/engine-headless-acceptance` | J | T10 | Digest + integration tests; no new features. |
| L | `docs/engine-milestone` | K | T11 | README + report only. |

## Commit sequence (per PR)

- `feat(engine): ...` for the production slice.
- `test(engine): ...` for the synthetic fixtures and assertions.
- `docs(engine): ...` only when the PR carries docs.

Conventional Commits, no AI attribution. `git status --short` is checked before
each commit; `.codegraph/`, game bytes and extracted files are never staged.

## Definition of done

- Every PR: `cargo test` green, no regression in the 32 legacy capabilities,
  whitelist `.gitignore` respected, no game bytes or absolute install paths.
- Rendering PRs: a manual playtest note in the PR body.
- The chain ends with a windowed textured `under1` demo and a deterministic
  headless line, with actor AI still absent and the seam ready for it.

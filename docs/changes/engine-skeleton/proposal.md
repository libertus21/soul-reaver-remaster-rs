# Proposal: engine-skeleton (headless core + renderer backend)

## Intent

The repository has a complete read-only asset layer and a viewer, but no
runtime. The goal is the smallest architecture that can grow into the game
engine without being rewritten when actor AI, animation and collision arrive.
The skeleton must make the next layer additive: adding actor AI should fill a
stage and a hook, not reshape the world, the spawner or the frame loop.

This is a planning change. No production code lands here.

## Scope

In scope:

- A headless simulation core that owns time, state, the placed-object database,
  the 48-slot tracker, live objects, the class hook table and a fixed frame
  pipeline.
- A name-keyed spawn layer and a data-keyed actor-profile layer, separated by a
  type boundary so neither leaks into the other.
- A `--play` mode that coexists with every existing CLI capability.
- A renderer boundary: the simulation emits a CPU scene; a wgpu backend draws
  it. The existing viewer paths stay untouched.
- The first demoable milestone: load `under1`, instantiate its placed objects
  through the tracker, tick a fixed number of frames, render textured, and move
  `raziel` with input.

Out of scope (deliberately not in the first milestone):

- Actor AI decisions, the shared numeric state machine, perception, engagement,
  attacks and damage.
- `.SRA` animation playback and the `0x6D00` animation block interior.
- Collision, the player's four-step movement solver, contact resolution, hit
  lists and trigger queries.
- Script/event units, audio sequencing, streaming, particles, menus, the full
  intro sequence, save/load, warps and cutscenes.
- Object orientation. Placed records carry position only; no rotation is
  invented.
- Async asset streaming. The tracker keeps its `Loading` state and load queue,
  but the milestone completes loads synchronously.
- ECS, a scripting runtime, or a new asset format.

## Decisions in one line each

| # | Decision | Why |
|---|---|---|
| 1 | Plain structs + ordered arenas, not ECS | The original is flat arrays, a live list and per-object hooks; its order is observable. ECS would hide the frame order and per-object identity that matching depends on. |
| 2 | Three explicit keying axes: spawn policy (name), render class (prototype prefix), actor profile (model data) | The notes prove spawning is name-keyed, hooks are class-keyed and AI is data-keyed. Modelling them as one "type" system is the documented trap. |
| 3 | `SpawnPolicy` trait fed by an immutable `EngineNameTable`; `ActorProfile` loaded per model | The spawner may consult names; the AI may not. The seam is a type boundary, not a convention. |
| 4 | Function-pointer hook table (event/tick/query), `None` = opt-out | Mirrors the original three slots and the six-class table, keeps null semantics explicit, and costs no per-object allocation. |
| 5 | Fixed 60 Hz simulation step, rational `ObjectTick` | Determinism. Variable dt makes AI and root motion impossible to compare across runs. |
| 6 | Renderer stays a backend; simulation is wgpu/winit-free | Protects the 328-test asset layer and the 30 CLI viewers. The engine core can be tested headless. |
| 7 | `--play` is a new top-level mode; legacy dispatch stays | Zero regression risk for existing commands; the CLI can be table-driven later. |
| 8 | First milestone is a deterministic headless tick plus an optional textured window | Demoable, testable in CI with synthetic fixtures, and proves the seams before AI exists. |

## The central tradeoff: match the original vs "better" design

This rewrite exists to reproduce a 1999 engine, so **traceability beats
elegance where they conflict**. The original's observable behaviour includes:

- a fixed 48-slot tracker that fails loudly when full;
- duplicate names sharing one tracker slot;
- a live-list tick order;
- per-class null hooks that make non-actors skip the tick;
- a placed-object table that is queried, never iterated per frame.

Those are not implementation details to improve away. The Rust design keeps
them as explicit types (`[TrackerSlot; 48]`, ordered `LiveArena`, `Hooks` with
`Option` slots, `PlacedDb`). Inside those constraints we use idiomatic Rust:
newtypes for ids and time, `Result` with typed errors, no global mutable state,
and module privacy to enforce the keying seams.

The cost is less ergonomic querying and no parallel systems. That is accepted:
the world is small (tens of objects), determinism matters more, and an ECS
would make the original frame order harder to see. If a future subsystem
genuinely needs data-oriented iteration, it can be added as a derived view over
the same authoritative state, not as a second world model.

## What "extensible" means here

Adding actor AI later must be:

1. a new `ActorRuntime` field on `LiveObject` (already reserved);
2. a real body in the `monster_` tick hook (already in the class table);
3. a new stage body in the frame pipeline (already ordered and stubbed);
4. no change to the tracker, spawn policy, renderer or CLI.

That is the acceptance test for the skeleton's shape.

## Forecast split

The implementation is chained from the start. The docs pass is one docs-only
PR; production slices are stacked and each is forecast under 400 authored
lines. Full task and PR boundaries live in `tasks.md`.

## Legal

No game bytes, no decompiled listings and no absolute install paths are used in
this change. Game data is read only at runtime from the user's own copy; tests
use synthetic bytes.

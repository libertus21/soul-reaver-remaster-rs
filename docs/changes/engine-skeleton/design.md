# Design: engine-skeleton

## Technical approach

Two layers with a one-way dependency:

```text
asset layer (existing, read-only)      engine core (new, headless)        backend (new)
bigfile/level/object/actor/textures -> World + frame pipeline + scene -> wgpu window
                                              |
                                        --play / headless
```

The engine core never imports `wgpu` or `winit`. It produces a CPU
`RenderScene`; a backend draws it. The existing viewer code in `src/render.rs`
stays a separate consumer of the asset layer and is not restructured.

## Architecture decisions

### 1. World model: plain structs + ordered arenas, not ECS

| Option | Tradeoff | Decision |
|---|---|---|
| ECS (`bevy_ecs`/`hecs`) | Ergonomic queries and parallelism, but hides per-object identity and frame order; components split state the original keeps together; new dependency | Rejected |
| Plain structs + `Vec` + fixed arrays | Less ergonomic, no parallel systems, but 1:1 with the original and deterministic | **Chosen** |

`World` owns four authoritative stores:

```rust
pub struct World {
    pub level: LevelData,          // rooms, links, spawn point
    pub placed: PlacedDb,          // authored database, queried by name/id
    pub tracker: ObjectTracker,    // fixed [TrackerSlot; 48], name-keyed
    pub live: LiveArena,           // ordered live list, generational ids
    pub assets: AssetStore,        // model + actor profile cache
    pub clock: SimClock,
    pub input: InputFrame,
}
```

`PlacedDb` is a database, not a per-frame queue: `Vec<PlacedObject>` plus
name/id indexes. `ObjectTracker` is deliberately `[TrackerSlot; 48]` so the cap
and first-free-slot allocation order are observable, exactly like the original.
`LiveArena` is `Vec<Option<LiveObject>>` plus a free list and an explicit
`order: Vec<LiveId>`; iteration follows `order`, so tick order is spawn order
and removals do not reorder survivors. Duplicate names share one tracker slot.

Tradeoff stated: this is less "modern" than an ECS, but the original's
observable behaviour (48 slots, name deduplication, live order, null hooks) is
the product. ECS would make matching harder, not easier.

### 2. The two keying philosophies: three axes, two seams

The notes prove three independent axes. They must not be collapsed:

| Axis | Key | Produces | Module |
|---|---|---|---|
| Spawn policy | object **name** | admit / skip / gate / asset path / load-complete role | `engine/spawn.rs` |
| Render class | **prototype prefix** | one of six classes + three hooks | `engine/class.rs` |
| Actor AI | **model data** | `ActorProfile` (trios, entries, kind) | `engine/actor.rs` |

Seam A - name-keyed spawning:

```rust
pub trait SpawnPolicy {
    fn admit(&self, placed: &PlacedObject) -> Admission;
    fn load_complete(&self, name: &str) -> LoadRole;
    fn asset_paths(&self, name: &str) -> AssetPaths;
}
pub struct EngineNameTable { /* always-spawn, roster, boss/special, E3, markers */ }
pub enum Admission { System, Actor, BossSpecial, PlacedModel, MarkerOnly, Inert }
```

`EngineNameTable` is immutable data. `SpawnPolicy` is the only interface that
sees the name lists. The tracker and the AI module never call it.

Seam B - data-keyed AI:

```rust
pub struct ActorProfile {
    pub kind: ActorKind,             // Monster | Player
    pub trios: [[u8; 3]; 8],         // block +0x18, stored order
    pub entries: Vec<BehaviourEntry>,// 0x20 records, id at +0x1B
    pub entry_array: EntryArrayKind, // Clean | Variant | None
}
pub struct ActorRuntime {
    profile: ActorProfile,           // no name field
    relation: u8,                    // numeric, no invented enum
    state: u8,                       // numeric action id, no invented enum
    target: Option<LiveId>,
    flags: u32,
}
```

`ModelAsset { model, textured, actor: Option<ActorProfile> }` is the only
bridge: the spawner loads the model, and `LiveObject.actor` is built from the
profile the model carries. The AI module has no access to `PlacedObject`,
`EngineNameTable` or `SpawnPolicy`; the spawn module never reads trios or
entries. Module privacy enforces the seam; a unit test asserts `ActorProfile`
has no name field and that two names with the same profile start identically.

The render class is a third axis, not a spawn rule: `classify(prototype_name)`
uses the six documented prefixes and is used only to install hooks.

### 3. Hooks: function-pointer table with explicit null slots

| Option | Tradeoff | Decision |
|---|---|---|
| `Box<dyn ObjectBehaviour>` | Idiomatic, but one allocation per object, dynamic dispatch, and state tends to move into the trait object | Rejected |
| Enum dispatch | No allocation, exhaustive, but moves the six-class table into control flow and hides null slots | Rejected |
| Function pointers | Closest to the original three slots; `None` is a first-class opt-out; no allocation; the class table stays data | **Chosen** |

```rust
pub struct Hooks {
    pub event: Option<fn(&mut World, LiveId, &Event)>,
    pub tick:  Option<fn(&mut World, LiveId, ObjectTick)>,
    pub query: Option<fn(&World, LiveId, Query) -> QueryAnswer>,
}
pub struct ClassEntry { pub prefix: &'static str, pub class: RenderClass, pub hooks: Hooks }
pub const CLASS_TABLE: [ClassEntry; 6] = [
    // raziel__, sreavr__, glphicon, physical, monster_, particle
];
```

`raziel__` and `monster_` carry a tick; the other four leave it `None`. This is
the original's opt-out mechanism, not an error. Trait objects remain the right
tool for genuinely open backend seams (`AssetSource`, `RenderBackend`), where
runtime polymorphism earns its cost.

### 4. Timing: fixed 60 Hz, rational per-object step

Variable dt would make actor state, root motion and contacts non-reproducible.
The simulation runs a fixed step:

```rust
pub const TICK_HZ: u32 = 60;
pub const MAX_CATCH_UP: u32 = 5;
pub struct ObjectTick { pub index: u64, pub num: u32, pub den: u32 } // 1/60
```

`SimClock` accumulates wall time, runs at most `MAX_CATCH_UP` steps per frame and
keeps the remainder. The per-object step is passed to every tick hook, preserving
the original's per-object timestep even when all objects currently share 1/60.
Simulation state uses integers (`i32` world units from `i16` records); floats
appear only at the render boundary. Input is sampled at tick boundaries, so
event bursts cannot change the tick count.

### 5. Rendering: existing wgpu renderer as a backend

Keep `src/render.rs` and all viewer entry points untouched. Add:

```rust
pub struct RenderScene { pub camera: CameraView, pub items: Vec<DrawItem> }
pub struct DrawItem { pub mesh: MeshHandle, pub world: [i32; 3], pub visible: bool }
pub trait RenderBackend { fn draw(&mut self, scene: &RenderScene) -> Result<(), RenderError>; }
```

The milestone backend rebuilds one merged CPU mesh per frame (rooms + live
objects, tens of low-poly models) and reuses the existing shader/pipeline
primitives. This avoids instancing complexity now and keeps the backend
replaceable later. The cost is a small per-frame CPU merge; at the 48-object cap
it is negligible. The alternative - extending `show_mesh_textured` with a
closure - would entangle the viewer and the engine, so it is rejected.

### 6. CLI and `main.rs`: `--play` beside legacy dispatch

`--play` is a new top-level mode. The 32 legacy arms remain byte-for-byte
compatible. `cli_catalog` gains a sixth group `game` with one `play`
capability; `GROUPS` becomes `[&str; 6]` and `CAPABILITIES` `[Capability; 33]`,
and the catalog tests are updated. `main.rs` afterwards:

```rust
fn main() -> ExitCode {
    let args = strip_launcher_noops(env::args().skip(1).collect());
    match cli_argv::route_with_terminal(&args, std::io::stdout().is_terminal()) {
        Route::Help(group) => cli::help(group.as_deref()),
        Route::Menu => cli::menu(),
        Route::Dispatch(argv) => cli::dispatch(&argv),
        Route::Error(message) => { eprintln!("{message}"); ExitCode::from(1) }
    }
}
```

`cli::dispatch` checks `--play` first and calls `engine::play::run(options)`;
otherwise it delegates to the existing `legacy::dispatch`. Migration to a fully
table-driven CLI is a later, mechanical slice: move each arm into
`src/cli/legacy/`, replace the if-chain with a table, keep the argv shapes
identical.

### 7. Frame pipeline: the 18 documented stages, ordered, mostly stubs

`engine/frame.rs` owns one ordered function per documented stage. Stubs are
explicit no-ops with a name, so adding AI later is filling a body, not editing
the loop.

| # | Stage | Milestone |
|---:|---|---|
| 1 | clock / fixed step | implement |
| 2 | scratch reset | implement |
| 3 | script units | stub |
| 4 | audio emitters | stub |
| 5 | time of day | stub |
| 6 | activation / cull (query hook) | implement (distance + `Query`) |
| 7 | animation timers | stub |
| 8 | player solver | stub (direct input translation only) |
| 9 | player input -> movement | implement |
| 10 | pairwise contact | stub |
| 11 | hit list | stub |
| 12 | per-object tick (`tick` hook) | implement |
| 13 | trigger query | stub |
| 14 | moving contact | stub |
| 15 | render submission | implement (scene snapshot) |
| 16 | streaming | stub |
| 17 | particles | stub |
| 18 | spawn/despawn + tracker recompute | implement |

The player is ticked directly before the generic pass, then again as an ordinary
live object, matching the documented double update.

### 8. State machine

```rust
pub enum GameState {
    Boot,
    Intro, KainIntro, Menu, Logos,   // present, not entered in the milestone
    Loading { unit: String },
    InGame,
    Shutdown,
}
```

The milestone path is `Boot -> Loading -> InGame -> Shutdown`. The intro/menu
states are real variants with explicit transitions but no bodies yet; they
return `Unsupported` rather than a `todo!()`. `Loading` polls the asset source
and moves to `InGame` only when the tracker has no `Loading` slot left.

## Module layout

New modules (all under `src/engine/` unless noted):

| File | Role |
|---|---|
| `engine/mod.rs` | `run(options)`, module wiring, typed `EngineError` |
| `engine/clock.rs` | `SimClock`, `ObjectTick`, fixed-step accumulation |
| `engine/state.rs` | `GameState`, transition table |
| `engine/world.rs` | `World`, `LevelData`, frame entry |
| `engine/placed.rs` | `PlacedObject`, `PlacedDb` |
| `engine/tracker.rs` | `ObjectTracker`, `TrackerSlot`, `SlotState`, `TrackerError` |
| `engine/live.rs` | `LiveArena`, `LiveId`, `LiveObject` |
| `engine/class.rs` | `RenderClass`, `Hooks`, `CLASS_TABLE`, `classify` |
| `engine/spawn.rs` | `SpawnPolicy`, `EngineNameTable`, `Admission` |
| `engine/assets.rs` | `AssetSource`, `AssetStore`, `ModelAsset`, load queue |
| `engine/actor.rs` | `ActorProfile`, `ActorRuntime` (data only) |
| `engine/frame.rs` | 18 ordered stages |
| `engine/input.rs` | `InputFrame`, edge queue |
| `engine/scene.rs` | `RenderScene`, `DrawItem`, `MeshHandle` |
| `engine/render_backend.rs` | `RenderBackend` trait, `NullBackend` for headless |
| `engine/play.rs` | `--play` option parsing, window/headless driver |
| `engine/backend_wgpu.rs` | wgpu implementation of `RenderBackend` |

Reused verbatim: `src/level.rs` (parse, geometry, objects, room textures,
archive scan), `src/object.rs` (model parse, rest pose, textured mesh,
`ModelResolver`), `src/object_texture.rs` (`PageBank`, `scene`), `src/actor.rs`
(actor block), `src/bigfile.rs`, `src/textures*.rs`, `src/dds.rs`,
`src/render.rs` (camera/shader helpers only), `src/cli_catalog.rs`,
`src/cli_argv.rs`, `src/cli_out.rs`.

Small additive asset-layer change: `level::ArchivedUnit` gains the archive
`entry: usize`, and `level::resolve_unit_entry(dat, name)` returns it, so
`--play --unit under1` never hardcodes entry 115. `level::ObjectRecord` gains
`archetype: u16` and `instance_id: u16` for the script seam; existing printed
lines stay unchanged.

## Data flow

```text
BIGFILE.DAT --bigfile::load_index--+
                                   +--> level::read_entry_* --> LevelData + PlacedDb
TEXTURES.BIG --PageBank::open------+
                                             |
                       SpawnPolicy::admit (name) --> ObjectTracker (48 slots)
                                             |
                       AssetSource (model + actor block) --> ModelAsset
                                             |
                       LiveObject { class, hooks, actor } --> LiveArena
                                             |
                       18-stage tick (fixed 1/60) --> RenderScene
                                             |
                        RenderBackend (wgpu) / NullBackend (headless)
```

## Interfaces / contracts

The key contracts are the code blocks above. Two invariants are enforced by
types:

- `ObjectTracker::slots` is a fixed array; there is no unbounded spawn path.
- `ActorRuntime` has no name field and `SpawnPolicy` has no actor methods, so
  the name/data seam cannot leak without a compile error.

## Testing strategy

| Layer | What | How |
|---|---|---|
| Unit | clock, state transitions, tracker cap/dedup, live order, class table, spawn policy, actor profile equality | synthetic bytes/structs, no game files |
| Integration | `--play --headless` on a synthetic archive | temp files built in tests, digest assertions |
| E2E / manual | `under1` windowed textured, player movement | playtest note in PR body; not in CI |

No test reads a game file. No game bytes, coordinates or absolute paths are
committed.

## Threat matrix

N/A - no shell, subprocess, VCS/PR automation, executable classification or
process-integration boundary is added. `--play` only parses argv and reads
user-supplied archives through the existing read-only loaders.

## Migration / rollout

1. **Skeleton (this plan).** Add `engine/`, `--play`, catalog group; keep all
   legacy arms. No existing output changes.
2. **Milestone.** Headless + windowed `under1`; README gains a `--play` row.
3. **Later.** Fill AI stage/hook, animation, collision, contacts, scripts,
   audio, streaming; then make the CLI table-driven. Each step is a stacked
   slice under 400 authored lines.

## Open questions

- [ ] Should the always-spawn system-object pass be executed in the milestone,
      or only wired and deferred? The milestone treats missing system assets as
      `unresolved` (counted, not fatal) so multi-part containers such as
      `paths` do not block the demo.
- [ ] Exact fixed-point representation for movement speeds; the first milestone
      uses integer units per tick and can move to Q16.16 without changing the
      `ObjectTick` contract.
- [ ] Whether the backend should rebuild a merged mesh per frame or instance
      per object; deferred until the first performance measurement.

# Milestone report: engine-skeleton

The first engine milestone is complete. It delivers a deterministic headless
core, a windowed renderer backend and a single-line acceptance contract, and it
leaves every absent subsystem behind a named seam instead of a silent gap. This
report separates what was observed at runtime from what is only tested, and
lists what is still stubbed.

Reviewed with:

```text
cargo test
cargo run -- --play --headless --bigfile <BIGFILE.DAT> --ticks 60
cargo run -- --play --headless --bigfile <BIGFILE.DAT> --ticks 120
```

No game bytes, extracted files, decompiled listings or install paths are
included here. All counts and digests come from read-only runtime runs against
the user's own copy.

## What this milestone delivers

- A headless simulation core: fixed 60 Hz rational clock, game-state machine,
  placed-object database, 48-slot object tracker, ordered live arena, six-class
  hook table, name-keyed spawn policy and a data-keyed actor profile.
- An ordered 18-stage frame pipeline with an explicit stage table. Seven stages
  have real bodies; eleven are named no-ops.
- A CPU render scene and a wgpu backend, plus `--play` as a new top-level mode
  beside the 32 legacy capabilities. The legacy arms are unchanged. The windowed
  path renders the room and the player; the authored placed-object seed pass is
  shared by both modes (see the gap table for the original milestone state).
- A production digest: `--play --headless` prints one line and exits 0. The
  digest is the canonical world-state fold, not a second hashing scheme.
- A determinism gate with eight numeric goldens plus the stage-order table.

## Observed at runtime

The default unit is `under1`. The three required runs produced these exact
lines (paths replaced with `<BIGFILE.DAT>`):

| Run | Exit | Observed line |
|---|---:|---|
| `--ticks 60`, first | 0 | `engine unit=under1 rooms=1 placed=95 admitted=3 tracked=3 skipped_markers=92 inert=0 unresolved=0 live=3 ticks=60 digest=fe951463efc0dbec` |
| `--ticks 60`, second | 0 | Same line; the digest repeated byte for byte. |
| `--ticks 120` | 0 | `engine unit=under1 rooms=1 placed=95 admitted=3 tracked=3 skipped_markers=92 inert=0 unresolved=0 live=3 ticks=120 digest=0fd624b5c1e0b988` |

The counters are consistent with the documented partition:

- `placed = admitted + skipped_markers + inert` is `95 = 3 + 92 + 0`.
- `tracked + unresolved <= admitted` is `3 + 0 <= 3`.
- `live >= tracked` is `3 >= 3`.

Two facts explain the small live count and are not a regression:

- `under1` is mostly navigation data. Of its 95 placed records, 92 are
  marker-only (`campath` 66, `marker` 26). The three admitted names are
  `undhost`, `hint` and the player `raziel`.
- `tracked` counts tracker slots, and stage 18 reconciles every live name,
  including the player. Two non-player spawns plus the already-bound player
  therefore produce three ready slots and three live objects.

Supporting observations from the other tutorial units at 60 ticks:

| Unit | placed | admitted | tracked | skipped_markers | live | digest |
|---|---:|---:|---:|---:|---:|---|
| `under2` | 1 | 1 | 1 | 0 | 1 | `31cfd134b2743e31` |
| `under3` | 35 | 5 | 4 | 30 | 5 | `cb8a1c09272021d2` |
| `under4` | 1 | 1 | 1 | 0 | 1 | `87a5ea45b8deed88` |

`under3` sees `warpg` twice. Duplicate names share one tracker slot, so five
live objects hold four ready slots. That is the duplicate-name rule visible
end-to-end, not only in the unit test.

`rooms=1` for every run: `--play` loads one unit. It has no group mode, so the
`under` family is not merged into one play world in this milestone.

## Seen, tested, or neither

| Area | Evidence | Status |
|---|---|---|
| Headless run on a real install | This agent ran `under1` at 60 ticks twice and at 120 ticks; exit 0 each time. | Observed |
| Digest repeatability | The two 60-tick runs printed the same digest. | Observed |
| Tick count is part of the digest | 60 and 120 ticks printed different digests. | Observed |
| Marker and admission counters | The real `under1` counters match the partition, and synthetic acceptance tests pin the same fields. | Observed and tested |
| Windowed textured `under1` | The user visually confirmed the window with a screenshot during the T9 playtest. At that milestone the code path rendered the room mesh plus the player live object without the seed pass; `fix/engine-windowed-seed` now shares the seed pass and pins it with the parity test. This agent did not re-open a window in T11. | User-observed |
| Windowed camera modes, keys and `Esc` exit 0 | Manual playtest only; the window loop is not exercised in CI. | User-observed, not agent-observed |
| One-line contract format | Integration test parses the exact line; real runs match it. | Tested and observed |
| 49th distinct name exits 4 | Synthetic archive only; not reachable on the `under1`/`under3` runs. | Tested only |
| Markers never enter the tracker | Synthetic test asserts the slot names; real counters agree. | Tested and observed |
| Fixed-step bound (`10 s` -> at most 5 ticks, then drains to 600) | Unit test in the acceptance module. | Tested only |
| Missing texture page exits 2 | The exit mapping is unit-tested; the negative windowed path is not run at runtime. | Tested only |
| Determinism goldens and wall-clock scan | `cargo test`. | Tested only |

`cargo test` on the milestone head: **451 passed, 0 failed**. The suite uses
synthetic bytes only; no test reads a game file.

## What remains stubbed

### The 18 stages

| # | Stage | Milestone status |
|---:|---|---|
| 1 | clock | Implemented: fixed-step tick index. |
| 2 | scratch-reset | Implemented. |
| 3 | script-units | Named no-op. |
| 4 | audio-emitters | Named no-op. |
| 5 | time-of-day | Named no-op. |
| 6 | activation-cull | Implemented: distance cull and query hook. |
| 7 | animation-timers | Named no-op. |
| 8 | player-solver | Named no-op. The four-step move/collide/commit solver is absent. |
| 9 | player-movement | Implemented: direct integer input translation only. |
| 10 | pairwise-contact | Named no-op. |
| 11 | hit-list | Named no-op. |
| 12 | per-object-tick | Implemented: live-order pass with the class tick hook. The player is updated twice, as documented. |
| 13 | trigger-query | Named no-op. |
| 14 | moving-contact | Named no-op. |
| 15 | render-submission | Implemented: CPU scene snapshot in live order. |
| 16 | streaming | Named no-op. |
| 17 | particles | Named no-op. |
| 18 | post-frame-changes | Implemented: despawn, spawn and tracker reconcile. |

Implemented: 1, 2, 6, 9, 12, 15, 18. Named no-ops: 3, 4, 5, 7, 8, 10, 11,
13, 14, 16, 17.

### The wider absent set

| Subsystem | State |
|---|---|
| Actor AI | `ActorRuntime` holds numeric relation and state only. No perception, decision, engagement, attack or damage logic exists. |
| Animation | No `.SRA` parsing or hierarchy. Mesh tearing at cluster seams is unchanged; stage 7 is a no-op. |
| Collision | No solver, no pairwise or moving contact, no hit list. The player passes through geometry. |
| Scripts | Stage 3 is a no-op. `archetype` and `instance_id` are reserved on records but unread. |
| Audio | Stage 4 is a no-op. `.SND` waves export to WAV, but there is no playback, mixing, ADSR or sequence engine. |
| Streaming | Stage 16 is a no-op. `ImmediateSource` loads synchronously; `LoadQueue` is only a deterministic FIFO shape. |
| Particles | Stage 17 is a no-op. |
| Menus | `Intro`, `KainIntro`, `Menu` and `Logos` are real state variants, but their transitions return a typed unsupported error. |
| Warps | No warp behaviour. `warpg` and `wrpface` are treated as ordinary placed models. |
| Save / load | Absent. |
| Orientation | Placed records carry position only; no rotation is invented. |
| Async loading | Absent; the milestone completes loads synchronously. |
| ECS | Deliberately absent. The world stays plain structs and ordered arenas. |

## Known gaps carried forward

| Gap | Detail | Disposition |
|---|---|---|
| `RunError::World` exit code | A world load/parse failure mapped to `RunError::World`, whose `exit_code` fell through to 1, while the spec says asset, archive, parse or load errors exit 2. | Closed in `fix/engine-windowed-seed`: `World` maps to 2 and the exit-code test pins it. |
| Window title mismatch | The spec asks for a title containing `engine under1`; the implementation opens `soul reaver - under1`. | Align spec and code in a later docs/UI slice. |
| Digest scope | The gate is same-platform, same-build. FNV-1a is byte-exact, but `write_path` hashes `Path::as_os_str().as_encoded_bytes()`, which is OS-native. | ASCII paths are identical everywhere; cross-platform non-ASCII comparison is out of scope. |
| Non-ASCII paths | The same non-ASCII path can hash differently on Windows and Unix. | Normalize or document at the CLI boundary in a later slice. |
| LIFO free list | `LiveArena` uses `free.pop()` and `free.push(index)`, so slot reuse is LIFO. Live order is preserved, but the original allocator's reuse order is unproven. | Mark UNKNOWN against the original; revisit with evidence. |
| `state = 0` placeholder | `ActorRuntime::initial` starts relation and state at neutral 0. The shipped action ids are model-relative; no action is selected. | Not proof of the original initial state. Replace when AI semantics land. |
| Tracker cap proof | The 48-slot cap is proven with a synthetic 49-name level, not on a real unit (real units are far below the cap). | Keep the synthetic gate; add a real corpus census when one exists. |
| Windowed seed gap | `run_windowed` received the frame built by `FrameState::new`, which binds only the player, so `seed_world` ran only in the headless path and the window showed the room and `raziel` but not the other authored placements. | Closed in `fix/engine-windowed-seed`: `boot_world` is shared, runs before the event loop, and a parity test pins the same live count and digest as headless. |
| Single-unit load | `--play` has no `--group` mode, so `rooms` is always 1. | Group play is future work. |
| No input script | Headless runs use zero input. Windowed input is keyboard-only and not reproducible in CI. | Add a deterministic input script if replay capture becomes necessary. |
| Render runtime not in CI | The wgpu/winit path is manual. The headless path is the only automated end-to-end boundary. | Accept for the milestone; add a smoke harness later if the CI image supports it. |

## Determinism gate inventory

All constants are in the modules named below. A change to any fold input moves
its digest and fails the gate; the constants are never re-pinned to go green.

| Gate | Constant | Location | Guards |
|---|---|---|---|
| Options | `0x27e6_be15_9907_b55c` | `src/engine/determinism.rs` | Argv parsing, fold order and field set. No simulation state. |
| Errors | `0xf7df_8b2c_3041_7acf` | `src/engine/determinism.rs` | Typed error variants and exact message text. |
| World state | `0xb5bb_59fd_97b9_50b1` | `src/engine/determinism.rs` | Canonical state and the production digest: tick, live order, class, integer position, visible. |
| Tracker | `0x4fb9_0f56_bbe9_c0f9` | `src/engine/determinism.rs` | All 48 slots in index order: index, state, name. Acquisition, duplicate sharing and lowest-free reuse. |
| Classification | `0x9bec_a441_2d97_8281` | `src/engine/spawn.rs` | The five name lists in documented order: list id, count, name bytes, admission, load role, asset tree. |
| Actor runtime | `0xe169_695d_0eda_ec99` | `src/engine/determinism.rs` | Profile kind, eight trios, entry-array kind, entry ids, then relation, state, target and flags. |
| Stage order | 18 `(code, name)` pairs | `src/engine/determinism.rs` | The exact stage numbers and names. Swapping two stages fails. |
| Frame state | `0x7b7f_16bc_a6af_b9b2` | `src/engine/determinism.rs` | Final state of a 40-tick scripted replay: per-tick stage trace, input sampling, player translation, cull, scene and post-frame bookkeeping. |
| Headless CLI | `0x8dd6_3315_4e26_f1c4` | `src/engine/play_acceptance.rs` | The full CLI-path acceptance run over a synthetic archive. |

Sensitivity evidence that the gates bite:

- Dropping the `visible` byte moves the world-state golden to
  `0xe34dc760aae69953` and the headless digest to `0x6094208b15fb134e`.
- Changing the initial actor state from 0 to 1 moves the actor-runtime golden to
  `0x04d3052a20e92660`.

Non-constant gates:

- Live-order replay in `src/engine/determinism.rs` pins slot index, generation
  and order across two identical spawn/remove runs.
- `simulation_path_never_names_a_wall_clock` scans the simulation modules for
  `Instant` and `SystemTime`.
- `equal_tick_counts_ignore_the_wall_clock_schedule` reaches the same 120-tick
  state through 120 exact `1/60 s` frames and through one 2 s advance plus
  capped drains.
- The acceptance module proves the fixed-step bound: a 10 s advance yields at
  most 5 ticks, the remainder is kept, and the drain reaches 600 ticks.

## Next slice: engine-actor-tick (plumbing only)

The first post-milestone slice wires the existing actor machinery into the frame
loop without adding AI semantics. Stage 12 now dispatches the class tick hook in
live order, a live object carries `Option<ActorRuntime>` filled from the loaded
model block, and the canonical digest folds actor presence plus the numeric
relation and state. The behaviour trios and entry ids remain data and are not
consumed; `ActorRuntime::advance` is explicitly `NotInterpreted`.

Still open and deliberately unimplemented: the named state machine, the meaning
of relation/action ids, perception, engagement, melee, turn-to-face, disengage,
animation, damage and death. See `docs/changes/engine-actor-tick/proposal.md`.

A 60-tick read-only `--play --headless` run on `under1` moved the production
digest from `fe951463efc0dbec` to `3ac33bf08aab697d` (two identical runs); the
counters and live set did not change.

## Definition of done for this milestone

- Headless `under1` runs, exits 0 and prints the contract line.
- The same run twice repeats the digest; a different tick count changes it.
- The windowed textured path exists and was manually confirmed.
- The 32 legacy capabilities and their exit codes are unchanged.
- `cargo test` is green at 451 tests.
- Every absent subsystem is either a named no-op stage or an explicit typed
  unsupported path. None is a silent placeholder.

# Proposal: engine-events

## Intent

Give the actor layer the event/message plumbing it needs without inventing what
an event does. The evidence establishes that actor state is event/data-driven and
never timer-driven (`docs/notes/actor-state-writer.md`, B1-B5), that the
event/message entry is hook slot 3 and receives hit payloads
(`docs/notes/actor-ai.md`, section 2), and that an attacker assembles a hit
record while the frame runs a hit-record pass and a trigger query
(`docs/notes/actor-ai.md`, sections 1 and 3). This slice builds the carrier, the
ordering and the delivery seam. It decides no state transition.

## Evidenced event categories

`src/engine/events.rs` carries exactly three families:

| Category | Fields | Evidence |
|---|---|---|
| Hit record | attacker, target, bone, radius mask | The base actor update assembles a hit record (target, bone, radius mask) in the strike transition; the hit-record pass resolves it (`actor-ai` section 3). |
| Contact/trigger | source, target | Stage 13 is a spatial query from a cylinder around each object (`actor-ai` section 1). The cylinder extent and trigger-record source are UNKNOWN, so only the pair is carried. |
| Message | target, raw code | The event/message dispatcher writes the state byte on two specific message ids (`actor-state-writer` section 3). The id space and payload are UNKNOWN, so the code stays a raw integer. |

No damage, health, knockback, pickup, death or script-command event is invented.
`EngineEvent::target` names the single delivery target of each family.

## What is plumbing

- **One deterministic FIFO queue.** `EventQueue` is a `Vec` behind push order.
  There is no producer priority, sorting or hash iteration. Two producers that
  push in the same interleaving drain the same sequence.
- **Tick-local ownership.** The queue is drained once per tick at the delivery
  stage and the drain clears it. An event belongs to one tick and can never be
  delivered by the next.
- **Stage 11 producer seam.** The hit-record pass has no evidenced producer in
  this engine: the producer is the actor base update's strike transition, which
  needs the animation timeline's action kind. There is no timeline, so the pass
  queues no hit and says so in code.
- **Stage 13 producer seam and delivery.** The trigger query has no pinned
  cylinder extent or trigger-record source, so it queues no contact. Stage 13 is
  also the defined delivery stage: it drains the queue in FIFO order, calls the
  target's event hook when the class installs one, and forgets the event.
- **Event hook slot.** The two attested actor rows (`raziel__`, `monster_`)
  install an empty witness body on the event slot. The evidence names the slot
  and per-actor binders; it does not pin the body, so the body changes nothing.
  The other four rows stay null rather than growing an unproven hook.
- **Carrier view.** Stage 12 forwards the tick's target-matched events, in FIFO
  order, to the actor tick body. `ActorRuntime::advance` now takes that slice.

## What is deliberately NOT interpreted

- Any damage, health, knockback, death or combat resolution.
- Any state, relation or action-id change caused by an event.
- Any message code meaning or payload layout.
- Any cylinder extent, trigger layer or hit filter.
- Any transition table or state-to-action map.

`ActorRuntime::advance` receives the events and still returns
`ActorTransition::NotInterpreted` after touching nothing. This is explicit and
tested, not a placeholder default.

## Ordering and no-leak proofs

Queue level (`src/engine/events.rs` tests):

- `fifo_order_is_push_order` - drain returns the push sequence.
- `replaying_the_same_pushes_drains_the_same_sequence` - two identical runs
  yield the same sequence.
- `a_second_drain_is_empty_and_cannot_leak` - a drained queue yields nothing on
  the next drain.
- `ordering_is_independent_of_the_producer` - two producers in a fixed
  interleaving drain exactly that interleaving, never a reordered one.

Frame level (`src/engine/frame.rs` tests):

- `queued_events_are_drained_once_at_the_delivery_stage` - delivery happens
  once and reaches the target's event hook.
- `an_event_produced_in_one_tick_is_never_delivered_in_the_next` - the delivered
  counter is frozen across five later ticks.
- `the_hit_and_trigger_passes_produce_no_event_without_an_evidenced_source` - a
  bare world produces zero events across ten ticks.

## The advance seam

`ActorRuntime::advance(&mut self, step, events: &[EngineEvent])` is the only
route by which the carrier may advance. It receives the target-matched events
for the current tick and selects nothing.

- `actor::tests::advance_receives_events_and_still_selects_nothing` calls
  `advance` directly with a hit, a contact and a message and asserts
  `NotInterpreted` plus byte-identical relation, state, target, flags and
  profile.
- `frame::tests::events_reach_the_actor_carrier_without_changing_its_state` runs
  the same actor world with and without events. The runtime is identical, the
  no-event run passes zero events to `advance`, the event run passes both, and
  the canonical digest is identical.
- `frame::tests::an_event_run_replays_the_same_digest_and_stats` replays the
  same event run and compares the digest and the diagnostics.

## Determinism

Events are transient: the queue is not part of the canonical fold. No golden
moves.

| Gate | Before | After |
|---|---|---|
| `GOLDEN_WORLD_STATE_HASH` | `0x2c8e_61aa_2f02_31b0` | unchanged |
| `GOLDEN_FRAME_STATE_HASH` | `0xa203_0add_093d_a5e6` | unchanged |
| `GOLDEN_HEADLESS_DIGEST` | `0x490c_8582_c297_df18` | unchanged |
| options, error, tracker, classification, actor-runtime, stage-order | - | unchanged |

The gate still bites. A temporary perturbation that drops the actor state byte
from the canonical fold moved the world-state test to `0xccb5_59a4_77be_1118`
(observed `14750794717440381208`); the perturbation was reverted before commit.

`events.rs` is added to the simulation-source wall-clock guard, so it may not
name `Instant` or `SystemTime`.

## Real data

Read-only headless runs on the user's own corpus, 60 ticks, twice per unit:

```text
unit=skinnr1 placed=83 admitted=41 tracked=15 skipped_markers=42 inert=0 unresolved=0 live=41 ticks=60 digest=ab4048e4183a00a6
unit=under1  placed=95 admitted=3  tracked=3  skipped_markers=92 inert=0 unresolved=0 live=3  ticks=60 digest=b0d675df0f56e371
```

Both digests are byte-identical across the two runs and match the
pre-slice values (`ab4048e4183a00a6` and `b0d675df0f56e371`). The slice changes
no simulation state.

## Verification

- `cargo test`: 481 passed, 0 failed (baseline 469; +12 new tests).
- `cargo build`: zero warnings.
- `git status`: clean after the work-unit commits.

## Non-goals

No animation timeline, no strike detection, no contact geometry, no damage, no
health, no message semantics, no script units. The producer bodies stay empty
until the evidence pins them.

## Legal

No addresses, offsets, disassembly listings, game bytes or absolute install
paths are recorded here. The real-data check ran read-only against the user's own
corpus; nothing from it is stored in the repository.

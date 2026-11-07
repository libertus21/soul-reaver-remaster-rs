# Proposal: engine-actor-refresh

## Intent

Freeze the one actor-state transition the evidence actually confirms: the
behaviour-set refresh. The authority is
`docs/notes/actor-transition-table.md` (the eleven writers, the refresh rule, the
four state bytes, the UNKNOWN list). This slice adds the state bytes to the
carrier and a pure `refresh` function. It does **not** implement any writer body
and does **not** wire the rule into `advance`.

## The confirmed rule

```text
refresh(current, queued, alternate):
    if current == 8 (the post-action value):
        next = alternate if set, else queued
    else:
        next = current
```

`POST_ACTION_STATE` is `8`. It is a raw byte constant, not an enum member: no
other state value has an established name, and the W5 action map is code-local
with no global meaning (`docs/notes/actor-transition-table.md` sections 1 and 5).

## The four state bytes

`ActorRuntime` now carries the four bytes the note identifies:

| Field | Role |
|---|---|
| `current` | the selector the AI tick and the animation set read |
| `queued` | a pending state |
| `alternate` | a higher-priority pending state |
| `entry_action` | the behaviour-entry companion byte; a state source, not `current` |

`relation`, `target`, `flags` and the embedded profile are unchanged.

### The "alternate is set" sentinel

The note records only that "alternate is set" means "any value other than the
not-set sentinel", and the concrete sentinel value is **UNKNOWN** (observed as
the all-ones byte in one shipped path, not proven as a general sentinel;
`docs/notes/actor-transition-table.md` section 9, item 7).

The carrier models it as `Option<u8>`. This is a Rust representation choice,
**not** an evidence claim:

- `None` means "not set" without inventing a sentinel constant;
- `Some(value)` means "set to `value`", including `Some(0)`.

The fold records alternate as a presence byte plus, when present, the value, so
`None` and `Some(0)` stay distinguishable. If a future runtime observation pins
the on-disk sentinel, this representation can be revisited without guessing it
now.

## Purity and scope

`refresh(current, queued, alternate) -> u8` is a free function with no world
access and no side effects. It is the only state transition this slice freezes.

- `ActorRuntime::advance` still returns `ActorTransition::NotInterpreted` and
  changes nothing, even when `current` is the post-action value and a pending
  state exists. A dedicated regression test proves it does not call `refresh`.
- Nothing else in the engine calls `refresh`.

## Deliberately NOT implemented

- The eleven writer bodies W1, W2, W4, W5, W6, W7, W8, W9, W10, W11.
- The parameter program, engagement record, target handling, alignment and
  action-kind dispatch.
- Any global action-id enum or state-name table.
- Any event, timer, counter or frame-driven state change.
- Any use of the file-side behaviour-entry id as a runtime state.

`NotInterpreted` remains the honest body.

## Determinism

The canonical fold previously covered the relation code and one actor `state`
byte. It now covers, per live entry, the relation code plus all four state bytes:

```text
actor presence, relation, current, queued,
alternate (presence byte + value when present), entry_action
```

The fold therefore covers strictly more of the carrier than before. The rule
itself does not run, so no simulation value changes: every moved digest is a
fold-extension move, not a behaviour change.

| Golden | Before | After |
|---|---|---|
| `GOLDEN_WORLD_STATE_HASH` | `0x2c8e_61aa_2f02_31b0` | `0x043f_f933_4dfe_3b58` |
| `GOLDEN_FRAME_STATE_HASH` | `0xa203_0add_093d_a5e6` | `0xd1f3_6008_a152_04a2` |
| `GOLDEN_HEADLESS_DIGEST` | `0x490c_8582_c297_df18` | `0x56d6_79ea_3c48_6e14` |
| `GOLDEN_ACTOR_RUNTIME_HASH` | `0xe169_695d_0eda_ec99` | `0xd7d6_0c24_050a_d833` |

The options, error, tracker, classification and stage-order goldens do not move.

### The gate bites

A temporary perturbation removed the alternate presence/value from
`world_state_hash`, then the suite was run and the perturbation reverted:

| Gate | Expected | Observed while perturbed |
|---|---|---|
| world state | `0x043f_f933_4dfe_3b58` | `0xcfa1_0a36_60a0_1290` |
| frame state | `0xd1f3_6008_a152_04a2` | `0x3ca4_7313_70c6_83d6` |
| headless digest | `0x56d6_79ea_3c48_6e14` | `0xa1e7_aa14_e1d4_70e8` |

The alternate-presence sensitivity test also fails while perturbed. The
perturbation was reverted before commit and the suite returned to green.

## Tests

Carrier and rule (`src/engine/actor.rs`):

- `refresh_keeps_a_non_post_action_current_unchanged` - several values including
  `0`, `7`, `9`, `0x25` and `u8::MAX`, with alternate set and not set.
- `refresh_selects_alternate_at_the_post_action_value` - including `Some(0)`
  winning over queued.
- `refresh_falls_back_to_the_queued_state_when_alternate_is_not_set`.
- `refresh_is_pure_repeatable_and_does_not_mutate_the_carrier`.
- `advance_does_not_apply_the_confirmed_refresh_rule` - `current = 8`, queued and
  alternate set, events delivered, every field unchanged.
- `advance_receives_events_and_still_selects_nothing` - updated to assert all
  four state bytes stay neutral.

Fold sensitivity (`src/engine/determinism.rs`):

- each of `current`, `queued`, alternate value, alternate presence and
  `entry_action` changes the world-state hash;
- each of the same fields changes the actor-runtime hash.

## Real data

Read-only headless runs on the user's own corpus, 60 ticks, twice per unit
(runtime only; nothing committed):

```text
unit=skinnr1 placed=83 admitted=41 tracked=15 skipped_markers=42 inert=0 unresolved=0 live=41 ticks=60 digest=d93bcb5cd913f812
unit=skinnr1 placed=83 admitted=41 tracked=15 skipped_markers=42 inert=0 unresolved=0 live=41 ticks=60 digest=d93bcb5cd913f812
unit=under1  placed=95 admitted=3  tracked=3  skipped_markers=92 inert=0 unresolved=0 live=3  ticks=60 digest=ede39c540a10ee39
unit=under1  placed=95 admitted=3  tracked=3  skipped_markers=92 inert=0 unresolved=0 live=3  ticks=60 digest=ede39c540a10ee39
```

Both units are reproducible. The digests move only because the fold now covers
the four state bytes:

| Unit | Before | After |
|---|---|---|
| `skinnr1` | `ab4048e4183a00a6` | `d93bcb5cd913f812` |
| `under1` | `b0d675df0f56e371` | `ede39c540a10ee39` |

The counters, live set and tick count are unchanged.

## Verification

- `cargo test`: 486 passed, 0 failed (baseline 481; +5 tests).
- `cargo build`: zero warnings.
- `git status`: clean after the work-unit commits.

## Legal

No addresses, offsets, disassembly listings, game bytes or absolute install
paths are recorded here. The real-data check ran read-only against the user's own
corpus; nothing from it is stored in the repository.

# Proposal: actor-ai (outcome: the actor update is a class hook, behaviour is data)

## Intent

Close the AI half of `object-behaviour`'s UNKNOWN #1: "the exact per-frame
AI/update function selected for each actor roster name". The open questions
were:

- Where is the per-frame actor update entry, if any?
- What does it do, in what order?
- Is there a shared idle/alert/pursue/attack/hurt/dead state machine?
- Do the roster names get their own code paths, or is behaviour data-driven?
- How does `raziel` share or diverge from the actor update?

## Outcome

**The per-frame actor pass exists and is a hook call.** The in-game frame tick
walks the live-object list and calls a per-object function pointer at `+0x1A8`
once per live object, with a per-object timestep. No name is consulted.

**Every live object carries three hooks**, installed at load-complete from a
six-class table (`raziel__`, `sreavr__`, `glphicon`, `physical`, `monster_`,
`particle`; class index at `+0x04`, chosen from the prototype name at `+0x30`):

| Hook | Class slot | Role |
|---|---:|---|
| `+0x1A0` | 3 | event/message entry (receives contact/hit payloads) |
| `+0x1A8` | 6 | per-frame tick |
| `+0x1B0` | 4 | query / state getter (visibility, cull) |

Only `raziel__` and `monster_` carry a non-null tick; the other classes opt out.
For `monster_` the tick resolves to the shared locomotion/animation body.

**The update's subsystem order** is: animation timeline advance → root motion
applied to position → ground/entity contact query → attack/hit-record emission
→ action abort rules → animation-state selection. The frame around it is:
timing → per-frame reset → script units → activation/cull → player solver →
player input → pairwise contact → hit-list pass → per-object tick → trigger
query → moving-object contact → render → streaming → particles → post-frame
spawn/despawn.

**There is one shared state machine**, expressed as model-relative numeric
action ids (not named enum constants). Evidenced states: idle, alert/tracking,
turn-to-face, attack/strike, recover, hurt/knockback, dead/removed. Evidenced
transitions: idle→alert on entering the engagement radius; alert→attack when
the melee gate passes; attack→recover on clip end; any→disengage through a
dedicated handler. The numeric ids are not pinned and must not be invented.

**Behaviour is data-parameterised.** An AI instance block at `+0x2C0` points at
a behaviour/action-set entry selected by a **byte in the model data**; the entry
supplies the initial animation state and the state→animation set. Native
variants exist per *family* (weapon user, hit-window user, knockback user),
installed by per-family setup code, never per roster name.

**The roster is management, not AI.** The ten-name roster gates spawning under
a debug flag, drives the low-memory "dump the largest monster" path and the
debug "kill all roster objects" path. No roster name is compared in any AI,
animation or state function. The eight-name boss list and the exact uppercase
specials (`MORLOCK`, `RONINBSS`, `ALUKABSS`, `SKINBOS`) only adjust spawn
counters and one actor flag.

**`raziel` is a class, not a special case in the loop.** It is the
`raziel__` instance and is ticked by the same mechanism, but the frame tick
also calls a dedicated player update directly; the player owns input, a 4-step
movement solver, and an interaction-query tick slot instead of the locomotion
thunk.

Full evidence: `docs/notes/actor-ai.md` (frame-order table, hook table, state
table, parameterisation answer, player-vs-actor answer, explicit UNKNOWN list).

## What changes versus the prior notes

- `object-behaviour` UNKNOWN #1 is answered at the mechanism level: the
  per-frame hook exists, is per class, and is reached from the live list. The
  name is confirmed as *not* the AI dispatch key.
- The render class at `+0x04` is promoted from "renderer selector only" to
  "class selector that also binds the behaviour hooks". It is still not an
  authoring key: it is derived from the prototype name at load-complete.
- `object-behaviour`'s "generic actor update" row is split into: class tick
  (locomotion), event hook (per-family variants), and the AI decision layer
  (behaviour entry + target + state handlers).
- `s4-run-internals-behavior` covers bootstrap only; this pass adds the in-game
  frame order that the actor update sits in.

## Forecast split

This is a docs-only slice. No production code is proposed in this change.

| PR | Content | Budget |
|---|---|---|
| A (this one) | `docs/notes/actor-ai.md` + this proposal | ~420 lines, docs only |
| B (optional, deferred) | A docs-only **actor census** table: for every actor name in the shipped `.PCM` corpus, the model byte that selects the behaviour entry, resolved through the existing level/model readers, printed as a table | ~60 lines of note + a scratch script kept out of the repo |
| C (only if B proves stable) | A `--actor-census` subcommand in the existing viewer binary that prints name, class, and behaviour-entry byte | ~150 lines code + tests |

PR B is the minimal reproducible slice worth doing: it needs **no new engine
knowledge** beyond a documented model byte offset and it validates the
parameterisation claim against all shipped actors at once. It stays docs-only
until the byte offset proves stable across the corpus; only then does C make
sense. Spec/design/tasks are intentionally omitted until B is scheduled,
because a spec that promised "actor X is aggressive" would encode a claim this
pass could not confirm (the behaviour entries' semantics are still UNKNOWN).

## PR plan

- Branch: `docs/actor-ai` from `main` (head `c97d133`).
- Commit: `docs(ai): map the per-frame actor update and its state machine`.
- One PR, docs only, based on `main`, not merged in this session.
- Validation: `cargo test` stays 321 green (no code touched); `git status`
  shows only the new markdown files.

## Risks

- **Numeric ids are not named.** The state machine is real but its id values
  are model-relative and partly unenumerated. The note keeps every id claim as
  UNKNOWN rather than guessing an enum.
- **Decision cadence is not pinned.** The per-frame hook is confirmed; whether
  the decision handlers run every frame or only on state change is UNKNOWN.
  The note says so and names the probe.
- **Damage/health are not pinned.** The hit record's consumer was not followed
  into the stat block. Any "attack does N damage" claim would be invented.
- **Sampling.** Only a few native variants were decompiled. The note lists the
  variant families found and labels the set as partial.
- **Script overlap.** Level scripts can drive actors through the event VM; the
  split between script and native per encounter is per level and stays
  UNKNOWN.
- **Legal.** Notes only: offsets, counts, roles, short strings. No addresses,
  no listings, no game bytes, no install paths. Scratch stayed in `%TEMP%`.

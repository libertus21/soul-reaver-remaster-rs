# actor-state-semantics - what the relation and state ids actually select

Bounded, read-only static pass on the shipped SR1 module (Ghidra 12.1.4) plus a
read-only headless run of the engine on real level units. Behaviour paraphrases
only: field roles, counts and small numeric ids. No addresses, no offsets, no
disassembly listings, no game bytes, no absolute install paths. Scratch work
stayed in `%TEMP%`; the binary and the archives were opened read-only.

Date: 2026-11-06. Evidence class per claim: **CONFIRMED** (code and data agree,
or two independent observations agree), **PARTLY KNOWN** (one side pinned, the
other not), **UNKNOWN** (explicitly not established).

This note continues `actor-ai` (the runtime AI block and the per-frame tick),
`actor-behaviour-data` (the shipped actor block and its 8x3 trio table),
`object-behaviour` (name-keyed dispatch), `determinism` (the actor fold) and
`docs/changes/engine-actor-tick/proposal.md`. It answers the four questions that
keep `ActorRuntime::advance` at `NotInterpreted`, and it records one dispatch
finding that changes how those questions must be read. The writer chain and the
exact forced-class bits are resolved in `actor-state-writer`.

## Verdict

1. **`relation` is an engagement phase tag, not a table index.** The prior
   CONFIRMED evidence says the perception handler writes a four-value phase code
   into the target/engagement record by distance-to-target branch shape. This
   pass examined the AI decision tick and the prowl/wander step and found no
   code that uses that code to index a relation/graph table or the 8x3 table.
   The exact reader was not re-located. → **PARTLY KNOWN**.
2. **The per-model selector byte selects the behaviour ENTRY, not a row of the
   8x3 table.** The runtime indexes the behaviour-entry array with one byte from
   a model sub-record, then feeds that entry's id byte to the animation setter.
   This closes the old "which model byte selects the behaviour set" UNKNOWN for
   one of the two documented selectors. The row selector for the 8x3 trio table
   is still **UNKNOWN**, and two natural candidates are refuted by data. →
   **PARTLY KNOWN**.
3. **There is no globally enumerable action-id → named-state map.** The state
   setter hard-codes a few state→action pairs and collapses everything else to
   the neutral action. The action setter accepts a bounded numeric range and
   treats a small subset specially. These are code-local fragments, not an enum,
   and the ids are model-relative. → **PARTLY KNOWN**.
4. **`state` drives the tick; `relation` does not.** The AI tick branches on the
   current state byte and the queued-state byte; the base actor update branches
   on the animation timeline's action kind and sub-phase. Neither reads a
   relation value. → **CONFIRMED** (within the examined paths).
5. **Real monsters never reach `advance()` today.** The engine derives the
   render class from the *placed record name*, but shipped levels place actors
   under their bare model names (`skinner`, `sluagh`, `ronin`, ...), which cannot
   carry the `monster_` prefix. The class therefore resolves to `Unclassified`,
   stage 12 installs no tick hook, and the carrier (which is attached) is never
   advanced. The player is bound with the `Raziel` class directly, so only the
   player ticks. → **CONFIRMED**.

## 1. Scope and method

Three of the four methods produced evidence; the fourth is the falsification
ledger in section 7.

- **A. Binary evidence.** Static, read-only decompilation of the shipped SR1
  module. The pass re-pinned the actor decision tick, the state setter, the state
  tick, the animation-set selector, the behaviour-entry selector and the base
  actor update, then followed the two numeric spaces they touch.
- **B. Data correlation.** Re-ran `--actor-census` on the shipped corpus and
  compared entry counts, id ranges and trio rows against the numeric values the
  code actually branches on.
- **C. Real monster observation.** Ran the engine headless (read-only, runtime
  only) on units that place monsters, twice per unit, and compared the report
  lines.
- **D. Falsification.** For every hypothesis, section 7 states what would refute
  it and whether that test was run.

## 2. The four questions

| # | Question | Finding | Class | Supporting evidence |
|---:|---|---|---|---|
| 1 | What does `relation` select? | A four-value engagement phase tag in the target/engagement record; no code examined uses it as a table index. It is written by the distance/engagement branch (`actor-ai` section 4). Its exact consumer was not re-located. | PARTLY KNOWN | `actor-ai` section 4 (writer, CONFIRMED); this pass found no relation-indexed read in the decision tick, the prowl step, the state tick or the base update |
| 2 | What selects the behaviour ROW? | The **behaviour-entry** selector is a single byte in a model sub-record; it indexes the fixed-stride entry array, and the selected entry's id byte feeds the animation setter. The **8x3 trio row** selector is still unknown; "state indexes the row" and "entry id indexes the row" are both refuted. | PARTLY KNOWN (entry) / UNKNOWN (trio row) | Binary: entry pointer = array base + selector × fixed stride; entry id byte passed to the animation setter. Data: entry ids span 2..60 and observed AI states reach 9, so neither can index 8 rows |
| 3 | What is the map from a numeric action id to a state? | No global map. A state setter maps a few new-state values to action ids and stores the raw state; the action setter accepts a bounded range and treats a small subset specially. Everything else collapses to the neutral action. | PARTLY KNOWN | Binary: state setter (`2→5`, `4/8→20`, `9→19`, otherwise `1`) and action setter's bounded-range test with a five-value special subset |
| 4 | Which of `relation`/`state` drives the tick? | `state` (current + queued) is the selector for the AI tick and the animation set. The base update is driven by the animation timeline. `relation` is not read by either path. `state` is a selector; `relation` is a phase tag; the action id is a third, separate field. | CONFIRMED (examined paths) | Binary: state branches in the AI tick/state tick; timeline action-kind/sub-phase in the base update; no relation read |

## 3. Question 1 - what `relation` selects

`actor-ai` already recorded the writer: the perception/engagement handler
computes the distance to the current target, compares it with the model's
engagement radius, selects the alert/combat set inside the radius and the idle
set outside it, and writes a four-value phase code (idle / closing / striking /
disengaging by branch shape) into the target/engagement record. That writer is
CONFIRMED there.

This pass looked for a **reader** that would make the code a selector:

- The AI decision tick reads the AI flag word, the current state byte, the cached
  target position and the engagement radius, then selects action ids. It does
  not read a four-entry table and it does not index anything by a phase code.
- The prowl/wander step picks a random point, runs a world query and re-caches
  the target position; it writes the cached position and a flag. It does not
  read or write a relation-indexed structure.
- The state tick and the animation-set selector branch on the state byte and the
  timeline, not on a phase code.

So the best-supported reading is: **`relation` is an output/phase tag of the
engagement handler**, not an index into a relation graph or the 8x3 table. The
prior note's "the machine is driven by three inputs, including the relation
code" is consistent with a later transition reading it, but this pass did not
pin that reader. The claim "relation never indexes anything" is therefore
INFERRED for the examined paths, not proven globally.

Refutation test: a function that loads the relation value and uses it as a
multiplier/index into a table. No such use appeared in the decision tick, the
prowl step, the state tick or the base update. Not exhausted across the whole
module → **PARTLY KNOWN**.

## 4. Question 2 - what selects the behaviour row

### 4.1 The entry selector is pinned

At the behaviour-set refresh, the runtime computes the AI block's behaviour-entry
pointer as:

- the entry-array base, plus
- one **single-byte selector from a model sub-record**, times the fixed entry
  stride.

It then stores that pointer on the AI block and passes the selected entry's id
byte (the byte `actor-behaviour-data` pinned inside each entry) to the animation
setter. This is the mechanism `actor-ai` described as "the entry is chosen from
a byte in the model data". The selector is now located structurally: it is a
byte in the sub-record reached through the model-data pointer, not a free
floating model-header byte.

This closes one half of `actor-ai` UNKNOWN #4 / `actor-behaviour-data` UNKNOWN
#2. The other half - the *second* selector used for the initial set - was not
located in this pass, so the two-selector story is still only half pinned.

### 4.2 The 8x3 trio row selector is still unknown

The shipped 24-byte table is eight trios. Two candidate row selectors are
refuted by the corpus and the code:

- **"The AI state is the row index."** The code branches on state values `0`,
  `2`, `4`, `8` and `9`. Eight rows can only be indexed `0..7`; `8` and `9` are
  outside that space. Refuted.
- **"The behaviour-entry id is the row index."** The shipped entry ids span
  `2..60` across 21 actors. That is far outside `0..7`. Refuted.

There is also no count invariant that would make trios an 8-row map: clean
actors have 1, 3, 4, 5, 7, 8, 9 and 10 entries. Some have exactly eight, but
`skinner` has nine and `aluka` has ten, so "one row per entry" does not hold.

What the trios *do* show is template sharing: whole rows are byte-identical
inside actor families (for example `skinner`/`morlock` share rows 0-5;
`hunter`/`wrshp`/`priests` and the three `vlgr` variants share rows 0-4). That is
consistent with eight shared animation-set slots that a family tunes, not with a
globally named state enum.

**Consequence:** the 8x3 table is best treated as an animation-set template. Its
row selector is not established; do not map rows to named states.

## 5. Question 3 - the numeric id spaces

The runtime carries at least three separate numeric spaces, and the current
`ActorRuntime` names only one of them (`state`). Keeping them apart is the whole
answer to "what is the mapping".

| Space | Where it lives | What it does | Class |
|---|---|---|---|
| AI **state** byte (current) | AI block | Branches the AI tick and the animation-set refresh; `0` means "no specific action" in the animation selector | CONFIRMED |
| AI **queued state** bytes | AI block | A queued/alternate state; one state transition consumes it when the current state is the post-action state | CONFIRMED |
| **Action id** | AI block | A separate transition/action code set through one setter; drives the base update's action-kind path indirectly | CONFIRMED |
| Behaviour-entry **id** | shipped entry record | Seeds the animation set when a behaviour entry is selected; model-relative | CONFIRMED bytes / role PARTLY KNOWN |
| Trio **clip-set ids** | shipped actor block | Animation-set template values; not a state enum | CONFIRMED shape / role INFERRED |

The state setter's code-local map, as observed:

- new state `2` → action `5`
- new state `4` or `8` → action `20`
- new state `9` → action `19`
- any other new state → action `1` (neutral)

It then stores the raw new state as the current state. The action setter accepts
action ids below `24`; a five-value subset takes an extra interrupt/reset path;
it also clears the attack slot and rewrites the action flags.

This is a fragment, not an enum. Most state values collapse to the neutral
action, the values are consumed by branch shape rather than by a table, and the
behaviour-entry ids are a different space from the state byte. **No global
action-id → named-state table exists in the shipped data or in the examined
code.** Any implementation that invents one is guessing.

## 6. Question 4 - what actually drives the tick

Within the examined paths:

- The **AI decision tick** is driven by the AI flag word, the current state byte,
  the queued state byte, the cached target position and the engagement radius.
  It selects action ids.
- The **state tick** is a small state machine over the current state byte:
  the post-action state consumes the queued state; the attack/recover state
  branches on a counter and the attack slot; the strike state sets a specific
  action; the default sets the neutral action.
- The **animation-set refresh** is driven by the timeline action kind/sub-phase
  and, when the state is non-zero, a weighted pick over the model's entry bytes.
- The **base actor update** is driven by the timeline's action kind (1..5), its
  sub-phase, its current/next clip pair and its root-motion deltas. It issues the
  clip change, applies root motion and, for the strike kind, assembles the hit
  record. It never reads a relation value.

So the counter/selector distinction is:

- **state** = selector (branch and animation-set choice),
- **relation** = phase tag (written by engagement, not read here),
- **action id** = transition/action code (third field).

## 7. Falsification ledger

| Hypothesis | Observation that would refute it | Tested? | Result |
|---|---|---|---|
| `relation` indexes a relation/graph table | a read that scales or adds by the relation value | Examined the decision tick, prowl step, state tick, base update | No such read; whole-module coverage not exhausted → PARTLY |
| `relation` is the tick driver | the tick branches on a relation value | Same paths | No; tick branches on state/queued state → REFUTED for examined paths |
| AI state indexes the 8x3 row | observed state values outside `0..7` | Corpus census + code constants | States `8`, `9` observed → REFUTED |
| Entry id indexes the 8x3 row | entry ids outside `0..7` | `--actor-census` | ids `2..60` → REFUTED |
| The model selector byte selects a fixed-stride behaviour entry | selector not used as a scaled index into the entry array | Decompiled the behaviour-set refresh | Selector × fixed stride + entry id passed to animation setter → CONFIRMED |
| The 8x3 table is the state→clip map | a consumer that indexes it by state | Searched the examined AI/animation paths and the corpus | No consumer found → UNKNOWN |
| A global action-id enum exists | ids that are reused with different meanings, or code-local collapse | Binary setter + corpus ids | Code collapses many states to neutral; ids are model-relative → REFUTED |
| A real monster reaches `advance()` | a live monster whose render class installs a tick hook | Headless run + code path | All shipped monster names classify `Unclassified` → REFUTED |

## 8. Method yields (what each produced and what it did not)

### A. Binary evidence

Produced:

- the **AI decision tick** (flag word + state + cached target + engagement
  radius → action ids),
- the **prowl/wander step** (random point + world query + re-cached target),
- the **state setter** (raw state stored; a few state→action pairs),
- the **state tick** (post-action/queued-state, attack/recover, strike, neutral),
- the **behaviour-entry selector** (model sub-record byte × fixed stride),
- the **animation-set selector** (state + weighted entry pick),
- the **base actor update** (timeline action kind/sub-phase/root motion).

It did **not** produce: the relation field's reader; the 8x3 row selector; the
second (initial-set) behaviour selector; the full transition table; any runtime
memory observation.

### B. Data correlation

Produced: 21 actor containers, 75 entries, 37 distinct ids; 14 clean entry
arrays; entry ids `2..60`; trios `0..66`; family-shared rows. It
refuted the two natural 8-row index hypotheses (section 4.2) and showed there is
no "entries = rows" invariant.

It did **not** produce: a row→named-state map; a trio→clip map.

### C. Real monster observation

Two level units that place monsters were run headless, read-only, 60 ticks,
twice each:

```text
unit=skinnr1 placed=83 admitted=41 tracked=15 skipped_markers=42 inert=0 unresolved=0 live=41 ticks=60 digest=5aefa6d0f4627f69
unit=mrlock1 placed=16 admitted=16 tracked=9  skipped_markers=0  inert=0 unresolved=0 live=16 ticks=60 digest=2ce859de83fb3110
```

Both units are deterministic across the two runs. `skinnr1` places `skinner`
(and other monsters); `mrlock1` places `sluagh` and `ronin`. The models are
census-clean actors, so the loader attaches an `ActorRuntime` to each of them
(`unresolved=0`, `live` equals the admitted count). The engine's CLI does not
print a carrier count, so this is code+data evidence, not a runtime counter.

It did **not** produce: a live state/relation trace (the engine does not expose
one), or a monster that actually ticked.

The command for reproduction is in section 11.

## 9. The dispatch gap: real monsters never reach `advance()`

This is the finding that reframes the four questions.

- The spawn path sets `LiveObject::class` from **`classify(&spawn.name)`**, where
  `spawn.name` is the *placed record name*.
- `classify` installs the monster hook only for names that **start with
  `monster_`**.
- Shipped levels place actors under bare model names. Direct readings of the
  shipped units show `object skinner`, `object sluagh`, `object ronin`,
  `object raziel`; none carries a `monster_` prefix. The placed-record name field
  is also too short to hold `monster_<name>`.
- Therefore every real monster live object is `RenderClass::Unclassified`, so
  stage 12 installs no tick hook and `actor.advance(tick)` is never called.
- The bound player is different: `bind_player` assigns `RenderClass::Raziel`
  directly, so the player's carrier does tick.

In the original, the class is selected from the placed record's **prototype
name**, which is a separate field from the instance name. This engine does not
model that field, so it uses the instance name and loses the class.

**Consequence for the design verdict:** `NotInterpreted` was not a wrong
decision - inventing transitions would still be wrong. But the milestone's
premise ("the actor tick runs for monsters") is false against real data: the
only real actor that reaches `advance()` is the player, plus synthetic names in
tests. The synthetic tests use `monster_wallcr` / `monster_wal`, which is why the
gap stayed invisible.

**Resolution (2026-11-06).** `object-prototype` established that the placed
record carries no prototype: the placed name is the model-container lookup key,
and the class comes from the loaded model's prototype plus the actor-family
identity. The dispatch key now uses the model prototype, so real monsters
classify `monster_` and reach `advance()`. See
`docs/notes/object-prototype.md` and the `engine-prototype-class` change.

## 10. What this does NOT establish

- The row selector for the 8x3 trio table.
- The reader of the relation code, and therefore whether it is ever used as an
  index.
- The second (initial-set) behaviour selector.
- A complete state set and the full transition table.
- That the 8x3 trio table is read at runtime at all (a transform at load remains
  possible; `actor-behaviour-data` flags this).
- That the state values seen statically are the only values at runtime.
- Any damage, health, death or hit-resolution semantics.
- Anything about `raziel`'s player state block (different family).
- Runtime timing/cadence of the decision handlers (static only).
- That `classify` from the instance name is wrong for *every* class - only that
  it cannot produce `monster_` for shipped monster instances.

## 11. Minimal, safe proposal for `ActorRuntime::advance`

**`advance` still cannot implement a transition safely.** The confirmed facts do
not supply an input the engine has:

- the state setter is driven by **event messages and a queued-state field**, and
  the engine has no event/message layer and no queued state;
- the AI decision tick needs a **target/engagement record, a cached target
  position, an engagement radius and a world query**, none of which the engine
  models at the actor layer;
- the action-id space is **code-local and partial** (a five-case state map and a
  five-value special subset), so copying it would be inventing a machine from a
  fragment;
- the base update needs the **animation timeline** (action kind, sub-phase,
  clip pair, root motion), which the engine does not have.

What is safe now, in order:

1. **Fix the dispatch key first (not `advance`).** Derive the render class from
   the model's prototype/class name (the field the original uses), not from the
   placed instance name. Until that is done, no monster carrier can ever be
   advanced, and any `advance` body is dead code for real levels. This is a
   production change for a later slice, not part of this docs-only PR.
2. **Keep `advance` returning `NotInterpreted`.** It should keep changing
   nothing. The fields are correctly raw bytes and the fold is correct.
3. **If a non-behavioural step is wanted**, the only defensible one is a
   *counter* owned by the engine (for example a tick counter), not a relation or
   state selector. Do not copy the state→action fragment into `advance`; it is
   not the machine and it would freeze a guessed table into the golden fold.
4. **Rename or document the field meaning.** `ActorRuntime::state` currently
   conflates the AI state byte, the queued state and the action id. Before any
   implementation, decide which one it is. The evidence supports the AI state
   byte (the branch selector), not the behaviour-entry id and not the action id.

The documented next probe is a controlled runtime observation of one monster:
watch the state and action fields across an idle → alert → strike → recover
cycle on a real unit, and confirm which writer runs per frame. Until that
exists, `NotInterpreted` remains the honest implementation.

## 12. Reproduction

Bounded static pass with Ghidra 12.1.4 on the shipped SR1 module, read-only,
scratch in `%TEMP%`. Two-clean-run corpus census:

```text
--actor-census --bigfile <game>/1/BIGFILE.DAT --objects-dir <game>/1/OBJECT
actors=21 entries=75 distinct_ids=37
```

Deterministic real-monster headless runs (read-only, runtime only):

```text
--play --headless --bigfile <game>/1/BIGFILE.DAT --unit skinnr1 --ticks 60
--play --headless --bigfile <game>/1/BIGFILE.DAT --unit mrlock1 --ticks 60
```

No game bytes, decompiled listings, addresses or absolute install paths are
recorded here. `cargo test` stayed at its 459-green baseline; no production code
was changed.

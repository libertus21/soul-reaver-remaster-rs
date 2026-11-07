# actor-state-writer - who writes the state, and what forces the class

Bounded, read-only static pass on the shipped SR1 module (Ghidra 12.1.4) plus a
read-only corpus correlation over the shipped object containers. Behaviour
paraphrases and structural facts only: field roles, bit masks, counts and short
asset names. No addresses, no field offsets, no disassembly listings, no game
bytes, no absolute install paths. Scratch work stayed in `%TEMP%`; the binary and
the archives were opened read-only.

Date: 2026-11-06. Evidence class per claim: **CONFIRMED** (code and data agree,
or two independent observations agree), **PARTLY KNOWN** (one side pinned),
**UNKNOWN** (explicitly not established).

This note continues `actor-state-semantics` (sections 9-10: what is confirmed and
what is still open) and closes the two items that kept `ActorRuntime::advance` at
`NotInterpreted`: the forced class flag and the state writer chain. It does not
change production code and does not move a golden.

## Verdict

| Item | Finding | Class |
|---|---|---|
| A. Forced class flag | The classifier reads a 32-bit class-flags word. Zero-based **bit 18** (mask `0x0004_0000`) forces the `physical` row; zero-based **bit 19** (mask `0x0008_0000`) forces the `monster_` row; the physical test wins when both are set. If neither bit is set, the prototype string is compared **exactly** against the six-row table. | CONFIRMED (code + corpus) |
| A. `physical` source | `physical` has its own forced source: bit 18 of the same class-flags word. No shipped container is named `physical`; shipped physical objects (`mfire`, `spout`, and 146 more) carry the bit instead. | CONFIRMED (code + corpus) |
| B1. State writers | The current-state byte has a bounded writer set: the parameter-program applier, the behaviour-entry applier, the behaviour-set refresh (queued-state consumer), the event/message dispatcher, and seven further state writers. Only the refresh is reached from the per-family bodies; its per-frame cadence is inferred, not re-observed. | CONFIRMED for the located writers; family-body cadence PARTLY KNOWN |
| B2. What drives state | State is a **selector**, driven by data (the actor parameter program and the selected behaviour entry) and by event messages. It is **not a counter** and **not a timeline result**. AI decisions write the separate action id, not the state byte. The base actor update is timeline-driven. The earlier "state (current + queued) is the selector; the base update goes through the timeline" reading is **VERIFIED**. | CONFIRMED (examined paths) |
| B3. 8x3 trio table | The shipped 24-byte 8x3 table is **not read in raw form** at runtime. At load it becomes a pointer array of expanded records (consistent with the eight shipped rows); the runtime state id sits in a different field from the file-side entry id. Whether the three clip bytes seed the expanded records is UNKNOWN. | Transform CONFIRMED; slot count and byte-to-record mapping UNKNOWN |
| B4. Queued state | The queued state is a **real second field**, and there is a third alternate-state field. When the current state is the post-action state, the current state is replaced by the alternate state if set, otherwise by the queued state. | CONFIRMED |
| B5. Minimal transition | There is **no timer transition**. Without an event or a data application, the state byte does not change on its own. The only non-event change is the post-action remap, and it requires the post-action state to have been set already. `advance` therefore needs **event input**, not a timer. | CONFIRMED statically; not runtime-observed |

**Consequence:** `ActorRuntime::advance` cannot get an evidence-backed transition
from a tick alone. Even with item A closed, the class fix is a dispatch-key fix,
not a state-machine step. `NotInterpreted` remains the honest body.

## 1. Method and what each method yielded

- **A. Static binary pass.** Re-pinned the class resolver, the object load-complete
  path, the AI initializer, the event/message dispatcher, the AI decision tick, the
  parameter-program applier, the behaviour-entry applier and the behaviour-set
  refresh. Then scanned the code section for the current/queued/alternate state
  fields and classified every candidate site as a store, a load, a compare or a
  branch target. The scan is exhaustive over the executable's code section for
  those field accesses; it is not a whole-program dataflow proof.
- **B. Data correlation.** Parsed every single-model object container in the
  archive index and the two loose actor containers, read the class-flags word, and
  correlated it with the 21 actor containers and the physical/effect objects. This
  is the independent side of item A.
- **C. Not run.** No game process was launched or instrumented. This pass is static
  plus data only, per the task boundary.
- **D. Falsification.** Section 8 states what would refute each hypothesis and
  whether that observation was tested.

## 2. Item A - the exact forced class bit

### 2.1 The classifier

The resolver takes an object and does, in this order:

1. If the object's class-flags word has **bit 18** set, choose the `physical` row
   directly.
2. Else, if it has **bit 19** set, choose the `monster_` row directly.
3. Else, compare the object's prototype string **exactly** (NUL-terminated
   equality, not a prefix) against the six class names. A match selects that row;
   no match stores the "no class" sentinel.

The physical branch is checked first, so a word with both bits set classifies as
`physical`. The six-row table is unchanged from `actor-ai` and `object-types`; the
new fact is the exact bit identity and the precedence.

### 2.2 Where the flags come from

The load-complete copy that fills the per-object flags slot takes the value from
the object container's header class-flags dword. The corpus confirms it:

- **All 21 actor containers** - the 19 archive actors plus the two loose actors -
  carry **bit 19** in that header word. The actor-family block signature and the
  bit agree on every shipped actor.
- **148 object containers** carry **bit 18**, including the physical/effect
  objects `mfire` and `spout`. No container is named `physical`, so the bit is
  the only shipped source for that row.
- Ordinary props (`blade`, `dndoor`, `hitme`, `turn`, `steam`, `funnel`, `lantrn`)
  carry neither bit; they fall to the exact-name comparison and, having no matching
  prototype, end at the no-class sentinel.

This closes the item-A INFERENCE: the forced monster family is not an unspecified
"actor-family flag" - it is **bit 19** of the object header class-flags word, and
`physical` has the symmetric **bit 18**.

### 2.3 What this does NOT prove

- It does not prove the bits have no other meaning in other subsystems; only their
  use by the classifier is observed.
- It does not prove every bit-18 container is a physical object; the bit role is
  confirmed, the per-container classification was spot-checked, not exhaustively
  labelled.
- It does not prove a script cannot change the bits at runtime.
- It does not prove the engine's prefix match is equivalent to the original's
  exact match for unshipped names. For the shipped padded eight-character names
  the two agree; for a longer prototype they would not.

### 2.4 A field-lifecycle hazard

The per-object slot the classifier reads for class flags is later read by the AI
initializer as a **pointer to the actor parameter program**. The class is stored
on the object before that reuse, so a class fix must run at load-complete, before
actor initialization. The exact writer that installs the parameter-program pointer
was not located; the reuse itself is observed by two reads of the same slot with
different roles on the same object layout. Treat the slot as load-time only.

## 3. Item B1 - which functions write the state

A byte scan of the code section for the current-state field returned 42 candidate
sites. Instruction-shape classification leaves **11 true writers**. The rest are
loads, compares or branch targets. The writers, by role:

| Role | What it writes | Trigger | Per-frame? |
|---|---|---|---|
| Parameter-program applier | Current state, queued state and alternate state from a compact opcode/parameter record list | Actor activation; the record list is already attached to the object | No (activation path) |
| Behaviour-entry applier | Current state and queued state from the selected behaviour entry's state byte, plus animation parameters | Called only by the parameter-program applier | No |
| Behaviour-set refresh | Current state, by consuming the alternate/queued state when the current state is the post-action state | Per-family body (installed by behaviour dispatch) | Conditionally; cadence inferred |
| Event/message dispatcher | Current state, from the current action byte, on two specific message ids | Event/message path | Event-driven |
| Seven further state writers | A fixed or parameter state value (observed 0, 7 and 9 among them) | Installed through behaviour/state dispatch | Cadence not traced |

The queued-state field has two writers (the parameter-program applier and the
entry applier). The alternate-state field has one writer (the parameter-program
applier). Those counts come from the same exhaustive field-access scan.

**Answer to B1:** the writers are not one "state setter" and not the reader-side
state tick. The state is written by the data appliers at activation, by the event
dispatcher, by the post-action remap, and by seven further state writers. Only the
remap could run without a new event, and it cannot introduce a new state - it only
substitutes a pre-set queued/alternate value for the post-action state.

## 4. Item B2 - what drives the writers

The earlier reading is **verified, with one correction**:

- **State is the selector.** The AI decision tick branches on the current state,
  the behaviour-entry refresh reads it, and the animation-set selection uses it.
  The state is not a counter: no writer increments or decrements it.
- **The base update is timeline-driven.** The locomotion/animation update branches
  on the timeline action kind and sub-phase, not on a counter and not on the state
  byte directly.
- **AI decisions write the action id, not the state.** The shared decision tick
  reads the AI flags, the state, the target position and the reach/engagement
  data, returns a small transition code, and the result goes to the action-id
  setter. The action id is a separate field from the state byte.
- **Correction to the earlier note:** the earlier note described a single "state
  setter" that maps a few new states to action ids (`2→5`, `4/8→20`, `9→19`,
  else `1`) and stores the raw state. This pass did **not** re-locate such a
  single function. What it found is the action-id setter plus individual call
  sites that choose action ids for event/message cases and AI decisions. Treat the
  single-map reading as **unverified**, not refuted: the call-site pattern is
  real, the one-function framing is not established.

So the driver classification is: **data/event-driven state selector + AI/action
decision output + timeline-driven animation**, not a counter and not a timeline
result.

## 5. Item B3 - is the 8x3 trio table read at runtime?

The shipped 24-byte table is **not consumed in raw form**. The evidence:

- The runtime behaviour entries are pointer-indexed records expanded well beyond
  the file-side record size, and the runtime state byte sits in a different field
  from the file-side entry id. The file-side entry-id position cannot be used
  against the runtime record.
- The runtime model asset exposes a pointer array of those expanded records plus
  a bound (a count and one pointer per slot). The refresh clamps its selector to
  that bound and picks one pointer.
- The raw table's eight rows are consistent with the eight slots; at runtime they
  are records, not three-byte rows. The three clip bytes are not observed being
  read as a unit. The runtime slot count itself was not directly observed.

**Verdict:** the table is **transformed at load** into a pointer-array runtime
structure; it is not read as the shipped 24-byte form. Whether the three bytes per
row seed the expanded records' clip lists is UNKNOWN. The earlier "row selector is
unknown" item is partially narrowed: the runtime pointer array is indexed by a
reserved selector byte in the AI block, seeded from model data and from a
parameter-program record, and clamped to the slot count. Whether that byte is the
trio row or an independent facing/side preference is UNKNOWN.

## 6. Item B4 - is the queued state real?

**Yes, CONFIRMED.** Three distinct state bytes are written and read:

- **Current state** - the selector used by the decision tick and the refresh.
- **Queued state** - written by the parameter-program applier and the entry
  applier; read by the refresh.
- **Alternate state** - written by the parameter-program applier; read by the
  refresh with priority over the queued state.

The refresh rule is: when the current state is the post-action state, replace the
current state with the alternate state if it is set, otherwise with the queued
state. This is a real second (and third) field, not an artefact of the read.

## 7. Item B5 - the observable shape of a minimal transition

Given an initial state S and no event input:

- **No timer transition exists.** The exhaustive writer scan found no writer that
  increments, decrements or time-derives the state byte.
- **The only non-event change** is the post-action remap in the refresh. It
  requires the current state to already be the post-action state, and it only
  substitutes an already-written queued/alternate value. It cannot create a new
  state from nothing.
- The parameter-program and entry appliers run at activation, not once per frame.
- The event dispatcher and the further state writers need an event or a
  state-specific condition; those conditions were not runtime-observed.

**Answer:** the state does not change on its own from a bare tick. `advance` needs
**event input**, not a timer. This was established statically; it was not
runtime-observed (no game process was launched).

## 8. Falsification ledger

| Hypothesis | Observation that would refute it | Tested? | Result |
|---|---|---|---|
| Bit 19 forces `monster_` | an actor container without bit 19, or a non-actor object with bit 19 | Container scan: all 19 archive actor containers have bit 19; the two loose actor containers were checked separately and also have it; parsed object names with bit 19 are the actor family only | CONFIRMED |
| Bit 18 forces `physical` | a bit-18 object that is not physical, or a physical object without bit 18 | Classifier branch + 148 bit-18 containers including `mfire`/`spout` | CONFIRMED (role); per-object labelling spot-checked |
| The class table is a prefix match | a prototype that matches a row only by prefix | Resolver compares NUL-terminated strings exactly; shipped names are padded to eight | REFUTED for the original; engine divergence noted |
| The state is a counter | any writer that increments/decrements or time-derives the state | Exhaustive field-access scan over the code section; all stores inspected | REFUTED for the scanned code |
| The state is a timeline result | a timeline value stored into the state byte | State writers are data/event appliers and further writers; timeline drives the action/anim path instead | REFUTED for the examined paths |
| A single state setter maps states to actions | a function that takes a new state, maps it to an action id and stores both | Writer/action-setter inspection; no such single function found | NOT RE-LOCATED (unverified, not refuted) |
| The queued state is an artefact | only one state byte with two names in the read | Distinct store and load sites for current, queued and alternate | REFUTED |
| The 8x3 table is read raw at runtime | a consumer that reads three contiguous bytes per row as clip ids | Runtime uses a pointer array of expanded records; file-side id field is not used | REFUTED for raw read |
| State changes from a bare tick | a per-frame writer that creates a new state without an event | Exhaustive writer scan; only the post-action remap exists, and it needs a pre-set post-action state | REFUTED (static) |

## 9. A minimal, safe `advance()` specification

### 9.1 What can be implemented now

**Nothing behavioural.** The confirmed facts still do not supply an input the
engine has:

- the state is written by the **actor parameter program** and by **event
  messages**, neither of which the engine models;
- the behaviour entry is a **load-transformed record**, not the shipped 32-byte
  form, so reading the file-side entry id as state would be wrong;
- the AI decision tick needs a **target/engagement record, a cached target
  position, a reach/engagement radius and a world query**, none of which exist at
  the actor layer;
- the action id is a **separate, decision-produced field**, not the state byte.

Therefore `advance` must keep returning `ActorTransition::NotInterpreted` and must
keep changing nothing. That is not a placeholder failure; it is the only body the
evidence supports.

### 9.2 What must remain unimplemented

- Any state transition table or state→action map.
- Any timer, counter or frame-count-driven state change.
- Queued/alternate-state consumption without the post-action precondition and
  without the activation/event writers that populate those fields.
- Any use of the file-side behaviour-entry id as a runtime state.
- Any relation write (relation is still a separate engagement-phase field; see
  `actor-state-semantics` section 3).

### 9.3 API implication

A tick-only signature cannot express the confirmed writers. A future
evidence-backed `advance` needs an **event/message input** (the parameter-program
record or the event payload) in addition to the tick. Until the engine has that
layer, the honest signature and body stay as they are.

### 9.4 The one adjacent change that is now evidence-backed

Item A closes the class flag, so a later production slice can replace the
actor-kind proxy with the real object-header class-flags bits: bit 18 for
`physical`, bit 19 for `monster_`, exact-name comparison otherwise, and the
physical check first. This is a dispatch-key fix, not an `advance` step, and it is
out of scope for this docs-only PR.

## 10. What is still unknown

- The writer that installs the parameter-program pointer into the reused class-flags
  slot, and the exact point after classification.
- The per-frame cadence of the seven further state writers.
- Whether the three clip bytes of each 8x3 row seed the expanded runtime records.
- Whether the reserved selector byte in the AI block is the trio row or a
  facing/side preference.
- The full event-message set and the payload layout for every message that writes
  state.
- Any runtime observation of an actual idle → alert → strike → recover cycle.
- Whether scripts can override the class-flags bits or the state at runtime.

## 11. Cross-check against shipped data

- **21 actor containers** (19 archive + 2 loose) - all carry the forced-monster
  bit. Two of them (`soul`, `walbosb`) have degenerate behaviour data but still
  carry the bit, consistent with the family being a load-time class, not a
  behaviour guarantee.
- **148 containers** carry the forced-physical bit; the named physical/effect
  objects (`mfire`, `spout`) are in that set.
- Entry ids span `0x02..0x3c` across 21 actors; the state values the code compares
  are small (`7`, `8`, `9`, `10`). The two spaces are different, which is why an
  entry id must not be used as a runtime state.
- The 8x3 table has eight rows for every actor; the runtime pointer array is
  consistent with eight slots. The row count agrees; the three-byte grouping does
  not survive as a read unit. The runtime slot count itself was not observed.

## 12. Reproduction and method notes

Bounded static pass with Ghidra 12.1.4 on the shipped SR1 module, read-only,
scratch in `%TEMP%`. The container correlation was a read-only parse of the
archive index and the loose object containers. No game process was launched.

No addresses, field offsets, disassembly listings, game bytes or absolute install
paths are recorded here. `cargo test` stayed at its 463-green baseline; no
production code was changed and no golden was moved.

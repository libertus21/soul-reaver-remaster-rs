# actor-transition-table - what the eleven state writers actually do

Bounded, read-only static pass on the shipped SR1 module (Ghidra 12.1.4) plus a
read-only census of the shipped actor behaviour blocks. Behaviour paraphrases
and structural facts only: writer roles, state values, small message codes,
counts and table shapes. No addresses, no field offsets, no disassembly
listings, no game bytes, no absolute install paths. Scratch work stayed in
`%TEMP%`; the binary and the archives were opened read-only. No game process was
launched or instrumented.

Date: 2026-11-07. Evidence class per claim: **CONFIRMED** (code and data agree,
or two independent observations agree), **PARTLY KNOWN** (one side pinned),
**UNKNOWN** (explicitly not established).

This note continues `actor-state-writer` (the eleven writers, B1-B5) and
`actor-state-semantics` (relation, the trait spaces, the shared machine). It does
not change production code and does not move a golden.

## Verdict first

| Question | Answer | Class |
|---|---|---|
| Can `ActorRuntime::advance` be implemented now? | **NO.** A per-tick step cannot reproduce the machine: most writers are gated on an engagement record, a parameter program, an action-kind dispatch table and message codes the engine does not model. | CONFIRMED (static) |
| Is the state a single table lookup? | **No single transition table exists.** The next value comes from eleven conditional writers; only the activation program and the refresh are table/record lookups. | CONFIRMED |
| What is the refresh rule? | When the current state is the post-action value (**8**), current becomes the alternate state if it is set, otherwise the queued state. | CONFIRMED |
| Which writers reach the state through the event/message entry? | Exactly one directly: the behaviour-command applier, on message code `0x100007`. The two-code dispatcher is reached from per-family bodies, not from that entry. | PARTLY KNOWN |
| Is the state static with no events? | **Refuted in the strict sense.** Six world-driven writers can write the state with no external event. No writer is timer-only, so "no timer transition" still stands. | CONFIRMED (static) |
| What is the 8x3 runtime shape? | The three clip bytes of a row are read, per animation channel, out of expanded animation-set records; the behaviour-entry array is bounded by the model's entry count (1..10 shipped), not by eight. | PARTLY KNOWN |

The two headline corrections to the authority notes are in section 8: the
"single state setter" that `actor-state-writer` could not re-locate **does**
exist (it is writer W5 here), and the "eight runtime slots" reading of the
behaviour-entry array is **refuted** by the bound the code actually uses.

## 1. The eleven writers

Roles are stable; ordering is by causal layer, not by address. "Also writes"
lists the other state bytes the same writer touches. The four state bytes are:

- **current** - the selector the AI tick and the animation set read,
- **queued** - a pending state,
- **alternate** - a higher-priority pending state,
- **entry/action** - a fourth companion byte written from the behaviour entry
  and copied into current by three writers. It is not the current state, but it
  is a state source.

| # | Writer | Trigger | Written value | Also writes | Class | What it does NOT prove |
|---:|---|---|---|---|---|---|
| W1 | Parameter-program applier | Object activation: the class initialiser allocates the AI block and calls it once. It calls W2 unconditionally before it applies its own records. | Current and queued from program opcode `1`; alternate from opcode `0x14`; entry/action from opcode `2`; selector from a model byte. It also seeds animation/behaviour parameters (reach, turn rate, flags, sound, sub-state bytes). | queued, alternate, entry/action, selector | CONFIRMED | That the shipped programs contain these opcodes in a given order; that activation happens once per spawn rather than on every re-activation; the meaning of the nine other opcodes. |
| W2 | Behaviour-entry applier | Called only by W1, after the program records are parsed and before W1 applies its overrides. | Current = queued = the selected behaviour entry's state byte; entry/action = the entry's companion byte; selector clamped to the entry count; animation parameters copied. | queued, entry/action | CONFIRMED | That the entry's state byte is semantic rather than an animation key; the loader transform that builds the runtime entry record. |
| W3 | Behaviour-set refresh | Per-family body; two known bodies call it, and one of them requires a non-null engagement record and a clear "special mode" flag. | Re-selects the behaviour entry, then, **only if current is the post-action value 8**, current becomes alternate when alternate is set, otherwise queued. | none | CONFIRMED for the rule; cadence PARTLY | That it runs every frame for an unengaged actor; that alternate is "set" by any value other than the not-set sentinel. |
| W4 | Event/message dispatcher | Shared dispatcher called by twelve bodies with a message block. | On message codes `0x100000E` and `0x1000012`, when a busy flag is clear and a target property bit is set, current becomes the entry/action byte. | none | CONFIRMED for the two codes and the guard shape | The producer of those two codes; the meaning of the target property bit; that the dispatcher itself is the class event hook. |
| W5 | State setter | Called from six handler sites with a new-state argument. | Current = the argument verbatim; action id = code-local map: `2 -> 5`, `4` or `8 -> 20`, `9 -> 19`, anything else `-> 1`. | none | CONFIRMED | That the six call sites are the whole caller set (only direct callers are known); that the action map has global meaning (it is code-local). |
| W6 | Behaviour-command applier | Message code `0x100007` through the class event entry, plus eight other bodies. | Current = a five-bit field of the command word; action kind = a six-bit field remapped through a switch; sub-phase = a six-bit field clamped to a model count. | selector, entry pointer, action kind | CONFIRMED for the field extraction | The command producer; that the five-bit field is a state rather than an animation id; that the receiving method is hook slot 3. |
| W7 | No-target queued consumer | Per-family body, when a target pointer is null. | Action id = `2`; current = queued. | none | CONFIRMED | What clears that pointer; cadence; whether queued is non-zero at that point. |
| W8 | Turn/align body | Per-family body, when an alignment flag and a non-zero timer field are both set. | Current = the entry/action byte; action id = `0x14`. It also steps a sub-object's angle toward the target. | none | CONFIRMED for the write and guard | The alignment condition source; that this is the "turn then strike" step (role inferred). |
| W9 | Idle-reset body | Per-family body, when the action-kind slot selects it. | Current = `0` (idle) on the non-reach path; action id `1`, or `0x1C` with a flag when the reach gate passes. | none | CONFIRMED for the constant and the two action ids | The reach constants; that `0` means idle. |
| W10 | Engagement body | Per-family body, when the engagement record and model target data are both present and the busy flag is clear. | Current = `0` first; then current = `7` when inside the engagement distance and the gate passes. | none | CONFIRMED for the values; role PARTLY | That `7` means alert (inferred from the radius branch); the gate functions' semantics. |
| W11 | State-9 body | Per-family body, when the action-kind slot selects it. | Current = `9`; sets an AI flag and requests a cooldown action. It also releases one sub-object and re-runs an animation setup when a pointer is present. | none | CONFIRMED for the constant | That `9` means strike (inferred from the action map); the trigger that selects this body. |

Queued-state writers: W1 and W2 only. Alternate-state writer: W1 only. That
matches `actor-state-writer` section 3 and is unchanged by this pass.

### 1.1 What each writer is *not*

- None increments, decrements or time-derives the state. The "no timer
  transition" result of `actor-state-writer` is unchanged.
- None writes the state from the behaviour-entry **file-side id**. W2 reads the
  runtime entry's state byte; W3 passes the file-side id only to the animation
  setter. The two numeric spaces stay separate.
- None of W3, W7-W11 writes queued or alternate. Only activation does.

## 2. The transition decision procedure

There is no `(current, queued, alternate, event) -> next` table. There is an
ordered set of writers, each with its own guard. The defensible decision
procedure is below; each branch is tagged with its confidence.

```text
INPUTS  S  current state
        Q  queued state
        A  alternate state
        E  entry/action byte
        X  behaviour-entry selector (0..7, clamped to the entry count)
        T  engagement record present (and, for some branches, its cached target)
        F  AI busy/special flags
        M  incoming message or command code

ON ACTIVATION (once, class initialiser, W1):
  1. entry := behaviour-entry array[X]
     S := Q := entry.state                    [CONFIRMED]
     E := entry.action                        [CONFIRMED]
     X := model selector byte, clamped        [CONFIRMED]
  2. for each program record in order:
       opcode 1  -> S := Q := value            [CONFIRMED]
       opcode 0x14 -> A := value               [CONFIRMED]
       opcode 2  -> E := value                 [CONFIRMED]
     (nine further opcodes write non-state fields)   [PARTLY]
  3. if E == 8 and Q not in {4, 0x10}:
       S := Q := 4                             [CONFIRMED]

EACH TICK, IF THE ACTION-KIND DISPATCH SELECTS A BODY:
  4. refresh (W3): if S == 8: S := A if set else Q        [CONFIRMED]
  5. if the target pointer is null:  S := Q                [CONFIRMED]
  6. if aligned and timer != 0: S := E                    [CONFIRMED guard shape]
  7. reach gate (W9):          S := 0                     [CONFIRMED]
  8. perception (W10):         S := 0, then 7 if inside   [CONFIRMED values]
  9. post-action (W11):        S := 9                     [CONFIRMED value]

ON AN EVENT / MESSAGE:
 10. codes 0x100000E / 0x1000012, busy clear, target bit set:
       S := E                                             [CONFIRMED]
 11. command 0x100007:  S := (command >> 9) & 0x1F        [CONFIRMED extraction]
 12. any handler that calls the state setter (W5): S := argument,
     action := map(argument)                              [CONFIRMED]
```

### 2.1 The order is not fully known

Steps 4-9 are separate bodies reached through the same action-kind dispatch
table, and the relative order of two bodies that both run in one tick is not
established. The procedure above must be read as "which writers can fire",
not "in this exact sequence". This is the main reason a faithful `advance` is
still out of reach.

### 2.2 Where the rule really is a lookup

| Lookup | Shape | Index | Bound | Consumer |
|---|---|---|---|---|
| Behaviour entries | Pointer array of expanded records | Selector `X` | Model's behaviour-entry count (shipped 1..10) | W2, W6 |
| Animation sets | Fixed-stride records, three clip bytes read per channel | File-side entry id or a six-bit command field | Model's animation-set count, and the entry's channel count | Refresh (via the animation setter), W6, W4 |
| Action-kind dispatch | Two function pointers per action kind (global default), with per-actor override records | Current action kind | Global table; per-actor record id search | The class update body |

The three clip bytes of a shipped 8x3 row **are** read: the animation-set
selector reads consecutive clip bytes out of one expanded record, one per
animation channel, and the channel count comes from the selected behaviour
entry. Whether the loader wrote those bytes from the shipped 8x3 row is still
UNKNOWN, but the runtime consumer of a three-clip group is now located.

## 3. The event/message entry (hook slot 3)

The class method whose signature is `(object, message code, payload)` handles
external codes and forwards unknown ones to a generic queue. In the examined
path it reaches exactly one state writer:

- on code `0x100007` it calls the behaviour-command applier (W6), which writes
  current from a packed five-bit field;
- on the other handled codes it reaches animation, position and parameter
  helpers, not the state byte;
- unknown codes are queued, not interpreted.

The shared dispatcher W4 (the two state-writing codes) is **not** called by this
method. It is called directly by twelve per-family bodies. So the chain is
"per-family body -> W4", not "event entry -> W4", for the two-code write.

What this does **not** prove: that the `(object, message code, payload)` method
really is hook slot 3. It is the only analysable class method with that shape
among the eight, and the contact resolver's use of the event hook matches it, but the
binder that copies class methods onto the live object was not located. Mark the
slot identity PARTLY KNOWN.

## 4. What an actor does with no events

The earlier claim "without an event or a data application the state byte does
not change" is **too strong** and is refuted as stated:

- W3, W7, W8, W9, W10 and W11 are per-frame or per-dispatch bodies. They write
  the state with no external event; their input is world/target data and AI
  flags. W9 (`0`), W10 (`0`, then `7`) and W11 (`9`) write constants.
- W1 and W2 run once at activation.
- W4 and W6 need an event or command code.
- W5 runs only when one of its six callers decides a new state is due.

What survives from the earlier result: **no writer is timer-only.** Nothing
increments, decrements or time-derives the state. So an idle, unengaged actor
looks static not because "the state only changes on events", but because the
world-driven writers are gated on an engagement record, a busy flag, or an
action-kind slot that an unengaged actor does not reach. Which of those six can
run for an actor that has never acquired a target is not proven: W7 and W3 need
a record test, and W9/W10 need target data.

Stated exactly: **six of the eleven writers can fire repeatedly without an
external event**, and two more fire once at activation; **zero writers can fire
from a bare tick with no world data and no activation**; **the no-timer result
is confirmed**.

## 5. The 8x3 table at runtime

- **Slot count.** The behaviour-entry pointer array is bounded by the model's
  entry count, which the census puts at 1..10 (nine for `skinner`, ten for
  `aluka`). Eight is **not** the behaviour-entry slot count.
- **What the three bytes index.** In an expanded animation-set record, the bytes
  after the record header are per-channel clip ids. The animation-set selector
  reads them one per channel; the channel count comes from the selected
  behaviour entry. A three-byte group is therefore a three-channel clip set,
  consistent with the shipped trio shape.
- **What reads the expanded records.** The animation-set selector, called by the
  refresh (with the file-side entry id), by the command applier (with a six-bit
  sub-phase field), and by the dispatcher. The refresh also re-selects the
  behaviour entry from a separate fixed-stride entry array and hands the
  file-side id to that selector.
- **Still unknown.** The load transform itself (the loader that expands a
  24-byte row into a fixed-stride runtime record) was not located; only its
  consumer is pinned.

The shipped census agrees with the shape: every actor block carries eight trios,
and entry counts are 1..10 with 75 entries and 37 distinct ids.

## 6. Can `advance()` be implemented now?

**NO.** The exact behaviour that *could* be implemented today is only the pure
refresh rule:

```text
advance(current, queued, alternate):
    if current != 8: return current
    if alternate is set: return alternate
    return queued
```

That is real, code-confirmed behaviour. But wiring it into `advance` would be
dishonest without the input layer:

1. The engine has no queued, alternate or entry/action byte. Adding them means
   inventing their activation values, which come from the parameter program.
2. The engine does not parse the parameter-program record list (opcode, count,
   payload words) or the runtime expanded behaviour entries. It parses only the
   24-byte trios and the shipped file-side behaviour records.
3. The engine has no engagement record, cached target, reach/engagement radii
   or world query, so W3, W7, W8, W9 and W10 cannot be guarded.
4. The engine has no action-kind dispatch table, so the order and selection of
   the seven further bodies are not modelled.
5. Only three state-affecting message/command codes are pinned; the rest of the
   event set and every payload layout are unknown.
6. The relative order of the seven bodies within one tick is not established.

`ActorTransition::NotInterpreted` remains the honest body. The one defensible
next change is to add a **pure, unwired** `refresh(current, queued, alternate)`
function with unit tests for the four cases (current not 8; alternate set;
alternate not set; queued equals current). That freezes only the rule the code
proves, and it does not touch the fold or any golden.

## 7. Falsification ledger

| Rule | Observation that would refute it | Tested? | Result |
|---|---|---|---|
| Eleven writers of the current state | a twelfth byte-store to current, or one of the eleven being a load/compare | Instruction-shape scan of the code section for the current-state byte: immediate and register byte stores, plus stack/SIB forms; a wider store covering the byte also returns none | Exactly eleven distinct procedures; no others |
| W1 writes current+queued+alternate | a program opcode that writes only one, or W1 not writing alternate | Decompiled W1 end to end; three distinct store sites, one per byte | CONFIRMED |
| W2 writes current+queued | W2 writing alternate, or more than one caller | Decompiled W2; caller list has exactly W1 | CONFIRMED |
| Refresh uses post-action value 8 and alternate-over-queued | a different constant, or queued priority over alternate | Decompiled W3; comparison against 8; alternate tested first | CONFIRMED |
| The event entry reaches a state writer | no state write on any message code | Decompiled the class method; one call reaches W6 on `0x100007` | CONFIRMED for that code; slot identity PARTLY |
| The two-code dispatcher writes current from the entry/action byte | a different source or a different guard | Decompiled W4; two codes share one branch; source is the entry/action byte | CONFIRMED |
| The single state setter does not exist | a function that takes a new state, maps it and stores it | Found W5; map `2->5`, `4/8->20`, `9->19`, else `1`; stores raw state | REFUTED the earlier "not re-located" status |
| The runtime behaviour-entry array has eight slots | a bound other than eight | W2 clamps by the model entry count; census counts 1..10 | REFUTED (bound is the per-model count) |
| The three clip bytes are never read as a unit | a consumer reading consecutive clip bytes per channel | Animation-set selector reads consecutive bytes, one per channel | REFUTED |
| No timer transition | any increment/decrement/time-derived store | Writer scan and all eleven bodies | CONFIRMED (unchanged) |
| The state only changes on events | any per-frame writer that writes without an event | Six bodies write on world data | REFUTED (refined in section 4) |
| The queued state has one consumer | a second read that assigns current from queued | W7 assigns current = queued on a null target pointer | REFUTED (two consumers) |

All of this is static. None of it was runtime-observed; a single controlled trace
of an idle -> alert -> strike -> recover cycle would test the ordering and the
cadence claims that remain PARTLY.

## 8. Corrections to the authority notes

1. **The single state setter exists.** `actor-state-writer` section 4 said the
   one-function framing was "unverified, not refuted". This pass locates it
   (W5) and confirms both the raw store and the exact `2/4/8/9` action map.
2. **The behaviour-entry array is not eight slots.** It is bounded by the
   model's entry count (1..10 shipped). The "consistent with eight slots"
   reading in `actor-state-writer` B3 applies to the animation-set records, not
   to the behaviour entries.
3. **There are four state bytes, not three.** Besides current, queued and
   alternate there is an entry/action byte that the activation applier and two
   per-frame writers copy into current.
4. **Queued has two consumers.** The refresh consumes it only after the
   post-action value; the no-target body consumes it whenever its target pointer
   is null.
5. **The three clip bytes are consumed**, per channel, by the animation-set
   selector; the earlier "whether the three bytes seed the expanded records is
   UNKNOWN" is narrowed to "the consumer is known, the loader transform is not".
6. **"State is static without events" is too strong.** The correct statement is
   "no timer transition; world-driven writers can change the state without an
   event".

These corrections do not overturn the implementability verdict: they make the
missing input layer more precise, not smaller.

## 9. Explicit UNKNOWN list

1. The relative order of the seven further writers within one tick.
2. The per-frame cadence of each further writer, and whether an unengaged actor
   can reach W9, W10 or W11.
3. The mapping from class-row method index to live-object hook offset, and
   therefore the exact identity of hook slot 3 (the event entry is identified by
   body shape only).
4. The load transform that expands a 24-byte trio row into an animation-set
   record, and the model's animation-set count.
5. The producer and payload of every message code except the three that write
   the state.
6. The nine parameter-program opcodes that do not write state.
7. The meaning of the not-set sentinel for alternate (observed as the
   all-ones byte, not proven as a general sentinel).
8. Whether scripts can write the state bytes directly.
9. Any runtime observation of an actual idle -> alert -> strike -> recover
   cycle.
10. Whether the six per-family bodies that call the event dispatcher cover every
    shipped actor family.

## 10. Cross-check against the shipped corpus

Read-only census over the shipped actor containers, two clean runs, reproduced
here because it constrains the table shapes:

```text
actors=21 entries=75 distinct_ids=37
```

- Entry counts: 1, 3, 4, 5, 7, 8, 9, 10. Nine for `skinner`, ten for `aluka`.
  Eight occurs, but it is not invariant, which is why the behaviour-entry array
  cannot be the 8-row trio table.
- Every actor block carries eight trios; the trio shape is invariant even for
  the actors whose entry array does not parse cleanly.
- Entry ids span the small global space; the state values the code writes are
  `0`, `4`, `7`, `8` and `9`, plus arbitrary values through the setter and the
  command field. The two spaces are different.

## 11. Method

- Byte scan of the code section for the current, queued and alternate state
  bytes, using instruction-shape masks so that loads, compares and branch
  displacements are excluded. Eleven distinct current-state writers survive;
  two queued writers and one alternate writer agree with the authority note.
- Decompiled each writer and its direct callers, and the class table row for
  `monster_` (name plus eight method pointers). Identified the activation,
  locomotion, query and message-entry methods by body shape.
- Decompiled the action-kind dispatch selector, the state setter, the action-id
  setter and the animation-set selector to pin the three lookup shapes.
- Read-only `--actor-census` over the shipped containers to cross-check counts,
  ids and trios. No game process, no instrumentation.
- Falsification ledger in section 7; every proposed rule names its refuting
  observation and whether it was tested.

`cargo test` was 481 green before and after this docs-only change. No production
code was changed and no golden was moved.

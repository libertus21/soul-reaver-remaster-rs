# actor-ai - how an actor is updated per frame

Bounded, read-only Ghidra 12.1.4 pass on the SR1 remaster binary. Behaviour
notes only: field offsets, counts, engine role names and short name lists. No
addresses, no disassembly, no decompiled text, no raw game bytes, no absolute
install paths. The Ghidra project and scripts lived in `%TEMP%` only; the game
binary was opened read-only and was never copied or modified.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (two independent
observations agree, or code and data agree), **INFERRED** (one side only),
**UNKNOWN** (explicitly not established).

This note continues `object-behaviour` (name-keyed dispatch, the 48-slot
tracker) and `s4-run-internals-behavior` (bootstrap). It answers the actor
question only: what happens per frame once an object is a live actor.

## Verdict

1. **There is a per-frame actor pass, and it is a hook call, not a name
   switch.** The in-game frame tick walks the live-object list and calls a
   per-object function pointer at `+0x1A8`. Nothing in that pass looks at the
   object name. (CONFIRMED)
2. **Each live object carries three behaviour hooks** installed from a
   six-entry class table at load-complete: `+0x1A0` (event/message entry),
   `+0x1A8` (per-frame tick), `+0x1B0` (query/state getter). The class is
   `raziel__`, `sreavr__`, `glphicon`, `physical`, `monster_` or `particle`,
   selected from the prototype name at `+0x30` and stored at `+0x04`.
   (CONFIRMED)
3. **The actor roster names are management keys, not AI keys.** The ten-name
   roster gates spawning under a debug flag, drives the low-memory "dump a
   monster" path and the debug "kill roster" path. No roster name is compared
   in any AI, animation or state function. (CONFIRMED)
4. **Behaviour is parameterised by data, selected per model.** An AI instance
   block at `+0x2C0` points at a behaviour/action-set entry chosen by a byte in
   the model data; the per-actor native update variants are installed per model
   class, not per name. Two actors with the same model share code and differ by
   data. (CONFIRMED for the selection mechanism; the full parameter set is
   partly UNKNOWN.)
5. **`raziel` is not a special case in the loop; it is a class.** The player is
   the `raziel__` class instance, ticked by the same hook mechanism, but its
   tick is a dedicated player update (input, movement solver, interaction
   queries), and it is additionally updated directly by the frame tick before
   the generic pass. (CONFIRMED for the wiring; per-branch detail partial.)

## 1. Where the frame is and where actors are ticked

The outer loop is a game-state machine. In its in-game state it runs one frame
coordinator, and the coordinator's ordered work is:

| # | Subsystem | Evidence | Class |
|---:|---|---|---|
| 1 | Frame delta / fixed-point timestep | frame clock reads, dt written to a global step | CONFIRMED |
| 2 | Per-frame scratch reset (contact lists, accumulators) | bulk clears of the world scratch block | CONFIRMED |
| 3 | Script/event unit execution (16 units) | `Processing unit %s` logged per active unit | CONFIRMED |
| 4 | Per-unit audio emitters | per-unit list walked, emitter positions pushed | INFERRED |
| 5 | Clock / time-of-day | table of day-phase values written | INFERRED |
| 6 | Object activation and distance culling | live list walked; per-object query hook called with `0x23` and `0x2f`; cull radius compares distance to camera | CONFIRMED |
| 7 | Per-unit animation/timer advance | per-unit table of counters incremented modulo a period | CONFIRMED |
| 8 | **Player movement solver** | 4-step sequential move/collide/commit driven from the frame tick with the `raziel` object | CONFIRMED |
| 9 | **Player input → movement** | stick bits read, camera-relative delta applied to the player object | CONFIRMED |
| 10 | **Pairwise contact broad phase + resolver** | O(n²) buckets, distance vs summed radii, both directions resolved | CONFIRMED |
| 11 | **Hit-record pass per object** | per-object attack/hit list walked, resolved | CONFIRMED |
| 12 | **Per-object tick (`+0x1A8`)** | live list walked, hook called with a per-object timestep | CONFIRMED |
| 13 | Area/trigger query pass | spatial query from a cylinder around each object | CONFIRMED |
| 14 | Moving-object world/contact pass | objects flagged "moving" get a world query, result written back | CONFIRMED |
| 15 | Render submission | scene/queue submission calls | CONFIRMED |
| 16 | Streaming / prefetch | level streaming call | INFERRED |
| 17 | Particles | particle update call | INFERRED |
| 18 | Post-frame spawn/despawn | spawn bookkeeping, object tracker recompute | CONFIRMED |

The actor answer is rows 10–14. Rows 8–9 show the player is handled twice: as
a dedicated solver before the generic passes, and again as an ordinary object in
them.

**Entry role names for re-derivation** (no symbols in the image, so re-pin by
structure, not by name): the game-state loop is the function whose switch has
an in-game case that calls the frame coordinator; the coordinator is the
function containing `Processing unit %s`; the actor tick pass is the function
that walks the live-object list and calls the `+0x1A8` hook; the contact
resolver is the function that callers reach from the pairwise broad phase and
that calls the `+0x1A0` hook on the victim.

## 2. The three behaviour hooks and the class table

At load-complete the object is classified from the prototype name at `+0x30`
against a six-entry table; the class index lands at `+0x04`. Later object
initialization binds three of that class's method slots into the live object:

| Hook | Slot | Meaning | Evidence |
|---|---:|---|---|
| `+0x1A0` | 3 | event/message entry; receives the world context and hit payloads | CONFIRMED (contact resolver calls it with the event block; per-actor variants installed here) |
| `+0x1A8` | 6 | per-frame tick | CONFIRMED (frame tick calls it once per live object) |
| `+0x1B0` | 4 | query / state getter (visibility, cull tests) | CONFIRMED (activation pass calls it with query ids) |

The class table is nine pointers per class: the class name plus eight method
slots. Classes with a missing slot get a null hook, which is how non-actors opt
out of the tick: `physical`, `glphicon`, `particle` and `sreavr__` leave the tick
slot empty. Only `raziel__` and `monster_` carry a non-null tick. (CONFIRMED)

For `monster_` the tick slot resolves to the same body as the class's
locomotion slot (a thunk), so a monster's confirmed per-frame hook is the
**base locomotion/animation update**. The interesting AI decisions are not in
that body; see 4. (CONFIRMED)

## 3. What the base actor update does

The base actor update is the body shared by the `monster_` locomotion slot and
the per-actor locomotion variants. In behaviour order it:

1. **Guards.** Returns immediately if the object has no controller, or if the
   animation timeline is not in its "running" mode. (CONFIRMED)
2. **Reads the animation timeline.** The timeline object hangs off the actor;
   it carries an action kind at `+0x0A` (1..5), an action sub-phase at `+0x0B`,
   the current clip/pose ids, and three root-motion deltas at `+0x40/+0x42/+0x44`.
   (CONFIRMED shape)
3. **Advances the timeline / resolves the pose change.** Depending on the pair
   (current clip, next clip) it either blends, snaps or holds; it issues the
   animation change into the two blend contexts at `+0x328` and `+0x448`.
   (CONFIRMED)
4. **Applies root motion.** The three deltas are added to the object position at
   `+0x9C/+0x9E/+0xA0` and, when the clip has a ground-contact flag, to the
   secondary position triple at `+0x180`. This is the actual "the actor moved"
   step; it is animation-driven, not steering-driven. (CONFIRMED)
5. **Ground/entity contact query.** For clips that need it, a world query with
   the model's contact extents returns a surface/entity and the result is merged
   into the timeline (step-up, stop, slide). (CONFIRMED)
6. **Combat/attack transition.** When the action kind is the "strike" kind, the
   attacker assembles a hit record (target, bone, radius mask) and queues it;
   the contact pass in the frame tick resolves it. (CONFIRMED)
7. **Action abort rules.** Certain action kinds abort on vertical velocity,
   water/lava flags, or "gate" objects; the code branches on the object class
   name string of the other body (`skgate__` appears in this path).
   (CONFIRMED for the branch; the object role is INFERRED)
8. **Animation state selection.** The final state is written as a small action
   id and pushed through a state→animation table (stride `0x10`) so the
   animation runtime knows which clip set to use. (CONFIRMED)

Per-actor native variants wrap this body and add one concern each, then tail
into it: a weapon/attack-window variant, an animation-hit-window variant that
pushes a damage event into the player object, and a knockback/hurt variant that
reacts to the timeline action kinds and to gate objects. (CONFIRMED as variants;
their assignment to specific models is INFERRED.)

## 4. The AI layer: behaviour entry, target, state ids

Each actor owns an AI instance block (`+0x2C0`). Its layout, as far as this pass
reached:

| Field | Meaning | Class |
|---|---|---|
| `+0x04` flags | bit 1 "engaged", bit 2 "busy/resolving", bit 4 "action running", bit 0x10 "post-action", bit 0x20000000 "ticked this frame" | INFERRED (individual bits from branch tests) |
| `+0x1A8` | **behaviour/action-set entry pointer** | CONFIRMED (written from model byte, read by state code) |
| `+0x23A` | cached target position (three `i16`) | CONFIRMED |
| `+0x26E/+0x26F/+0x271` | current/queued state bytes | INFERRED |
| `+0x272` | facing/side preference used by the strike sequence | INFERRED |
| `+0x27A` | current attack/animation slot, `0xFF` = none | CONFIRMED |
| `+0x280` | model data pointer | CONFIRMED |
| `+0x290` | target/engagement struct (target pointer, radii, relation code, next) | CONFIRMED |

**Behaviour selection is data.** The behaviour entry is chosen from a byte in
the model data (two different model offsets are used, one for the initial set
and one for the later set). The entry is 0x20 bytes: an entry index at `+0x1B`
gives the initial animation state, and the entry feeds the state→animation
table. This is the mechanism that makes `skinner` and `morlock` differ without
different code. (CONFIRMED for the mechanism; which model byte selects which
named set is INFERRED.)

**Perception and engagement.** The state handler computes the 3D distance to
the current target and compares it with the model's engagement radius (a model
`i16`) and with a vertical offset. Inside the radius it selects the alert/
combat animation trio (three ids from the behaviour entry); outside it selects
the idle trio. It also writes a relation code 0..3 into the target struct
(idle / closing / striking / disengaging by the branch shape). (CONFIRMED)

**Attack gate.** A separate handler checks melee reach: horizontal+yaw to the
target relative to the actor's facing, the target's "attackable" flag, and the
target's type; only then does it request the strike state. A turn-to-face step
rotates the actor at a model-driven angular rate and only then transitions to
the strike, which is why attacks visibly wind up. (CONFIRMED)

**Disengage.** Another handler clears the engagement, resets the attack slot,
and restores the idle set; it is the "lost target / blocked / gate" exit.
(CONFIRMED as a handler; its exact trigger set is partly UNKNOWN.)

## 5. Shared state machine

There is **one** shared actor state machine, and it is expressed as action ids,
not as named enum constants in the image. The machine is driven by three
inputs: the AI relation code, the animation timeline action kind, and the
contact events. The states that this pass can evidence:

| State | Evidence | Class |
|---|---|---|
| idle / unengaged | idle animation trio selected; engagement flag clear | CONFIRMED |
| alert / tracking | alert trio selected; target cached at `+0x23A`; relation code closing | CONFIRMED |
| turning to face | angular step toward target, then transition | CONFIRMED |
| attacking / strike | hit record queued; attack slot set; `MultiHit` diagnostic exists for double hits | CONFIRMED |
| recovering / post-action | post-action flag; returns to idle or alert set | INFERRED |
| hurt / knocked back | knockback handler reacts to action kinds and to the attacker's hit | INFERRED |
| dead / removed | object hidden then destroyed; `LOW MEMORY: Dumping monster %s` path shows removal semantics | INFERRED |

Evidenced transitions:
- idle → alert when the target enters the engagement radius (CONFIRMED).
- alert → attack when the melee gate passes (CONFIRMED).
- attack → recover when the clip ends; recover → alert or idle (INFERRED).
- any → disengage when the target leaves range or the gate blocks (CONFIRMED
  for the handler, INFERRED for the trigger set).
- any → hurt on an incoming hit record (INFERRED; the damage handler is in the
  per-actor variant, but the magnitude source is not pinned).

The numeric ids for these states are **not** pinned: the machine stores small
bytes and small shorts (action ids up to `0x26`), and the same numbers mean
different clips per model. Do not invent an enum. (UNKNOWN)

## 6. Per-actor parameterisation: data, not code paths per name

The answer to "do the roster names get separate AI code" is **no**:

- The dispatch key found at the top is the **class**, six values, from the
  prototype name.
- The class's tick is one shared body per class family.
- Inside, the **behaviour/action-set entry** is looked up by a **model byte**,
  and the model is per actor file (`.crm`/`.pcm`/`.sra` family), so `skinner`
  and `morlock` differ because their models carry different entries.
- The native variants that exist are per *family* (weapon user, hit-window
  user, knockback user), installed by per-family setup code, not per name.
- The roster list itself is only used for spawn gating, the low-memory dump
  path, and the debug kill path.

So behaviour is **data-parameterised** with a thin per-family code layer.
(CONFIRMED for the selection; UNKNOWN for the full parameter semantics of the
behaviour entries — that is the next probe.)

## 7. `raziel` (the player): shared mechanism, divergent body

`raziel` is the `raziel__` class instance. It shares the object model, the live
list, the class table and the hook mechanism, and it is ticked in the same
generic pass. It diverges in four ways:

1. **Direct call.** The frame tick calls a dedicated player update before the
   generic passes, so the player is advanced even if the generic tick pass were
   skipped. (CONFIRMED)
2. **Input.** Only the player reads the stick/button state; the input handler
   feeds the player's movement and action selection. (CONFIRMED)
3. **Movement solver.** The player uses a 4-step sweep (probe, resolve,
   re-resolve, commit) that other actors do not use; monsters rely on
   animation root motion plus the shared contact pass. (CONFIRMED)
4. **Interaction queries.** The player's tick slot (`+0x1A8`) is an interaction
   query body (reach/attack/interact rays and camera-relative tests), not the
   locomotion thunk the monster class uses. (CONFIRMED)

Player damage taken, death and respawn were not followed in this pass; the
damage event path is shared with actors (the contact resolver's `+0x1A0` call)
but the player-specific handling is UNKNOWN.

## 8. The trap: roster names vs behaviour

Prior notes established the roster. This pass shows why it must not be read as
an AI table:

| Use of the roster | What it does | Class |
|---|---|---|
| Spawn gate | if the "no monsters" debug flag is set, roster names are refused at spawn (two lists, one narrower) | CONFIRMED |
| Low-memory dump | under memory pressure, the largest live roster object is logged with `LOW MEMORY: Dumping monster %s` and destroyed | CONFIRMED |
| Debug kill | sets a global flag and destroys every live roster object | CONFIRMED |
| Anything AI | **no** roster name is compared in AI, animation or state code | CONFIRMED |

The eight-name boss/special list and the exact-name load-complete specials
(`MORLOCK`, `RONINBSS`, `ALUKABSS`, `SKINBOS`) adjust spawn counters and one
actor flag; they do not select AI code either. (CONFIRMED)

## 9. Evidence classes at a glance

| Claim | Class |
|---|---|
| Per-frame actor pass exists and calls `+0x1A8` per live object | CONFIRMED |
| Three hooks (`+0x1A0`, `+0x1A8`, `+0x1B0`) mapped to class slots 3/6/4 | CONFIRMED |
| Six-class table, class from prototype name, class stored at `+0x04` | CONFIRMED |
| Base actor update order (timeline → root motion → contact → attack → state) | CONFIRMED |
| AI instance block and behaviour entry selected from model byte | CONFIRMED (mechanism) |
| Engagement radius, melee gate, turn-then-strike | CONFIRMED |
| Named state enum | UNKNOWN (ids are numbers and model-relative) |
| Exact trigger set for disengage, hurt, death | INFERRED |
| Which model byte selects which named behaviour set | INFERRED |
| Damage magnitude source and health storage | UNKNOWN |
| Player death/respawn and player damage handling | UNKNOWN |

## 10. UNKNOWN (explicit) and next probes

1. **The numeric state ids and the full transition table.** Probe: enumerate
   the behaviour-entry state tables in read-only data (entry +0x1B and the
   state→animation table) and map id → clip set per model; then trace the
   transition writers.
2. **The scheduler for AI decisions.** The confirmed per-frame hook is the
   class tick; the decision handlers are reached from the state tables and the
   event hook, but whether the decision body runs every frame or only on state
   change is not established. Probe: instrument the call count of the decision
   handler per frame in a controlled run, or find its only caller and read the
   cadence.
3. **Health and damage magnitude.** The hit record carries target, bone and
   radius mask; where hit points live and how much damage a strike applies is
   not pinned. Probe: follow the hit-record consumer into the model stat block.
4. **Which model byte selects the named behaviour set.** Probe: read the model
   header bytes named in 4 across the actor corpus and correlate with the
   behaviour entries actually used.
5. **Script vs native ownership of specific encounters.** Level scripts can
   drive an actor through the event VM; how much of a given fight is script and
   how much is native is per level. Probe: per level, census the script
   opcodes that target actor ids and compare with the native state hits.
6. **The full native variant set.** Only samples were decompiled (weapon,
   hit-window, knockback). Probe: enumerate every installer that writes
   `+0x1A0` and group by body shape.
7. **The `0x6D00` animation block interior.** Still unknown from the prior
   note; this pass used it only as the load-complete anchor.

## 11. Method and legal

Three bounded passes inside the agreed budget:

1. Re-pin the frame: game-state loop → frame coordinator → the live-object
   walk that calls `+0x1A8`.
2. Re-pin the object: class table from the six class-name strings, the
   load-complete binder, and the three hook slots; confirm with the cross
   references of each hook.
3. Re-pin the AI layer: AI instance block, behaviour entry selection from the
   model byte, engagement/attack/state handlers; cross-check with the roster
   management paths to rule them out as AI keys.

Code was decompiled in `%TEMP%` only; nothing executable was planted in the
repo. Behaviour notes only: roles, offsets, counts, short strings. No
addresses, no listings, no decompiled bodies, no game bytes, no install paths.
The binary was opened read-only. No protection, packing or anti-tamper was
encountered on this line.

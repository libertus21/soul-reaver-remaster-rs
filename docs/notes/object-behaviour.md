# object-behaviour - how the engine turns placed objects into behaviour

Bounded, read-only Ghidra 12.1.4 pass on the SR1 remaster binary. Behaviour
notes only: field offsets, counts, engine role names and short name lists. No
addresses, no disassembly, no decompiled text, no game bytes, no absolute
install paths. The Ghidra project and scripts lived in `%TEMP%` only; the game
binary was opened read-only.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (code and bytes
agree, or two independent observations agree), **INFERRED** (one side only),
**UNKNOWN**.

## Verdict

1. **There is no per-frame dispatcher over the placed-object table.** The
   runtime table at level `+0xD0` (count `+0xC8`, stride `0x60`) is a
   **placement/lookup table**, not an update queue. Every direct reader found
   is a load-time pass, an id lookup or a name lookup. (CONFIRMED for the
   stride-`0x60` readers found by the scan; see 1.2.)
2. **The dispatch key is the 12-byte object name**, not the archetype code
   (`+0x26`), not `type`, not the render class (`+0x04`) and not a function
   pointer in the record. Names select the model container, the object-tracker
   slot and a small set of hardcoded engine roles. (CONFIRMED)
3. **Per-object runtime state lives outside the placed record**, in a fixed
   48-slot, name-keyed **object tracker** and in a global live-object list. The
   placed record keeps only the authored fields plus the flag word `+0x2C`.
   (CONFIRMED)
4. **Behaviour is partitioned by name lists**, not by the 74 archetype codes or
   the 25 types. The engine holds a ten-name always-spawn set, a ten-name
   actor roster, an eight-name boss/special set, a 22-name E3 asset override and
   a few exact-name special cases. (CONFIRMED)
5. **The ground-truth cases land as follows.** `warpg`/`wrpface` are load-time
   warp markers consumed by the level/link system, not actors. `marker` is
   inert. `campath` is path/waypoint data and never becomes an actor; its camera
   consumer is INFERRED, not directly located. (details in 5)

The prior `type` -> behaviour dispatch is therefore not merely unlocated: the
runtime does not use the type as a dispatch key at all. The type and the
`+0x26` code are authoring metadata that the loader preserves and no consumer
reads.

## 1. What iterates the placed-object table

### 1.1 The table

The placed-object table holds the runtime records described in `object-types`
and `level-geometry`: stride `0x60`, name at `+0x00`, authored id at `+0x14`,
the second `i16` triple at `+0x18`, world position at `+0x20`, archetype code at
`+0x26`, id/type dword at `+0x28`, flags at `+0x2C`, the prototype blob pointer
at `+0x30` and the sub-object pointer at `+0x38`. Count is level `+0xC8`, array
is level `+0xD0`.

### 1.2 Every direct reader found

A scan of the binary for the `+0xD0` field together with the `0x60` stride
returns ten functions. All of them fall into one of these buckets:

| Reader role | What it does with the table | Class |
|---|---|---|
| Instance-id lookup | Linear search comparing the authored id at `+0x14`; returns the record pointer | CONFIRMED |
| Name lookup | Linear search from a start index comparing the 12-byte name | CONFIRMED |
| Player spawn | Finds the `raziel` record and binds it | CONFIRMED |
| Load-time flag pass | Walks all records and sets/clears flag `0x4000` in `+0x2C` by matching the name against a 48-entry name table | CONFIRMED |
| Level initialization | Seeds the current level, then spawns a fixed ten-name set | CONFIRMED |
| Script operand relocation | Rewrites a source-record offset (`0x4C` stride) into a runtime-record pointer (`0x60` stride) for opcode `0x1B` of an event/command program | CONFIRMED |
| Link/transition resolver | Operates on a **different** `0x60` array (the link table, see 2.2), not the placed table | CONFIRMED (field shape) |

`+0x26` was searched together with the `+0xD0` field: the six functions that
contain both constants do not read `+0x26` from a placed record; their `+0x26`
hits are unrelated struct fields or opcode switches. The same holds for `type`
in the high half of `+0x28`. **No consumer of either field was found.**
(CONFIRMED absence in this scan; not a proof that none exists in an unsearched
path.)

### 1.3 Consequence

The placed table is a database that is **queried**:

- by **name**, for spawning and for the load-time flag pass;
- by **authored id `+0x14`**, for script/entity target lookup;
- by **source-record offset**, for event-program operand relocation.

Nothing walks it per frame and nothing switches on `+0x26` or `type`. A
"per-frame consumer of the placed array" does not exist in this binary; the
per-frame work happens on the live-object structures in sections 2 and 3.

## 2. The record families that were being confused

Several unrelated structures have a `0x60` or `0x38` stride. Keeping them
apart is the main trap in this area.

### 2.1 Distinct families

| Family | Owner | Stride | Count | Key fields | Class |
|---|---|---:|---|---|---|
| Placed objects | level `+0xD0` | `0x60` | level `+0xC8` | name `+0x00`, id `+0x14`, code `+0x26`, `+0x28`, flags `+0x2C`, blob `+0x30` | CONFIRMED |
| Link/transition records | level owner `+0x50` | `0x60` | first dword of the array | name `+0x00`, id `+0x14`, offset triple `+0x18`, flags `+0x1E`, destination pointer `+0x28` | CONFIRMED (shape) |
| Object tracker | global, 48 slots | `0x38` | fixed `0x30` | name `+0x00`, resource `+0x10`, state `+0x18`, anim index `+0x1A`, flag `+0x28`, aux `+0x30` | CONFIRMED |
| Level slots | global, 48 slots | `0x48` | fixed `0x30` | id `+0x00`, status `+0x04`, flags `+0x06`, level pointer `+0x08`, name `+0x10` | CONFIRMED |
| Live objects | global list | variable | list | animation block `+0x1C`, counters, flags, render class `+0x04` | CONFIRMED (list and block); internal fields partial |
| 36 -> 40 records | level | `0x28` | per level | tagged blob pointer, indices | CONFIRMED (unrelated to objects) |

The link array is the one the earlier notes called "the second pass that uses
the `+0x18` triple". It resolves a link record's destination by name against
the 48-entry level table and overwrites `+0x28` with the destination record
pointer. That is a **link**, not a placed-object behaviour field.

### 2.2 The link/transition path (portal and warp)

- Each link record carries a target name at `+0x00`, an id at `+0x14` and the
  offset triple at `+0x18`.
- The resolver matches the target name against the 48 level slots (status field
  `+0x04`, name at `+0x10`), finds the destination record in the target level
  and stores its pointer at `+0x28`, while adding the `+0x18` triple as a
  cross-level offset. (CONFIRMED)
- The literal `warpgate` is compared in the link/level-transition family. When
  a link target is `warpgate`, the warp index is written into the level slot
  instead of an ordinary destination name. This is the mechanism already
  documented in `warp-level-selection`. (CONFIRMED)
- `wrpface` appears once in read-only data and has **no code reference**; it is
  data, not a comparison key. (CONFIRMED)

## 3. The object tracker: the real per-object runtime

### 3.1 Allocation by name

The spawner is a single function that takes a name and a level:

1. Scan a fixed **48-slot** tracker (stride `0x38`) for a live slot whose name
   matches. (CONFIRMED)
2. If absent, take the first free slot (`+0x18 == 0`) and initialize it.
   (CONFIRMED)
3. Build three archive paths from the name:
   `\kain2\object\<name>.sra` (skeleton), `\kain2\object\<name>\<name>.crm`
   (mesh) and `\kain2\object\<name>\<name>.pcm` (model). In a special engine
   mode an alternate `object_e3` tree is used for a 22-name override list.
   (CONFIRMED)
4. Queue the loads asynchronously and write the name and state into the slot.
   (CONFIRMED)
5. When the model load completes, set slot state `2`, load the optional
   `\kain2\sfx\object\<name>\<name>.pnf` sound emitter, zero a `0x6D00`-byte
   animation block, and bind animation slots. (CONFIRMED)
6. If all 48 slots are live, print `The Object tracker is full
   MAX_OBJECTS=48.` and fail. (CONFIRMED)

A small wrapper spawns a name and returns the loaded object pointer; another
returns the tracker slot. User/level code therefore instantiates actors **by
name**, on demand. (CONFIRMED)

### 3.2 Ten always-spawned objects

Level initialization spawns these ten names unconditionally, in this order:

```text
raziel, paths, glphicon, sreavr, soul, force, particle, healths, eaggot, eaggots
```

These are engine **system objects** (player, path/camera support, renderer
helpers, pickup/effect managers), not placed-level content. (CONFIRMED)

### 3.3 Load-complete special cases

The load-complete callback partitions names into engine roles:

| Name or list | Action on load | Class |
|---|---|---|
| `MORLOCK` (exact, upper-case) | Set an actor flag and adjust a spawn counter | CONFIRMED |
| `RONINBSS`, `ALUKABSS`, `SKINBOS` | Adjust the same counter | CONFIRMED |
| boss/special list: `walbos`, `alklens`, `astat`, `wallcr`, `aluka`, `ronin`, `sluagh`, `vwraith` | Set a special flag | CONFIRMED |
| any other name | Generic path: model + skeleton + optional sound + animation | CONFIRMED |

At spawn time a separate ten-name roster is used as a gate when the engine's
"no monsters" debug flag is set:

```text
skinner, morlock, wallcr, ronin, aluka, sluagh, vwraith, vlgra, vlgrb, vlgrc
```

(CONFIRMED)

The 22-name E3 override list is an asset-path switch only:

```text
tripod, floor, flor, flr, sfxworm, switch, torcha, sfxpipe, sfxgear,
sfxvort, stair, sfxfalb, sfxwinb, sfxwinc, fire, lantern, sfxfall,
sfxwood, torchd, coffin, cptrcha, sfxrush
```

(CONFIRMED role as build-variant path; behaviour of the individual names
UNKNOWN.)

### Case rule in the engine skeleton (implementation decision)

The five name lists are matched ASCII case-insensitively. This follows the
level name resolver (`resolve_unit_entry`, `spawn_position`), which already
compares unit and object names without case. The four load-complete literals
(`MORLOCK`, `RONINBSS`, `ALUKABSS`, `SKINBOS`) are compared exactly, including
case, because the note records `MORLOCK` as exact upper-case.

The consequence is deliberate and tested: `MORLOCK` takes the actor-flag role,
while `morlock` (the roster spelling) takes the generic load path. This is an
implementation decision for the name-keyed spawn policy, not new evidence about
the original. See `src/engine/spawn.rs`.

## 4. Behaviour table by key

Legend: the key is the **name**, because that is the only runtime selector
found. "Generic" means the engine only loads the model/skeleton/sound and binds
animation; no dedicated game handler was located.

| Key (name) | Behaviour on load | Per-frame behaviour | Class |
|---|---|---|---|
| `raziel` | Player spawn; every level finds or binds it; a fallback name literal exists | Player/AI update family | CONFIRMED spawn; per-frame role INFERRED |
| `paths` | Always-spawned system object | Path/camera support (INFERRED) | INFERRED |
| `glphicon`, `sreavr`, `particle` | Always-spawned renderer/effect objects | Renderer/effect update | CONFIRMED spawn; role INFERRED |
| `soul`, `force`, `healths`, `eaggot`, `eaggots` | Always-spawned pickup/effect objects | Generic object update | CONFIRMED spawn; role INFERRED |
| `MORLOCK` and the actor roster | Tracker actor; debug-gated by roster | Generic actor update | CONFIRMED classification; AI UNKNOWN |
| `walbos`, `alklens`, `astat`, `wallcr`, `aluka`, `ronin`, `sluagh`, `vwraith` | Boss/special flag | Generic actor update | CONFIRMED flag; AI UNKNOWN |
| `skinner`, `vlgra`, `vlgrb`, `vlgrc` | Actor roster | Generic actor update | CONFIRMED classification; AI UNKNOWN |
| `warpgate` (link target) | Warp index in the level slot; load-by-name | Warp selection UI | CONFIRMED |
| `warpg`, `wrpface` | Warp-unit markers; no model container | None (not tracked) | CONFIRMED marker; see 5 |
| `campath` | Path node; no model container | None on the node itself | CONFIRMED; consumer INFERRED |
| `marker` | Marker; no model container | None | CONFIRMED |
| `splob` | Effect placeholder; no model container | None | CONFIRMED (prior note); role INFERRED |
| `cliff1` | Level-variant special case in the level builder | None per frame | CONFIRMED |
| `movie1` / `movie*` | Cutscene command special case | Cutscene sequencer | CONFIRMED literal; role INFERRED |
| any other name | Generic model+skeleton+sound+animation load | Generic object update | CONFIRMED |

No row is keyed by `+0x26`, by `type` or by the render class.

## 5. Ground-truth validation

### 5.1 `warpg` and `wrpface`

- Both are placed objects of type 7 and 12. They have no model container in the
  OBJECT archive (the prior `object-models` finding lists the three names
  without containers; `warpg`/`wrpface` belong to the no-container marker
  family).
- The engine never spawns them into the 48-slot tracker: the spawner is reached
  with names that resolve to model containers, and these do not.
- The warp handler is reached through the **link/level path**: the 14 warp
  units carry `warpgate` link records; the link resolver compares the literal
  `warpgate` and writes the warp index into the level slot. `wrpface` is data
  with no code reference.
- Conclusion: `warpg`/`wrpface` are **load-time warp markers**, not per-frame
  actors. They reach the warp system as markers for the level/link path, not
  through an object dispatch. **CONFIRMED.**

### 5.2 `campath`

- No model container, therefore never tracked as an object. (CONFIRMED)
- Its role is path/waypoint data: the engine always spawns a system object
  named `paths`, and the placed records carry authored ids that script/event
  operands can bind to. (CONFIRMED for no-actor and for `paths`; INFERRED for
  the id-to-path link.)
- The exact spline/camera function that consumes the `campath` ids was **not**
  located in this pass. The camera consumer is therefore **INFERRED**, not
  confirmed. This is the one ground-truth case that does not fully close.

### 5.3 `marker`

- No model container, no tracker slot, and no per-frame reader. The only code
  that touches it is the load-time flag pass that sets/clears `0x4000` in
  `+0x2C` by name. **CONFIRMED inert.**

## 6. Does the runtime record carry behaviour state?

**Not in the placed record.** The placed record carries:

- the authored id `+0x14`, the offset triple `+0x18`, the world position
  `+0x20`, the code `+0x26`, the id/type dword `+0x28`, the prototype and
  sub-object pointers, and the flag word `+0x2C`;
- no timer, no active flag, no target pointer. The `+0x28` pointer seen in the
  link path belongs to the **link record**, not the placed record.

The state that explains the multi-code naming lives elsewhere:

| State | Where | Meaning | Class |
|---|---|---|---|
| `0x4000` in `+0x2C` | placed record | object is **not** in the level's 48-name table (set) or is (cleared) | CONFIRMED |
| `0x8000` in `+0x2C` | placed record | set/cleared by player/actor interaction (a carried/held style flag) | CONFIRMED (set and clear sites); meaning INFERRED |
| `0x800`, `0x8`, `0x1` in `+0x2C` | placed record | runtime interaction flags | CONFIRMED writes; roles UNKNOWN |
| tracker `+0x18` | object tracker | `0` empty, `1` loading, `2` ready | CONFIRMED |
| tracker `+0x1A` | object tracker | animation index | INFERRED |
| live object `+0x1C` | live object | `0x6D00`-byte animation/state block | CONFIRMED shape; fields UNKNOWN |
| live object `+0x04` | live object | renderer class index (0..5, `0xFFFF`) | CONFIRMED (prior note) |
| live object counters | live object | per-object animation/timer counters | CONFIRMED writes; units UNKNOWN |

So the multi-code naming (`campath` 0/1/3, `marker` 0/3/4/9, `raziel` 2/4) is
**authoring-time variation in the placement record**, and runtime state is
created later on the tracker/live object. The code is not a state machine key.

## 7. Direct answers

1. **Dispatch key.** The 12-byte **name**. Not `+0x26`, not `type`, not the
   render class, not a record function pointer. A secondary key is the authored
   **id `+0x14`**, used by script/entity target lookup. (CONFIRMED)
2. **What is registered per key.** Model+skeleton+optional sound+animation for
   every tracked name; a fixed ten-name system-object set; a ten-name actor
   roster; an eight-name boss/special set; a 22-name E3 asset override; exact
   specials for `raziel`, `MORLOCK`/boss names, `warpgate`, `cliff1`, `movie*`.
   No trigger-volume, portal, moving-platform, camera-path or enemy handler is
   selected by `+0x26`/type. (CONFIRMED for selection; per-handler internals
   partly UNKNOWN.)
3. **Ground truth.** `warpg`/`wrpface` -> warp/link path (CONFIRMED marker);
   `marker` -> inert (CONFIRMED); `campath` -> not an actor (CONFIRMED), camera
   consumer INFERRED.
4. **Runtime state.** Not on the placed record beyond `+0x2C`; it lives on the
   tracker (`+0x18` state) and the live object (animation block, render class,
   counters). (CONFIRMED)

## 8. UNKNOWN (explicit)

1. The exact per-frame AI/update function selected for each actor roster name.
   Spawn-time classification is confirmed; the AI body was not enumerated.
2. The camera/spline function that consumes `campath` ids. The name is
   `paths`-related and the nodes are position data, but the consumer is not
   pinned.
3. The interior meaning of the `0x6D00` animation/state block and of the
   per-object counters.
4. The exact semantics of every `+0x2C` bit; only `0x4000` (name-table
   membership) and `0x8000` (interaction set/clear) have evidence.
5. Whether any engine path reads `+0x26`/`type` outside the scans performed
   here (for example a tooling-only or debug-only path).
6. Why the editor stamps the large type values (26..1376); the runtime does not
   consume them.
7. Whether the 48-slot tracker limit is ever exceeded in normal play; the
   engine prints a loud error and returns failure.

## 9. Method and legal

Three bounded passes inside the agreed budget: (1) re-pin the placed table from
its `+0xD0`/`+0xC8` readers and the name/id lookups; (2) distinguish the
`0x60`/`0x38` record families and trace the link and level-slot paths;
(3) trace the name-keyed object tracker, its load-complete callback, and the
name lists. Scripts and decompiler output stayed in `%TEMP%`.

Behaviour notes only. Field offsets, counts, engine role names and short name
lists. No addresses, no listings, no decompiled bodies, no game bytes, no
absolute install paths. The binary was opened read-only. No protection or
packing was encountered.

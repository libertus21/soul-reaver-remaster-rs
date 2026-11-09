# Runtime capture plan - where the SR1 spline set comes from

This is a **preparation** slice. It pins the runtime consumer side of the object
spline system in `sr1.dll`, designs one read-only runtime capture that answers
two questions, and ships the instrumentation as a local script. Nothing was
attached, launched or instrumented for this document, and no engine behaviour
changed.

The two questions the capture must answer:

1. **Provenance.** Which asset or table populates the spline key count and the
   runtime key pointer of a live object?
2. **Clip identity.** How does a `(model name, clip id)` pair resolve to a
   spline?

Why this is the right next step: the file-side search is closed. All 21 actor
containers carry zero valid channel descriptors, `.SRA` is negative under the
validated key-shape detector, `.SRL` is dialogue, and no animation extension
exists. The remaining evidence is runtime-only, so the next slice is a
controlled capture rather than another static sweep.

## Quick path

1. Launch the game normally and reach a controllable scene (any level with the
   player standing on ground).
2. Run the probe from the scratch directory and attach to the launcher process
   (see `docs/changes/runtime-capture/README.md` for why the script is not in
   the repo).
3. Wait for the probe to report the engine module and that its hooks are armed.
4. Play the scripted sequence: idle, walk, attack, turn, stop.
5. Press Ctrl+C, send back the log file, and read it against the outcome table
   in this document.

## What the static pass pinned

Every row is a **role**, not an address. The numeric module offsets needed to
hook these roles live only in the local script; the re-pin rule for a future
reader is the identity column, not a number.

| Role | Stable identity (how to re-pin it) | What it does | Argument / field carrying the source pointer |
|---|---|---|---|
| **P0 driver** | The only procedure that references all three `Spline %s%d ...` diagnostic literals (`playto`, `clip(...)`, `prevFrame`) | Walks the animated-object list once per frame; applies the playto and clip windows; calls the selector and the sampler wrapper; copies the previous-sample transform mirrors | Reads the object's own name buffer and clip fields; obtains the set from **P1** |
| **P1 set selector** | The procedure that returns a set pointer for one object and writes two optional out-flags; called by **P0** and by the snapshot encoder | Chooses between two carriers: the **companion object's set field**, else the **sub-model record's set field**; also reports which cursor mode is active | Returns the set in the return register; the object is the first argument |
| **P2 sampler** | The only procedure that both references the `def->currkey` / `spline->numkeys` diagnostic literal and writes the three instance transform groups (three shorts plus three float mirrors each) | Reads the definition's key pointer, key count and stride flag; advances each slot; interpolates; writes position, rotation and scale into the instance | **First argument:** the object instance. **Second argument:** the four-slot set (this is the source pointer the capture wants). The three cursor-state blocks follow in the third and fourth argument registers and on the stack |
| **P3 sampler wrapper** | Sole caller of **P2** | Resolves the set (same rules as **P1**) and the three cursor-state blocks, then calls the sampler | Passes the resolved set through to **P2** |
| **P4 key-cursor advancer** | References the `def->currkey=%d spline->numkeys=%d` diagnostic; accumulates frame deltas and advances or clamps the cursor | Turns elapsed time into a key cursor using the per-key frame delta; clamps to the last key or wraps, by behaviour bits | First argument: the spline **definition** (holds the key pointer, key count, stride flag). Second: the cursor state. Third: elapsed time |
| **P5 key interpolator** | Called by **P2** and **P3** immediately after the advancer; returns three shorts | Evaluates the two keys around the cursor (Hermite for position-stride records, spherical for rotation-stride records) | First: definition. Second: cursor state. Third: output triple |
| **P6 end-of-clip predicate** | Called by **P0** with the set as its only argument; returns start / at-end / not-at-end | Decides whether the position, rotation and scale slots have all reached their last key | First argument: the set |
| **P7 post-spawn refresh** | Indexes a global class table with the object's type index and then performs a non-advancing pose evaluation | Per-type hook that runs after a type-specific body, then samples the pose at time zero | First argument: the object |
| **P8 snapshot encoder** | Serialises object state (including spline state) into a record buffer | Records the object for replay or network sync; calls **P1** | First argument: the object |
| **P9 descriptor-to-set builder** | **NOT PINNED in this pass** | The load-time producer that expands the four channel descriptors into the four-slot set | See "The one open role" below |

Key facts that follow from the table and that the capture depends on:

- The **key count and stride flag are never arguments**. They are read from the
  spline definition, which the set owns. So observing the sampler is enough to
  see `numkeys`, the stride flag, the behaviour bits and the key pointer.
- The **source pointer in the question "where does the set come from" is the
  sampler's second argument**. That pointer is the runtime artefact; its
  contents are what we need to classify.
- The set is **position, rotation, scale, raw**, in that slot order, and each
  slot has its own cursor block inside the set. A fourth slot exists and is not
  sampled by **P2**.
- The clip identity is readable from the instance at sample time: the name
  buffer, the current clip id, the playto target and the clip window are all
  adjacent fields on the object **P2** receives.

### The one open role

**P9 is not pinned.** What is known and what the capture does about it:

- **Known:** the set selector reads a set pointer from one of two carriers and
  never builds one; no caller of the selector is a load-time function. The
  builder therefore runs somewhere else, on the model-load path, and its effect
  is observable as the sub-model record's set field going from empty to
  populated.
- **Consequence for this slice:** the capture is designed to observe the
  *produced* set, not the producer. That still answers question 1, because the
  produced set carries the key pointer and the counts.
- **Follow-up (second pass, only if needed):** identify the producer with a
  single write watch on the sub-model record's set field and read back the
  faulting instruction. The Ghidra debugger fallback below does this with a
  hardware write breakpoint; the Frida primary can do it with a page access
  monitor filtered to that exact address. This pass deliberately does not arm
  it, because page-level monitoring on a live heap page is the one operation
  most likely to disturb the process.

## Runtime set layout (roles only)

| Object | Fields, by role |
|---|---|
| set | four slot pointers (position, rotation, scale, raw), then three cursor blocks |
| definition | key-array pointer, key count, stride flag, behaviour flags |
| cursor block | current key, a re-seek flag, a fractional accumulator |

The stride flag chooses the runtime key record: rotation-stride records carry a
frame delta plus a quaternion and its float mirror; position-stride records
carry a frame delta plus a position, its float mirror and in/out tangents. The
capture reports the stride flag per slot instead of assuming it, which is how we
tell a real channel from a coincidence.

## Mechanism choice

### Primary: Frida (user-mode instrumentation)

Chosen because:

- **No debugger is installed** on this host. Frida is a pip install and needs no
  kernel debugging, no PDB, and no symbol server.
- The engine lives in a **DLL loaded after the launcher starts**, so the probe
  polls for the module and hooks it by module base. A user-mode instrumentation
  library does this cleanly; a debugger would need to be attached after the
  module appears anyway.
- The interesting reads are **structured field reads at function entry and
  return**. Frida gives those directly, with rate limiting and de-duplication in
  the agent, so the log stays small enough to read by hand.
- Spawn mode can load the agent **before** the engine module exists, which is
  the only way to have a chance at hooks that fire during level load.

### Fallback: the Ghidra debugger

Use it if Frida cannot attach, cannot hook, or if the process refuses
user-mode instrumentation. Ghidra's debugger module can break on the same roles
by their module-relative offsets and read the same registers and stack slots.
It is the fallback, not the default, because it needs an interactive session per
breakpoint, gives no de-duplication, and is far slower to iterate. It is also
the mechanism of choice for the **P9 write watch** in the second pass, where a
hardware write breakpoint is exactly the right tool.

## Capture design

One script, four probes, one shared rate limiter.

| Probe | Hooks | Emits | Why |
|---|---|---|---|
| **H1** | **P2 sampler** | object name, clip id, playto/clip window, set pointer, per-slot key count / stride flag / first key / key-pointer region, cursor positions | This is the source-pointer observation point. Answers question 1 directly and question 2 by correlation |
| **H2** | **P1 set selector** | which carrier supplied the set (companion vs sub-model), the descriptor probe on the sub-model record | Tells us whether the actor set is present at all, and which of the two carriers is live |
| **H3** | **P6 end-of-clip predicate** | one line per clip completion | Turns a continuous stream into clip boundaries, which is what "clip identity" needs |
| **H4** | **P7 post-spawn refresh** | one line per object birth, with the type index | Separates "object never had a set" from "object had a set and we missed the window" |

Deliberate choices:

- **P0 (driver) and P4 (advancer) are not hooked.** They fire per object per
  frame and would swamp the log; everything they would tell us is already
  reported by H1, once per object per half second.
- **Rate limiting and de-duplication** are per object: a line is emitted only
  when the signature (set pointer, per-slot counts, clip id) changes, or at most
  once per half second per object. Repeated identical frames collapse.
- **Hard line cap** so a long run cannot exhaust memory; the agent detaches
  itself and says so.
- **A heartbeat every ten seconds** with per-probe hit counters. This is the
  single most useful line when the answer is "nothing happened", because it
  separates "hooks armed but never called" from "agent never armed".
- **Key-pointer region tag.** For each slot the probe reports whether the key
  array lies inside the engine module or outside it. Outside means a heap or
  load-buffer allocation, which is the signal that the keys were converted at
  load rather than compiled in.

## Session procedure (follow live)

1. **Start the game and get in-world.** Load any save, stand still on ground.
   Do not run the probe yet.
2. **Run the probe and attach.** From the scratch directory:
   `python sr1_spline_capture.py --attach SRX.exe`
   Wait for the two `READY` lines. If you only see `WAIT` lines listing modules,
   the engine module has a different name or the game is not in a level yet -
   send that log anyway, it is diagnostic.
3. **Play the sequence, watching the terminal:**
   - 10 s idle (baseline),
   - 15 s walking forward,
   - 10 s attacking a wall or air,
   - 10 s turning on the spot and jumping,
   - 5 s idle again.
4. **Stop** with Ctrl+C. The log file path is printed at start.
5. **Send back** the log file only (no screenshots needed). Note roughly when
   each phase started if the answer looks ambiguous.

## Outcome table

| Observation | Conclusion |
|---|---|
| H1 lines for a moving actor, non-empty slots, key pointer outside the engine module | **Q1 answered.** The actor set exists at runtime and its keys are a load-time allocation, not a compiled-in table. The reported name and counts are the provenance record. |
| H1 lines but only for `font`-like props, none for the player or actors | **Q1 answered negatively for actors.** The spline system is prop-only in practice; actor motion must come from another system, and the clip-identity question moves to the animation-set path. |
| H2 lines with an empty set (selector returns null) for every actor | Actors genuinely carry no spline set. Same conclusion as the row above, reached from the other side, and it explains the corpus census. |
| H2 lines non-null, H1 never fires with the same set | The set exists but the sampler is not reached for that object in this window: the driver path is gated, or the object is not in the animated list. Re-run with the object actually moving; if it persists, the walk order is the finding. |
| H3 lines only at clip boundaries and clip ids change monotonically | **Q2 partly answered.** Clip id is a per-object animation selector; correlate its values with the H1 name lines to build the `(name, id)` table. |
| H1 fires with several slots whose first-key deltas do not form a coherent span | The set is present but the descriptor interpretation is wrong for actors. Report the raw counts; do not guess a layout. |
| **No H1, no H2, no H3, no H4 at all**, heartbeat still ticking | Hooks armed but the roles are never called. Either this build routes the system through another module (the launcher executable or the SR2 module), or all four roles are dead code in this build. Next: re-run with the probe also watching the launcher process and the SR2 module, and check whether the in-game animation is object-transform at all. |
| **No heartbeat at all** | The agent never loaded. Not a finding about the game; fix the attach first. |
| H1 fires only before the level finishes loading, then never again | **The builder ran before our hook window.** The set was produced during load and never sampled afterwards, or sampling is disabled for those objects. Re-run in spawn mode so hooks exist before the module loads, and add the P9 write watch. |
| Set pointer is inside a loaded container buffer rather than a fresh allocation | The keys were referenced in place, not converted. That contradicts the documented load-time conversion and must be reported as such, with the pointer region and the object name. |

The negative case is a real result, not a failure: "no hits" plus a live
heartbeat is evidence that the consumer roles are unreachable from the observed
process, and it narrows the next slice to module coverage and the load window
rather than to more hooks.

## What must never be committed

The repository keeps roles and structure. It never keeps module offsets, raw
field offsets, disassembly, game bytes or install paths. The numeric offsets and
engine field offsets needed to hook the roles live only in the local script,
which is intentionally outside the repository. See
`docs/changes/runtime-capture/README.md`.

## Risks and limits

- **Instrumentation can be detected or can perturb timing.** The capture is
  read-only and hooks only entry/return, but a render or physics stall is still
  possible. If the game stutters, note it; do not conclude anything from timing.
- **Breakpoint placement is by offset, not by symbol** (the module ships a PDB
  reference but Ghidra reports its load state unknown). If the shipped binary is
  ever updated, every offset must be recomputed from the re-pin rule in the
  role table; a stale offset would silently hook the wrong function. The
  `READY` line prints the module base so a mismatch is at least visible.
- **Only the SR1 module is covered.** The launcher executable and the SR2
  module are out of scope for this capture by design. A clean negative therefore
  does not prove the system is absent from the product, only from this module.
- **The descriptor-to-set builder stays unpinned** unless the second-pass write
  watch is run. This slice states that limitation rather than hiding it.

## Legal

Read-only. No game process is launched or instrumented by this slice. No game
file is written, no game bytes are copied into the repository, and no Ghidra
project is created inside the repo or the install. Counts, roles and structural
facts only.

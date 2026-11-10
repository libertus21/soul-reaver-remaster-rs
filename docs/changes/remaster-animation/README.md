# remaster-animation - committed note, local probe

This folder records where the remaster's real body animation path lives. It is a
**preparation** slice: the static verdict is committed, and the runtime probe
that would confirm cadence and data flow is intentionally kept outside the
repository.

## The split

| Part | Where | Why |
|---|---|---|
| `animation-source.md` | committed | Roles, record shapes, counts, confidence tags and the explicit UNKNOWNs |
| This README | committed | The committed/local boundary and the human procedure |
| Probe script | local scratch only, outside the repository | It must carry module-relative addresses and engine field offsets, which the repository is not allowed to contain |

The repository keeps role names, record shapes and small counts. It never keeps
module addresses, field offsets, disassembly, game bytes or absolute install
paths.

## What the committed note claims

- The `.SRM` 128-slot pool is consumed by a **skin-palette assembler** that runs
  from the draw path and writes one 3x4 matrix per active slot.
- The per-slot pose it consumes is produced by a **slot-pose builder** from a
  per-model **animation record** (a per-slot descriptor array plus a separate
  track table).
- `.SRL` is loaded whole and sampled by a global loader/sampler, and it feeds
  only the `extra > 0` HD-matrix class.
- `.SRA` is not read on this path.
- A file-only body pose is **not** possible yet; the missing pieces are listed
  in the note.

## Human procedure

The probe is read-only. It installs entry hooks, reads arguments and a few
bounded records, and never writes process memory or calls game code.

1. **Launch the game normally** and reach a controllable scene with a moving
   actor (the player is enough). Do not attach anything yet.
2. **Run the probe from the scratch directory** and attach to the launcher
   process. Wait for the two `READY` lines. If the probe only prints `WAIT`
   lines, the engine module is not loaded yet or the process name is wrong; send
   the log anyway, it is diagnostic.
3. **Play the sequence** while watching the terminal:
   - idle for a baseline,
   - walk forward,
   - attack or climb (any change of pose that is not a walk),
   - turn on the spot,
   - stop.
4. **Stop** with `Ctrl+C`. The log path is printed at start.
5. **Send back the log file only.** Note roughly when each phase started if the
   answer is ambiguous.

## Reading the log

| Observation | What it means |
|---|---|
| Palette-assembler lines for the moving actor, with a non-zero slot count and a stable pose pointer | The writer chain is confirmed; the slot range and the active-flag count are the per-frame identity |
| Palette-assembler lines but the pose pointer is null or the count is zero | The draw path is reached but the pose is absent; the pose builder is not running for that object |
| Pose-builder lines only at spawn, never during play | The pose is cached at spawn; the delta-driven condition is not firing on movement |
| Pose-builder lines continuously for an idle object | The delta-driven reading is wrong; cadence is unconditional |
| Track-selector lines with changing selected indices | The time/divisor playhead is live; compare the index with the visible motion |
| Track-selector lines with a constant index | The clock or divisor is wrong, or the track table is empty for that object |
| `.SRL` sampler lines only for dialogue/facial models | The facial-only reading holds; no body binding |
| `.SRL` sampler lines for a normal moving actor | The facial-only reading is wrong; re-open Q2 |
| Heartbeat with zero hits in every probe | Hooks armed but the roles are not reachable from this process; do not conclude absence, report it |

The probe de-duplicates per object and rate-limits, so repeated identical
frames collapse into one line. A heartbeat every ten seconds keeps a zero-hit
run distinguishable from a dead agent.

## Safety

- Read-only: entry hooks only, no memory writes, no code calls, no byte patches.
- No game file is written and no Ghidra project is created inside the game
  directory.
- If the game stutters while attached, note it and do not conclude anything
  from timing.
- The probe is intentionally not committed. A future reader re-pins the roles
  from this note by identity (role, record shape, call relation), never from a
  copied number.

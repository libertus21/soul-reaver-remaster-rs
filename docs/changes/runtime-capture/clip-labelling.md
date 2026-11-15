# Clip labelling from runtime table selection

The offline clip ranking cannot identify a real idle or walk, so the label
source moves to the runtime: a probe observes which frame producer the engine
selects for the player while a human performs each state, fingerprints the
selected table, and maps packed tables to a clip index. This note records the
failure of the heuristic, the packed and keyed fingerprint schemes, the
segment aggregation, the keyed-locomotion hypothesis, the vertical-motion
segmentation that replaced time windows for the jump, and the exact operator
procedure. The probe itself stays in the session scratch directory because it
carries module-relative anchors and engine field offsets.

The first revision of this note described a docs-only slice: nothing was
launched and no production code changed. The capture has since been run (see
"Runtime result" below), and the measured idle and walk indices are now the
default state map in `sra::ClipMap::for_clip_count`. No golden digest moved.

Jump labelling no longer asks a human to keep a rhythm. The probe classifies
each observed frame by the player's own vertical velocity and aggregates per
phase, so the measurement does not depend on the operator's cadence.

## Quick path

1. Start the game and load a save where Raziel stands on solid ground.
2. Run the probe in its default mode with the path to `RAZIEL.SRA`:
   `--attach SRX.exe --sra <RAZIEL.SRA> --slots 66 --motion-seconds 15`.
3. Stand still for a moment, then jump normally and land. No steady rhythm is
   required; the probe reads vertical motion, not your timing.
4. Read `ZCALIB`, the four `PHASE` lines and the final `LABELS` line. A
   numeric index is a packed clip; `unknown` means the phase was too thin or
   the signal failed calibration.

`--segments` (three timed prompts) and `--auto-windows` (a diagnostic time
series) still work, but they are no longer the way to label the jump.

## Runtime result (2026-11-14)

The three-window capture was run against our own build and produced these
labels. `unmapped` was `0.00` in all three segments, so every sampled table
mapped to a file index after the invariant-fingerprint fix.

| Index | State | Evidence class | Measurement |
|---:|---|---|---|
| 55 | idle | **CONFIRMED** | Coverage `1.00`, observed in 3 of 4 runs. |
| 2 | walk | **CONFIRMED** | Coverage `0.91`, `fp=12:209:3acaf951`; predicted from the file before the run, then confirmed. |
| 27 | jump | **PARTIAL / UNKNOWN** | Coverage `0.16`; four clips shared the window (`27:0.16`, `16:0.15`, `2:0.12`, `37:0.07`, keyed `0.21`). |

Supporting `--list-clips` facts for those indices: clip `0` is frames `50`
divisor `100`, clip `2` is frames `12` divisor `209`, clip `27` is frames `6`
divisor `240`, clip `55` is frames `6` divisor `580`.

Two caveats belong to the result, not to a footnote:

- **Idle has a variant.** One earlier run reported clip `0` (`50:100`) at
  coverage `1.00` instead of clip `55`. Both runs are clean, so idle appears to
  select more than one table, likely per section or variant. `55` is the most
  frequent observation and is the default.
- **Jump is not confirmed by a timed window.** A single timed window contains
  jump-up, fall and land, so a spread across four clips is expected. `27` is
  the largest share and is kept as a documented best-effort `jump_up` default
  only until the vertical-phase run below reproduces it.

These values are wired into `sra::ClipMap::for_clip_count` with a per-index
provenance doc comment, clamped to the decoded clip count so a short animation
falls back to its last clip, and remain overridable with
`--clip-idle/--clip-walk/--clip-jump`. See
`docs/notes/packed-channel-format.md`, "State defaults".

## The keyed hypothesis (test pending)

> **2026-11-14 correction.** A later capture with the fixed fingerprint shows
> idle = clip 0 (`50:100`) and the walk samples on a packed table with pair
> `12:209` (clip 2). The walk `unmapped` status was the legacy pointer-hash
> defect, not a keyed producer. The keyed producer still exists (see "The keyed
> fingerprint"), but it is not the walk explanation. See
> `docs/notes/packed-channel-format.md`, "Clip fingerprint".

Idle maps cleanly to one clip (`6` frames, divisor `580`, coverage `1.00` on
both runs). Walk is `97-98%` `unmapped`, and jump is inconsistent (two runs
return `34` and `0`, coverage `0.45-0.85`). The file side is exhausted:
`RAZIEL.SRA` holds exactly one genuine table per payload record, and extending
the enumeration to every structurally valid table added **zero** tables. The
walk data is therefore not a second table hiding inside the record.

The remaining explanation is the producer, not the file. A channel selects one
of two writers per invocation:

- the **packed** decoder, which reads the channel's packed table (the path the
  probe has always fingerprinted), and
- the **keyed** quaternion sampler, which runs only when the channel's keyed
  head and its frame-array pointer are both non-null.

The earlier probe fingered packed tables and **skipped keyed channels
entirely**. If locomotion selects the keyed producer, the packed path sees
nothing and the segment arrives as `unmapped` even though a frame array is
playing. The hypothesis is:

> Walking engages the keyed sampler (the quaternion/frame-array path). The
> probe's silence on keyed channels is what turns walk into `unmapped`; it is
> not a missing packed table in `RAZIEL.SRA`.

The probe now fingerprints both producers and classifies every sample. The run
is the test.

### What confirms or refutes it (stated before the run)

| Observation | Verdict |
|---|---|
| `walk` is dominated by `keyed`, with a share close to the old `unmapped` share, and `unmapped` falls to the transition residue | **Confirms** the keyed-producer hypothesis |
| `walk` stays dominated by `unmapped` with `keyed` near zero, and `unmapped_top` prints stable packed keys | **Refutes** it: the walk producer is packed and its table is absent from the file index; the captured keys are the next evidence |
| `keyed` is non-zero but never dominant, while `unmapped` still dominates | **Refutes the strong form**: keyed channels participate, but another producer writes most sampled slots |
| `idle` loses its coverage or changes clip | The keyed path is not locomotion-specific; the measurement is confounded and must be repeated |
| A keyed key is identical across idle and walk | The keyed view does not distinguish frames; it proves the path, not the selection |

A keyed sample is never turned into a clip index. A keyed segment is a
producer classification, not a `--clip-walk` value.

## Why the offline heuristic failed

The shipped offline ranking, run over the player body, produced:

```text
rank clips=230 still=73 locomotion=0 impact=157
```

That result cannot contain a genuine idle or walk:

- **The STILL bucket is dominated by one-frame clips.** 33 of the 230 clips
  have a single frame. A one-frame table has no consecutive pair, so its
  motion metric is zero by construction. It lands in STILL for a structural
  reason, not because it looks like an idle.
- **LOCOMOTION is empty because period detection collapses.** For a smooth
  trajectory the best period is the trivial adjacent-frame shift `p=1`, which
  says nothing about repetition. The rule that requires a real period `p>=2`
  therefore rejects every clip, and the bucket is empty instead of populated
  with walks.
- **The tables carry no identifier.** A clip record has a stream count, a
  frame count, a divisor, duplicated header words, value offsets and bit
  planes. There is no name, state byte or clip index. Motion statistics are
  the only offline signal, and they are measuring the wrong thing.

The engine, by contrast, selects a table per state. The player is driven by
the operator, so the selection is the label. That is the ground truth used
here.

## The fingerprint scheme

### What is read at runtime

A channel record points at the frame producer it is playing. For each
animation invocation the probe reads:

- the channel's producer: **keyed** when the keyed head and its frame-array
  pointer are both non-null, **packed** otherwise;
- for a packed channel, the table's frame count and divisor from the
  form-invariant header, and the complete bit-plane block (flag byte plus the
  present planes);
- for a keyed channel, the head's frame count and the bounded content view
  defined below.

The same packed values are computed offline from the shipped file for every
clip.

### Why `(frame_count, divisor)` alone is insufficient

The shipped player has 230 clips but only **98 distinct pairs**: 173 clips
share a `(frame_count, divisor)` pair with at least one other clip, and the
largest group is **33 clips at `(1, 100)`**. A runtime observation of a pair
therefore names a bucket of up to 33 candidates, not a clip.

### Why the raw first 32 table bytes are not usable

The first 32 table bytes straddle the value-offset array. In the file that
array is four bytes per entry; in memory it is eight bytes per pointer. The
same table therefore has different bytes at that position in the two forms.
A hash of the literal first 32 bytes could never match across the file and
the runtime, no matter how the mapping is written.

### The chosen fingerprint

The probe hashes a **content-only canonical view**:

```text
FNV-1a 32-bit over:
  the 16-byte form-invariant header (+0x00..+0x10)
  the complete bit-plane block (flags byte + present planes)
```

Those bytes are static table content, not addresses or offsets, so relocation
copies them unchanged. Frame count and divisor are carried separately because
they are cheap to read and make the key human-readable:

```text
fp = <frame_count>:<divisor>:<8 hex digits>
```

The first version of this scheme hashed 20 header bytes. That included
`+0x10..+0x13`, the low half of a field that is a 4-byte offset in the file and
an 8-byte absolute pointer in memory. Every clip whose field was non-null
therefore failed to map (idle was the exception because its field is null). The
scheme now hashes only `+0x00..+0x10`; `docs/notes/packed-channel-format.md`,
"Clip fingerprint", records the definition, the measured file/memory pairs and
the collision count. The probe still emits the old digest as `legacy=` so a
stale key is diagnosable, but it maps on the invariant key only.

The host enumerates the shipped file with the engine's rule (one packed table
per payload record, in file order) and fingerprints every clip. This slice
checked that enumeration against `--animate --list-clips`: **230 of 230 clips
match in order, record, frame count and divisor**.

### Uniqueness

The canonical view yields **225 distinct fingerprints for 230 clips**: five
pairs collide. The earlier 20-byte view had one collision, but it was not
form-invariant, so its extra discrimination came from a pointer. The probe
reports `ambiguous=<n>` at startup and maps an ambiguous key to the lowest
index; it never hides the collision.

## The keyed fingerprint

The keyed head is a small runtime record. The observed shape is a `u16` frame
count at `+0x00`, content bytes up to `+0x0f`, and the frame-array pointer at
`+0x10`. The producer is keyed only when that pointer is non-null. The observed
heads carry **no divisor field**, so the keyed key omits the divisor and the
note says so instead of inventing one.

The key is:

```text
keyed fp = k<frame_count>:<8 hex digits>

FNV-1a 32-bit over:
  the 16-byte head prefix [+0x00, +0x10)   (pointer not included)
  the first 16 bytes at the frame-array pointer
    (the leading floats of the first 20-float frame)
```

Both hashed regions are content, not addresses: the frame-array pointer is
excluded, and the array sample is the leading numeric components of frame 0.
Relocation therefore leaves the hashed bytes unchanged. The `k` prefix keeps
keyed keys out of the packed `frame_count:divisor:hash` namespace.

The 16-byte array sample is a **bounded** view, not the whole array. It is
enough to separate keyed tables whose first frame differs. It cannot separate
two tables that share their first four floats; they collapse to one key with a
larger count. It also does not establish the frame stride, a divisor, or the
interpolation, which remain UNKNOWN for the keyed producer.

## The segment design

The probe hooks the per-object animation channel loop (the same hook class
already validated by the staging-split probe) and works in segments:

1. The host announces a segment and starts the agent's segment accumulator.
2. Every animation invocation contributes the active table fingerprint for
   its object.
3. The host waits for the announced duration, then stops the segment.
4. The agent returns one aggregate for the segment: per-fingerprint sample
   counts for the selected player object.

### Player selection

There is no player id in the data. The probe chooses the object with the
largest covered slot count in the segment (Raziel's body is 66 slots) and
requires at least 40 covered slots. Other objects are counted as candidates
but never merged into the player's histogram. If no object reaches the
threshold, the largest one is still used and the line says `low_slots=1`.

### Aggregation

For the selected object the probe classifies every sample as **packed**,
**keyed**, or **unmapped**, and prints one line per segment:

```text
CLIPLABEL segment=idle seconds=10.0 packed=1.00 keyed=0.00 unmapped=0.00 clips=55:0.96,6:0.02 dominant=55 coverage=0.96 fp=6:580:<hash> legacy=6:580:<hash>
CLIPLABEL segment=walk seconds=20.0 packed=0.02 keyed=0.96 unmapped=0.02 clips=55:0.02 dominant=keyed coverage=0.96 fp=k300:<hash> legacy=-
CLIPLABEL segment=jump seconds=10.0 packed=0.21 keyed=0.04 unmapped=0.75 clips=212:0.21 dominant=unmapped coverage=0.75 fp=1:100:<hash> legacy=1:100:<hash> unmapped_top=1:100:<hash>=1:100:<hash>:75,...
LABELS raziel idle=55 walk=keyed jump=unmapped source=runtime
```

The three fractions are disjoint and sum to 1 (rounding aside):

- `packed=` is the share whose packed fingerprint maps to a clip index.
- `keyed=` is the share produced by the keyed sampler.
- `unmapped=` is the share on a packed table whose fingerprint is not in the
  file index.

`dominant=` is a clip index, `keyed`, or `unmapped`; `fp=` is the invariant key
of that dominant bucket and `legacy=` the pre-fix digest for the same sample (a
keyed sample prints `-`), so a mismatch stays diagnosable. `unmapped_top=`
prints up to `--max-unmapped` packed keys as `<invariant>=<legacy>:<count>`;
that list is the run summary and feeds `--match-unmapped`.

Rules that keep the output honest:

- A keyed sample is never called unmapped: the file index only lists packed
  tables.
- Unmapped samples are never silently dropped; their raw keys are printed.
- If unmapped samples outnumber the best mapped clip, the dominant label is
  `unmapped`, not a guessed index.
- An empty segment prints `note=empty`; it is missing evidence, not a vote.
- Only one line per segment and one `LABELS` line are printed. There is no
  per-call text, and the host enforces a hard line cap.

## Why time windows were replaced

The time-window profile never reproduced. Two identical 15-second jump runs,
same save, same operator, one-second windows:

```text
run 1  SEQUENCE dominant=55,36,16,2,27,36,35,35,27,27,0,27,27,35,0
run 2  SEQUENCE dominant=0,36,36,36,0,36,39,35,27,0,36,16,2,27,27
```

The two sequences are incompatible, and the numbers say why:

| Measurement | Value |
|---|---|
| Per-window coverage | `0.21` to `0.52` |
| Clips mixed into one window | 4 to 6 |
| Dominant entries that are 1-2 frame poses | several (`55`, `36`, `0`) |
| `unmapped` across all 30 windows | `0.00` |

One second is longer than a phase and close to the whole jump cycle, so each
window averages ascent, fall and landing instead of separating them. The
dominant clip of a window is then whichever pose happened to win a blend, and
several of those poses last one or two frames: they cannot be an animation.
Asking a human to hold a rhythm only helps if that rhythm matches the engine's
cycle, which is a coincidence, not a measurement.

The fingerprint side is not at fault: `unmapped = 0.00` in every window. Only
the labelling window was wrong, so the window is what this slice replaces.

## The vertical signal

Three candidates were evaluated in order:

| Candidate | Verdict |
|---|---|
| Root pose slot's world translation Z | **Chosen.** The runtime slot layout is documented and measured: 13 floats per slot, translation at float indices 10..12, and the walking series moves `x`/`y` while `z` stays constant in a Z-up world. Reachable from the same channel loop hook. |
| The player object's world position field | Rejected. The runtime object's field layout is not pinned by any note; using an unverified offset would move the uncertainty from the measurement to the address. |
| Any other vertical already in the channel loop | Rejected. The channel record carries frame-producer data, not motion. |

Reading it does not assume an object layout. At the channel loop the object
base is recovered when the return address is one of the three known
per-object sites. The offset of the object field that points at the pose array
is then discovered **once per object** by a bounded scan of aligned pointer
fields, scored by pose shape: a near-orthonormal 3x3 rotation block per sampled
slot and a finite translation. The offset is cached; the pointer is re-read
every frame because the pose array lives in a per-frame arena. Each sample is
`float 12` (Z) of slot 0.

The probe does not trust the signal. It prints one bounded calibration line:

```text
ZCALIB signal=root-slot-z samples=812 zmin=-4831.5 zmax=-4632.0 dzmin=-37.2 dzmax=39.8 sign_changes=41 flat=0.12 calibrated=yes
```

Calibration requires at least 24 samples, at least 95% finite values and
`|z| <= 1e6` world units. The line also shows the observed range, the `dz`
range, how often `dz` changes sign and the flat fraction, so a wrong signal is
visible immediately: standing should be flat with about zero sign changes, and
a jump must change sign. If calibration fails, the run prints
`calibrated=no reason=<insufficient|nonfinite|range|no-player>` and **no phase
is labelled**; an uncalibrated run is missing evidence, not a result.

## Phases and hysteresis

`dz` is the difference in Z between consecutive samples of the same object, so
one sample is one animation invocation, not one wall-clock tick. Three named
constants govern the machine, in world units per frame:

| Constant | Value | Role |
|---|---:|---|
| `DZ_ENTER` | `4.0` | Larger threshold: used to start ascent or fall |
| `DZ_LEAVE` | `1.5` | Smaller threshold: used to return to a grounded phase |
| `DZ_HOLD` | `3` | Consecutive frames required before a change commits |

| Condition | Phase |
|---|---|
| `dz > DZ_ENTER` | `ASCENT` |
| `dz < -DZ_ENTER` | `FALL` |
| `abs(dz) <= DZ_LEAVE` after having been airborne | `LANDED` |
| `abs(dz) <= DZ_LEAVE` and never airborne in this run | `GROUND` |
| `DZ_LEAVE < abs(dz) <= DZ_ENTER` | dead band: hold the current phase |

The dead band is the hysteresis. A phase is entered only above `DZ_ENTER`, but
it is left only once a different phase has satisfied its own entry condition
for `DZ_HOLD` consecutive frames. An apex that hovers near zero therefore holds
`ASCENT` instead of flapping, and a jittery standing signal stays `GROUND`.
`GROUND` is only reachable while the object has never been airborne; after the
first ascent or fall the grounded phase is `LANDED`, which is what makes a
landing distinguishable from the initial stance.

## Aggregation per phase

There are no windows. Every sampled frame contributes to exactly one phase, and
each phase is aggregated over the whole run: frame count, mapped clip
distribution, keyed share, dominant clip and coverage.

```text
PHASE name=ascent frames=41 clips=27:0.88,16:0.07 dominant=27 coverage=0.88
PHASE name=fall frames=38 clips=2:0.71,29:0.18 dominant=2 coverage=0.71
PHASE name=land frames=17 clips=55:0.94 dominant=55 coverage=0.94
PHASE name=ground frames=120 clips=55:0.96 dominant=55 coverage=0.96
```

`clips=` lists the top mapped indices with their share of that phase, so a
mixed phase is visible rather than hidden behind one number. A phase whose
dominant clip is `keyed` or `unmapped` keeps that class; it never becomes a
clip index.

The run ends with one line that is directly usable:

```text
LABELS raziel idle=55 walk=2 jump_up=27 fall=2 land=55 source=runtime-vertical
```

`jump_up`, `fall` and `land` are this run's measurement (`ASCENT`, `FALL`,
`LANDED`). `idle` and `walk` are **carried**: a jump run does not observe
either state, so their values are the confirmed three-segment labels and can
be overridden with `--idle-clip` / `--walk-clip`.

A phase is printed as `unknown` when its frame count is below `6` or its
dominant coverage is below `0.40`, and every field is `unknown` when the run is
uncalibrated. A thin or blended phase is not worth a guess.

### Modes and flags

| Flag | Meaning |
|---|---|
| `--by-motion` | Default mode: one run, frames classified by vertical motion, aggregated per phase. |
| `--motion-seconds <SECONDS>` | Length of that single run (default `15`). `--total` overrides it. |
| `--dz-enter`, `--dz-leave`, `--dz-hold` | Override the hysteresis constants; `leave` must stay below `enter`. |
| `--idle-clip`, `--walk-clip` | Carried labels for the final line (defaults `55` and `2`). |
| `--segments` | Named, operator-paced segments (the three-prompt capture). Still supported. |
| `--auto-windows <SECONDS>` | Diagnostic time series, one numbered window per step. Still supported, no longer used for labelling. |
| `--total <SECONDS>` | Stop after this much wall time. |

Supplying `--by-motion` together with `--segments`, `--segment` or
`--auto-windows` is a typed error, never a silent override.

### The three-step jump procedure

1. **Stand.** Load a save where Raziel stands on solid ground in a quiet room,
   focus the game window and leave the game unpaused.
2. **Run.** From the scratch directory:

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --slots 66 --motion-seconds 15
   ```

   Wait for `CLIPLABEL HOST attached`, then for the `SEGMENT name=motion`
   prompt. Stand still for a second, then jump normally and land, as many times
   as fits in the run. No rhythm is required.
3. **Read.** Check `ZCALIB` first: `calibrated=yes` and a `dz` range that
   changes sign. Then read the `PHASE` lines and the `LABELS` line. A phase
   printed `unknown` is missing evidence, not a negative.

### How to read the result

| Pattern | Reading |
|---|---|
| `calibrated=no` | The vertical signal failed. Do not read the phases; re-run and check `ZCALIB`. |
| One clip dominates `ascent` and a different one dominates `fall` | The two phases are distinct animations. That is the jump profile. |
| The same clip dominates `ascent` and `fall` | One animation covers the airborne body; the phase split is real but the clip is shared. |
| `land` dominated by the `ground` clip | Landing returns to the stance animation; the phase boundary is real but the clip is shared. |
| Coverage below `0.40` in a phase | The phase is a blend. Report the class, not the guessed index. |
| `keyed` or `unmapped` dominant | That phase selected a producer the file index cannot name. |

## The exact four-step segment procedure (optional)

The three-prompt capture is still the way to label idle and walk. It is not
used for the jump.

1. **Set up.** Start `SRX.exe`, load a save where Raziel stands on solid
   ground in a quiet room, focus the game window, and leave the game
   unpaused.
2. **Run the probe.** From the scratch directory:

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --slots 66 --segments idle:10,walk:20,jump:10
   ```

   Optionally add `--max-unmapped <N>` (default `5`) to change how many raw
   unmapped keys each segment prints. Wait for `CLIPLABEL HOST attached`; the
   hook arms on the animation channel loop.
3. **Drive the three windows.** At the `idle` prompt, stand still: no
   movement, no attack, no camera turn. At `walk`, hold one movement
   direction without jumping. At `jump`, jump repeatedly and land between
   jumps. Each window is timed by the probe; the next prompt is the signal to
   change action.
4. **Record the result.** Read the three `CLIPLABEL` lines and the final
   `LABELS` line. Use a numeric index directly:

   ```text
   --clip-idle <n> --clip-walk <n> --clip-jump <n>
   ```

   A `dominant=keyed` line is the keyed hypothesis holding; a
   `dominant=unmapped` line is the hypothesis refuted and carries the packed
   keys in `unmapped_top`. If a line says `note=empty`, `low_slots=1`, or has
   a low `coverage`, repeat that segment. A state may legitimately select more
   than one table; the dominant bucket is the stable label for that window.

## What it cannot establish

- **One actor, one build.** The measurements are Raziel's shipped body in
  this build. Other actors, builds and animation sets are unobserved.
- **A state may pick different producers as the state machine evolves.** The
  label is the table or producer the engine selected in that window, not a
  promise that the same state always picks it. Context, speed, weapon, damage
  or progression can change the selection.
- **The label is not a semantic name in the file.** The `.SRA` carries no
  idle/walk/jump identifier. `idle=55` means "the engine selected packed clip
  55 while the operator stood still", not "clip 55 is named idle".
- **A keyed key is not a clip index.** The file index lists packed tables
  only. A keyed key proves the producer and separates frame arrays at their
  first frame; it does not name a clip, and `LABELS` reports it as `keyed`.
- **The keyed frame layout is only partly observed.** The frame count and the
  frame-array pointer are read; the stride, a divisor and the interpolation
  are not established here.
- **The player is inferred, not identified.** The probe uses the
  largest-coverage object. A scene with another 66-slot actor could compete
  for that role.
- **The vertical signal is measured; the phase-to-clip mapping is still an
  inference.** `ASCENT` means "the root pose slot's Z was rising for at least
  `DZ_HOLD` frames". It does not prove the engine's state machine considered
  the actor to be jumping, and a clip may span two phases.
- **A clip can span two phases.** If one animation covers the whole airborne
  arc, `ascent` and `fall` legitimately report the same index. That is a
  property of the animation, not a failed measurement.
- **The pose array is the builder's output.** The sampled slot may be the
  previous frame's build. At animation rate that is a one-frame lag in the
  phase boundary, not a wrong sign.
- **A phase label is not a semantic name.** `jump_up=27` means "the engine
  selected packed clip 27 while the root slot was rising", not "clip 27 is
  named jump-up".
- **`idle` and `walk` are carried, not re-measured.** A jump run never observes
  either state; their values come from the confirmed three-segment capture and
  are printed as such.
- **No transitions or blends are modelled.** A phase boundary is committed
  after `DZ_HOLD` frames, so the first frames after a boundary may still carry
  the previous phase. Dense transitions are not resolved.
- **Not every frame is sampled.** A table active for a single frame may never
  win a phase bucket. Absence from the top buckets is not proof the table is
  unused.

## Verification

| Gate | Result |
|---|---|
| `--selftest` | 31 cases, 31 green; the four mandatory vertical series are `motion-clean-arc`, `motion-jitter-holds`, `motion-ground-only` and `motion-uncalibrated`, plus the `ZCALIB`, `PHASE`, `LABELS`, `unknown` and mode-conflict cases |
| `--live-selftest` | 14 cases, 14 green in a real Frida runtime; the vertical machine runs in the real agent and matches the host for the clean arc, the jittery series, the grounded series and the uncalibrated one; `motion-scan-read-path` allocates a synthetic object and proves the bounded pose-field discovery plus the root-slot Z read in the real runtime |
| `--dry-run` | prints the plan; attaches nothing. Default mode reports the signal, the hysteresis constants and the per-phase aggregation; `--by-motion` plus `--segments` exits with the typed conflict; `--auto-windows 1.0 --total 3` still reports `planned=3` |
| `py_compile` | clean |
| `node --check` on `--print-agent` | clean |
| `cargo test` | 688 green, unchanged by this docs-only slice |

The live harness spawns a throwaway system process, never the game. It
exercises the real `NativePointer`, `Memory`, `Process`, `Interceptor`,
`NativeCallback` and `NativeFunction` bindings. The keyed case asserts the
keyed key and the keyed segment fraction; the negative case asserts that a
keyed head with a null frame array still falls back to the packed producer. A
missing Frida capability fails loudly before any hook is armed, rather than
reporting an empty segment.

## Reviewer checklist

- [ ] The diff contains no addresses, module-relative numbers, disassembly,
      game bytes or absolute install paths.
- [ ] The two-run incompatibility and the low coverage are the stated reason
      for replacing time windows, not a preference.
- [ ] The vertical signal is named, its validation is described, and the
      hysteresis constants are explicit.
- [ ] The phase definitions include `GROUND` versus `LANDED`.
- [ ] A failed calibration yields `uncalibrated`, never a phase.
- [ ] The packed fingerprint is described as content, not as a raw byte range
      at a fixed position.
- [ ] The keyed fingerprint names its bounded view and excludes the pointer.
- [ ] The collision policy is explicit: lowest index, `ambiguous` reported.
- [ ] The procedure requires no rhythm and names the result lines.
- [ ] The limitations say the phase-to-clip mapping is an inference and a clip
      may span two phases.
- [ ] `cargo test` stays at its baseline.

## Next step

Idle and walk are labelled; the jump is now measured per phase instead of
guessed from a window. Run the three-step motion procedure twice on the same
save. A phase label is confirmed only when the same dominant clip reproduces
in the same phase across runs. Until then `27` stays the documented
best-effort `jump_up` default and must not be reported as confirmed. A second
idle run on a different section would characterise the clip-0 variant. Do not
change the hysteresis constants on a single blended phase: re-run first,
because a low-coverage phase is a blend, not a wrong threshold.


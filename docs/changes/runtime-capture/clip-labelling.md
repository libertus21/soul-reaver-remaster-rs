# Clip labelling from runtime table selection

The offline clip ranking cannot identify a real idle or walk, so the label
source moves to the runtime: a probe observes which frame producer the engine
selects for the player while a human performs each state, fingerprints the
selected table, and maps packed tables to a clip index. This note records the
failure of the heuristic, the packed and keyed fingerprint schemes, the
segment aggregation, the keyed-locomotion hypothesis, the per-jump aligned
template that now carries the jump result, and the exact operator procedure.
The probe itself stays in the session scratch directory because it carries
module-relative anchors and engine field offsets.

The first revision of this note described a docs-only slice: nothing was
launched and no production code changed. The capture has since been run (see
"Runtime result" below), and the measured idle and walk indices are now the
default state map in `sra::ClipMap::for_clip_count`. No golden digest moved.

Jump labelling no longer asks a human to keep a rhythm. The probe classifies
each observed frame by the player's own vertical velocity and aggregates per
phase, so the measurement does not depend on the operator's cadence.

Labels are no longer transferred between actors. A clip index measured for the
player is not a label for an enemy: the shipped files have different clip and
slot counts, so an index such as `55` may not even exist for another body. The
default mode now measures **every live object that the hook observes**,
independently, and derives idle/walk from each object's own observed horizontal
motion. An object that cannot be measured is printed as `unknown` with the
measured reason; nothing is carried across actors. This multi-actor revision is
a scratch-probe change only: the committed artifact is this note, no engine code
changed, and no golden digest moved.

## Quick path

1. Start the game and load a save where the actors you care about are visible,
   on solid ground, with at least one of them free to walk.
2. Run the probe in its default mode with the path to the `.SRA` file whose
   clip indices you want (for example `RAZIEL.SRA` for the player):
   `--attach SRX.exe --sra <RAZIEL.SRA> --motion-seconds 20`.
3. Let each actor stand still, then walk a little, then jump normally once or
   twice. No steady rhythm is required; the probe reads each object's own
   vertical and horizontal motion, not your timing.
4. Read the `IDENTITY` verdict first, then one `ACTOR` / `PHASE` / `LABELS`
   block per object. A numeric index is a packed clip; `unknown` means that
   field was not measured well enough to name a clip.

For a repeating action such as jumping, three windows cannot separate its
phases. Use the per-jump aligned template in "Per-jump alignment: the primary
jump result". The older time-series path is kept as a secondary view.

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
4. The agent returns the segment aggregate: per-fingerprint sample counts, and
   in the default mode one measured block per object plus the identity verdict.

### Player selection versus per-object aggregation

The operator-paced segment mode still selects one object: it has no motion
signal, so it uses the object with the largest covered slot count in the
segment and requires at least 40 covered slots. That mode is a single-actor
capture and is reported as such.

The default `--by-motion` mode no longer selects a player. Every object the
hook observes is tracked under its own identity (see "Per-object identity and
its measured stability"), labelled from its own samples, and printed in its own
block. `--max-actors` (default `8`) bounds how many blocks are printed; when
more objects qualify, the ones with the most samples are kept and a single
`ACTORS dropped=<n>` line says so. Nothing is merged across objects.

### Segment aggregation (operator-paced mode)

For its selected object the operator-paced mode still classifies every sample
as **packed**, **keyed**, or **unmapped**, and prints one line per segment:

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

## Multi-actor labelling: one measured map per live object

### Why per-actor measurement is required

Per-actor indices cannot be transferred, and the shipped files prove it:

| File | Clips | Slots |
|---|---:|---:|
| `RAZIEL.SRA` | 230 | 66 |
| `SLUAGH.SRA` | 23 | 24 |

Index `55` does not exist for the sluagh. A map copied from the player would
name a clip that the other body cannot select, and a 24-slot body cannot fill a
66-slot template. Every actor therefore needs its own measured map, and the
measurement must not require a human timing window per actor. The same root
pose-slot signal that already gives trustworthy vertical phases also gives
horizontal motion for free, so one operator run can label several actors at
once.

### Per-object identity and its measured stability

The object base is recovered from the invocation context exactly as before, and
its identity is a hash of that recovered base. A hash is used so the printed
`id=<hex>` is not an address. The object pointer may rebind every frame when
the engine uses a per-frame arena, so stability is **measured, not assumed**:

- `distinct` is the number of different ids seen in the whole segment.
- `expected` is the largest number of different ids seen in one invocation
  sweep, capped at `32`. A sweep ends when an id repeats, which is what a normal
  per-object loop does once per frame.
- `STABLE` means `distinct` is within a small slack of `expected`
  (`max(2, 25% of expected)`); `CHURN` means it is far above, or the sweep cap
  was hit.

The pose-shape scan that discovers the pose-array field is scored against the
object's own slot count, not the player's, so a 24-slot body is not judged
against a 66-slot layout. `--min-slots` gates only the single-actor segment
mode; the multi-actor mode observes every object with at least one slot and
bounds the printed blocks with `--max-actors` and the sample ordering.

```text
IDENTITY distinct=3 expected=3 cap=32 key=object-node verdict=STABLE alt=shape alt_distinct=3 alt_verdict=STABLE adopted=object-node ambiguous=0
```

A content-only alternative key is also evaluated: a digest of the covered slot
count and the channel slot ranges. It survives pointer rebinding but collides
for two instances of the same model. It is adopted (`adopted=shape`) only when
it is itself stable **and** does not collapse distinct live ids; otherwise
`ambiguous=1` is printed and the pointer key is kept. On `CHURN` with no
unambiguous alternative, no actor is labelled: the run prints
`ACTORS count=0 reason=identity-churn` instead of inventing per-id actors. That
is the difference between "I could not attribute samples" and "this actor
idled".

### Horizontal motion for idle and walk

The root pose slot translation is read as three world components. The shipped
world is Z-up, so the first two components are the ground plane and the third is
the vertical already used for phases. Horizontal speed is the distance between
two consecutive samples of the same object, in world units per sampled frame.

| Constant | Value | Role |
|---|---:|---|
| `H_ENTER` | `1.5` | Larger threshold: speed above it starts a walk |
| `H_LEAVE` | `0.5` | Smaller threshold: speed below it returns to idle |
| `H_HOLD` | `3` | Consecutive frames required before idle/walk commits |

The machine is the same shape as the vertical one: enter above `H_ENTER`, leave
below `H_LEAVE`, and hold the current state inside the dead band for `H_HOLD`
consecutive samples before a change. That is what stops a speed that sits at the
threshold from flapping idle/walk. A calibrated series that never exceeds
`H_ENTER` is measured **no-motion**; it is not reported as idle-walking.

The vertical machine is reused unchanged: `DZ_ENTER = 4.0`, `DZ_LEAVE = 1.5`,
`DZ_HOLD = 3`, ascent/fall/land, and the per-jump aligned template that carries
`jump_up`, `fall` and `land`. Only the sample source changed (each actor now
contributes its own series), not the logic or the thresholds.

### Per-actor output

Each measured object prints one bounded block:

```text
ACTOR id=<hex> slots=66 samples=44 identity=STABLE
PHASE id=<hex> name=idle frames=22 clips=55:0.91,2:0.09 dominant=55 coverage=0.91
PHASE id=<hex> name=walk frames=22 clips=2:1.00 dominant=2 coverage=1.00
PHASE id=<hex> name=ascent frames=0 clips=- dominant=unknown coverage=0.00
PHASE id=<hex> name=fall frames=0 clips=- dominant=unknown coverage=0.00
PHASE id=<hex> name=land frames=0 clips=- dominant=unknown coverage=0.00
LABELS id=<hex> slots=66 idle=55 walk=2 jump_up=unknown fall=unknown land=unknown agreement=0.91 source=runtime-motion reason=no-jumps
```

`idle` and `walk` come from the horizontal states; `ascent`, `fall` and `land`
from the vertical phases. `jump_up`, `fall` and `land` follow the same per-jump
template gate as before, one template per actor. Output stays bounded:
`--max-actors` limits blocks, each block has one `ACTOR`, five `PHASE` and one
`LABELS` line, and the host line cap grows to fit exactly that many blocks. The
`CALIB actors=<n> zcalib=<n>/<n> hcalib=<n>/<n> movers=<n>` summary line is the
only segment-wide calibration text.

### An unmeasured actor says `unknown`

No label is produced without measurement. When a field cannot be established,
the `LABELS` line names the measured reason:

| Reason | Meaning |
|---|---|
| `too-few-samples` | The object was observed, but below the minimum sample count (`12`); every field is `unknown`. |
| `no-motion` | Horizontal speed never exceeded `H_ENTER`, so `walk` is `unknown`. `idle` may still be measured. |
| `no-idle` | The object never sat below `H_LEAVE`; `idle` is `unknown`. |
| `no-jumps` | Fewer complete arcs than `--min-jumps`; the jump fields are `unknown`. |
| `low-agreement` | A phase or bucket existed but its coverage was below `0.40` or the weakest used bucket agreed below `0.60`. |
| `uncalibrated` | The vertical or horizontal signal failed its sample, finite or range check. |
| `identity-churn` | Object identity was unstable and no unambiguous alternative key existed; no actor was labelled. |

If every field is unknown the line uses the reason form with no agreement and no
source:

```text
LABELS id=<hex> slots=8 idle=unknown walk=unknown jump_up=unknown fall=unknown land=unknown reason=too-few-samples
```

Defaulting such an actor to the player's `55`/`2`/`35`/`36`/`37` would be
fabrication: those numbers were measured on a different body with a different
clip and slot count. A documented `unknown` is a successful measurement of the
absence of evidence.

### The three-step multi-actor procedure

1. **Prepare.** Load a save with the actors you want in view, on solid ground,
   and leave the game unpaused. One of them must be able to walk; an enemy that
   only stands still can still yield `idle`, but not `walk` or a jump.
2. **Run.** From the scratch directory:

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --motion-seconds 20 --max-actors 8
   ```

   Let the subject stand, walk a little, and jump a few times. The probe reads
   motion, not rhythm.
3. **Read.** Check the `IDENTITY` line first: `verdict=STABLE` and
   `adopted=object-node` (or a deliberate `adopted=shape`) mean per-actor
   attribution is trustworthy. Then read each `LABELS` line. A field at
   `unknown` is missing evidence; repeat that behaviour rather than forcing a
   value.

### What the multi-actor mode cannot establish

- **An idle-only actor has no walk label.** If the object never crosses
  `H_ENTER`, `walk` is `unknown`, by construction. That is the honest result,
  not a failure to try.
- **An actor whose AI never engages cannot be labelled.** If a creature never
  moves or jumps in the session, its motion states were never selected and the
  probe has nothing to attribute. Scripted idles are measured; unobserved
  states are not.
- **Identity is measured, not guaranteed.** A busy arena with per-frame pointer
  rebinding can produce `CHURN`; the probe then refuses to label rather than
  merge objects. A scene with more than the sweep cap of live ids is reported
  as churn even if identity was in fact stable.
- **Agreement is not semantic truth.** `agreement=0.91` says the same clip
  occupied the same relative position or state in 91% of the measured samples
  for that actor. It does not name the pose, and it does not prove the clip is
  the only one the engine could select there.
- **Two instances of one model can share a shape key.** The alternative key is
  adopted only when the shape count still separates the live ids; otherwise the
  run reports the ambiguity instead of merging identical bodies.

## Why time windows were replaced

> **Secondary view.** The per-jump aligned template in the next section is the
> primary jump result. Keep this time-series path for a quick cadence check; it
> cannot align jumps of different lengths, which is why it diluted each phase.

A single jump window blends three different animations: ascent, fall and
landing. The measured jump window proves it — four clips shared it (`27:0.16`,
`16:0.15`, `2:0.12`, `37:0.07`, keyed `0.21`) and no clip passed `0.16`
coverage. Lengthening that window does not clean the blend; it averages more
phases into the same histogram. A single window asks "which clip is jump?",
and the honest answer is "none of them, separately".

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
every frame because the pose array lives in a per-frame arena. Each sample takes
the slot-0 translation: component 12 is Z, and components 10 and 11 are the
horizontal X/Y used for idle/walk.

The probe does not trust the signal. In the single-actor segment mode it prints
one bounded calibration line:

```text
ZCALIB signal=root-slot-z samples=812 zmin=-4831.5 zmax=-4632.0 dzmin=-37.2 dzmax=39.8 sign_changes=41 flat=0.12 calibrated=yes
```

In the multi-actor default, calibration is summarised once for the segment as
`CALIB actors=<n> zcalib=<n>/<n> hcalib=<n>/<n> movers=<n>`, and a failed
signal makes that actor's fields `unknown` with `reason=uncalibrated`.

Calibration requires at least 24 samples, at least 95% finite values and
`|z| <= 1e6` world units. The line also shows the observed range, the `dz`
range, how often `dz` changes sign and the flat fraction, so a wrong signal is
visible immediately: standing should be flat with about zero sign changes, and
a jump must change sign. If calibration fails, that actor's phases are not
labelled; an uncalibrated actor is missing evidence, not a result.

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
distribution, keyed share, dominant clip and coverage. In the multi-actor
default the same `PHASE` line carries `id=<hex>` and the block repeats per
actor; the single-actor view below is the operator-paced output.

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

The single-actor run ends with one line that is directly usable:

```text
LABELS raziel idle=55 walk=2 jump_up=27 fall=2 land=55 source=runtime-vertical
```

`jump_up`, `fall` and `land` are this run's measurement (`ASCENT`, `FALL`,
`LANDED`). In that single-actor mode `idle` and `walk` are **carried**: a jump
run does not observe either state, so their values are the confirmed
three-segment labels and can be overridden with `--idle-clip` / `--walk-clip`.
In the multi-actor default nothing is carried: `idle` and `walk` come from that
actor's own horizontal states, and every field is `unknown` when it was not
measured.

A phase is printed as `unknown` when its frame count is below `6` or its
dominant coverage is below `0.40`, and every field is `unknown` when the actor
is uncalibrated. A thin or blended phase is not worth a guess.

### Modes and flags

| Flag | Meaning |
|---|---|
| `--by-motion` | Default mode: one run, every observed object classified by its own horizontal and vertical motion, aggregated per phase. |
| `--motion-seconds <SECONDS>` | Length of that single run (default `15`). `--total` overrides it. |
| `--dz-enter`, `--dz-leave`, `--dz-hold` | Override the vertical hysteresis constants; `leave` must stay below `enter`. |
| `--h-enter`, `--h-leave`, `--h-hold` | Override the horizontal idle/walk hysteresis constants; `leave` must stay below `enter`. |
| `--max-actors <N>` | Maximum actor blocks printed per run (default `8`). |
| `--min-slots <N>` | Minimum covered slots for the single-actor segment player (default `40`); it does not gate the multi-actor mode. |
| `--idle-clip`, `--walk-clip` | Carried labels for the single-actor segment line only. |
| `--slots` | Model slot count the host uses to validate the file tables (default `66`); it is not a per-actor filter. |
| `--segments` | Named, operator-paced segments (the three-prompt capture). Still supported. |
| `--auto-windows <SECONDS>` | Diagnostic time series, one numbered window per step. Still supported, no longer used for labelling. |
| `--total <SECONDS>` | Stop after this much wall time. |

Supplying `--by-motion` together with `--segments`, `--segment` or
`--auto-windows` is a typed error, never a silent override.

### The three-step single-actor jump procedure

1. **Stand.** Load a save where the subject stands on solid ground in a quiet
   room, focus the game window and leave the game unpaused.
2. **Run.** From the scratch directory:

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --slots 66 --motion-seconds 15 --max-actors 1
   ```

   Wait for `CLIPLABEL HOST attached`, then for the `SEGMENT name=motion`
   prompt. Stand still for a second, then jump normally and land, as many times
   as fits in the run. No rhythm is required.
3. **Read.** Check `IDENTITY` and `CALIB` first. Then read the `PHASE` lines and
   the `LABELS` line of that block. A field printed `unknown` is missing
   evidence, not a negative.

### How to read the result

| Pattern | Reading |
|---|---|
| `verdict=CHURN` | Object identity was unstable. Do not read per-actor labels; the run refuses them instead of merging objects. |
| `adopted=shape` | The content key separated the live ids and replaced the unstable pointer key. |
| `ambiguous=1` | The alternative key would merge distinct live ids; it was reported, not used. |
| `reason=too-few-samples` | The object was seen, but not enough to measure. Re-run with more time on that actor. |
| `reason=no-motion` | The actor never crossed `H_ENTER`; `walk` is legitimately unknown. |
| `calibrated=no` | The vertical signal failed. Do not read the phases; re-run and check `CALIB`. |
| One clip dominates `ascent` and a different one dominates `fall` | The two phases are distinct animations. That is the jump profile. |
| The same clip dominates `ascent` and `fall` | One animation covers the airborne body; the phase split is real but the clip is shared. |
| `land` dominated by the `ground` clip | Landing returns to the stance animation; the phase boundary is real but the clip is shared. |
| Coverage below `0.40` in a phase | The phase is a blend. Report the class, not the guessed index. |
| `keyed` or `unmapped` dominant | That phase selected a producer the file index cannot name. |

## The exact four-step segment procedure (optional)

## Per-jump alignment: the primary jump result

### Why phase aggregation failed

The first vertical run reported the failure directly:

```text
ZCALIB samples=451 z=-299..756 dz=-459..463 sign_changes=103 flat=0.34 calibrated=yes
ascent  131 samples dominant=35 coverage=0.35 (36:0.31)
fall    164 samples dominant=36 coverage=0.29 (37:0.29, 35:0.12, 34:0.12)
land    136 samples dominant=34 coverage=0.35 (35:0.22, 0:0.11)
ground   20 samples dominant=0  coverage=0.60
LABELS jump_up=unknown fall=unknown land=unknown
```

Three facts explain it, and none is a threshold to tune:

- **The signal is coarse for the arc.** 451 samples over 15 s is about 30 Hz,
  while the vertical delta peaks near 460 units per sample and the whole Z
  span is about 1055. A jump crosses in a handful of samples.
- **A phase aggregate pools different relative positions.** Roughly fifteen
  jumps share each phase bucket, and inside one jump the clip changes
  (takeoff, rise, apex, fall, impact, recovery). Pooling those positions
  dilutes every phase to about 0.3 coverage, because the same bucket holds a
  takeoff pose in one jump and a recovery pose in another.
- **The dominant clips are held poses, not cycles.** The winners are one- and
  two-frame tables, consistent with short held poses rather than a loop.

Phase aggregation answers "which clip is jump?" over the whole run, and the
honest answer is "several, because a jump is a sequence." Alignment asks the
useful question instead: "what is the clip at the same relative point of every
jump?"

### The jump detection rule

The probe reads the root pose slot's world Z on every hook invocation and tags
each sample with the selected clip. Two consecutive samples give the vertical
delta `dz`. A jump:

- **starts** at the sample just before `dz` crosses above `+DZ_ENTER` (default
  `4.0` world units per sample), so the takeoff ground pose anchors the
  template;
- **ends** once the arc has turned downward (`dz` below `-DZ_ENTER`) and the
  dead band `|dz| <= DZ_LEAVE` (default `1.5`) has held for `DZ_HOLD` (default
  `3`) consecutive samples.

The downward requirement and the quiet hold are the same hysteresis the phase
machine uses; together they stop a long apex hover from closing the jump early.
A gap in the signal closes the jump at the previous sample, and an arc still
airborne at the end of the run is not counted. At most `--max-jumps` (default
`6`) complete jumps are kept, and each prints one bounded line:

```text
JUMP n=1 samples=9 dur=0.30 z=0..1055 seq=35,35,35,36,36,36,36,34,34
```

`samples` and `dur` describe the arc, `z` is the measured minimum and maximum,
and `seq` is the ordered clip per sample. A `keyed` or `unmapped` producer
prints its class, never an invented index.

### The resampler

Every jump has a different length, so the sequences are compared on relative
time. For a jump of `N` samples the probe assigns each of the `--jump-buckets`
(default `10`) buckets to the sample nearest the bucket's center:

```text
bucket b center   = (b + 0.5) / buckets
sample i position = i / (N - 1)
nearest center, ties to the lower sample index
```

Nearest-center always yields exactly one sample per bucket, so a short jump
cannot leave a hole and every jump contributes to every bucket. The mapping is
exact for a known length: `N = 10` with `10` buckets is the identity, and
`N = 5` maps to `0,1,1,1,2,2,3,3,3,4`.

### The agreement metric and the template

For each bucket the probe takes the modal clip across all jumps and reports its
agreement, the modal share:

```text
TEMPLATE buckets=10 jumps=6 b0=35:1.00 b1=35:1.00 b2=36:0.83 b3=36:1.00 b4=36:1.00 b5=37:0.67 b6=34:0.83 b7=34:1.00 b8=0:0.83 b9=0:1.00
```

Agreement is not semantic truth. It says the same clip occupied the same
relative position in that fraction of the detected jumps. A bucket at `1.00`
is stable across the run; a low bucket is a transition the jumps did not share.

### The label rule

The template is split into thirds plus a landing anchor:

| Label | Buckets | Rule |
|---|---|---|
| `jump_up` | first third | modal clip in buckets `[0, buckets/3)` |
| `fall` | middle third | modal clip in buckets `[buckets/3, 2*buckets/3)` |
| `land` | after the last airborne bucket | modal clip in the buckets after the last bucket whose median Z is above the template baseline by more than the leave threshold |

The line is:

```text
LABELS raziel idle=<carried> walk=<carried> jump_up=<clip> fall=<clip> land=<clip> source=runtime-jump-template agreement=<min>
```

Two constants gate it, and `--dry-run` prints both:

- `--min-jumps` (default `3`): fewer complete jumps and every jump label is
  `unknown`, never a guess.
- `--jump-agreement` (default `0.60`): the weakest bucket used for a label
  must reach this share; below it the label is `unknown`.

`agreement=` is the minimum share over every bucket the three labels used.
`keyed`, `unmapped` and empty buckets can never become a clip label. `idle`
and `walk` stay carried from the confirmed three-segment run, because a jump
capture does not observe those states.

`--by-motion` still prints the phase aggregate as secondary output (`ZCALIB`
plus one `PHASE` line per phase); the template is the primary result.

### The three-step jump capture

1. **Stand.** Load a save where Raziel stands on solid ground in a quiet room
   and leave the game unpaused.
2. **Jump normally several times.** Start the run and jump at a natural
   cadence, landing fully between jumps. Do not try to hold a rhythm and do not
   walk or turn between jumps. The run needs about fifteen seconds and at least
   `--min-jumps` complete arcs.

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --slots 66 --motion-seconds 15
   ```

3. **Read the template, then the labels.** The `JUMP` lines are the raw arcs,
   `TEMPLATE` is the aligned profile, and the final `LABELS` line is the derived
   result. A label at `unknown` means the run did not contain enough consistent
   jumps; repeat it rather than forcing a value.

### What the template cannot establish

- **A jump shorter than the sample period.** If the whole arc falls between two
  samples, no complete jump is detected and the line stays silent rather than
  inventing one. At the engine's animation rate a very short hop is the case
  this method cannot see.
- **Different heights or different jump styles.** The template assumes the
  jumps are comparable. A high jump and a short hop align badly and the bucket
  agreement drops; that is the signal to capture one style at a time.
- **Agreement is not semantic truth.** A bucket at `1.00` means the same clip
  held the same relative position across the detected jumps. It does not name
  the pose, and it does not prove the clip is the only one that could play
  there.
- **A coarse rate is only partly fixed.** Sampling every hook invocation and
  printing the measured rate makes the limit visible, but the engine's
  animation rate still bounds resolution. Alignment compares relative
  positions; it does not recover frames that were never sampled.

## The exact four-step procedure

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

- **Per actor, per build.** Each label is one body in this build. A map is not
  transferable between actors (different clip and slot counts) and is not a
  claim about a different build or animation set.
- **A state may pick different producers as the state machine evolves.** The
  label is the table or producer the engine selected in that window, not a
  promise that the same state always picks it. Context, speed, weapon, damage
  or progression can change the selection.
- **The label is not a semantic name in the file.** The `.SRA` carries no
  idle/walk/jump identifier. `idle=55` means "the engine selected packed clip
  55 while the object's measured horizontal speed stayed below `H_LEAVE`", not
  "clip 55 is named idle".
- **A keyed key is not a clip index.** The file index lists packed tables
  only. A keyed key proves the producer and separates frame arrays at their
  first frame; it does not name a clip, and `LABELS` reports it as `keyed`.
- **The keyed frame layout is only partly observed.** The frame count and the
  frame-array pointer are read; the stride, a divisor and the interpolation
  are not established here.
- **Actor identity is measured.** In the operator-paced single-actor mode the
  object is inferred by its slot coverage. In the default mode every object is
  tracked by a hashed identity whose stability is printed; a `CHURN` verdict
  refuses labels instead of guessing which samples belong together.
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
- **`idle` and `walk` are measured in the default mode and carried only in the
  single-actor segment mode.** A single-actor jump run never observes either
  state; the multi-actor run derives both from each object's own horizontal
  speed, and prints `unknown` when it did not.
- **No transitions or blends are modelled.** A phase boundary is committed
  after `DZ_HOLD` frames, so the first frames after a boundary may still carry
  the previous phase. Dense transitions are not resolved.
- **Not every frame is sampled.** A table active for a single frame may never
  win a phase bucket. Absence from the top buckets is not proof the table is
  unused.

## Verification

| Gate | Result |
|---|---|
| `--selftest` | 54 cases, 54 green; the existing fingerprint, segment, keyed, phase and jump cases plus the horizontal dead-band, exact-threshold, walk-commit, no-motion and uncalibrated cases; the identity STABLE/CHURN line cases; and the per-actor label cases (`actor-labels-measured`, `actor-line-shape`, `actor-unknown-no-motion`, `actor-unknown-too-few`, `actor-unknown-low-agreement`, `actor-unknown-no-jumps`, `actor-block-bounded`) |
| `--live-selftest` | 24 cases, 24 green in a real Frida runtime; adds `multi-actor-independent`, `multi-actor-own-labels`, `multi-actor-no-bleed`, `identity-churn-no-merge` and `horizontal-hysteresis` on top of the previous hardware-surface cases |
| `--dry-run` | prints the plan; attaches nothing. It names the per-actor aggregation, the identity verdict, both hysteresis families with their constants, the unknown reasons and the label gates |
| `py_compile` | clean |
| `node --check` on `--print-agent` | clean |
| `cargo test` | 694 green, unchanged by this docs-only slice |

The live harness spawns a throwaway system process, never the game. It
exercises the real `NativePointer`, `Memory`, `Process`, `Interceptor`,
`NativeCallback` and `NativeFunction` bindings. The keyed case asserts the
keyed key and the keyed segment fraction; the negative case asserts that a
keyed head with a null frame array still falls back to the packed producer. The
jump cases drive the agent's `detectJumps` with synthetic arcs, then assert the
template on the returned records: three identical arcs reach `1.00` per bucket,
one outlier lowers only its own bucket, fewer than the minimum arcs and a
grounded series both produce `unknown`, and the resampler is exact for a known
length. The multi-actor case drives three objects with 66, 24 and 8 slots
through the real hook and asserts that each block carries only its own clips:
the walker labels `idle` and `walk`, the never-moving actor labels `idle` and
`walk=unknown reason=no-motion`, and the 5-sample actor prints
`reason=too-few-samples`. The churn case rebinds the object pointer every frame
and asserts `verdict=CHURN`, `ambiguous=1` and zero merged actors. The
hysteresis case runs the agent's own horizontal machine over a dead-band series
and an exact-threshold series and asserts both stay idle. A missing Frida
capability fails loudly before any hook is armed, rather than reporting an
empty segment.

## Reviewer checklist

- [ ] The diff contains no addresses, module-relative numbers, disassembly,
      game bytes or absolute install paths.
- [ ] The per-actor rationale states the measured clip-count and slot-count
      difference, so a transferred index is visibly impossible.
- [ ] Per-object identity names its key, prints `distinct` versus `expected`,
      and states the `STABLE`/`CHURN` rule.
- [ ] Horizontal idle/walk states reuse the vertical hysteresis shape and name
      `H_ENTER`, `H_LEAVE` and `H_HOLD`.
- [ ] The unknown reasons are enumerated and defaulting is called fabrication.
- [ ] The multi-actor procedure has three steps and names the result lines.
- [ ] The multi-actor limitations include an idle-only actor, an actor whose AI
      never engages, and agreement not being semantic truth.
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
- [ ] The jump section states the detection rule, the resampler, the agreement
      metric and the label gate with both constants.
- [ ] Per-jump output is bounded by `--max-jumps` and per-actor output by
      `--max-actors`; there is no per-call text.
- [ ] The procedure names the three actions and the three result lines.
- [ ] The confirm/refute table appears before the run, not after it.
- [ ] The limitations section is not softened into a promise of semantic
      clip names.
- [ ] `cargo test` stays at its baseline.

## Next step

> **2026-11-15 multi-actor revision.** The probe now labels every observed live
> object from its own motion, with a measured identity verdict and a documented
> `unknown` policy. The scratch probe is green at 54 host cases and 24
> live-runtime cases; the identity stability of a real capture is still the
> first thing to read in the next session.

The multi-actor mode is measured but has not yet been run against a live scene
with several actors. The next capture should confirm the `IDENTITY` verdict in
a busy area, then compare two actors' LABELS blocks to show that the maps differ.

> **2026-11-15 resolution.** The aligned per-jump template was run twice. Every
> jump played `35 -> (keyed transition) -> 36 -> (keyed transition) -> 37` (24
> samples over about `0.76 s`) and all six jumps produced the same order, so
> jump-up, fall and land are `35`, `36` and `37` at agreement `0.83 -> 1.00`.
> The keyed buckets are transitions, not clips. See
> `docs/notes/packed-channel-format.md`, "State defaults". The paragraph below
> is the pre-run state, kept for the record.

Idle and walk are labelled; jump was still PARTIAL then. A clip is confirmed
for a phase only when the same group occupies the same position in the
repeating tail across runs.

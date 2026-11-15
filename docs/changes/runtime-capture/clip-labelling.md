# Clip labelling from runtime table selection

The offline clip ranking cannot identify a real idle or walk, so the label
source moves to the runtime: a probe observes which frame producer the engine
selects for the player while a human performs each state, fingerprints the
selected table, and maps packed tables to a clip index. This note records the
failure of the heuristic, the packed and keyed fingerprint schemes, the
segment aggregation, the keyed-locomotion hypothesis, and the exact operator
procedure. The probe itself stays in the session scratch directory because it
carries module-relative anchors and engine field offsets.

The first revision of this note described a docs-only slice: nothing was
launched and no production code changed. The capture has since been run (see
"Runtime result" below), and the measured idle and walk indices are now the
default state map in `sra::ClipMap::for_clip_count`. No golden digest moved.

## Quick path

1. Start the game and load a save where Raziel stands on solid ground.
2. Run the probe with the three timed segments and the path to `RAZIEL.SRA`.
3. Follow the three prompts: stand still, then walk, then jump.
4. Read the final `LABELS` line. A numeric index is a packed clip; `keyed`
   and `unmapped` are producer classes, not indices. The probe prints both the
   invariant key and the legacy digest for the dominant sample.

For a repeating action such as jumping, three windows cannot separate its
phases. Use the time-series path in "The jump profile: a time series, not a
longer window".

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
- **Jump is not confirmed.** A single timed window contains jump-up, fall and
  land, so a spread across four clips is expected; the probe cannot separate
  them. `27` is the largest share and is kept as a documented best-effort
  default only.

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

## The jump profile: a time series, not a longer window

A single jump window blends three different animations: ascent, fall and
landing. The measured jump window proves it — four clips shared it (`27:0.16`,
`16:0.15`, `2:0.12`, `37:0.07`, keyed `0.21`) and no clip passed `0.16`
coverage. Lengthening that window does not clean the blend; it averages more
phases into the same histogram. A single window asks "which clip is jump?",
and the honest answer is "none of them, separately".

A time series asks a different question: "which clip is playing right now?"
Jumping repeats, so a short window per step lands in one phase of the cycle at
a time. The dominant clip of each window tracks that phase, and the repeating
tail of the sequence is the cycle. The phase is inferred from **ordering**, not
from content: the probe never reads vertical velocity. That is enough to
separate ascent from fall when the cadence is steady, and it is the reason the
output is a sequence rather than a longer label.

### New flags

| Flag | Meaning |
|---|---|
| `--auto-windows <SECONDS>` | Emit a numbered window every `SECONDS`; names are generated (`w1`, `w2`, ...). Default off. Replaces named `--segments`. |
| `--total <SECONDS>` | Stop after this much wall time. Optional; without it the run continues until Ctrl+C or the line cap. |
| `--segments` | Unchanged: named, operator-paced windows. Supplying it together with `--auto-windows` is a typed error, never a silent override. |

### Output shapes

One line per window, carrying the same buckets as a named segment plus the
window index and its scheduled span:

```text
CLIPLABEL window=3 t=2.0-3.0 packed=0.80 keyed=0.20 unmapped=0.00 clips=27:0.78,16:0.22 dominant=27 coverage=0.78 fp=6:240:81ba1565 slots=66
```

The run ends with one sequence line, one token per window:

```text
SEQUENCE windows=12 step=1.0 dominant=27,27,16,16,2,2,55,55,27,27,16,16
```

Each token is the window's dominant clip index, or `keyed` / `unmapped` when a
producer class won, or `-` when the window received no sample. An empty window
is missing evidence, not a vote; it stays visible in the sequence instead of
silently disappearing.

### The three-step jump profile

1. **Hold the rhythm.** Start from standing still, then jump repeatedly at a
   steady rhythm for the whole run. Do not stop between jumps longer than the
   cycle you want to observe.
2. **Run the series.** From the scratch directory:

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --slots 66 --auto-windows 1.0 --total 15
   ```

   The hook arms on the animation channel loop and the probe emits fifteen
   one-second windows.
3. **Read the cycle.** Take the repeating tail of `SEQUENCE` and tag the first
   distinct group as **ascent**, the next as **fall**, and the group that
   returns to the idle clip as **landing / recovery**. The tail repeats because
   the action repeats; the first full cycle is the profile.

### How to read the sequence

| Pattern | Reading |
|---|---|
| Two windows of one clip, two of the next, then the idle clip, repeating | Ascent, fall, landing/recovery. The cycle is the profile. |
| The same clip in every window | No phase separation. The window is as long as the phase, or the jump cadence is irregular; shorten `step` and steady the rhythm. |
| One clip for many windows, then a sudden switch | The window straddled a phase edge. One window is one blend; move to a shorter step to locate the boundary. |
| `keyed` or `unmapped` runs | That phase selected a producer the file index cannot name. Report the class, not an invented index. |
| No repeating tail at all | The cadence is irregular; the sequence smears and only the largest bucket per window is trustworthy. |
| `-` anywhere | An empty window: no sample arrived. Repeat the run; do not read it as a phase. |

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
- **The probe never reads vertical velocity.** The jump phase is inferred from
  the ordering in `SEQUENCE`, not from motion. An irregular jump cadence, or a
  step comparable in length to ascent or fall, smears the sequence; ascent and
  fall cannot be told apart by the content of one window.
- **A sequence token is not a semantic name.** `SEQUENCE ... 27,27,16,16`
  means "the engine selected clip 27 in two consecutive windows while the
  operator was ascending", not "clip 27 is named ascent". The tag comes from
  where the group sits in the cycle, and a different cadence can shift it.
- **No transitions, blends or mixed windows.** Only the dominant producer per
  window is reported; the frames at a transition are part of the next or
  previous bucket. In the jump profile that means the boundary window is a
  blend and only the repeating interior of a group is clean.
- **Not every frame is sampled.** A table active for a single frame may never
  win a bucket. Absence from the top buckets is not proof the table is unused.

## Verification

| Gate | Result |
|---|---|
| `--selftest` | 21 cases, 21 green; adds `auto-window-lines`, `auto-window-empty`, `auto-window-sequence` and `auto-window-conflict` to the keyed, keyed/unmapped, capped-list and form-invariance cases |
| `--live-selftest` | 9 cases, 9 green in a real Frida runtime; adds `auto-window-sequence`, which drives four synthetic windows (one empty) through the real agent segment lifecycle and asserts the token order on top of the packed, keyed and negative cases |
| `--dry-run` | prints the plan; attaches nothing. `--auto-windows 1.0 --total 15` reports `planned=15`; `--auto-windows` plus `--segments` exits with the typed conflict |
| `py_compile` | clean |
| `node --check` on `--print-agent` | clean |
| `cargo test` | 688 green after wiring the labelled defaults; `packed_channel` owns the invariant definition and its file/memory tests |

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
- [ ] The packed fingerprint is described as content, not as a raw byte range
      at a fixed position.
- [ ] The keyed fingerprint names its bounded view and excludes the pointer.
- [ ] The keyed structure is not given a divisor it was never observed to
      have.
- [ ] The collision policy is explicit: lowest index, `ambiguous` reported.
- [ ] The procedure names the three actions and the three result lines.
- [ ] The confirm/refute table appears before the run, not after it.
- [ ] The limitations section is not softened into a promise of semantic
      clip names.
- [ ] `cargo test` stays at its baseline.

## Next step

Idle and walk are labelled; jump is still PARTIAL. The time-series probe is the
instrument for the jump profile: run the three-step procedure above, then
repeat it twice at the same cadence. A clip is confirmed for a phase only when
the same group occupies the same position in the repeating tail across runs.
Until then, `27` stays the documented best-effort default and must not be
reported as a confirmed jump clip. A second idle run on a different section
would characterise the clip-0 variant. Do not change the heuristic on a single
unmapped window: re-run it first, because that is an unobserved table, not a
wrong label.

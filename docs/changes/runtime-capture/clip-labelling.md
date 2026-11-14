# Clip labelling from runtime table selection

The offline clip ranking cannot identify a real idle or walk, so the label
source moves to the runtime: a probe observes which packed frame table the
engine selects for the player while a human performs each state, fingerprints
that table, and maps it to a clip index. This note records the failure of the
heuristic, the fingerprint scheme, the segment aggregation, and the exact
operator procedure. The probe itself stays in the session scratch directory
because it carries module-relative anchors and engine field offsets.

Nothing was launched, attached or instrumented for this slice. No production
code changed and no golden moved.

## Quick path

1. Start the game and load a save where Raziel stands on solid ground.
2. Run the probe with the three timed segments and the path to `RAZIEL.SRA`.
3. Follow the three prompts: stand still, then walk, then jump.
4. Read the final `LABELS` line and pass the three indices to
   `--clip-idle`, `--clip-walk` and `--clip-jump`.

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

A channel record points at the packed frame table it is playing. For each
animation invocation the probe reads:

- whether the channel is on the keyed producer (keyed channels are skipped
  because they are not the packed clip table);
- the table's frame count and divisor from the form-invariant header;
- the complete bit-plane block (flag byte plus the present planes).

The same values are computed offline from the shipped file for every clip.

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

For the selected object the probe turns fingerprint counts into fractions,
maps each fingerprint to a clip index, and prints the four largest buckets:

```text
CLIPLABEL segment=idle seconds=10.0 clips=6:0.94,17:0.03,2:0.03 dominant=6 coverage=0.94
CLIPLABEL segment=walk seconds=20.0 clips=162:0.91,6:0.05 dominant=162 coverage=0.91
CLIPLABEL segment=jump seconds=10.0 clips=212:0.72,30:0.18 dominant=212 coverage=0.72
LABELS raziel idle=6 walk=162 jump=212 source=runtime
```

Rules that keep the output honest:

- `coverage` is the dominant bucket over **all** samples, including samples
  whose fingerprint is not in the file index.
- Unmapped samples are reported as `unmapped=<fraction>` and are never
  silently dropped.
- If unmapped samples outnumber the best mapped clip, the dominant label is
  `unmapped`, not a guessed index.
- An empty segment prints `note=empty`; it is missing evidence, not a vote.
- Only one line per segment and one `LABELS` line are printed. There is no
  per-call text, and the host enforces a hard line cap.

## The exact four-step procedure

1. **Set up.** Start `SRX.exe`, load a save where Raziel stands on solid
   ground in a quiet room, focus the game window, and leave the game
   unpaused.
2. **Run the probe.** From the scratch directory:

   ```text
   python sr1_clip_label_probe.py --attach SRX.exe --sra <RAZIEL.SRA> \
       --slots 66 --segments idle:10,walk:20,jump:10
   ```

   Wait for `CLIPLABEL HOST attached`; the hook arms on the animation channel
   loop.
3. **Drive the three windows.** At the `idle` prompt, stand still: no
   movement, no attack, no camera turn. At `walk`, hold one movement
   direction without jumping. At `jump`, jump repeatedly and land between
   jumps. Each window is timed by the probe; the next prompt is the signal to
   change action.
4. **Record the result.** Read the three `CLIPLABEL` lines and the final
   `LABELS` line. Use the indices directly:

   ```text
   --clip-idle <n> --clip-walk <n> --clip-jump <n>
   ```

   If a line says `note=empty`, `dominant=unmapped`, `low_slots=1`, or has a
   low `coverage`, repeat that segment. A state may legitimately select more
   than one clip; the dominant bucket is the stable label for that window.

## What it cannot establish

- **One actor, one build.** The measurements are Raziel's shipped body in
  this build. Other actors, builds and animation sets are unobserved.
- **A state may pick different clips as the state machine evolves.** The
  label is the table the engine selected in that window, not a promise that
  the same state always picks the same table. Context, speed, weapon, damage
  or progression can change the selection.
- **The label is not a semantic name in the file.** The `.SRA` carries no
  idle/walk/jump identifier. `idle=6` means "the engine selected clip 6 while
  the operator stood still", not "clip 6 is named idle".
- **The player is inferred, not identified.** The probe uses the
  largest-coverage object. A scene with another 66-slot actor could compete
  for that role.
- **No transitions, blends or mixed windows.** Only the dominant table per
  window is reported; the frames at a transition are part of the next or
  previous bucket.
- **Not every frame is sampled.** A table active for a single frame may never
  win a bucket. Absence from the top four is not proof the table is unused.

## Verification

| Gate | Result |
|---|---|
| `--selftest` | 17 cases, 17 green (includes `fingerprint-form-invariant`) |
| `--live-selftest` | 8 cases, 8 green in a real Frida runtime; synthetic channel records and tables with known fingerprints assert the hash, the mapping and the segment aggregation |
| `--dry-run` | prints the plan; attaches nothing |
| `py_compile` | clean |
| `node --check` on `--print-agent` | clean |
| `cargo test` | 675 green; `packed_channel` owns the invariant definition and its file/memory tests |

The live harness spawns a throwaway system process, never the game. It
exercises the real `NativePointer`, `Memory`, `Process`, `Interceptor`,
`NativeCallback` and `NativeFunction` bindings. A missing Frida capability
fails loudly before any hook is armed, rather than reporting an empty segment.

## Reviewer checklist

- [ ] The diff contains no addresses, module-relative numbers, disassembly,
      game bytes or absolute install paths.
- [ ] The fingerprint is described as content, not as a raw byte range at a
      fixed position.
- [ ] The collision policy is explicit: lowest index, `ambiguous` reported.
- [ ] The procedure names the three actions and the three result lines.
- [ ] The limitations section is not softened into a promise of semantic
      clip names.
- [ ] `cargo test` stays at its baseline.

## Next step

Use the `LABELS` indices as the presentation map, then verify them in one
rendered pass with `--play ... --clip-idle/--clip-walk/--clip-jump`. If a
segment returns `dominant=unmapped`, the runtime table is not in the file
index: re-run the segment before changing the heuristic, because that is an
unobserved table, not a wrong label.

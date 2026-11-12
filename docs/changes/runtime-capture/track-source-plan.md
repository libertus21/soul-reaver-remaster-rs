# track-source-plan - capture what the track sampler reads (2026-11-11)

This is a **preparation** slice. It states the refutation that makes the question
urgent, pins the runtime role that samples track frames, designs a compact
read-only capture of the memory it reads, and ships the instrumentation and the
byte-comparison tool as local scratch files. Nothing was attached, launched or
instrumented for this document, and no engine behaviour changed. The committed
diff is this plan only.

## Quick path

1. Read "The refuted conclusion" - it supersedes the earlier verdict.
2. Read "The question" and "The pinned role" - the role table is the deliverable.
3. Read "The capture design" and "The classification rule".
4. Read "The three outcomes" - each one states what it establishes and refutes.
5. Run the 5-step human procedure when a session is available.
6. Read "What this cannot establish" before writing any conclusion.

## The refuted conclusion

`docs/notes/animation-conclusion.md` concluded that the remaster does not author
skeletal body animation. **That conclusion is refuted by a live full-pose
capture.** The observed numbers:

| Object | Slots changed | Samples | Max rotation delta | Pose pointer changes |
|---|---:|---:|---:|---:|
| `raziel` | **66 / 66** | 1,939 | up to **1.84** | **203** |
| `sluagh` | **24 / 24** | 726 | up to **1.57** | **688** |

The track sampler fired **13,449** times in the same window. Every bone rotates.
The pose buffer is rebuilt constantly - the pointer changes hundreds of times -
which is exactly why a static file scan could never find a stable authored block.

What this does and does not overturn:

- It **does** overturn "the body pose has no per-bone motion beyond aim". Per-bone
  motion exists at runtime in this build.
- It does **not** retroactively invalidate the file scans. Those scans were
  negative for the carrier shapes they tested; they were answering a different
  question than "does runtime motion exist". The correct reading is that the
  motion's **source was never observed**, not that the earlier counts were wrong.
- The source is now the precise target. That is what this experiment captures.

## The question

The track sampler runs at full rate and produces real per-bone motion, but its
**input** has never been observed. One question:

> When the track sampler selects a track frame, what memory does it read, and
> where does that memory come from?

Everything this plan designs is in service of that one question. It does not try
to re-derive the pose math or to rescan a file family.

## The pinned role

The track sampler is the **per-object animation tick** - the same role the earlier
probes counted. Its read site was pinned statically, read-only, from the shipped
module. The decisive structural fact is a correction to a natural assumption:

> **The sampler does not receive a source pointer and does not receive a frame
> index.** A caller cannot hand it the animation data, because neither is an
> argument.

| Question the task asks | Answer |
|---|---|
| Which argument/register carries the source pointer? | **None.** The only data-bearing argument is the **object** (the fourth argument). The source pointer is derived: object -> actor -> animation table -> animation record by animation id -> the record's **track-table pointer**. |
| Which argument/register carries the track/frame index? | **None.** The index is computed per track: `index = (clock / divisor) % frame_count + 1`, using the entry's own divisor and frame count and a clock. |
| Where does the sampled output land? | The **16-byte head cell at the entry's destination pointer**. The source frame is at that same pointer plus `index x 16` bytes; the head cell is immediately followed by the frame bank. |

The per-track entry is 16 bytes: an 8-byte **destination pointer**, a **frame
count**, and a **divisor**. The destination pointer is simultaneously the output
cell and the base of the frame bank - the head cell comes first, then the frames.

The clock is one of two sources: the object's own animation counter when the
object's mode bit selects it, otherwise a global time counter. When that clock
reads its dead value the sampler skips the copy entirely, which the probe reports
as a missing observation rather than an empty table.

### The gap, and how to close it

The sampler is the **consumer**. It does not author the frames; it copies them.
The routine that **fills** a frame bank is still unpinned, and that routine is the
actual source of the animation data. Close it in one of two ways:

1. **Static xref:** from the track-table pointer field, follow the store that
   writes it. That store is inside the object-container relocation/install path
   already named by `docs/notes/animation-loader-trace.md`.
2. **Runtime write-watch:** arm a page access monitor on a dumped source pointer
   and read back the faulting instruction address. That address is the producer.
   This is the operation most likely to perturb timing, so it is opt-in and is not
   part of the default capture.

The capture below is deliberately aimed at the consumer first, because it is
already pinned and fires steadily; the producer is the follow-up, not the
starting point.

## The capture design

Compact and aggregated. The previous two captures hit a 5,000-line cap because
they wrote per-call records; this design cannot.

| Scope | Recorded | Written where |
|---|---|---|
| Per object, **once per distinct source pointer** | object name, slot count, track count, source pointer, region class, frame count, divisor, first index | one bounded `.bin` dump of the source region (head cell plus frame-bank start) plus one manifest line |
| Per object, **once** | the track table: count and up to a bounded number of 16-byte entries | one `.bin` dump plus one manifest line |
| Per call, **aggregated only** | call count per object, the set of distinct source pointers, the observed index range | in memory, flushed to the summary |
| Per call | **nothing** | never a per-call dump |

Rules that keep it small:

- **Deduplication:** a source pointer is dumped once per `(object, pointer)`; the
  table once per object. A reloaded object with new pointers is a new key.
- **Bounded dumps:** a hard `--dumps N` cap (default 40). When the cap is reached,
  further distinct pointers are counted but not written, so the summary still
  reports the true distinct-source count.
- **Bounded dump span:** the default 256 bytes covers the 16-byte head cell and the
  first frames of the bank. Longer banks are only partly sampled and that is
  stated, not hidden.
- **Bounded index work:** a full aggregation scan runs on the first few calls per
  object and then every Nth call. The observed index range is therefore a
  **lower bound**, and the summary prints how many scans produced it.
- **Hard line cap** with a clean detach, exactly as the earlier probes.
- **Aggregation is gated:** only calls in which the sampler actually ran its body
  are counted. A gated call is not a sample.

### Reporting that cannot be mistaken for absence

The summary and heartbeat distinguish these states explicitly:

- `unobserved` - the hook never fired; this is a missing observation.
- `gated` - the sampler fired but its visibility gate rejected the object.
- `noTracks` - the record's track-table pointer is null.
- `clockDead` - the sampler skipped the copy because the clock was dead.
- `chain-unresolved` - the object/actor/table/record chain did not resolve.

None of these is reported as "zero sources, therefore no data". A `STAT-NOTE`
names every hook that never fired.

## The classification rule

The probe prints the engine module's **base and range** on `READY`, so the
classification is verifiable by hand. For each distinct source pointer it assigns
one of four placements:

| Placement | Meaning |
|---|---|
| inside the engine module's mapped range | a **compiled, module-resident** table |
| a mapped file range | an **archive-mapped** buffer |
| an anonymous range (heap) | a **load-time allocation** |
| anything else | **unresolved** |

The per-object summary prints `source_is_in_module=yes`, `no`, `mixed`, or
`unobserved`. `mixed` is a real result and must not be collapsed to `yes`.

## The three outcomes

The comparison tool classifies every dump against the game archive with three
symmetric outcomes. A negative is as meaningful as a positive.

### EXACT - the dumped run appears verbatim in the archive

- **Shows:** the full dumped span is found contiguously in `BIGFILE.DAT`.
- **Establishes:** the sampled frame bank is **stored authored data**; a file-only
  replay is a parser, and the archive offset is the locator.
- **Refutes:** "the frames exist only as a load-time product."
- **Does not prove:** that these frames drive the body pose. That remains
  correlation until the producer is pinned.

### PARTIAL - a prefix and/or suffix survives, the whole run does not

- **Shows:** the longest surviving prefix and suffix, and 16-byte block survival.
- **Establishes:** a stored record whose middle diverged - the signature of
  rebased pointer fields, or of a dump that spans the head cell plus frames. It
  **localises** the authored run.
- **Refutes:** "no stored bytes of this shape survive at all."
- **Does not prove:** that the whole region is authored, or that the divergent
  middle is not a computed field.

### NONE - no meaningful run survives

- **Shows:** no contiguous match, no surviving prefix/suffix, no block survival.
- **Establishes:** the frame bank is a **load-time product** (decompressed) or a
  **runtime-computed** buffer; a file-only replay needs the decompressor or the
  math.
- **Refutes:** "the source is a stored archive run of this shape."
- **Does not prove:** that the body animation data is absent elsewhere; it bounds
  the answer to the sampled pointers.

### The fourth state - not captured

If the sampler never ran, or the object chain never resolved, the run is
**unobserved**. Re-pin before drawing any of the three verdicts. A short or
stationary window is not a result.

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Start the probe from the session scratch directory.** Run
   `python sr1_track_source_probe.py --attach SRX.exe --wait 300
   --log <scratch>\track_source.log --dump-dir <scratch>\track_source
   --dumps 40`. Wait for the `READY` line (it prints the module base **and**
   range) and at least one `TRK STAT` heartbeat.
3. **Exercise the body path.** Idle about 10 s, walk continuously for at least
   20 s, then attack and turn for about 10 s each. The point is a window in which
   the body is visibly moving.
4. **Stop and keep.** Press `Ctrl+C`. The probe writes `track_summary.txt` before
   detaching. Read the `SUMMARY`: per object `calls=`, `distinct_sources=`,
   `source_is_in_module=`, `index_range=a..b`. Read `STAT-NOTE`: any hook listed
   as `unobserved` is a missing observation, not evidence of absence.
5. **Compare, then re-run once.** Run
   `python track_source_compare.py --dump-dir <scratch>\track_source
   --bigfile <install>\1\BIGFILE.DAT --bigfile <install>\2\BIGFILE.DAT`.
   Read the per-dump verdicts as `EXACT` / `PARTIAL` / `NONE` (or `unobserved` if
   the log shows the sampler never ran). A verdict used to refute or confirm the
   conclusion must reproduce on a second run.

The summary has this shape (template, not a result):

```text
TRK SUMMARY object="raziel" slots=66 tracks=<N> calls=<N> distinct_sources=<N> source_is_in_module=<yes|no|mixed> index_range=<a..b> scans=<N> dumped_table=<yes|no>
TRK SUMMARY   regions <placement>=<count> ...
TRK SUMMARY tally objects=<N> calls=<N> distinct_sources_total=<N>
```

## What this cannot establish

- **It captures what the sampler reads, not why a bone moved.** A non-empty source
  does not prove it is the body-pose source; only correlation does, and the
  producer is still unpinned.
- **A zero distinct-source count is bounded.** If the table is empty or the chain
  never resolves, the body source is outside this role; that is a real negative for
  the sampler, not proof that no data exists anywhere.
- **The dump is bounded.** A 256-byte span samples the head cell and the first
  frames; a longer bank is only partly captured.
- **The index range is sampled.** It is a lower bound measured between retained
  scans, not the true per-frame peak.
- **One build, one window, one actor.** A negative is bounded by the observed
  process, not the product.
- **PARTIAL does not prove causality.** Surviving bytes localise a stored run; they
  do not prove that run drives the pose.
- **NONE does not prove the bytes never existed on disk.** It rules out a stored
  run of this shape and encoding, not every encoding.
- **Placement is not authorship.** `source_is_in_module=yes` proves where the bytes
  live, not who wrote them.
- **Instrumentation can perturb timing.** Do not infer frame cadence or performance
  from the probe.

## Roles

| Role | Identity | What it hands the probe |
|---|---|---|
| Track sampler (consumer, pinned) | The per-object animation tick that fires steadily; reads the track table and copies one frame per track into the head cell. | The object argument, the derived track table, every entry's destination pointer, frame count and divisor, and the computed index. |
| Track table | Reached through the animation record; a count plus 16-byte entries. | The per-track destination pointer, frame count and divisor. |
| Frame bank | Head cell plus frames, at the destination pointer. | The bytes the sampler reads; the probe dumps a bounded prefix. |
| Object-container relocator (producer, **unpinned**) | The load-time pass that rebases the record's pointers. | The follow-up target; close it by static xref or by a write-watch on a dumped pointer. |
| Pose builder / palette assembler | Downstream of the sampler. | Not the sampler's output; named so a reader does not confuse them with the read site. |

No addresses or field offsets are recorded here. The probe and the comparison
tool carry them and live outside the repository, as
`docs/changes/runtime-capture/README.md` explains. If the scripts are lost, the
role table above is enough to re-pin them: each row names a stable behaviour, not
a number.

## Risks and limits

- **Hooks are pinned relative to the engine module.** If the shipped module
  changes, each site must be re-derived from the roles. A stale site silently
  hooks the wrong function; `READY` prints the module base and range so a mismatch
  is at least visible.
- **A zero must be labelled `unobserved`.** The probe prints `STAT-NOTE` for every
  hook that never fired. Do not read an unobserved hook as an absent role.
- **Gated calls are not samples.** The count is taken after the gate, so a
  visibility-rejected object does not inflate `calls`.
- **A per-object copy can hide a stored run.** If the loader copies the record into
  a fresh buffer before rebasing, the comparison tests the copy's bytes. That is
  exactly what the prefix/suffix metric exposes.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes.

## Checklist for review

- [ ] The refuted conclusion is stated first, with the live numbers.
- [ ] The role table states that the sampler has no source-pointer and no index
      argument.
- [ ] The capture is aggregated, deduplicated and capped; there is no per-call
      dump.
- [ ] The classification rule prints the module base and range at `READY`.
- [ ] The three outcomes each state what they establish and what they refute.
- [ ] The procedure has five steps and an explicit re-run.
- [ ] The limits say a negative is bounded and placement is not authorship.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear in
      the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this slice.
No game file is written, no game bytes are copied into the repository, and no
address, file-position offset, disassembly listing or absolute install path
appears here. Roles, structure, format constants and observed counts only.

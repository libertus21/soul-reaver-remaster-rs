# descriptor-dump-plan - settle whether the per-slot descriptor array is stored or a load-time product (2026-11-09)

This is a **preparation** slice. It pins the role that materialises the per-slot
animation descriptor array, designs a read-only memory dump of that array and its
track table, and ships the instrumentation and the byte-comparison tool as local
scratch files. Nothing was attached, launched or instrumented for this document,
and no engine behaviour changed.

The one question: **is the runtime descriptor array a byte run that exists
somewhere in the shipped `BIGFILE.DAT`, or is it produced by the object-loader
decompress-and-relocate pass and present only in memory?** Two independent
on-disk scans are already negative, so the answer decides whether a file-only
replay is a parser (outcome a) or a decompressor (outcome b), or whether the
probe itself is aimed at the wrong site (outcome c).

## Quick path

1. Read "The question" and the branch table - they are the whole deliverable.
2. Read "Pinned role" for what to hook and which argument carries what.
3. Read "Dump format" for the per-object record and its de-duplication key.
4. Read "Comparison method" for how the scratch tool turns bytes into a/b.
5. Run the 5-step human procedure when a session is available.

## The question

The authority note `docs/notes/animation-loader-trace.md` classifies the array as
relocatable object-container records that the loader decompresses and rebases in
place; `docs/notes/procedural-slot-pose-gate.md` then found the corrected gate
negative on every gate model. The archive cannot see a load-time product, and the
static trace cannot prove what the loader actually materialises. The only place
the array is guaranteed to exist in its final form is the running process, after
the load step. So: dump it there and compare.

The probe answers exactly one of three things:

| Outcome | What the dump shows | What it means |
|---|---|---|
| **(a) stored** | the dumped bytes appear verbatim, contiguously, inside `BIGFILE.DAT` | a file-only replay is a parser: read the region, no decompressor needed. The earlier negatives were a gate/region problem, not a missing carrier. |
| **(b) load-time product** | the dumped bytes do **not** appear verbatim | the array is produced by the loader's decompress-plus-relocate pass; a file-only replay needs the decompressor and the exact record placement. Confirms the trace's carrier kind. |
| **(c) not captured** | no descriptor array is dumpable while its consumer fires | the hook site is wrong; re-pin it before drawing any a/b conclusion. |

## Pinned role

The trace separates the **materialiser** (the loader pass that writes the array)
from the **consumer** (the pose builder that reads it). The plan pins both.

| Role | Identity and how it is distinguished | What carries the data |
|---|---|---|
| **Object-container relocator** (the materialiser) | The pass the object-container loader drain calls after decompressing the model definition; it rebases every internal base-relative pointer of the object-container records. Distinguished by its effect: after it runs, the animation record's descriptor and track-table pointer fields are absolute. | **Array base:** the relocator's input **container-buffer base argument**. The per-record descriptor array base is the record's own descriptor-pointer field, which this pass rebases. **Slot count: not an argument** - it is the record's per-slot count field, which equals the model bone count by construction. |
| **Slot-pose builder** (the observation site, default dump site) | Reads the materialised record: the slot-count field and the descriptor pointer, then writes the 13-float per-slot pose. Observed live (627 hits in ~50 s in the earlier run). Distinguished by writing the per-object slot pose. | **Array base:** the record's descriptor-pointer field. **Slot count:** the record's per-slot count field. It reads the array *after* the loader has built and relocated it, so a dump here is exactly "the array after load". |

Why the dump is taken at the observer, not at the relocator: the relocator's
entry address is not pinned by the trace, and its slot count is not an argument
(it processes a whole container, not one animation record), so hooking it
directly would require the record offset inside the container as well. The pose
builder is already pinned, fires steadily, and hands us the relocated array base
and the exact slot count with no new reverse engineering. The relocator stays the
**producer to pin for outcome (c)**, not the default observation point.

### The gap, and how to close it

The trace left the relocator's entry address and the indirect actor-table install
unpinned (`animation-loader-trace.md` question 2 is PARTLY). The exact routine to
locate is the **class-dispatch path that installs the animation record into the
actor**, ending at the **object-container relocator** that rebases the record's
pointer fields. Close it one of two ways:

1. **Static xref:** from the pose builder's descriptor-pointer read, follow the
   record field to the store that writes it; that store is inside the
   relocation/install path. The writer is the role to pin.
2. **Runtime write-watch:** in spawn mode, arm a page access monitor filtered to
   the animation record's descriptor-pointer field and read back the faulting
   instruction address; that address is the producer's code. This is the
   page-level operation the earlier capture plan deferred as most likely to
   perturb timing, so it stays opt-in.

Once the producer is pinned, the probe's optional relocator hook reports the
entry's argument registers so the container-buffer base argument is confirmed on
the shipped build. This is diagnostic only; it does not change the default dump.

## Dump format

For each object of interest, at the first moment the pose builder can resolve it,
the probe emits one record. It never dumps per tick.

| Field | Source (by role) |
|---|---|
| object name | the object's inline, NUL-terminated model-name field |
| slot count | the animation record's per-slot count field (== bone count) |
| descriptor array base | the animation record's descriptor-pointer field, with its region tag (in-module / outside) |
| descriptor bytes | `slot_count x 32` raw bytes, written to one binary file per object |
| descriptor hex | a bounded head/tail hex string in the text log; a full hex sidecar only with `--hex-full` |
| track table base | the animation record's track-table pointer field, with its region tag |
| track entry count | the track table's count field |
| track bytes | the header plus up to a bounded number of 16-byte entries, written to one binary file |
| stable key | `(object name, slot count, descriptor pointer)` for the array; `(object name, track-table pointer)` for the table |

De-duplication rules:

- Each `(object, descriptor base)` is dumped **once**. A later call with the same
  key is silent. A reloaded object with a new pointer is a new key and dumps
  again, which is what we want.
- Each track table is dumped once per `(name, table pointer)`.
- Per-frame consumers (`pose builder` entry, `track selector`) are otherwise
  rate-limited and signature-de-duplicated so the text log records distinct
  events, not frames.
- The raw arrays go to binary files, so the hard line cap can never be exhausted
  by hex. The cap still exists and stops the agent cleanly if reached.

A manifest (`manifest.jsonl`) records one line per dump with the fields above and
the written file name, so the comparison tool and the human share one index.

### Reporting that cannot be mistaken for absence

Every hook reports `unobserved` (never a silent zero) when it has not fired, and
a periodic `STAT-NOTE` names every unobserved hook. This reuses the earlier run's
two failures: the 5000-line cap that came from per-call dumps, and the palette
hook that never fired but was read as `palette=0`.

The **palette assembler** is re-pinned by behaviour, not by name or call site:
it reads the 128 rest translations, reads the 128 activation flags, reads the
per-slot 13-float pose, writes one 12-float 3x4 matrix per active slot into a
global palette, and publishes the written count. A routine missing any of those
five is not this role. The hook reports `palette=unobserved` when it never fires,
`palette-ran-empty` when it fires but publishes zero. The old ambiguous
`palette=0` is gone.

## Comparison method

A standalone scratch tool takes the dumped binary files and `BIGFILE.DAT` and
classifies each array:

1. **Exact contiguous search** of the whole `slot_count x 32` run in the archive.
   A hit is outcome (a); report every archive offset (bounded).
2. **No exact hit:** measure the longest matching prefix and suffix, and the
   fraction of aligned blocks that still occur anywhere in the archive.
   - A large surviving prefix and suffix with a divergent middle is the
     signature of an otherwise identical record whose pointer fields were
     rebased - a relocated load-time product (b). Report the first divergent
     byte as the likely first pointer field.
   - Almost nothing surviving is a decompressed product (b).
3. **Undersized or unreadable dump:** undecided; do not conclude (a) or (b).

The tool also re-checks `bytes == slot_count x 32` for descriptor files so a
truncated dump is never read as a real negative. The report is a scratch artifact
and is never committed.

## Branch table and falsification

| Branch | Observation that establishes it | Observation that refutes it |
|---|---|---|
| **(a) stored** | an exact contiguous match of a dumped array in `BIGFILE.DAT`, reproducible across a rerun | a second dump of the same object that no longer matches, or a match that depends on a byte gate rather than the whole run |
| **(b) load-time product** | no exact run, and prefix/suffix/blocks show absence or a pointer-field divergence | the comparison finding the exact run (moves the case to (a)) |
| **(c) not captured** | the pose builder fires but no descriptor array is dumpable, with a `DUMP-NOTE` reason (animation record or descriptor pointer unresolved) | a dump produced after the site is re-pinned |

A mixed result is possible: some objects could be stored while others are
produced, or a track table could be present while the descriptor array is not.
The comparison reports per array; do not collapse it to one global verdict.

## Human procedure (5 steps)

1. **Get in-world.** Launch the game normally, load a save, and stand a body
   actor (e.g. `raziel`) on solid ground. Do not attach yet.
2. **Attach and wait.** From the session scratch directory run the probe with
   `--attach SRX.exe --dump-dir <scratch>\\dumps`. Wait for the `READY` line and at
   least one `DUMP desc` line. If you only see `WAIT` lines, the engine module is
   not up yet or the name differs - keep the log, it is diagnostic.
3. **Exercise the body path.** Idle 10 s, walk 15 s, attack 10 s, turn 10 s.
   This first-sights several objects without needing a specific animation.
4. **Stop and keep.** Press Ctrl+C. Keep the text log, `manifest.jsonl`, and
   the `.desc.bin` / `.tracks.bin` files. Read the final `STAT` and `STAT-NOTE`
   lines: an `unobserved` hook is a missing observation, not proof of absence.
5. **Compare.** Run the comparison tool with `--dump-dir <scratch>\\dumps
   --bigfile <install>\\1\\BIGFILE.DAT`. Read the per-array verdicts as (a)/(b),
   or (c) if the log shows the pose builder fired but nothing was dumpable.

## What stays outside the repository

The committed slice is this plan and, if added, a one-line note. The probe and
the comparison tool live in the session scratch directory, never in the repo,
because they carry module-relative offsets, engine field offsets and host paths
the repository may not contain. The text log and the raw dumps are runtime data
from a shipped binary and are never repo material. If the scripts are lost, the
role tables above are enough to rebuild them: each row names a stable identity,
not a number.

## Risks and limits

- **Instrumentation can perturb timing.** The probe hooks entry points and reads
  bounded records only; the optional page access monitor is the one operation
  most likely to stall and is off by default. If the game stutters, note it and
  do not conclude anything from timing.
- **Hooks are by module-relative offset, not by symbol.** If the shipped module
  changes, every offset must be recomputed from the role table. A stale offset
  silently hooks the wrong function; the `READY` line prints the module base so a
  mismatch is at least visible.
- **Only the SR1 module is covered.** A clean (c) is a statement about this
  module and the observed process, not about the whole product.
- **The relocator stays unpinned** unless the static xref or the write watch is
  run. This slice states that limitation; it does not hide it.
- **A per-object copy can hide a stored array.** If the loader copies the
  container record into a fresh heap buffer before rebasing, a byte comparison
  tests the copy's bytes, not the container's placement. That is exactly what the
  prefix/suffix and block-survival metrics are there to expose: a mostly
  surviving record with a rebased middle is a copy of a stored record, not an
  invented one. Read the per-array metrics, not just the headline.

## Checklist for review

- [ ] The plan states the one question, the pinned role, and the three outcomes.
- [ ] The materialiser role names its buffer-base argument and states that the
      slot count is a record field, not an argument.
- [ ] The observation site is justified as the already-pinned consumer.
- [ ] The dump format has a stable key and a de-duplication rule.
- [ ] The comparison method distinguishes an exact match from a relocated record.
- [ ] Every branch has an explicit refutation.
- [ ] The palette role is behavioural and `unobserved` is never a silent zero.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear
      in the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this slice.
No game file is written, no game bytes are copied into the repository, and no
Ghidra project is created inside the repo or the install. Roles, structure and
format constants only.

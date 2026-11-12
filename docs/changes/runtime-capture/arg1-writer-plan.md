# arg1-writer-plan - who writes the animated pose pool? (2026-11-12)

This is a **preparation** slice. It states why the writer of the `arg1`
pose-buffer pool is the last open question in the body-animation
investigation, evaluates the available write-watch mechanisms against the real
platform, designs one read-only probe around the mechanism that actually
works, and ships the instrumentation as a local scratch file. Nothing was
attached, launched or instrumented against the game for this document, and no
engine behaviour changed. The committed diff is this plan only.

## Quick path

1. Read "What is already settled" - the verdict that makes the writer the answer.
2. Read "The mechanism decision" - the Windows/Frida verdict, including the
   one platform fact that changes the design.
3. Read "The single-arm design" and "The flooding cap" - the deliverable.
4. Read "What a found writer establishes" and "The escalation".
5. Run the 5-step human procedure when a session is available.
6. Read "What this cannot establish" before writing any conclusion.

## What is already settled

A live `arg1` content probe measured the builder's second argument across one
session. These are **inputs** to this experiment, not things it re-derives.

```text
ARG1_ANIMATES  readable=1609  read_failures=0  hashes_distinct=1609
               consecutive_changes=1608  equal_pairs=0  addr_distinct=20
               region_bytes=3432
VERDICT ARG1_ANIMATES   CONTROL OUT_ANIMATES (capture valid)
```

Read plainly:

- The builder's second argument is **content-changing** between consecutive
  calls. It is not a static array and it is not a pointer that merely rotates
  over fixed data (`same_addr_changes` was zero and every hash differed).
- The builder **reads** a pose that is already animated. It is a consumer of
  motion, not the producer.
- The pool is small and bounded: twenty distinct base addresses over the run,
  each holding a pose-shaped region of about 3.4 KiB.

Because the content changes between calls, **someone writes the pool between
builder calls**. That writer is where the animation is actually produced. The
builder's arithmetic cannot be the source, because its input is already
moving. Naming the writer is therefore the next - and last - step on this
route: it converts "the builder receives motion" into "this instruction
produces the motion".

## The mechanism decision

Four mechanisms were considered. The requirement is narrow: report the
**instruction pointer of a store** into the pool pages, without pausing the
target and without modifying game data.

| Mechanism | Reports a writing IP? | Granularity | Windows x64 / Frida 17.x verdict |
|---|---|---|---|
| `MemoryAccessMonitor.enable()` | Yes - `details.from` is the faulting instruction, `details.operation` is `read` / `write` / `execute` | page (ranges rounded out to pages) | **SUPPORTED.** frida-gum ships a dedicated Windows backend that uses `PAGE_GUARD` and Frida's own in-process vectored exception handler. No debugger, no target suspension. The JS binding arms all-access and is **one-shot per page per arming**. |
| `Process.setExceptionHandler` + `VirtualProtect` to read-only | Yes - the handler receives the faulting context | page | **Possible, but manual and risky.** It is the only way to get write-only pages (reads pass), but a throwaway test showed the handler can be re-entered in a fault loop if the page protection is not restored in the right order. Documented here as a fallback only, not the default. |
| Hardware breakpoints (debug registers) | Yes | four addresses per thread | **Insufficient.** Four watchpoints cannot cover a pool that spans many pages, and the writer may run on an unknown thread. |
| `Stalker.follow()` with write-range events | No | instruction | **Not applicable as described.** Frida Stalker's event model is control flow (`call`, `ret`, `exec`, `block`, `compile`); it has no data-write event. It would add code-block rewriting cost without detecting writes. |

### The platform fact that shapes the design

`MemoryAccessMonitor` is supported, so the mechanism question is settled. But
its JS binding is **all-access** (`read | write | execute`) and **one-shot per
page** (`auto_reset`): each watched page reports its first access and is then
no longer watched. A second `enable()` in the same session was measured **not
to report** on this build - the monitor does not re-arm. That was verified in a
throwaway process, not against the game, and it is the reason this probe
**arms exactly once and never re-arms**.

The naive all-access bias is that the builder reads the pool every call, so a
single arming could catch a **reader**. Two facts make a single arm sufficient
anyway:

1. The builder reads only the **current** buffer per call. The other pool
   buffers are idle between rotations.
2. The animation writer must touch a buffer **before** the builder consumes it.

So the first access to most pool pages after a single arm is expected to be the
**write**, while only the buffer that is actively being read can report a read
first. With roughly twenty buffers that yields many independent writer
reports, which is exactly what cross-region deduplication needs.

A read event is still useful: it names the consumer and it is reported as a
reader. A page whose first access is a read is not re-armed; it is reported
truthfully as `reader-only` (unobserved writer), never as "no writer".

## The single-arm design

| Aspect | Design |
|---|---|
| Discovery site | the slot-pose builder (main and alternate paths) |
| What discovery records | distinct second-argument base addresses for the focus object, with a pose-sized region; nothing is hashed or dumped |
| Discovery bound | a call budget and a time budget, whichever expires first; a cap on stored regions |
| Watch site | every page spanned by the discovered regions, armed in **one** call |
| Event filter | `operation` containing `write` is a **writer**; `read` is a **reader**; anything else is counted only |
| Writer identity | the faulting instruction pointer, projected to a module-relative RVA for the engine module, or `module+offset` for a foreign module, or absolute as a last resort |
| Role resolution | an optional role map names known RVAs; an unmapped writer is reported as `unknown`, never guessed |
| Region writer cap | at most a stated number of distinct writers recorded per region; further writes still count as hits |
| Disarm | each page disarms itself after its first access (`auto_reset`); no region is ever re-armed |
| Finish | when every page has fired, when the watch window elapses, or on `Ctrl+C`; the summary is written in all three cases |
| Text output | `READY` with module base and range, one `POOL` block, first-occurrence `WRITE` lines, periodic `STAT`, one final `SUMMARY` block, deduplicated and line-capped |
| Safety | never writes game memory, never calls game code, never patches bytes; the only mutation is a transient page-protection guard that the in-process handler resolves and the access completes |

### Why discovery is separate from the watch

The pool addresses are runtime data, so they cannot be committed or predicted.
The probe discovers them by observing the builder's second argument during a
short, bounded window. Discovery only records pointer values; it does not read
the pose. Once discovery closes, the builder hook is detached and the watch
begins, so the watch phase adds no interceptor overhead on the hot path.

### Why the watch is page-granular and honest about it

`MemoryAccessMonitor` rounds each range out to whole pages. A region may span
one or two pages, and two regions can in principle share a page. The probe maps
pages back to the regions that contain them, so an event is attributed to every
region that covers the faulting page. It never claims byte-level or slot-level
precision; it claims "an instruction stored into this page while it was
watched".

## The flooding cap

The previous captures were degraded by per-frame work, so the cost floor is a
design constraint. This probe bounds cost in four independent ways:

- **One fault per page per arming.** `auto_reset` means the monitor strips the
  guard after the first access, so a page that is written every frame produces
  exactly one event, not one per frame.
- **One arm, no re-arm.** Because re-arming does not report on this build, the
  probe cannot and does not loop over readers. The worst case is one access
  event per watched page for the whole run.
- **A per-region writer cap.** Once a region has the stated number of distinct
  writers, further writers are counted but not recorded, and the region is
  considered resolved.
- **A line cap and deduplication.** First-occurrence lines only, a periodic
  heartbeat, and a hard cap on emitted text lines. The summary is aggregated
  and always written.

The dominant cost is therefore the discovery hook (pointer reads only) plus at
most one guard fault per page. This is far below the earlier per-frame region
hashes.

## What a found writer establishes

A write event with a faulting instruction pointer establishes:

- **The instruction that stores into the posed pool** in the observed window,
  as a module-relative RVA (the role identity).
- Either the **animation producer itself**, or **one step in its chain** - for
  example a per-slot pose writer, a small transform helper, or a copy loop.
- A concrete, reproducible target for the next slice: read that routine's
  inputs, or (if it is a copy) locate the copy's source.

It does **not** establish:

- That the writer is the **source** of the motion. A store into the pool can be
  a `memcpy`/`memmove` that copies from an upstream buffer. In that case the
  writer is one link in the chain, not the producer.
- What the writer reads or computes. The probe reports where the store happens,
  not the arithmetic or the data flow behind it.
- Per-frame cadence or call counts. The instrumentation perturbs timing; only
  the writer identity is meaningful.
- That one writer serves every region, or that every region has a writer. The
  summary reports the observed writers and explicitly labels the rest.

## The escalation - when the writer is a generic copy

If the reported writer resolves to a **foreign module** (for example a system
C-runtime) or to a routine whose only job is a bulk copy, do not treat it as
the animation source. Record that finding and run the next probe on the copy's
**source**:

1. Report the copy: writer `where`, its `rva_role`, and the regions it serves.
2. Identify the copy's source operand. That is a static question about the
   writer routine's arguments, answered outside the repository; the probe
   already hands over the routine's role identity.
3. Re-run this same write-watch against the **source** region once it is
   known. The pool is no longer the target; the source buffer is.
4. Only when a non-copy writer stores the animated data is the motion producer
   identified.

If the writer is a per-slot pose-writing routine rather than a copy, the
animation producer is identified and the next slice is the **input** side of
that routine - the same relationship the builder-input slice established for
the builder.

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Validate the setup without arming.** From the session scratch directory,
   run `python sr1_arg1_writer_probe.py --dry-run`. This prints the mechanism,
   the platform verdict, the caps and the phases, and attaches to nothing.
3. **Run the probe.** Run
   `python sr1_arg1_writer_probe.py --attach SRX.exe --wait 300
   --log <scratch>\arg1_writer.log
   --summary <scratch>\arg1_writer_summary.txt --writers 1 --max-lines 400`.
   Wait for the `READY` line (module base and range), the `POOL` block, and the
   `MECH` armed line.
4. **Exercise the body path.** Idle about 10 s, then walk continuously for at
   least 20 s, then attack and turn for about 10 s each. The point is a window
   in which the pool is being written and the body is visibly moving. The probe
   finishes on its own when every page has reported or the watch window ends;
   otherwise press `Ctrl+C`. The summary is written before detaching.
5. **Classify, then re-run once.** Read `regions_watched`,
   `regions_with_writer`, `distinct_writers`, and each writer's `where`,
   `rva_role`, `hits` and `regions`. Apply the escalation if the writer is a
   copy. A writer used to confirm or refute a conclusion must reproduce on a
   second run. Any region with no write is `UNOBSERVED`, not "no writer".

The summary has this shape (template, not a result):

```text
WATCH SUMMARY regions_watched=<N> regions_with_writer=<K> distinct_writers=<M>
WATCH REGION index=<i> base=<addr> size=<bytes> pages=<p> state=<...>
             writers=<where,...> readers=<n> hits=<n>
WATCH WRITER where=<rva:... | module+off | abs:...> rva_role=<role|unknown>
             hits=<n> regions=<i,...>
WATCH STAT-NOTE region=<i> unobserved -> ... (missing observation, not absence)
WATCH SUMMARY counters build=<> regions=<> pages=<> events=<> writes=<> ...
WATCH MODULE name=<> base=<> end=<>
```

## What this cannot establish

- **It does not prove the writer is the source.** A store can be a copy. The
  escalation exists precisely because this experiment cannot distinguish a copy
  loop from a producer by the store alone.
- **It does not decode the writer.** The probe reports the instruction pointer
  and its role; it does not report the routine's arguments, reads or math.
- **Page granularity.** An event attributes a store to a page, not to a byte or
  a slot. A writer that touches one slot and one that touches all slots look
  identical at this granularity.
- **First-access bias.** Because the monitor is all-access and one-shot, a page
  whose first access is a read reports a reader and is not retried. Such a page
  is `reader-only`, an unobserved writer, not evidence that no writer exists.
- **One arm only.** Re-arming was measured not to report on this build, so the
  experiment cannot improve a page after its first event. This is a platform
  limitation, stated rather than worked around silently.
- **Single-thread assumption is not guaranteed.** The writer is expected on the
  main animation thread; a pool written concurrently by many threads is a
  different observation and the probe reports the thread only as context, not
  as proof of ordering.
- **One build, one window, one focus actor.** A result, positive or negative,
  is bounded by the observed process and actor.
- **Instrumentation perturbs timing.** A guard fault is handled in-process and
  the access completes, but do not infer frame cadence or performance.
- **External writers are named, not explained.** A store from a foreign module
  is reported as `module+offset`; the caller and the source remain unknown.

## Roles (for re-pinning without the script)

| Role | What it hands the probe |
|---|---|
| Slot-pose builder (main path) | the call site and the second argument during discovery |
| Slot-pose builder (alternate path) | the same for the alternate class |
| Object header chain | the per-object slot count, which sizes the region |
| Pose pool | the set of distinct second-argument base addresses |
| Pool pages | the page set armed in one `MemoryAccessMonitor` call |
| Writer event | the faulting instruction pointer and its operation |
| Role map (optional) | names known RVAs; unknown stays unknown |

No addresses or field offsets are recorded here. The probe carries them and
lives outside the repository, as this directory's `README.md` explains. If the
probe is lost, the roles above are enough to re-pin it.

## Risks and limits

- **Hooks are pinned relative to the engine module.** A shipped-module change
  requires re-deriving each site from the roles. The `READY` line prints the
  module base and range so a mismatch is visible.
- **An unobserved region is not a negative.** Every region without a write is
  printed as `UNOBSERVED` with a `STAT-NOTE`.
- **A reader-only region is not proof of absence.** It means the first watched
  access was a read; the writer may still exist.
- **The arm can fail.** If the monitor refuses the pages (for example one was
  freed between discovery and arming), the probe reports a mechanism error and
  marks the result unobserved rather than guessing.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes. The only state it changes is a transient
  page-protection guard resolved by the in-process handler.

## Checklist for review

- [ ] The established verdict is stated before the writer question.
- [ ] The mechanism table states the Windows/Frida support verdict per option.
- [ ] The one-shot, no-re-arm platform fact is stated with its consequence.
- [ ] The flooding cap is stated as one fault per page plus the writer cap.
- [ ] A found writer states what it establishes and what it does not.
- [ ] The copy-writer escalation names the next target (the copy's source).
- [ ] The procedure has five steps including a dry run and a re-run.
- [ ] `UNOBSERVED` and `reader-only` are kept as missing observations.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear in
      the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, file-position offset, disassembly listing or absolute install
path appears here. Roles, structure, format constants and observed counts only.

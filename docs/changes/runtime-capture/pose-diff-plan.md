# pose-diff-plan - before/after diff across the pose-builder call (2026-11-12)

This is a **preparation** slice. It states the last open question in the body
animation investigation, designs one read-only runtime probe that captures the
slot pose **before** and **after** the same pose-builder call and diffs it per
slot, and ships the instrumentation as a local scratch file. Nothing was
attached, launched or instrumented for this document, and no engine behaviour
changed. The committed diff is this plan only.

## Quick path

1. Read "What is already settled" - the two facts that close every other route.
2. Read "The question, and why it is the last one" - that is the whole reason
   for this experiment.
3. Read "The capture design" and "The decision rule"; that is the deliverable.
4. Read "The three outcomes" for what each result establishes and refutes.
5. Run the 5-step human procedure when a session is available.
6. Read "What this cannot establish" before writing any conclusion.

## What is already settled

Two observations are established and are **not** re-derived by this experiment.

**Fact 1 - the body pose carries real per-bone motion.** A live full-pose
capture saw **66 of 66** slots of `raziel` and **24 of 24** slots of `sluagh`
change while the body moved, with rotation deltas up to **1.84**. The motion is
real and it is in the pose array, not only in the object transform.

**Fact 2 - the track sampler is not the source.** The track selector fired
**8,861** times in the same window, but every call reported `bodyRan=0` and
`noTracks=8,861`: the animation record's track-table pointer was null for
`raziel`, `sluagh`, `soul` and `font`. Only `wrpface`, a prop, carried a track
table. The sampler copies authored frames when a table exists; here it has
nothing to copy, so the body motion cannot come from it.

Read together: the body motion is real, and it is produced somewhere other than
the track sampler. The file families were already closed with explicit counts.
The only remaining fork is **where in the running process the pose array is
written**.

## The question, and why it is the last one

The slot-pose builder is the observation site we already have, and the earlier
capture read the pose **after** it ran. One question remains:

> When the pose builder is called, does it **compute** the animation into the
> pose array, or does it **receive** an array that something earlier already
> animated?

This is the last question because the other two routes are closed. The file
side is exhausted (the carrier families have explicit negative counts), and the
track-sampler route is refuted by Fact 2. Whatever writes the body pose must be
either the builder itself or a writer that runs before it. This experiment
distinguishes exactly those two, and either answer hands the next slice a single
precise target: the builder's own inputs (if it computes), or the writer of a
localised region (if it receives).

## The capture design

Hook the same slot-pose builder, and inside that one hook read the pose array
**on entry** and **on exit**. Nothing is written, called or patched; entry and
leave observation only.

| Aspect | Design |
|---|---|
| Observation site | the slot-pose builder (main path), plus the alternate path for completeness; the track selector is counted only, as a liveness witness |
| What is read | the per-object slot pose: `slot_count x 13` floats (a 3x3 rotation block, a scale marker and a translation block, per slot) |
| Entry read | snapshot the pose array at function entry |
| Exit read | snapshot the pose array at function exit |
| `body_delta` | this call's entry vs this call's exit - did the builder change it? |
| `pre_delta` | **this** call's entry vs the **previous** call's exit, same buffer only - did the pose change while the builder was not running? |
| Lanes | per phase: the set of changed slots, and the maximum rotation, translation and scale delta, reported per slot and per lane |
| Sampling | per call, but aggregated in memory; a hard cap on emitted lines; **never** a per-call full dump |
| Raw data | the first N calls (default 10) store one entry/exit `.bin` pair per object, plus a manifest line, for offline inspection |
| Output | one compact summary block per object and a final `VERDICT` line |

Why this site:

- It is already pinned and fires steadily, so the capture is reliable.
- The only alternatives (another file sweep, another track-sampler pass) are
  settled negatives; repeating them buys nothing.
- A before/after read at the same call is the minimal observation that splits
  "computed here" from "computed earlier".

The per-call work is deliberately cheap and bounded. The previous captures hit
a 5,000-line cap because they wrote per-call records; this probe writes one
aggregated block per object, a bounded number of first-change event lines, and
a bounded number of `.bin` pairs. The raw arrays exist for offline examination
without ever flooding the text log.

### Why `pre_delta` compares consecutive calls

`pre_delta` is defined against the **previous call's exit**, not the previous
sample of an arbitrary stride. If a writer runs between two consecutive builder
calls, the entry of call N already differs from the exit of call N-1. Sampling
every call (the default) is therefore required: a stride would blend skipped
builder runs into the "pre" window and make the answer ambiguous. The probe
runs at the full builder rate and aggregates.

Buffer identity matters. A pose pointer can be rebound between calls. When the
entry pointer is not the same buffer as the previous exit pointer, the probe
does **not** diff them; it counts the rebind as a separate writer signal and
says so. A rebind between calls is itself evidence that something outside the
builder touched the pose.

## The decision rule

The rule is stated before the run so the result cannot be rationalised after
it. `body_changed_slots` and `pre_changed_slots` are the number of slots that
ever changed in each phase, over the observed window. "Approximately zero" is a
stated threshold (default: at most 5% of slots), not a silent rounding.

| Condition | Verdict |
|---|---|
| `body_changed_slots > 0` and `pre_changed_slots` approximately zero | **COMPUTES** |
| `pre_changed_slots > 0` and `body_changed_slots` approximately zero | **RECEIVES** |
| both are substantially non-zero | **BOTH** |
| neither phase changes, or the hook never fires | **UNOBSERVED** |

The thresholds and the exact counts are printed with every verdict, so a
reviewer can re-apply the rule by hand. A verdict used to refute or confirm the
conclusion must reproduce on a second run.

## The three outcomes

### Outcome A - COMPUTES (the change happens inside the builder)

The entry snapshot is static against the previous exit, and the exit snapshot
differs from the entry.

**Establishes:**

- The builder is the **producer**: it writes the per-slot animation into the
  pose array during its own call.
- The source is now local: the builder's **input pointer and arithmetic** are
  what must be read next. There is no upstream writer to hunt.
- It is consistent with a procedural pose (orientation computed from
  descriptor mode and aim data), which the loader trace already suggested.

**Refutes:**

- "Something earlier writes the pose array." In the observed window and for the
  observed objects, the pose changes only while the builder runs.
- The need for a separate write-watch slice on the pose array.

**Does not establish:** what the builder reads, or whether the input is
file-resident or derived. That is the next slice, aimed at the builder's
arguments.

### Outcome B - RECEIVES (the pose is already animated before the builder)

This call's entry already differs from the previous call's exit, in the same
buffer, before the builder runs.

**Establishes:**

- An **earlier writer** animates the pose array; the builder consumes or
  finalises an already-animated pose.
- The probe's per-slot report localises the pre-existing change: **which slots**
  change between calls and **which lanes** (rotation vs translation vs scale)
  carry the delta. That localisation is the target for the writer hunt.
- It reopens the "procedural orientation" account: whatever writes earlier is
  the real source and must be identified.

**Refutes:**

- "The builder computes the body animation." It would be a consumer, not the
  producer.
- Any plan that stops at the builder's arguments; the arguments are not where
  the motion is produced.

**Does not establish:** who the earlier writer is, or what data it reads. The
next slice is a write-watch on the localised slots/region.

### Outcome C - UNOBSERVED (neither phase changes, or the hook never fires)

No change in either phase, or the builder hook did not fire at all.

**Establishes:** nothing about the engine. This is a **missing observation**.

**Refutes:** nothing. Do **not** read it as "the builder neither computes nor
receives". Re-pin before classifying: confirm the module base and range, the
slot count and pose pointer actually resolve, and that the character was moving
for a usable window.

### The middle result - BOTH

Both phases change substantially. This is a real outcome, not a tie to be
collapsed. It means writes happen both inside the builder and between calls
(for example, an in-place update inside the call plus a foreign write or rebind
between calls). Report both localisations; the between-call one is the writer
target, and the in-call one is the builder's own contribution.

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Start the probe from the session scratch directory.** Run
   `python sr1_pose_diff_probe.py --attach SRX.exe --wait 300
   --log <scratch>\pose_diff.log --dump-dir <scratch>\pose_diff
   --dump-pairs 10`. Wait for the `READY` line (it prints the module base and
   range) and at least one `POSEDIFF STAT` heartbeat.
3. **Exercise the body path.** Idle about 10 s, then walk continuously for at
   least 20 s, then attack and turn for about 10 s each. The point is a window
   in which the body is visibly moving.
4. **Stop and keep.** Press `Ctrl+C`. The probe writes `pose_diff_summary.txt`
   before detaching. Read the per-object block: `body_changed_slots`,
   `pre_changed_slots`, the per-lane peak deltas, and the `VERDICT` line. Read
   `STAT-NOTE`: any hook listed as `unobserved` never fired, which is a missing
   observation, not evidence of absence.
5. **Classify, then re-run once.** Apply the decision rule. A verdict used to
   refute or confirm the conclusion must reproduce on a second run. If the
   verdict is `UNOBSERVED`, re-pin the site and the pointer before drawing any
   conclusion.

The summary has this shape (template, not a result):

```text
POSEDIFF SUMMARY object="raziel" slots=66 calls=<N> samples=<N> pairs=<N>
POSEDIFF SUMMARY   body_changed_slots=<K1> max_rot=<R1> max_trans=<T1> ...
POSEDIFF SUMMARY   pre_changed_slots=<K2>  max_rot=<R2> max_trans=<T2> ...
POSEDIFF SUMMARY   body_slot=<n> rot=<..> trans=<..> scale=<..> lanes=<RT>
POSEDIFF SUMMARY   pre_slot=<n>  rot=<..> trans=<..> scale=<..> lanes=<RT>
POSEDIFF VERDICT object="raziel" <COMPUTES|RECEIVES|BOTH|UNOBSERVED> reason=<..>
POSEDIFF SUMMARY tally objects=<N> computes=<a> receives=<b> both=<c> unobserved=<d>
```

## What this cannot establish

- **It does not decode the builder's input.** Outcome A locates the producer;
  it does not reveal whether the input is file-resident, procedural or derived.
- **It does not identify the earlier writer.** Outcome B localises where and in
  which lanes the pre-existing change lives; naming the writer needs a separate
  write-watch.
- **A `pre_delta` zero is bounded by the same-buffer condition.** If the pose
  pointer is rebound between every pair of calls, `pre_delta` is skipped for
  those pairs and the summary prints `skipped_ptr`. A skipped comparison is not
  proof that nothing changed between calls.
- **Deltas are sampled at builder calls, not per frame.** The maximum delta is
  measured between the observed entry and exit (or exit and next entry), not the
  true per-frame peak. The experiment answers "does this phase change", not
  "what is the exact curve".
- **One build, one window, one actor.** A negative is bounded by the observed
  process, not the product. Another class or another actor may behave
  differently.
- **Instrumentation can perturb timing.** Do not infer frame cadence or
  performance from the probe; only the change classification is meaningful.
- **A mid-call rebind can smear `body_delta`.** The probe compares values even
  when the buffer was rebound inside the call, and counts the rebind. A large
  `body_delta` on a rebind call is consistent with a computed pose but should be
  confirmed against the dumped pairs.

## Roles

| Role | What it hands the probe |
|---|---|
| Slot-pose builder (main path) | the object's slot pose and slot count at entry and exit; the observation site |
| Slot-pose builder (alternate path) | the same pose for the alternate class; hooked for completeness |
| Track selector | a liveness count only, so a dead run is visible as `unobserved` |
| Pose pointer (per object) | the buffer identity; a rebind between calls is counted as a writer signal |

No addresses or field offsets are recorded here. The probe carries them and
lives outside the repository, as `README.md` in this directory explains. If the
probe is lost, the roles above are enough to re-pin it: each row names a stable
behaviour, not a number.

## Risks and limits

- **Hooks are pinned relative to the engine module.** If the shipped module
  changes, each site must be re-derived from the roles. A stale site silently
  hooks the wrong function; the `READY` line prints the module base and range so
  a mismatch is at least visible.
- **A zero must be labelled `unobserved`.** The probe prints `STAT-NOTE` for
  every hook that never fired. Do not read an unobserved hook as an absent role.
- **A short or stationary window is not a result.** Below a usable sample count,
  or without visible movement, classify as `unobserved`.
- **Per-object scope.** Each object is tracked under its name and slot count; a
  reloaded object with a new pose buffer resets its baseline, and the summary
  reports how often that happened.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes.

## Checklist for review

- [ ] The two established facts are stated before the question.
- [ ] The question is stated as compute-versus-receive at the same site.
- [ ] The capture reads entry and exit in the same hook and defines
      `pre_delta` against the previous exit.
- [ ] The design aggregates, caps and never dumps per call.
- [ ] The decision rule is stated before the outcomes and is falsifiable.
- [ ] Outcome A and Outcome B each state what they establish and what they
      refute.
- [ ] The middle result (BOTH) is kept as a real outcome, not collapsed.
- [ ] The procedure has five steps and an explicit re-run.
- [ ] The limits say a skipped comparison and a rebind are not proof of absence.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear in
      the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, file-position offset, disassembly listing or absolute install
path appears here. Roles, structure, format constants and observed counts only.

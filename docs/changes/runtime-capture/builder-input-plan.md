# builder-input-plan - what the slot-pose builder reads (2026-11-12)

This is a **preparation** slice. It states the last open question in the body
animation investigation, designs one read-only runtime probe that enumerates the
slot-pose builder's **inputs** instead of assuming them, and ships the
instrumentation as a local scratch file. Nothing was attached, launched or
instrumented for this document, and no engine behaviour changed. The committed
diff is this plan only.

## Quick path

1. Read "What is already settled", including the **verdict bug** the previous
   probe shipped.
2. Read "The question" - it is the whole reason for this experiment.
3. Read "Why the arguments are enumerated, not assumed" and "The capture
   design"; that is the deliverable.
4. Read "The three verdicts" and "The decision rule" for what each result
   establishes and refutes.
5. Run the 5-step human procedure when a session is available.
6. Read "What this cannot establish" before writing any conclusion.

## What is already settled

These observations are established and are **not** re-derived here.

**Fact 1 - the body pose carries real per-bone motion.** A live full-pose
capture saw **66 of 66** slots of `raziel` and **24 of 24** slots of `sluagh`
change while the body moved.

**Fact 2 - the track sampler is not the source.** The track selector fired
**8,861** times in the same window, but every call reported `bodyRan=0` and
`noTracks=8,861`: the animation record's track-table pointer was null for the
body models. Only a prop carried a track table. The sampler copies authored
frames when a table exists; for the body it has nothing to copy.

**Fact 3 - the builder writes every slot on every call.** The slot-pose builder
was observed writing all **66** slots on **3,827 of 3,827** calls.

**Fact 4 - the builder's output buffer rebinds almost every call.** The output
pointer changed between calls on **3,826 of 3,827** comparisons. That is why a
before/after comparison of the output could not produce a usable `pre` phase:
the comparison was skipped, not negative.

**Fact 5 - the authority note describes a delta-driven, procedural pose.** The
pose is rebuilt when the object transform or pose dirtiness changes, and the
builder writes a per-slot pose record (a 3x3 rotation, a scale marker and a
translation). The note does **not** say whether the builder reads an input pose
array; that is exactly the gap.

### The verdict bug from the previous probe

The previous probe reported:

```text
POSEDIFF VERDICT object="raziel" COMPUTES reason=pre-unobserved
```

That verdict violates the probe's own stated rule: a phase that was never
measured must be reported as **UNOBSERVED**, never folded into a positive
verdict. For `raziel` the `pre` phase had **zero comparisons** because the
output buffer rebound between calls. The `body` phase was real, but it cannot
distinguish "computed here" from "received from an upstream writer". Reporting
`COMPUTES` from a body-only measurement is precisely the error this slice must
not repeat.

The corrected rule is mechanical: **a positive verdict is emitted only when its
discriminating phase has at least one comparison**. If the only available phase
is the builder's own write, the result is `UNOBSERVED`.

## The question

The builder produces the animated pose. One question remains:

> What does the builder **read** to produce the pose?

There are two materially different answers, and they lead to different next
slices:

- It reads a **pose-shaped array** that something earlier animated. That array
  is the source, and the next slice is a write-watch on it.
- It reads **no pose array** and computes the pose from the descriptor and the
  object transform. The arithmetic is in the builder, and the next slice is a
  static reconstruction of that arithmetic from the dumped inputs.

The earlier probes assumed the first argument was the object. That is fine for
reading the object, but it is not evidence about the pose source. The builder's
input could be any argument, a field, or no pose array at all. This experiment
enumerates and scores instead of assuming.

## Why the arguments are enumerated, not assumed

A probe that only inspects argument zero can only confirm the assumption it was
built on. To answer "what does it read", the probe must look at every integer
argument slot it can enumerate - the register slots and the first stack slots -
classify each value, and test each readable pointer as a candidate pose array.
No argument index is treated as the input by construction.

The method has four parts:

1. **Enumerate.** At builder entry, read every integer argument slot: the
   register argument slots first, then the first stack argument slots. Record
   the value and how it classifies.
2. **Classify.** Each value is one of: null, inside the engine module range
   printed at `READY`, readable mapped memory, or unreadable/unmapped. Only the
   readable values become candidates.
3. **Score.** For each readable candidate, probe a bounded set of offsets and
   test whether the region looks like a per-slot pose array: `slot_count` pose
   records, each with a near-orthonormal 3x3 block and a finite, plausible
   translation. The score is the fraction of slots that pass. Candidates above
   a stated floor are reported with their score.
4. **Stabilise and diff.** The best-scoring candidate is tracked across calls.
   Its pointer is reported as `stable_pointers=N of M`. If it is stable, the
   probe dumps two consecutive samples and diffs them per slot: entry against
   the previous call's exit (did something else change it?) and entry against
   its own exit (did the builder change it?).

The object's current output pose buffer is a special case. It is pose-shaped by
construction and it rebinds almost every call. The probe labels any candidate
that aliases the current output buffer and excludes it from the input verdict,
so the output cannot masquerade as the source.

## The capture design

| Aspect | Design |
|---|---|
| Observation site | the slot-pose builder (main path), plus the alternate path for completeness; the track selector is counted only, as a liveness witness |
| Argument slots | the enumerable integer argument slots: the register slots and the first stack slots; no index is assumed |
| Value classes | null, inside the engine module range, readable mapped memory, unreadable/unmapped |
| Candidate test | a bounded offset scan inside each readable pointer; `slot_count` pose records with a near-orthonormal 3x3 block and a plausible translation |
| Score | fraction of slots that pass; printed per candidate above the floor |
| Output aliases | any candidate inside the object's current output pose buffer is labelled and excluded from the input verdict |
| Best candidate | highest score, preferring a non-output candidate; reported with `stable_pointers=N of M` |
| `pre` diff | this call's entry sample vs the previous call's exit sample, same pointer only |
| `body` diff | this call's entry vs its own exit, same pointer only |
| Fallback | when no candidate qualifies: a bounded object-header dump plus the known position triples and the descriptor array, for static reconstruction |
| Sampling | per call, but aggregated in memory; a hard line cap; **never** a per-call full dump |
| Raw data | a bounded number of stored regions (`--dumps`), each with a manifest line |
| Output | one compact block per object and a final `VERDICT` line |

The probe is deliberately bounded. Candidate scoring runs on the first calls
per object (the discovery window) and has a global read budget; after that only
the chosen candidate is tracked. The text log carries aggregate lines and a
bounded number of first-change events. The raw regions exist for offline
inspection without flooding the log.

### Why the score is a shape test, not a guess

A per-slot pose record has a rigid-rotation shape: the 3x3 block is
near-orthonormal, and the translation is finite and plausible. A random heap
window almost never satisfies that for every slot of a body-sized pose. The
score is therefore a cheap, falsifiable discriminator. It is reported as a
number, not a boolean, so a near miss is visible and can be judged by hand.

### Why the fallback dumps the object header and the descriptor array

If no candidate scores, the honest conclusion is "no pose-shaped input was
found". To reconstruct the arithmetic statically, the probe stores:

- a bounded window of the object header, which contains the position triples
  and the rotation/facing material the builder would consume;
- the two known position triples, printed as witnesses;
- a bounded dump of the established per-slot descriptor array.

The rotation/facing field is not pinned by this slice. The bounded header dump
is the material from which it can be located, and the plan says so rather than
inventing an offset.

## The three verdicts

The rule is stated before the run so the result cannot be rationalised after
it.

| Condition | Verdict |
|---|---|
| a stable, pose-shaped candidate that is not the output buffer and whose `pre` phase changed with at least one comparison | **READS_POSE** |
| no pose-shaped candidate qualifies as an input (none scored, or the only one is static or aliases the output), and transform inputs were captured | **COMPUTES** |
| builder never fired, no readable argument, no candidate scan, no transform captured, or the candidate's `pre` phase had zero comparisons / unstable pointer | **UNOBSERVED** |

A scored pose-shaped candidate that is **static** across calls does not satisfy
`READS_POSE`; it falls to `COMPUTES` with a reason that names the static
candidate. A pose-shaped candidate that is **unstable** cannot be compared; it
falls to `UNOBSERVED`, not to a positive verdict.

### Outcome A - READS_POSE (a stable pose-shaped input changes before the write)

A candidate scores as a pose array, its pointer is stable across calls, and its
entry sample differs from the previous call's exit sample with at least one
comparison.

**Establishes:**

- A pose-shaped array exists as an input in the observed window, and it is
  animated between builder calls.
- The builder is a **consumer** of that array, or at least shares it with an
  upstream writer.
- The next slice is a write-watch on that region: name the writer, then read
  its inputs.

**Refutes:**

- "The builder computes the body pose from non-pose inputs alone." In the
  observed window, a changing pose-shaped input is present.
- Any plan that stops at the builder's arguments; the source is upstream.

**Does not establish:** that the builder actually dereferences the region (a
stable pose-shaped region that changes between calls is consistent with a read,
but a read-watch is needed to prove consumption), or who the writer is.

### Outcome B - COMPUTES (no pose-shaped input found; transform inputs present)

No candidate qualifies as a pose-shaped input, the scan examined readable
candidates, and the object transform was captured.

**Establishes:**

- The builder's inputs are non-pose in the observed window. The motion is
  produced by the builder from the descriptor and the object transform, not
  copied from an input pose array.
- The next slice is static: reconstruct the arithmetic from the dumped
  descriptor array, the object header and the transform witnesses.

**Refutes:**

- "The builder receives an already-animated pose array through its arguments."
- The need for a write-watch on an input pose array.

**Does not establish:** which descriptor fields or transform fields are used,
the exact arithmetic, or whether an object-field pose read exists outside the
argument set. That is the next slice.

### Outcome C - UNOBSERVED (a missing observation)

The builder never fired, no readable argument was scanned, the transform was
not captured, or the chosen candidate's `pre` phase had zero comparisons or an
unstable pointer.

**Establishes:** nothing about the engine. This is a **missing observation**.

**Refutes:** nothing. Do **not** read it as "the builder neither reads nor
computes". Re-pin before classifying: confirm the module base and range, the
slot count, the pose pointer, and that the character was moving for a usable
window.

### The middle result - a pose-shaped candidate that is written too

A candidate can be pose-shaped, stable, change before the write, and also be
written by the builder. That still satisfies `READS_POSE` operationally, but
the reason string carries `also-written`. Report both facts: the pre-existing
change is the upstream signal, and the in-call change is the builder's own
contribution.

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Start the probe from the session scratch directory.** Run
   `python sr1_builder_input_probe.py --attach SRX.exe --wait 300
   --log <scratch>\builder_input.log --dump-dir <scratch>\builder_input
   --dumps 12`. Wait for the `READY` line (it prints the module base and range)
   and at least one `INPUT STAT` heartbeat. Optionally run `--selftest` first:
   it exercises the exact verdict function without a game.
3. **Exercise the body path.** Idle about 10 s, then walk continuously for at
   least 20 s, then attack and turn for about 10 s each. The point is a window
   in which the body is visibly moving.
4. **Stop and keep.** Press `Ctrl+C`. The probe writes
   `builder_input_summary.txt` before detaching. Read the per-object block:
   the argument table, the candidate scores, the best candidate's
   `stable_pointers=N of M`, the `pre` and `body` counts, the transform dump,
   and the `VERDICT` line. Read `STAT-NOTE`: any hook listed as `unobserved`
   never fired, which is a missing observation, not evidence of absence.
5. **Classify, then re-run once.** Apply the decision rule. A verdict used to
   refute or confirm the conclusion must reproduce on a second run. If the
   verdict is `UNOBSERVED`, re-pin the site and the candidate before drawing
   any conclusion.

The summary has this shape (template, not a result):

```text
INPUT SUMMARY object="raziel" slots=66 calls=<N> discovery=<yes|no> scan_reads=<n> readable_args=<n>
INPUT SUMMARY   arg=<i> cls={<class>:<count>} [roles={<role>:<count>}] distinct=<k> stable=<n>/<m>
INPUT SUMMARY   cand arg=<i>(<slot>) off=<o> score=<s> stable=<f> output_hits=<k> role=<r>
INPUT SUMMARY   best arg=<i>(<slot>) off=<o> score=<s> stable_pointers=<n> of <m> (frac=<f>)
INPUT SUMMARY   best pre_compared=<n> pre_changed_slots=<k> pre_changed_calls=<n> pre_skipped=<n>
INPUT SUMMARY   best body_compared=<n> body_changed_slots=<k> body_changed_calls=<n>
INPUT SUMMARY   transform samples=<n> changed=<yes|no> dumped=<yes|no> first=<..> last=<..>
INPUT VERDICT object="raziel" <READS_POSE|COMPUTES|UNOBSERVED> reason=<..>
INPUT SUMMARY tally objects=<N> reads_pose=<a> computes=<b> unobserved=<c>
```

## What this cannot establish

- **A stable pose-shaped region is not proof of a read.** It is consistent with
  a consumed input, but only a read-watch proves the builder dereferences it.
- **It does not identify the writer.** `READS_POSE` localises the input region;
  naming the upstream writer needs a separate write-watch.
- **The argument set is not the whole object graph.** A pose array read only
  through an object field, never passed as an argument, is outside the argument
  verdict. The descriptor and transform dumps cover the non-pose side; a
  field-only pose read would need a follow-up.
- **A negative score is a negative for this layout only.** A quaternion input,
  a per-slot curve, or a differently ordered pose record would not score as the
  established pose layout. `COMPUTES` means "no pose-shaped array of the
  established layout was found", not "the builder reads nothing".
- **The transform fields are not decoded.** The bounded header dump and the
  position triples are raw material. The rotation/facing field is not pinned.
- **Discovery is bounded.** Candidates are scored on the first calls per object
  and under a global read budget. A candidate that appears later, or after the
  budget is exhausted, is unobserved, not absent.
- **A zero is bounded by the same-pointer condition.** If the chosen candidate
  rebinds between every pair of calls, its `pre` phase has no comparisons and
  the verdict is `UNOBSERVED`. A skipped comparison is not proof that nothing
  changed.
- **One build, one window, one actor.** A negative is bounded by the observed
  process, not the product. Another class or another actor may behave
  differently.
- **Instrumentation can perturb timing.** Do not infer frame cadence or
  performance from the probe; only the change classification is meaningful.

## Roles

| Role | What it hands the probe |
|---|---|
| Slot-pose builder (main path) | the argument slots at entry, the object identity and slot count; the observation site |
| Slot-pose builder (alternate path) | the same inputs for the alternate class; hooked for completeness |
| Track selector | a liveness count only, so a dead run is visible as `unobserved` |
| Object pose field | the current output buffer; a candidate that aliases it is labelled and excluded from the input verdict |
| Animation record | the slot count and the descriptor array used for the fallback dump |
| Object header | the position triples and the rotation/facing material for static reconstruction |

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
  every hook that never fired, and the verdict function refuses a positive
  verdict whose supporting phase has zero comparisons. The `--selftest` mode
  includes the previous probe's exact bug as a regression case.
- **A short or stationary window is not a result.** Below a usable sample count,
  or without visible movement, classify as `unobserved`.
- **Output aliasing must be excluded.** The output pose buffer is pose-shaped
  and rebinds almost every call; treating it as an input would invert the
  conclusion.
- **The scan budget is real.** When it is exhausted, later candidates are
  unobserved. The summary prints the budget state so this is visible.
- **Per-object scope.** Each object is tracked under its name and slot count; a
  reloaded object with a new pose buffer resets its baseline.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes.

## Checklist for review

- [ ] The established facts are stated before the question.
- [ ] The previous probe's verdict bug and the rule it violated are stated
      explicitly.
- [ ] The question is stated as read-pose versus compute-from-non-pose.
- [ ] The method enumerates every argument slot and does not assume an index.
- [ ] The candidate test is a stated, falsifiable pose-shape score.
- [ ] The output buffer is excluded from the input verdict.
- [ ] The three verdicts each state what they establish and what they refute.
- [ ] The decision rule forbids a positive verdict with zero comparisons.
- [ ] The procedure has five steps and an explicit re-run.
- [ ] The limits say a stable pose-shaped region is not proof of a read, and a
      skipped comparison is not proof of absence.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear in
      the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, file-position offset, disassembly listing or absolute install
path appears here. Roles, structure, format shapes and observed counts only.

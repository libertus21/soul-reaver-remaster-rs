# pose-motion-plan - sample the full 66-slot pose while the character walks (2026-11-11)

This is a **preparation** slice. It plans one runtime experiment that can settle
whether the body pose buffer carries per-bone motion, and ships the
instrumentation as a local scratch file. Nothing was attached, launched or
instrumented for this document, and no engine behaviour changed. The committed
diff is this plan only.

## Quick path

1. Read "The contradiction" - it is the whole reason for the experiment.
2. Read "The experiment" and the decision rule; that is the deliverable.
3. Read "The two outcomes" for what each result would establish and refute.
4. Run the 5-step human procedure when a session is available.
5. Read "What this cannot establish" before writing any conclusion.

## The contradiction

Three observations do not fit together:

| Observation | Number |
|---|---|
| The runtime track sampler fired | **9,454** times in about 50 s (pose builder: **627** hits) |
| The authored track data in the files for `raziel` | **4 frames total** across 3 slots (frame counts 2 / 1 / 1) |
| The earlier capture's pose readout | only **slots 0 and 1** of `raziel`'s **66**, and **both changed** |

Read together: a per-frame sampler runs at full rate, the authored data is four
frames, and the two slots that were ever printed both moved. That leaves exactly
one thing unobserved: **whether slots 2..65 move at all.**

- If many slots move, the body pose carries genuine per-bone motion at runtime,
  and its source is unlocated. That would refute the current
  "orientation-only" account of the body pose and reopen the data question with a
  precise target.
- If only the root (and at most the three authored aim slots) moves, then the
  body pose is orientation-only in this build, and the file verdict stands.

Every statement on this question so far is inference. This experiment is the
deciding observation.

## The experiment

Hook the already-pinned **slot-pose builder** - the same observation site the
descriptor-dump probe uses - and sample the object's **full pose array**, not the
first two records.

| Aspect | Design |
|---|---|
| What is read | the per-object slot pose: `slot_count x 13` floats (a 3x3 rotation, a scale marker and a translation, per the existing pose notes) |
| Slot count | the animation record's per-slot count field, which equals the model bone count (66 for `raziel`) |
| Sampling | bounded: every Nth invocation plus the first sight, never a full snapshot on every call |
| Comparison | each sample is diffed against the previous sample of the same object, per slot |
| Per-slot record | rotation changes and maximum rotation delta (3x3 block); translation changes and maximum translation delta; a whole-record change count |
| Output | one compact line per slot that ever changed, plus a final tally; the raw sampled arrays go to a bounded number of binary snapshots and a manifest |
| Target | `raziel` specifically; `monster_` / `sluagh` objects are tracked too if they appear |

The N-th-call bound is deliberate. The previous capture exhausted a 5,000-line
cap because it wrote per-call records. This probe writes one line per *slot that
ever changed*, plus a bounded number of raw snapshots, so the text log stays
small while the raw data remains available for offline analysis.

Why this site and not the file or the static trace:

- The file families are closed with explicit counts (`.SRA`, `.SRM`, `.PCM`,
  `.SRL`; see `animation-conclusion.md`). Another blind sweep would repeat
  settled negatives.
- The static trace establishes the *roles* and that no per-slot rotation matrix
  is authored in the descriptor. It cannot observe what the running pose buffer
  actually contains.
- The running process is the only place where "does this slot move?" is directly
  observable.

## The decision rule

The rule is stated before the run so the result cannot be rationalised after it.

| Changed slots | Classification |
|---|---|
| Any slot **outside** `{0, 1, 31, 41}` | **REFUTES orientation-only** for the body pose buffer: per-bone motion exists and its source is unlocated |
| Only slots **within** `{0, 1, 31, 41}` | **SUPPORTS orientation-only**: the only movement is the root and the three authored aim slots |
| **No** slot changes while the character visibly walks | orientation-only is supported even more strongly, and the visible motion is not in the pose buffer at all |
| No samples during walking, or a single sample | **unobserved** - not a result; fix the session before classifying |

`0` is the root (`bone-hierarchy.md`), and `1`, `31`, `41` are the three slots
that carry authored aim frames (`aim-stream-bounds.md`). Those identities are
named here so the rule is falsifiable: a changing slot outside that set is a
clear refutation signal.

A result is only counted when the character was actually walking for the sampled
window and the probe produced a reasonable number of samples. The summary prints
the sample count for exactly this reason.

## The two outcomes

### Outcome A - motion outside the root/aim set

A changed slot outside `{0, 1, 31, 41}`, reproducible across two runs.

What it establishes:

- The body pose buffer contains per-bone motion at runtime. The visible body
  deformation is not only the object transform plus aim corrections.
- The current conclusion, read as "the body pose buffer has no per-slot motion
  beyond aim", is **refuted for this build**.
- The source is **unlocated**. Three candidates remain open, and the experiment
  deliberately does not pick one: procedural aim/face composition on more slots
  than the file form suggests; an unlocated authored carrier; or a writer path
  the file scans never touched.
- The next search has a precise target: the writer and the inputs of the moving
  slots, and the mode/aim math. It is no longer a blind file-format sweep.

### Outcome B - motion confined to `{0, 1, 31, 41}`

Every changed slot is the root or one of the three authored aim slots.

What it establishes:

- The body pose buffer is **orientation-only in this build**: limb motion is the
  object/root transform plus the authored aim corrections on the active slots.
- The file verdict in `animation-conclusion.md` stands for the observed body
  pose.
- It does **not** locate a hidden carrier and does **not** prove one cannot exist
  elsewhere (see "What this cannot establish").

### A middle result

Root plus the three aim slots plus a few more is still classified by the rule
above, by the exact slot set - not by a count threshold. The summary prints the
slot numbers so the set is inspectable. Do not collapse a middle result into
"mostly static": one reproducible out-of-set slot is Outcome A.

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Start the probe from the session scratch directory.** Run
   `python sr1_pose_motion_probe.py --attach SRX.exe --wait 300
   --log <scratch>\pose_motion.log --dump-dir <scratch>\pose_motion
   --snapshots 30`. Wait for the `READY` line (it prints the module base) and at
   least one `POSE STAT` heartbeat. The probe samples the full pose array at a
   bounded rate.
3. **Walk the character.** Idle about 10 s, then walk continuously for at least
   20 s, then turn and attack for about 10 s each. The point is a window in
   which the body is visibly moving.
4. **Stop and keep.** Press `Ctrl+C`. The probe writes `pose_summary.txt` and
   appends to `manifest.jsonl` before detaching. Read the `SUMMARY` block: one
   `slot=` line per slot that ever changed, and a tally
   `slots_changed=K/66 slots_static=(66-K)/66`.
5. **Classify, then re-run once.** Apply the decision rule. A result used to
   refute or confirm the conclusion must reproduce on a second run. Also read
   `STAT-NOTE`: any hook listed as `unobserved` never fired, which is a missing
   observation, not evidence of absence.

The summary has this shape (template, not a result):

```text
POSE SUMMARY object="raziel" slots=66 samples=<N> calls=<N> changed=<K> static=<66-K>
POSE SUMMARY   slot=<n> rot_changes=<X> max_rot_delta=<Y> trans_changes=<Z> max_trans_delta=<W>
POSE SUMMARY tally slots_changed=<K>/66 slots_static=<66-K>/66
```

## What this cannot establish

- **Only the root moving does not prove no data exists elsewhere.** It proves the
  body pose buffer is orientation-only in this build, on this actor, in this
  window. Authored data could still exist for another class or another actor.
- **The probe reads the pose buffer, not the object transform.** If the buffer is
  static while the character moves, the motion lives outside the sampled
  structure; the probe cannot attribute it.
- **Deltas are sampled, so they are a lower bound.** The maximum delta is measured
  between consecutive retained samples, not the true per-frame peak. The
  experiment answers "does this slot ever change", not "what is its exact curve".
- **It cannot say why a slot moved.** A changed slot proves motion; it does not
  identify an authored clip, a procedural formula or a specific input.
- **One build, one window, one actor.** A negative is bounded by the observed
  process, not the product.
- **Instrumentation can perturb timing.** Do not infer frame cadence or
  performance from the probe; only the change classification is meaningful.

## Roles

| Role | What it hands the probe |
|---|---|
| Slot-pose builder (main path) | the object's slot pose and the slot count; the observation site |
| Slot-pose builder (alternate path) | the same pose for the alternate class; hooked for completeness |
| Track selector | an optional context count, so a dead run is visible as `unobserved` |

No addresses or field offsets are recorded here. The probe carries them and lives
outside the repository, as `README.md` in this directory explains. If the probe
is lost, the roles above are enough to re-pin it: each row names a stable
behaviour, not a number.

## Risks and limits

- **Hooks are pinned relative to the engine module.** If the shipped module
  changes, each site must be re-derived from the roles. A stale site silently
  hooks the wrong function; the `READY` line prints the module base so a mismatch
  is at least visible.
- **A zero must be labelled `unobserved`.** The probe prints `STAT-NOTE` for every
  hook that never fired. Do not read an unobserved hook as an absent role.
- **A short or stationary window is not a result.** Below a usable sample count,
  or without visible walking, classify as `unobserved`.
- **Per-object scope.** Each object is tracked under its name and slot count; a
  reloaded object with a new pose buffer resets its baseline, and the summary
  reports how often that happened.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes.

## Checklist for review

- [ ] The contradiction is stated before any method.
- [ ] The experiment names the observation site by role.
- [ ] The decision rule is stated before the outcomes and is falsifiable.
- [ ] Both outcomes state what they establish and what they refute.
- [ ] The middle result is classified by the exact slot set, not a count.
- [ ] The procedure has five steps and an explicit re-run.
- [ ] The limits say a negative does not prove data is absent elsewhere.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear in
      the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this slice.
No game file is written, no game bytes are copied into the repository, and no
address, file-position offset, disassembly listing or absolute install path
appears here. Roles, structure, format constants and corpus counts only.

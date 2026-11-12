# arg1-hash-plan - does the builder receive an animated pose? (2026-11-12)

This is a **preparation** slice. It states the last open question in the body
animation investigation, designs one minimal-overhead read-only runtime probe
that hashes the **content of the builder's second argument** on every call, and
ships the instrumentation as a local scratch file. Nothing was attached,
launched or instrumented for this document, and no engine behaviour changed.
The committed diff is this plan only. The builder-input slice is not merged on
this branch, so this plan stands alone and reuses only the facts it needs.

## Quick path

1. Read "What is already settled" - the three argument roles, already measured.
2. Read "The instrumentation lesson" - why the previous probe degraded the run
   and the cost budget this one obeys.
3. Read "The question, the verdicts and the control" - the whole deliverable.
4. Read "The capture design" and "The decision rule".
5. Run the 5-step human procedure when a session is available.
6. Read "What this cannot establish" before writing any conclusion.

## What is already settled

Two live runs of the builder-input probe established the argument roles. These
are **inputs** to this experiment, not things it re-derives.

| Role | Argument | Observation |
|---|---|---|
| The object | first integer argument | Stable: the same object identity across calls |
| The candidate pose input | second integer argument | Pose-shaped (orthonormal slots), but its **pointer** rotates: 7 distinct values over 273 calls |
| The output pose | reached through the third integer argument | Pose-shaped, and its slots changed on every one of 257 calls |

Two more facts bound the question:

- Body animation is real in the pose array: a full-pose capture saw 66 of 66
  `raziel` slots and 24 of 24 `sluagh` slots change while the body moved.
- The track sampler is not the source: it fired 8,861 times with `noTracks`
  every time, because the observed body models carry no populated track table.

The second argument's score was 1 and it was stable in **value** only 1 time in
273, which is exactly why its **content** must be measured directly. A rotating
pointer alone does not tell us whether the data behind it changes.

## The instrumentation lesson

The previous probe classified roughly **8,000** argument values and performed
roughly **3,400** region reads in one session. The per-frame cost was large
enough to visibly lag the game and degrade the capture it was taking. A probe
that changes the timing of the thing it measures is measuring itself.

The cost budget for this probe is therefore a hard design constraint, not a
preference:

| Work item | Previous probe | This probe |
|---|---|---|
| Argument classification | every integer slot, every call | none |
| Candidate scan / shape scoring | every readable argument and offset | none |
| Pose region reads | thousands, plus float math | one region for the input, one for the output, per focus call |
| Per-call text | allowed | none (heartbeats only) |
| Raw dumps | bounded pairs, still large | first N calls only, one region each |
| Text line cap | 3,000 | 400 by default |

The measured cost per focus call is: one object-header chain read for the slot
count, one region read plus hash of the candidate input, and one region read
plus hash of the output. For a 66-slot body the region is 66 x 13 x 4 bytes,
about 3.4 KiB. Nothing else runs per call.

## The question

> Between two consecutive builder calls on the same object, does the **content**
> of the second argument change?

This is the last question because the two other routes are closed: the file
carriers have explicit negative counts, and the track-sampler route is refuted
above. If the builder's second argument is an already-animated pose, the
producer is upstream of the builder. If it is constant, the builder is the
producer and the motion comes from what it computes.

## The capture design

Hook the same slot-pose builder paths, and hash regions only. Nothing is
written, called or patched; entry and exit observation only.

| Aspect | Design |
|---|---|
| Observation site | the slot-pose builder (main and alternate paths); no other hooks |
| Measurement | a digest of the second argument's content, once per call |
| Control | a digest of the output pose, once per call, taken on exit |
| Region | slot count x 13 floats, capped at a stated byte limit with a printed `prefix_cap` marker |
| Hash | a two-lane FNV-1a digest over the raw bytes; deterministic and reproducible |
| State | per object: calls, readable calls, read failures, distinct digests, consecutive changes |
| Raw data | the first N calls (default 6) store one candidate-input region per focus object, plus a manifest line |
| Output | `READY` with module base and range, one `STAT` heartbeat, one compact summary block per object, one `VERDICT` and one `CONTROL` line |
| Scope | focus objects only (default `raziel`); other objects are counted but not hashed, which is how the cost stays bounded |

Why hash the content and not the pointer:

- A pointer can rotate between buffers that hold identical data, and a stable
  pointer can sit on data that is rewritten in place. Only content separates
  "the builder receives motion" from "the builder receives a fixed array".
- Consecutive changes are counted across consecutive readable calls. The
  summary also prints **same-address changes**, so a rotating buffer cannot be
  mistaken for in-place animation, and an in-place writer cannot hide behind a
  stable pointer.
- 3.4 KiB per call is cheap compared with the previous probe's float scoring,
  and it is the smallest observation that answers the question.

## The decision rule

Stated before the run so the result cannot be rationalised afterwards. The
classifier is one pure function in the scratch script, exercised by
`--selftest` (12 candidate cases plus 3 control cases plus hash sanity checks).

| Condition | Verdict |
|---|---|
| consecutive changes >= max(2, 10% of calls) **and** distinct digests >= 3 | **ARG1_ANIMATES** |
| exactly one distinct digest over >= 10 readable calls | **ARG1_STATIC** |
| fewer than 10 readable calls, builder never fired, slot count unresolved, or read failures > 50% | **UNOBSERVED** |

`UNOBSERVED` is also the result for any middle case that satisfies neither
positive rule, such as two distinct digests or a change rate below the
threshold. It is a **missing observation**, never a quiet third answer.

### Outcome A - ARG1_ANIMATES

The builder's second argument carries different content between calls.

**Establishes:**

- The builder **reads a pose that is already animated**. It is a consumer of
  motion, not its producer.
- Something upstream writes that region between builder calls. The next slice
  is a targeted write-watch on the region the builder reads, not a broad hunt.
- The procedural-orientation account is incomplete on its own: whatever feeds
  this argument must be identified before the pose can be replayed offline.

**Refutes:**

- "The builder computes the body animation from static inputs." Its input is
  not static.
- Any plan that stops at the builder's arithmetic. The arithmetic consumes a
  changing array; the array's writer is the target.

**Does not establish:** who the upstream writer is, what it reads, or whether
the writer runs per frame or at a coarser cadence.

### Outcome B - ARG1_STATIC

The same content digest repeats across every readable call.

**Establishes:**

- The second argument is **static** in the observed window. The builder cannot
  be consuming motion from it.
- The builder **computes** the pose from the object and its transform (or from
  other inputs), consistent with a procedural pose derived from descriptor
  mode, the authored aim vector and the object transform.
- The next slice reads the builder's input arithmetic and its non-pose inputs,
  with no upstream writer to chase.

**Refutes:**

- "The builder receives an already-animated pose array." In the observed
  window it does not.
- The need for a write-watch on this argument.

**Does not establish:** which non-pose inputs actually drive the computation,
or whether a different animation id or actor uses a different path.

### Outcome C - UNOBSERVED

No usable observation: the hook never fired, the slot count did not resolve,
fewer than 10 calls were readable, or read failures dominated.

**Establishes:** nothing about the engine.

**Refutes:** nothing. Do **not** read it as "the builder neither receives nor
computes". Re-pin the module, the site and the slot-count chain, confirm the
character was moving, then re-run.

## The output control

The probe also hashes the builder's **output** pose, on exit, with the same
digest and the same region size. The output is already known to animate: its
slots changed on 257 of 257 calls.

- If the output digest animates, the capture is live and the candidate-input
  verdict can be read.
- If the output digest is **static**, the capture is broken. The summary emits
  a `CONTROL-NOTE` saying so, and an `ARG1_STATIC` result from that run must be
  discarded. A static output cannot coexist with a working measurement of a
  pose that is known to move.
- An output that does not animate while the candidate input does is also a
  broken run: it means the region sizes or the observation site are wrong.

The control is the reason a negative on the candidate argument can be trusted.

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Start the probe from the session scratch directory.** Run
   `python sr1_arg1_hash_probe.py --attach SRX.exe --wait 300
   --log <scratch>\arg1_hash.log --dump-dir <scratch>\arg1_hash --samples 6`.
   Wait for the `READY` line (module base and range) and at least one `STAT`
   heartbeat.
3. **Exercise the body path.** Idle about 10 s, walk continuously for at least
   20 s, then attack and turn for about 10 s each. The point is a window in
   which the body is visibly moving.
4. **Stop and keep.** Press `Ctrl+C`. The probe writes
   `arg1_hash_summary.txt` before detaching. Read the per-object block:
   `readable`, `read_failures`, `hashes_distinct`, `consecutive_changes`,
   `same_addr_changes`, then the `VERDICT` and `CONTROL` lines. Read
   `STAT-NOTE`: any hook listed as `unobserved` never fired, which is a
   missing observation, not evidence of absence.
5. **Classify, then re-run once.** Apply the decision rule. A verdict used to
   confirm or refute the conclusion must reproduce on a second run. If the
   verdict is `UNOBSERVED`, re-pin the site and the slot count before drawing
   any conclusion.

The summary has this shape (template, not a result):

```text
ARG1 SUMMARY object="raziel" slots=66 calls=<N>
ARG1 SUMMARY   arg1 readable=<N> read_failures=<N> hashes_distinct=<K>
               consecutive_changes=<C> same_addr_changes=<S>
               equal_pairs=<E> addr_distinct=<A> region_bytes=<B>
ARG1 SUMMARY   control out_readable=<N> out_failures=<N>
               out_hashes_distinct=<K2> out_consecutive_changes=<C2>
ARG1 VERDICT object="raziel" <ARG1_ANIMATES|ARG1_STATIC|UNOBSERVED> reason=<..>
ARG1 CONTROL object="raziel" <OUT_ANIMATES|OUT_STATIC|OUT_UNOBSERVED> reason=<..>
ARG1 SUMMARY tally objects=<N> animates=<a> static=<b> unobserved=<c>
```

## What this cannot establish

- **It does not name the upstream writer.** Outcome A locates the read side;
  naming the writer needs a separate write-watch on the same region.
- **It does not decode the builder's arithmetic.** Outcome B locates the
  producer; it does not reveal which descriptor fields or transform values the
  computation uses.
- **Content equality is probabilistic.** Two distinct byte sequences can
  collide under a 64-bit digest, which is astronomically unlikely per
  comparison but not zero. A digest change is positive evidence; a digest
  match is evidence bounded by the digest width.
- **Samples are taken at builder calls, not per frame.** The experiment
  answers "does this phase change between calls", not "what is the exact
  curve" or "how often per frame".
- **Pointer rotation and content motion are separated, not unified.** The
  summary prints same-address changes next to total changes; a rotating buffer
  that happens to hold equal content is reported honestly, but the experiment
  does not attribute rotation to a particular owner.
- **A negative is bounded by the window.** One build, one actor, one session,
  and only while the body visibly moves. Another animation id, actor or class
  may behave differently.
- **Instrumentation can perturb timing.** Do not infer frame cadence or
  performance from the probe; only the change classification is meaningful.
- **The argument is one input, not every input.** Outcome B does not prove the
  builder has no other inputs, and Outcome A does not prove the candidate
  argument is the only animated one.

## Roles (for re-pinning without the script)

| Role | What it hands the probe |
|---|---|
| Slot-pose builder (main path) | the call site; the object, the candidate input and the output argument |
| Slot-pose builder (alternate path) | the same roles for the alternate class; hooked for completeness |
| Object header chain | the per-object slot count, which fixes the region size |
| Candidate input region | the second argument's content; the measurement |
| Output pose region | the output pose's content; the control |

No addresses or field offsets are recorded here. The probe carries them and
lives outside the repository, as the runtime-capture `README.md` explains. If
the probe is lost, the roles above are enough to re-pin it: each row names a
stable behaviour, not a number.

## Risks and limits

- **Hooks are pinned relative to the engine module.** A shipped-module change
  requires re-deriving each site from the roles. The `READY` line prints the
  module base and range so a mismatch is visible.
- **A zero must be labelled `unobserved`.** The probe prints `STAT-NOTE` for
  every hook that never fired and a `CONTROL-NOTE` for a broken control.
- **A short or stationary window is not a result.** Below 10 readable calls, or
  without visible movement, the verdict is `UNOBSERVED`.
- **Focus-scoped cost.** Only focus objects are hashed; non-focus objects are
  counted. This keeps the per-frame cost bounded but means the verdict is about
  the focus actor, not every actor in the scene.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes.

## Checklist for review

- [ ] The three established roles are stated before the question.
- [ ] The instrumentation lesson and the cost budget are explicit.
- [ ] The question is stated as content-change between consecutive calls.
- [ ] Both positive verdicts state what they establish and what they refute.
- [ ] `UNOBSERVED` is kept as a missing observation, not a third answer.
- [ ] The output control is stated, including the broken-run rule.
- [ ] The procedure has five steps and an explicit re-run.
- [ ] The limits say a digest match is probabilistic and a negative is bounded.
- [ ] No addresses, offsets, disassembly, game bytes or install paths appear in
      the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, file-position offset, disassembly listing or absolute install
path appears here. Roles, structure, format constants and observed counts only.

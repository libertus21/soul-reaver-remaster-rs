# angle-input-plan - do the three scalar angle inputs animate? (2026-11-12)

This is a **preparation** slice. It states why the three scalar angle inputs of
the two orientation constructors are the last measurable link in the
body-animation chain, records the calling convention that was determined
statically for both constructors, designs one read-only probe that samples the
per-slot angle table without touching the hot path beyond three dword reads,
and ships the instrumentation as a local scratch file. Nothing was attached,
launched or instrumented for this document, and no engine behaviour changed.
The committed diff is this plan only.

## Quick path

1. Read "Why the angle inputs are the animation" - the one-paragraph case.
2. Read "The ABI, determined" and "The CALIB safeguard" - how a wrong read is
   made impossible to miss.
3. Read "The sampling design" and "Why this does not lag" - the deliverable.
4. Read "The verdicts" - what each answer establishes and what it does not.
5. Run the 5-step human procedure when a session is available.
6. Read "The escalation" before choosing the next slice.

## Why the angle inputs are the animation

The animation investigation is closed at the producer level, and the producer
is now named: the two sibling **orientation constructors**. Each writes a
slot's nine-float rotation block as an identity followed by three in-place axis
rotations, and each derives every written value from **three scalar angles**
through the single-precision trigonometric pair - there is no source pointer,
no length and no loop, so the routines compute rather than copy. Both are
called from the per-object pose builders, per active slot, per frame, into the
object's own per-frame pose array. The earlier `arg1` "animated pose" was a
**dead register** carrying a leftover pointer inside that same arena, so the
content probe measured the builder's output, not an input, and
`ARG1_ANIMATES` was a false positive. The file carriers are eliminated with
explicit negative counts (`.SRA`/`.PCM`/`.SRM`/`.SRL`), and no authored
per-slot rotation exists for the body. Motion at runtime is therefore
produced, and the producers consume exactly three numbers per slot. If those
three numbers change while the character walks, the animation is contained in
them; if they do not, the motion is not being carried by this path.

## What is already settled

These are **inputs** to this experiment, not things it re-derives.

| Fact | Value | Class |
|---|---|---|
| Body motion exists in the output pose | 66 of 66 `raziel` slots changed during movement | Measured, two runs |
| Authored per-slot rotation in any file carrier | none (`.SRA`, `.PCM`, `.SRM`, `.SRL` eliminated with numbers) | Negative, counted |
| The constructors compute, not copy | identity init plus three trig axis rotations; every written value derives from the angles | Static, two routines |
| The constructors' caller phase | per-object pose pass inside the main frame update | Static, call graph |
| The earlier `arg1` reading | dead register aliasing the builder's own per-frame arena | Static, instruction-level |
| The angles' three sources | object aim triple, per-slot descriptor triple, sampled channel frames | Static, role-level |

The open item is not who produces the pose any more. It is **what moves**: the
three angles, or something downstream of them.

## The ABI, determined

The constructors were read statically, read-only, from the shipped module. The
determination below is a behaviour paraphrase, not a listing.

| Aspect | Finding |
|---|---|
| Parameters | two: a **pointer to three consecutive 32-bit floats**, and a **pointer to the pose slot** |
| Argument registers | the angle pointer arrives in the **first integer argument register**; the slot pointer in the **second** |
| Return value | the slot pointer |
| Angle access | three consecutive dword loads through the first pointer, before any axis rotation is applied |
| Destination | nine floats, the slot's 3x3 rotation block; the rest of the slot is written by the callers |
| Units | the values are consumed **directly** by the sine/cosine pair, so they are radians as received; caller-side quantization is a separate question the probe measures |
| Composition order | reverse-order sibling applies third, second, first; forward-order sibling applies first, second, third |

Three alternative readings were tested and rejected before the probe was
written:

| Alternative | Why it is rejected |
|---|---|
| Three floats in the first three vector registers | the entries write the identity before reading any vector register, and then read the three dwords through the first integer pointer |
| Three floats spilled to the incoming stack argument area | the entries never read that area before the first trigonometric call |
| A struct passed by value | the first argument is dereferenced three times; the second is used only as a destination |

**One important correction to the earlier scratch note.** The two numbers that
note carries are **interior identity stores**, not function entries. Hooking an
interior store would capture a partially built rotation and would also fire on
every slot build for a site that is shared. The probe therefore pins the two
**function entries**, which is the only position where the three angle inputs
already exist and the slot is still the caller's.

## The CALIB safeguard

An ABI determination is a static claim. The probe makes a wrong read
impossible to miss instead of trusting the claim:

| Check | Rule | Failure means |
|---|---|---|
| Triplet plausibility | all three values finite and bounded by a plausible angle magnitude | the pointer is not an angle triplet |
| Alignment | the triplet pointer is 4-byte aligned | the read is not scalar floats |
| Destination readability | the slot pointer is readable for the rotation block | the second argument is not a slot |
| Rotation sanity | on leave, for the first few calls only, the nine-float block is finite and every entry is bounded by a small tolerance around one | the destination is not a rotation block, or the call crashed mid-build |
| Calibration rate | at least half of all constructor calls must pass plausibility | the read is systematically wrong |

A rotation matrix entry cannot exceed one, so a wrong ABI shows up on the very
first calls as an out-of-range value rather than as plausible-looking noise.
The probe prints the first few triplets and the matching rotation verdict as
`CALIB` / `CALIB-ROT` lines, so the operator sees the ABI work or fail before
any verdict is written. If the calibration rate is below the threshold, the
result is `UNOBSERVED`, never `STATIC`.

## The question

> Between two consecutive sample windows on the same actor, does the **per-slot
> table of three scalar angles** change?

The numeric values are the secondary payload: the observed absolute maximum and
the smallest non-zero step separate radians from degrees from fixed point
without a second probe.

## The sampling design

Hook the two constructor entries. Nothing else is hooked.

| Aspect | Design |
|---|---|
| Hot-path work | three dword reads through the angle pointer, one slot pointer, one map update keyed by the angle source |
| Per-call text | none; only the first few calls print `CALIB` |
| Per-call hashing | none |
| Region reads | none on the hot path; the nine-float block is read only for the first few calibration calls |
| Sample window | a dump interval, default a quarter second; the buffer is cleared on each dump so a dump describes one interval |
| Slot key | the **angle-source pointer** |
| Table | per source: the latest triplet bits; the row index is the source's order among the interval's distinct sources |
| Dump line | one line: slot count, measured stride and its exactness, stable flag, dump hash, changed/added/missing counts, the first N numeric triplets, and a compact per-slot triplet-hash vector |
| Dump hash | a two-lane FNV-1a over the canonical row sequence (row index plus the three raw bit patterns); identical content gives an identical digest, and a row moving between indices changes it |
| Numerics | global absolute maximum, smallest non-zero step between consecutive dumps, and the share of steps at or below a small quantum threshold |
| Caps | dump rate, numeric head length, hash-vector length, text lines, buffered sources, cached source classes |
| Finish | on `Ctrl+C`, or after an optional fixed duration; the summary is written in both cases |

### Why the source pointer, and not a call counter

The counter-modulo-slot-count idea is **rejected**. Inactive slots are skipped
before the constructors are reached, and the constructors are also used by the
object-aim path, the attachment-frame path and unrelated transform sites, so a
per-call counter does not map to a slot index. The register the older probes
sampled was already shown to be dead; a second counting assumption would
reintroduce exactly that class of error.

The source pointer is the key because it is the one address that identifies
*which* angle triple was passed. For descriptor-driven slots it points into
the loaded descriptor record, which is file-resident and stable across frames;
for a stack temporary it sits at a stable stack address for a given call depth.
The probe **measures** that assumption instead of asserting it:

- the **stride** between consecutive sources in a window, and the share of
  deltas that are an exact multiple of it;
- the **share of dump pairs whose row set is identical**.

Only pairs with an identical row set are comparable, and only comparable pairs
feed the verdict. If every pair is unstable, the answer is `UNOBSERVED` with
`reason=slot-key-unstable` - which is itself a finding about the source path,
not a failure.

## Why this does not lag

The earlier probes did thousands of argument classifications and thousands of
region reads per session, and the frame cost degraded the capture. The budget
here is a hard design constraint:

| Work item | Earlier probes | This probe |
|---|---|---|
| Argument classification | every integer slot, every call | none |
| Candidate/shape scoring | every readable argument | none |
| Per-call region reads | kilobytes per call | none |
| Per-call hashing | one or more digests per call | none |
| Per-call text | allowed | none |
| Per-dump work | n/a | one sort of the distinct sources and one 14-byte-per-row digest |
| Text lines | thousands | one line per dump, hard line cap |
| Buffer growth | unbounded structures | one row per distinct source, with a source cap |

At four dumps per second and a body-scale slot count, the dominant cost is the
per-call map update on a hot path that already does three trigonometric calls
per slot. The probe is designed to be invisible at the frame level; it is not
designed to measure cadence.

## The verdicts

Stated before the run so the result cannot be rationalised afterwards. The
classifier is one pure function in the scratch script, exercised by
`--selftest` (12 candidate cases plus hash-mirror and units-hint checks).

| Condition | Verdict |
|---|---|
| uncalibrated, or fewer than half of calls plausible | **UNOBSERVED** |
| fewer than three dumps, or no slots, or no comparable pair | **UNOBSERVED** |
| comparable pairs, at least one slot triplet changed | **ANGLES_ANIMATE** |
| comparable pairs, exactly one distinct dump hash, zero changed slots | **ANGLES_STATIC** |
| anything else | **UNOBSERVED** |

`UNOBSERVED` is a **missing observation**, never a quiet third answer.

### Outcome A - ANGLES_ANIMATE

The three scalar angle inputs change per slot while the body moves.

**Establishes:**

- The animation is **fully contained in these three angles per slot**. The
  constructors are the animation's only producer, and their inputs move.
- The remaining work is only the angle **source**: which of the descriptor
  path, the object aim triple and the attachment frames feeds each slot, and
  at what cadence.
- Offline replay is reachable: parse the object container, relocate it, take
  the same three angles per slot, and evaluate the same rotation composition.

**Refutes:**

- Any account that needs a separate motion producer downstream of the angles.
- The possibility that the constructors receive a constant pose and only the
  object transform moves.

**Does not establish:** which source feeds each slot, whether the angles are
pre-quantized by the caller, or whether a different actor uses another path.

### Outcome B - ANGLES_STATIC

The per-slot triplets are constant across every comparable pair.

**Establishes:**

- The constructors' scalar inputs are **not** the motion carrier in the
  observed window. The body's visible motion must come from elsewhere.
- The two candidate "elsewhere" paths are now specific: the shared-transform
  proxy route (which does not use these constructors at all) and the object
  transform (root motion and facing), with attachment-channel frames as the
  third possibility.
- A negative here is informative rather than a dead end: it removes the whole
  procedural-orientation branch from the animation question.

**Refutes:**

- "The three angles per slot animate." In the observed window they do not.
- The plan that stops at the constructors' inputs.

**Does not establish:** that no slot anywhere animates. The result is bounded
by the observed actor, window and source set. A slot that was never active is
not covered.

### Outcome C - UNOBSERVED

No usable observation: the hooks never fired, the calibration failed, the
window was too short, or the slot key never stabilised.

**Establishes:** nothing about the engine.

**Refutes:** nothing. Do **not** read it as static. Re-pin the entries, confirm
`CALIB` and `CALIB-ROT` pass, confirm the character was actually moving, then
re-run.

## The escalation

| Verdict | Next slice |
|---|---|
| ANGLES_ANIMATE | Run the per-slot **mode census** statically to say which of the four production paths each slot takes, then re-run this probe with a per-slot source tag so the angle source is named per slot. Only after that is a file-only replay meaningful. |
| ANGLES_STATIC | Move the same question one step downstream: (1) the shared-transform proxy route, whose rotation is copied from a fixed-point transform and never touches these constructors; (2) the object transform, i.e. root motion and facing; (3) the attachment channel frames. The cheapest next probe tags the *call site* rather than the angle source, which shows immediately whether the constructors fire at all while walking. |
| UNOBSERVED | Re-pin the two entries from the role table below, verify the calibration lines, verify movement, re-run once. Do not reinterpret a failed calibration as a negative. |

## Human procedure (5 steps)

1. **Get in-world, do not attach yet.** Launch the game normally, load a save,
   and stand `raziel` on solid ground with room to walk.
2. **Validate the setup without attaching.** From the session scratch
   directory, run `python sr1_angle_input_probe.py --dry-run`. It prints the
   ABI model, the two roles, the caps and the verdicts, and imports nothing.
3. **Run the probe.** Run
   `python sr1_angle_input_probe.py --attach SRX.exe --wait 300 --hzs 4
   --dump-slots 8 --log <scratch>\angle_input.log
   --summary <scratch>\angle_input_summary.txt`.
   Wait for the `READY` line (module base and range), the `CALIB` and
   `CALIB-ROT` lines, and at least one `DUMP` line.
4. **Exercise the body path.** Idle about 10 s, walk continuously for at least
   20 s, then attack and turn for about 10 s each. The point is a window in
   which the body is visibly moving. Press `Ctrl+C`; the summary is written
   before detaching.
5. **Classify, then re-run once.** Read `slots`, `dumps`,
   `distinct_dump_hashes`, `changed_slots`, `stable_pairs` and the
   `VERDICT`. A verdict used to confirm or refute a conclusion must reproduce
   on a second run. If the verdict is `UNOBSERVED`, re-pin before drawing any
   conclusion.

The output has this shape (template, not a result):

```text
ANGLE READY module=<name> base=<addr> end=<addr> abi="<model>" hooks=<roles> ...
ANGLE CALIB n=1 ctor=<role> src=<addr> srcclass=<file|anon> dest=<addr>
            units=radians-direct angles=(<a>,<b>,<c>) absmax=<v>
ANGLE CALIB-ROT n=1 ctor=<role> dest=<addr> finite=1 bounded=1 max_abs=<v> ok=1
ANGLE DUMP n=1 slots=<N> stride=<s> sx=<pct> stable=<0|1> hash=<digest>
           changed=<n> add=<n> miss=<n> calib=<ok>/<total>
           head=[0:(<a>,<b>,<c>) ...] hashes=[<h>,<h>,...]
ANGLE STAT calls=<N> dumps=<n> distinct=<k> changed=<c> stable_pairs=<p> ...
ANGLE SUMMARY slots=<N> dumps=<n> distinct_dump_hashes=<k> changed_slots=<c>
              added_slots=<a> missing_slots=<m> stable_pairs=<p> ...
ANGLE SUMMARY numerics units=radians-direct abs_max=<v> min_step=<v>
              step_samples=<n> quant_frac=<f> units_hint=<hint>
ANGLE VERDICT <ANGLES_ANIMATE|ANGLES_STATIC|UNOBSERVED> reason=<...>
```

## Roles (for re-pinning without the script)

| Role | What it hands the probe |
|---|---|
| Orientation constructor, reverse composition order | the entry to hook; receives the angle pointer and the slot |
| Orientation constructor, forward composition order | the same contract, opposite rotation order |
| Angle triple | three consecutive 32-bit floats at the first argument |
| Pose slot | the destination at the second argument; nine floats of rotation |
| Angle source | the per-slot table key; descriptor-resident or stack-temporary |
| Dump hash | the exact comparator between two sample windows |

No addresses or module-relative numbers are recorded here. The probe carries
them and lives outside the repository, as this directory's `README.md`
explains. If the probe is lost, the roles above are enough to re-pin it: each
row names a contract, not a number.

## Risks and limits

- **Hooks are pinned relative to the engine module.** A shipped-module change
  requires re-deriving both entries from the roles. The `READY` line prints the
  module base and range so a mismatch is visible.
- **The ABI is a static claim.** `CALIB` and `CALIB-ROT` are the runtime
  refutation; if they fail, the read is wrong and the run is `UNOBSERVED`.
- **The table is window-local.** Each dump describes the interval since the
  previous dump, not a frame-by-frame curve. A change is positive evidence; a
  match is bounded by the observation window.
- **Digest equality is probabilistic.** Two distinct bit sequences can collide
  under a 64-bit digest. A change is positive evidence; equality is evidence
  bounded by the digest width. Row-level comparison uses exact bits, so a
  per-slot change is never missed by a collision.
- **A source key can be unstable.** Stack temporaries and shared scratch can
  move. Unstable pairs are counted and excluded; if too few comparable pairs
  remain, the verdict is `UNOBSERVED` with an explicit reason.
- **A negative is bounded.** One build, one actor, one window, and only the
  slots that actually fired. Another actor, animation id or class may differ.
- **Instrumentation perturbs timing.** Do not infer frame cadence or
  performance from the probe; only the change classification is meaningful.
- **The probe is read-only.** It never writes process memory, never calls game
  code and never patches bytes.

## Checklist for review

- [ ] The producer verdict and the dead-register correction are stated before
      the question.
- [ ] The ABI is stated as a contract (two pointers, three consecutive floats,
      slot returned) with the rejected alternatives.
- [ ] The interior-store/entry distinction is stated, with its consequence for
      hooking.
- [ ] The CALIB safeguard lists the plausibility, alignment, rotation and rate
      checks.
- [ ] The sampling design states the hot-path budget and the per-dump budget.
- [ ] The counter-as-slot-key idea is rejected with a reason.
- [ ] Both positive verdicts state what they establish and what they refute.
- [ ] `UNOBSERVED` is kept as a missing observation, not a third answer.
- [ ] The procedure has five steps, a dry run and an explicit re-run.
- [ ] The escalation names a concrete next probe for each verdict.
- [ ] No addresses, module-relative numbers, disassembly, game bytes or
      absolute install paths appear in the committed diff.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, file-position offset, disassembly listing or absolute install
path appears here. Roles, contracts, structure and format constants only.

# angle-correlation-plan - pair the constructor input with the produced rotation (2026-11-12)

The open question is what the three source floats are. v2 proved they animate;
it could not say what they mean. This slice adds a **v3 probe** that dumps the
constructor's input triplet and the produced pose slot on the **same line**, so
the input-to-output relationship can be computed offline. The decisive number is
the smallest mean wrapped delta between the produced rotation angle and six
candidate quantities. A near-zero delta names the model; a large one removes it.

This is a **preparation** slice. It commits this plan only. The probe lives in
the session scratch directory, carries module-relative entries, and is never
committed. Nothing was attached, launched or instrumented for this document.

## Quick path

1. Read "Why a correlation dump settles the four candidates" - the one-table case.
2. Read "Theta and the six candidates" - the exact math.
3. Read "Offline decision procedure" - what each `best` implies and what to do.
4. Read "Two-phase human procedure" - the realm-shift segment is Phase 2.
5. Read "What the experiment cannot establish" before writing any conclusion.

## Why a correlation dump settles the four candidates

v2 measured the input triplet alone. "ANGLES_ANIMATE" therefore left the meaning
of the three floats ambiguous: they could be an angle, a direction, a position,
or a mixed state, and every interpretation predicted "they change while walking".
A correlation dump breaks the tie because each candidate makes a **specific
numeric prediction about the produced rotation**:

| # | Candidate interpretation | Prediction the dump tests |
|---|---|---|
| 1 | Euler / yaw angle in some unit (one of the three floats) | `theta` equals that float, up to a constant offset and a unit scale |
| 2 | Direction / look-at target | `theta` equals `atan2` of two of the components |
| 3 | Position unrelated to the rotation | no candidate is close; all six deltas stay large |
| 4 | A realm / state flag mixed into the triplet | not a per-sample formula; seen as a **discrete jump** at the realm change that is not continuous with movement |

Models 1 and 2 are point-predictions and fail loudly when wrong. Model 3 is the
absence of all point-predictions. Model 4 is a **temporal signature**, so it is
tested by the Phase 2 realm segment rather than by one dump's arithmetic.

## Inputs already established (not re-derived here)

These are the starting conditions, measured or confirmed in earlier slices.

| Fact | Value | Class |
|---|---|---|
| Inputs are three consecutive 32-bit floats reached through the first argument pointer | 3 dword reads | Static, verified by runtime calibration |
| Destination is the slot in the second argument; the slot is returned | 13-float slot, 52-byte stride | Static plus measured stride |
| Slot layout | rotation rows `[0..8]`, scale `[9]`, translation `[10..12]` | Measured |
| Runtime calibration | every constructor call calibrated; rotation sanity passed | Measured, two runs |
| Rotation animates | a smooth Z sweep while walking; identity when idle | Measured |
| Input magnitudes | absolute maximum in the thousands, smallest step about 0.02, near-zero quantization | Measured |
| Input shape | mostly "neither unit nor Q12"; a small minority unit-ish or Q12-ish | Measured |
| Destination key quality | about 95% stable pairs; 52-byte stride exact in about 99% of deltas | Measured |

The destination pointer is the key. The source pointer and its triplet are the
payload, as in v2.

## Theta and the six candidates

Let the destination rotation block be row-major `m[0..8]`.

**Known case - pure Z rotation.** The live matrices have the form
`(c, s, 0, -s, c, 0, 0, 0, 1)`. Then:

```
theta = atan2(m[1], m[0])            axis = z
```

**Other single-axis rotations.** The probe also recognises a pure X block
`(1, 0, 0, 0, c, s, 0, -s, c)` and a pure Y block `(c, 0, s, 0, 1, 0, -s, 0, c)`
within a small tolerance and reports a signed `atan2` with the matching axis.

**Fallback.** If no single-axis pattern matches:

```
theta = acos(clamp((m[0] + m[4] + m[8] - 1) / 2, -1, 1))     axis = none
```

The fallback loses the sign; it is reported honestly as `axis=none` rather than
guessed.

**The six candidates**, computed from the source triplet `(a, b, c)`:

| Label | Quantity |
|---|---|
| `src0` | `a` |
| `src1` | `b` |
| `src2` | `c` |
| `atan2_01` | `atan2(b, a)` |
| `atan2_02` | `atan2(c, a)` |
| `atan2_12` | `atan2(c, b)` |

**The correlation number.** For sampled slots `i = 1..N` in one dump, and each
candidate `k`:

```
d_k = (1 / N) * sum_i | wrap_pi(theta_i - candidate_k,i) |
wrap_pi(x) = atan2(sin x, cos x)
best = argmin_k d_k   if  min_k d_k <= tau   else   none
```

`tau` defaults to `0.20` radians (about 11 degrees) and is settable with
`--corr-tol`. `wrap_pi` folds multiples of `2*pi`, so a value that differs from
`theta` by a whole turn still scores zero. It does **not** fold a unit scale, so
a degrees-valued candidate shows a large delta unless the two numbers coincide
by chance. That property is what lets the raw-delta table distinguish radians
from degrees without a second probe.

## Offline decision procedure

`best` is emitted per dump; the summary also reports the mode (`dominant`) across
dumps and the mean of the two headline deltas. A model is adopted only when it
wins on **repeated** dumps and survives the offset/scale fit below.

| Dominant `best` | Model | What it implies | What to do |
|---|---|---|---|
| `src2` | 1 | yaw is the third component | Fit `offset = median(wrap(theta - src2))` and a scale. Offset ~0 and scale ~1 means radians-direct. A fixed offset is a convention; apply it. |
| `src0` | 1 | yaw is the first component (axis order differs) | Same fit; verify the axis and handedness. |
| `src1` | 1 | yaw is the second component (axis order differs) | Same fit; verify the axis and handedness. |
| `atan2_01` | 2 | `(a, b)` is a planar direction; yaw is its `atan2` | Treat the first two components as a direction. Confirm sign consistency on a second window. |
| `atan2_02` | 2 | direction in the `a`/`c` plane | Map the axes, then confirm. |
| `atan2_12` | 2 | direction in the `b`/`c` plane | Map the axes, then confirm. |
| `none` | 3, 4, or offset/unit | no raw wrapped correlation in this window | First test a constant offset and a unit scale. If still none, inspect the per-sample series for a discrete realm jump (Phase 2). Only then treat the triplet as unrelated to the rotation. |
| no clear mode / mixed | inconclusive | different slots, different constructor sites, or inactive slots | Segment by constructor site and by axis; raise `--dump-slots`; re-run. |

A near-zero `src2` delta plus a stable offset means the third float carries the
angle in some integer or scaled unit; the offset fit turns it into radians. A
`none` on the movement window plus a discrete jump on the realm window is the
signature that model 4 gains support: movement is carried elsewhere and the
triplet reacts to state, not to the pose.

## Diagnostics that are not verdicts

- If `theta` is **identical across the sampled slots** while the triplets differ,
  the rotation is shared (a global facing) and the per-slot triplet is not its
  direct source. That points away from models 1 and 2 for those slots.
- If the triplet is zero for many slots, those slots are inactive or unwritten;
  they are not evidence for any model.
- A single dump is not a result. Use the mode across many dumps.

## The v3 probe: what changed from v2

| Aspect | v2 | v3 |
|---|---|---|
| Per-dump text | one `DUMP` line plus optional `FULL` lines | one `DUMP`, one `CORR`, and one `PAIR` per sampled slot |
| Input and output coupling | input triplets and slot floats printed separately | input triplet, source pointer, destination pointer, `f13`, `theta` and `axis` on **one** `PAIR` line |
| Correlation | none | six raw candidate deltas and a `best` label per dump |
| Hot path | three dword reads, one destination pointer, one map update | unchanged |
| Region hashing | none | none |
| Key | destination pointer | unchanged |
| Safeguards | `CALIB`, `CALIB-ROT`, `ENTER-ERR`, `SEMANTIC-NOTE` | all kept, plus a `CORR-NOTE` |

Costs: `--hzs 4`, `--dump-slots 4`, `--max-lines 400` by default. Per sampled
slot the dump reads three source dwords (hot path) and thirteen destination
floats (dump time only). No region hashing, no per-call text, no argument scan.

The probe is read-only: it never writes process memory, never calls game code and
never patches bytes.

## Output shape (template, not a result)

```text
ANGLE READY module=<name> base=<addr> end=<addr> abi="<model>" key=dest_pointer ...
ANGLE CALIB n=1 ctor=<role> src=<addr> dest=<addr> angles=(<a>,<b>,<c>) absmax=<v>
ANGLE CALIB-ROT n=1 ctor=<role> dest=<addr> finite=1 bounded=1 ok=1
ANGLE DUMP n=<n> slots=<N> dest_stride=<s> dest_sx=<pct> stable=<0|1> hash=<digest> ...
ANGLE PAIR n=<n> i=<i> dest=<addr> src=<addr> in=(<a>,<b>,<c>)
           f13=[<13 floats>] theta=<rad> axis=<z|x|y|none>
ANGLE CORR n=<n> slots=<k> ztheta_src2_delta=<d> atan2_src01_delta=<d>
           best=<src0|src1|src2|atan2_01|atan2_02|atan2_12|none>
           deltas=[src0=<d> src1=<d> src2=<d> atan2_01=<d> atan2_02=<d> atan2_12=<d>]
ANGLE CORR-SUMMARY dumps=<n> tol=<t> best_counts=<...> dominant=<label>
ANGLE CORR-SUMMARY mean_delta ztheta_src2=<d> atan2_src01=<d>
ANGLE CORR-NOTE <interpretation reminder>
ANGLE SEMANTIC-NOTE <unchanged caveat about the radians claim>
ANGLE VERDICT <ANGLES_ANIMATE|ANGLES_STATIC|UNOBSERVED> reason=<...>
```

## Two-phase human procedure

### Phase 1 - movement baseline

1. **Get in-world, do not attach yet.** Load a save and stand the character on
   solid ground with room to walk.
2. **Validate without attaching.** From the session scratch directory run
   `python sr1_angle_corr_probe.py --dry-run`. It prints the ABI model, the new
   `PAIR`/`CORR` lines, the caps and the verdicts, and imports nothing.
3. **Run the probe.** For example:
   `python sr1_angle_corr_probe.py --attach SRX.exe --wait 300 --hzs 4
   --dump-slots 4 --max-lines 400 --log <scratch>\angle_corr.log
   --summary <scratch>\angle_corr_summary.txt`.
   Wait for `READY`, `CALIB`, `CALIB-ROT`, and at least three `PAIR`/`CORR`
   pairs.
4. **Exercise the body path.** Idle about 10 s, walk continuously for at least
   20 s, then strafe and turn for about 10 s each.
5. **Stop and read.** `Ctrl+C` writes the summary before detaching. Read the
   per-dump `best` mode, the two headline deltas, and the key-quality lines.
6. **Repeat once.** A model must reproduce on a second run before it is adopted.
   If the default 400-line cap is reached before the window ends, raise
   `--max-lines` or split the window.

### Phase 2 - realm-shift segment (tests the user's hypothesis)

The hypothesis: some of the three floats carry the spectral-to-material realm
state rather than the rotation. This is a temporal test, so isolate it:

1. **Re-attach with a fresh log.** Same flags as Phase 1.
2. **Capture, in order:** stand still in the material realm; walk a few steps;
   stop; trigger the realm shift **while standing still**; stand still in the
   other realm; walk a few steps; shift back while standing still.
   Standing still during each shift avoids confounding the jump with body
   translation.
3. **Compare the series.** Orient by the `DUMP`/`CORR` sequence:
   - Continuous, smooth change while walking -> movement / geometry.
   - A discrete step at the shift, in a float that did **not** move with
     position, reversing on the shift back -> a state component (model 4 gains
     support).
   - A step that coincides with a body move -> confounded; repeat standing still.
4. **Repeat both directions.** Material to spectral and back; a reversible jump
   is stronger evidence than a one-way step.

A discrete jump is necessary but **not sufficient** for the realm hypothesis: the
shift may also change valid geometry, animation state or the active slot set.

## What the experiment cannot establish

- **Causality.** Value correlation is not proof that the constructor reads those
  exact bytes as its angle; a coincident buffer or a copied temporary can mimic
  it. The ABI was established statically in earlier slices; v3 tests the value
  relationship, not the read.
- **The full source map.** Only sampled slots (default four per dump) and only
  slots that actually fire are covered.
- **Units when an offset remains.** A constant offset or a unit scale is fitted
  offline; the probe only shows the raw delta. A `none` result does not mean
  "unrelated" until offset and scale have been tested.
- **The realm flag's identity.** The probe can show that a discrete component
  exists and when it changes; it cannot name the bit or field that encodes it.
- **Handedness and sign.** `theta` is defined by the matrix convention. Without
  an independent ground truth, a global sign flip cannot be separated from a
  sign in the input convention.
- **Other actors, paths and builds.** A negative or positive result is bounded by
  the observed actor, window, slots and shipped build.
- **Sub-frame timing and cadence.** The probe must not be used to infer frame
  rate or performance.
- **Whether a shared rotation comes from a per-slot input or a global facing.**
  When `theta` is identical across slots, the dump shows the sharing but not its
  origin.

## Risks and limits

- Hooks are pinned relative to the engine module. A shipped-module change
  requires re-deriving both entries from the roles in the previous plan.
- The ABI is a static claim; `CALIB` and `CALIB-ROT` are its runtime refutation.
- The table is window-local; each dump describes the interval since the prior
  dump, not a frame-by-frame curve.
- Digest equality is probabilistic; row comparison uses exact bits.
- A source key can be unstable; unstable pairs are counted and excluded by v2's
  key-quality reporting.
- Instrumentation perturbs timing; only the change and correlation
  classifications are meaningful.

## Roles (for re-pinning without the script)

| Role | Contract |
|---|---|
| Orientation constructor, reverse composition order | entry to hook; receives the angle pointer and the slot |
| Orientation constructor, forward composition order | same contract, opposite rotation order |
| Angle triplet | three consecutive 32-bit floats at the first argument |
| Pose slot | destination at the second argument; thirteen floats, rotation first |
| Produced angle | `atan2` of the matching rotation pair, or the trace fallback |
| Candidate | one of `src0`, `src1`, `src2`, `atan2_01`, `atan2_02`, `atan2_12` |
| Correlation number | mean absolute wrapped delta between `theta` and a candidate |

No addresses or module-relative numbers are recorded here. The probe carries
them and lives outside the repository, as the `runtime-capture` README explains.

## Checklist for review

- [ ] The four candidate interpretations are stated, and each has a falsifiable
      prediction.
- [ ] `theta` is defined for pure Z and for the trace fallback, with `axis`.
- [ ] All six candidates and the wrapped mean-delta formula are defined.
- [ ] The decision table maps every `best` value to a model and a next action.
- [ ] `none` is not read as independence until offset and scale are tested.
- [ ] The procedure has two phases and includes the realm-shift segment.
- [ ] "What the experiment cannot establish" is present and specific.
- [ ] No addresses, RVAs, disassembly, game bytes or absolute install paths
      appear in the committed diff.
- [ ] `cargo test` stays at its baseline; this change touches no production code.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, module-relative offset, disassembly listing or absolute install
path appears here. Roles, contracts, formulas and measured invariants only.

# axis-order-plan - capture the constructor input and output atomically (2026-11-12)

The three-angle rotation constructors are now known to produce the pose
rotation. What is still open is the **application order**: in which sequence the
three scalar angles become the 3x3 block. The previous probe could not answer
it, and its failure was mechanical, not mathematical: it read the input triplet
when the constructor was entered and the produced slot at dump time, so the two
halves of every pair came from different moments.

This slice plans a **v4 atomic probe** that reads the returned slot inside the
same call that supplied the input, and an **offline fit tool** that tests a
closed family of 192 rotation models and returns one of three auditable
verdicts: `UNIQUE`, `AMBIGUOUS`, `NO-FIT`.

This is a **preparation** slice. It commits this plan only. The probe and the
fit tool carry no repository material and stay in the session scratch directory.
Nothing was attached, launched or instrumented for this document.

## Quick path

1. Read "The bug" and "Evidence" - why the previous 0/27 was unreadable.
2. Read "The atomic capture" - the one mechanical change that makes the question
   answerable.
3. Read "The 192-model search" - the closed model family and the verdict rules.
4. Read "Three-step human procedure" to run it, and "What it cannot establish"
   before writing any conclusion.

## The bug

v3 paired the two halves of a sample like this:

```text
onEnter:  read 3 floats at the input pointer   -> in=(a,b,c)      (60 Hz)
... the constructor runs, the slot is written ...
buildDump (4 Hz):  read 13 floats at dest      -> f13=[...]
```

The input came from hook time; the destination was read up to a quarter of a
second later, from whatever the slot contained by then. Every recorded pair is
a mixture of two frames.

That mixture is invisible exactly where v3 was used:

| Slot behaviour | Effect of the skew |
|---|---|
| Slow single-axis slot (a straight Z yaw) | The angle moves little between the two reads; the pair still looks consistent |
| Fast 3-axis slot | The rotation changes materially between the reads; the pair is arithmetic nonsense |

This is why the earlier Z correlation looked convincing while the three-axis
Euler search produced nothing. The tool was not wrong about the Euler models; it
was fed pairs that no single model could explain.

## Evidence

Both captured windows were re-parsed and re-fitted for this plan.

| Observation | Window 1 | Window 2 | Reading |
|---|---|---|---|
| Pair lines present | 262 | 262 | One line per sampled slot per dump |
| Rows usable after parsing | 220 | 262 | Window 1 lost 42 lines to non-finite floats |
| Z-only rows with a non-zero third input | 68 | 96 | Comparable, single-axis rows |
| ... of those, produced angle within 0.01 rad of the raw scaled input | 66 | 94 | The Z fit is real and reproducible |
| Rows where all three inputs exceed 1.0 in magnitude | 27 combined | | The only rows that can separate orders |
| ... of those, best model within 1e-3 on every matrix element | 0 combined | | 0 of 27 |
| Best mean elementwise error over the 27 rows | about 0.88 | | Large; no model is close |

The asymmetry is the whole story. **66/68 and 94/96** is what a correct
single-axis model looks like. **0/27** is what a pair with two timestamps looks
like, regardless of the model.

Zero hits is therefore **not** a refutation of all 192 Euler models. It is the
expected output of the broken instrument applied to the only data that could
have decided the question.

## Why v3 cannot answer the 3-axis question

Three independent reasons, in order of importance:

1. **The pairs are skewed.** Covered above. No offline transform can undo a
   skew whose size is unknown per row.
2. **The usable rows are order-blind.** A pure Z, X or Y rotation is a product
   with two identity factors; every ordering of identities agrees. Single-axis
   rows can confirm the scale and sign of one factor, and nothing about sequence.
   These are the rows that scored 66/68 and 94/96, and they cannot separate the
   six orders by construction.
3. **The order-carrying rows are the fast ones.** Three-axis rows are exactly
   the rows the skew destroys. v3 has 27 of them and every one is untrustworthy,
   so raising the sample count or the dump rate cannot recover the information.

Consequence: the 3-axis question must be re-observed, not re-analysed.

## The atomic capture (v4)

### Capture point

The constructor returns the slot it filled. v4 reads that returned slot in the
return handler, on the same invocation that already read the input triplet:

```text
onEnter:  read 3 floats at the input pointer, stash {dest, triplet} on the call
onLeave:  read 13 floats at the RETURNED slot, store them on that record
```

One call, one pair. A marker line proves the capture point is the return, not
the dump:

```text
ANGLE ATOMIC ok=1 pairs=<n> captured_in_leave=1
```

If the return value is unusable, the recorded destination is used and a mismatch
counter is incremented; the pair is still from the same call. A slot whose
thirteen floats are not all finite is counted and kept out of the ring rather
than poisoning the fit.

### Bounded pair ring and dedupe

| Property | Value | Why |
|---|---|---|
| Storage | ring of `--max-pairs` records, default 4096 | A fixed upper bound on memory |
| Record | destination, triplet, thirteen floats, capture sequence, 3-axis flag | One record is one call |
| Dedupe key | (destination, triplet hash, slot hash) | A steady frame contributes one record, not one per tick |
| Eviction | oldest record, its key removed with it | The key set can never exceed the ring |
| Short-circuit | a repeated key is skipped before allocation | Bounded work per call |

### Emission

At the dump rate (default 4 Hz) the probe emits up to `--dump-pairs` pair lines
(default 8), choosing in this order:

1. rows where all three input components exceed `--axis-min` (default 1.0);
2. then the newest captures;
3. never a pair already emitted.

```text
ANGLE PAIR3 n=<dump> dest=<slot> in=(a,b,c) f13=[13 floats] axis=<z|x|y|none>
```

The legacy `ANGLE PAIR` line is still emitted for the sampled slots, built from
the same cached floats, so earlier reading habits keep working. It no longer
performs a read of its own.

### Cost

| Aspect | v3 | v4 |
|---|---|---|
| Input reads per call | 3 | 3 |
| Destination reads per call | 0 | 13 (one 52-byte read) |
| Destination reads per dump | 13 per sampled slot | 0 |
| Per-call text | none | none |
| Persistent memory | one row per distinct destination | bounded ring, default 4096 records |

The destination read moved from the dump loop into the call. That is the entire
cost of the fix, and it is paid once per call instead of once per sampled slot.

Kept unchanged: destination-pointer keying, the ABI calibration lines, the
rotation sanity check, the script-fault lines, the alignment guard, the animate
/ static / unobserved verdict, `--selftest`, `--dry-run`, and `--print-agent`.

## The 192-model search

The fit tool consumes `ANGLE PAIR3` lines (and v3 `ANGLE PAIR` lines, so old
logs remain comparable) and tests a **closed** family. Closed means the family
is enumerated in advance and the verdict is a function of that enumeration; no
model is invented after seeing the data.

### Model definition

Each model is the tuple:

| Dimension | Values | Count |
|---|---|---|
| Application order | XYZ, XZY, YXZ, YZX, ZXY, ZYX | 6 |
| Sign per component | each of the three may be negated | 8 |
| Composition | forward product or reverse product of the same three factors | 2 |
| Matrix layout | transpose or no transpose | 2 |
| **Total** | | **192** |

For a row with input `(a, b, c)`, each component becomes an angle

```text
angle_i = sign_i * value_i * (2 * PI / 4096)
```

and the model forms the standard row-major right-handed single-axis matrices,
multiplies them in the selected order, and optionally transposes the result.
The row is a **hit** when the maximum absolute elementwise difference over the
nine rotation entries is below 1e-3.

### Row selection

Only rows where all three inputs exceed the magnitude threshold are fitted
(default 1.0). Single-axis rows are counted and excluded, because they cannot
separate orders. Duplicate observations - the same destination, triplet and
slot - are collapsed before scoring so a repeated frame cannot double a hit
count.

### Verdict rules

| Verdict | Condition | Meaning |
|---|---|---|
| `UNIQUE` | Every selected row is explained by exactly one equivalence class of models | The order, signs and layout are identified up to provably identical parameterizations |
| `AMBIGUOUS` | Two or more distinct model classes tie on the highest hit count | The evidence does not separate those classes; collect 3-axis rows |
| `NO-FIT` | No model reaches the tolerance on any selected row | The family does not contain the transform; move to the next hypothesis classes |

**Equivalence is decided on a fixed generic probe, never on the selected rows.**
Some parameterizations are the same map; for example, the transpose of a forward
product equals a reverse product with negated signs. Those must be grouped. But
models that merely agree on a single-axis dataset are *not* the same map -
collapsing them would hide an ambiguous window behind a false `UNIQUE`. This
distinction is what makes `AMBIGUOUS` an honest answer rather than a bug.

The tool prints, for every model it shows: rank, order, signs, composition,
transpose, hits over selected rows, mean error, maximum error. It also prints
the parse and rejection counts, so a low row count is never mistaken for a
negative result.

### Tool self-test

The fit tool has its own `--selftest`. It builds a synthetic matrix from a known
order, sign set and layout and asserts that the search recovers exactly that
model's equivalence class and nothing outside it. It also asserts that a
single-axis dataset is reported `AMBIGUOUS` rather than `UNIQUE`, that a matrix
outside the family is `NO-FIT`, and that both line shapes parse with rejections
attributed to a reason.

## If NO-FIT: the next hypothesis classes

`NO-FIT` on clean atomic data is informative: it removes the entire standard
Euler family. The next classes, named here so they are tested rather than
guessed, are:

| Class | What would distinguish it |
|---|---|
| Non-Euler composition | The product carries an extra pre- or post-multiplied basis change, or a fourth factor. The matrix would still be a rotation but no three-factor order would reproduce it. Test by fitting a free basis change on both sides. |
| Per-axis arbitrary axes | Each factor rotates about a fixed unit axis that is not a coordinate axis. Three arbitrary axes add six free parameters; the matrix stays orthogonal and no coordinate order explains it. Test by solving for the three axes from several rows. |
| Quaternion from axis-angle | The constructor builds a quaternion from one or more angles and converts it once, so the compound matrix is not a product of per-axis matrices in any order. Test by fitting quaternion composition and swing-twist splits. |
| Scale or offset per component | The fixed-point scale or a per-component offset differs from the assumed one. Not a new composition, but it breaks every model equally; test by allowing one free scale and one free offset per component before declaring a family dead. |

Each class has a falsifiable prediction and a distinct fingerprint; none is
claimed here.

## Three-step human procedure

### 1. Validate without attaching

From the session scratch directory, run the probe's `--selftest` and
`--dry-run`, then the fit tool's `--selftest`. All three must pass. `--dry-run`
prints the capture point, the ring and emission rules, the axis threshold and
the verdicts, and imports nothing. Confirm the fit tool still reproduces the
historical result on the older logs.

### 2. Capture with the atomic probe

Attach, wait for `READY` and the calibration lines, and confirm the marker line
`ANGLE ATOMIC ok=1 pairs=<n> captured_in_leave=1` appears before any pair line.
Then exercise the body path so that **many slots rotate on more than one axis at
once**: walk, turn while walking, and strafe while turning. Watch for `PAIR3`
lines whose `axis` is not a single letter; those are the rows that carry order
information. Stop with Ctrl+C so the summary is written before detaching.

### 3. Fit offline and read the verdict

Run the fit tool over the new log and over the previous logs for comparison.
Read the parse and rejection counts first, then the selected row count, then the
verdict. A verdict is adopted only after a second run reproduces it. If the
verdict is `AMBIGUOUS`, collect more 3-axis rows; if `NO-FIT`, move to the next
hypothesis class above rather than widening the tolerance.

## What the experiment cannot establish

- **Sign versus transpose.** Some parameterizations are the same map by
  construction, and some datasets cannot separate the remaining classes. The
  verdict reports that as `AMBIGUOUS`; it does not resolve it. A unique result
  identifies an equivalence class, not a source-level operation order.
- **The engine's code path.** The fit compares produced matrices against a model
  family. It shows which composition reproduces the output, not which statement
  the engine executed.
- **One build, one window.** The result is bounded by the shipped build, the
  observed actor, the observed slots and the captured interval.
- **Sampled slots only.** The ring is bounded and the dump emits a bounded
  number of pairs; slots that never fire are never covered.
- **Unit truth.** The fixed-point scale is assumed, and a wrong scale breaks
  every model equally. A `NO-FIT` must be re-read after the free-scale test.
- **Causality.** Agreement between an input and an output is not proof that the
  constructor reads those exact bytes as the angle.
- **Timing and frame rate.** Instrumentation perturbs both; they are not
  measurable from this data.

## Review checklist

- [ ] The non-atomic capture and its two-timestamp shape are stated plainly.
- [ ] The evidence separates the good single-axis fit from the unusable
      three-axis rows, and does not call 0/27 a refutation.
- [ ] The atomic capture point is named, and the marker line proves it.
- [ ] The ring, the dedupe key and the emission preference are specified.
- [ ] The 192-model family is closed and its dimensions and counts add up.
- [ ] The hit criterion, the row selection and the equivalence rule are stated.
- [ ] All three verdicts have conditions and meanings, and `AMBIGUOUS` is not
      treated as a bug.
- [ ] The next hypothesis classes are named and each has a distinguishing test.
- [ ] The human procedure has exactly three steps and no game is run here.
- [ ] No addresses, module offsets, disassembly, game bytes or absolute install
      paths appear in the committed diff.
- [ ] `cargo test` stays at its baseline; this change touches no production code.

## Legal

Read-only preparation. No game process is launched or instrumented by this
slice. No game file is written, no game bytes are copied into the repository,
and no address, module-relative offset, disassembly listing or absolute install
path appears here. Roles, contracts, formulas and measured invariants only.

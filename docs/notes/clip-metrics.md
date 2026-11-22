# clip-metrics - offline motion metrics rank clips, they do not name them

`--clip-metrics` decodes every clip of an `.SRA` offline, measures how its bones
move and ranks a shortlist of candidates for `idle`, `walk`, `jump_up`, `fall`
and `land`. It exists so the game is used only to confirm two or three
candidates instead of discovering a whole actor from scratch.

The decisive result is the **validation gate**: on the two files with
runtime-measured labels it passes **2 of 7**. The classifier is therefore **not
validated** and must not be treated as ground truth. It still prints a
measurement table and a `heuristic` shortlist; nothing it prints is a label.

```text
CLASSIFIER VALIDATION pass=2/7 heuristic
RAZIEL.SRA   idle=55  walk=2  jump_up=35  fall=36  land=37
SLUAGH.SRA   idle=8   walk=1
```

## Command

```text
--clip-metrics --sra <FILE.SRA> [--slots <N>] [--bigfile <DAT> --entry <INDEX>] [--top <N>]
--clip-metrics --sra-dir <DIR>  [--slots <N>] [--top <N>]
--clip-metrics --sra-dir <DIR>  --validate [--top <N>]
```

`--bigfile <DAT> --entry <INDEX>` loads the archived object model for the same
actor and derives the `leg`/`hip`/`upper` regions from that model's real bone
tree and rest offsets. The model's bone count becomes the slot count. Both flags
must be given together, and they apply only to one `--sra` file.

`--slots <N>` is the model bone count stated directly. When it is omitted (and
no `--bigfile/--entry` model is loaded) the tool tries `1..=128` and keeps the
count that decodes the most clips, preferring the larger count on a tie. That
inference is a **best-effort default**: an `.SRA` has no slot-count field and a
trailing empty slot is indistinguishable from a real one, so it can over-count.
Pass `--slots` when the model bone count is known. The validation gate always
passes explicit counts.

`--validate` scores the shortlist against the runtime-measured fixtures in
`data/clip-labels.json` (see `docs/notes/clip-label-store.md`). Those fixtures are
**expectations only**; they never feed the classifier and are never written.

## Metrics

Every metric is computed from decoded poses: the packed angle lanes, plus the
packed translation lanes the angle-only presentation path discards. No mesh and
no window are needed. The optional `--bigfile/--entry` skeleton basis reads the
archived object model's bone table; without it the region columns are the proxy
described below.

| Field | Meaning |
|---|---|
| `frames`, `divisor` | The table's frame count and clock divisor. |
| `dur` | `frames * divisor / 5000` seconds (`ANIMATION_UNITS_PER_SECOND`). |
| `varc` | `max - min` of the root slot's decoded world Z translation. |
| `hdisp` | The larger of the root slot's `max - min` X and Y translations. |
| `ang_mean` | Mean per-frame total rotation delta across all slots, in degrees. |
| `ang_max` | Largest single-frame total rotation delta, in degrees. |
| `period_lag` | Best autocorrelation lag over `1..=frames/2` (`0` when `frames < 2`). |
| `score` | `1 / (1 + residual_degrees)` at that lag (`1.0` = exact repeat). |
| `leg`, `hip`, `upper` | Share of total per-bone motion in each region (see below). |

Every angle delta is wrapped into `[-half_turn, half_turn]` before it is summed,
so a stored angle crossing the turn boundary never becomes a full-turn spike.

### Root slot

The archived descriptor rest translations are **not** in `.SRA`; only authored
translation overrides are. The root slot is therefore the lowest-index slot that
animates any translation lane in any frame, falling back to slot `0`. On the
measured bodies this is the pelvis, whose translation carries the body's vertical
and horizontal travel. `varc` and `hdisp` are ranges, not absolute values, so a
constant rest height contributes nothing.

### Periodicity

`score` is a normalised autocorrelation over the flattened angle window. Lag `1`
is allowed and means adjacent frames repeat; a perfectly still clip scores `1.0`
at lag `1`. Lags beyond `frames/2` are not searched, so a nearly full-length lag
cannot win on a handful of samples.

### Regions: archived skeleton, proxy fallback

The `.SRA` carries no parent tree or rest offsets, but the archived object model
does. With `--bigfile/--entry` the tool loads that model, walks each bone's
`+18` parent word (`0xFFFF` is a root) and accumulates the rest `local`
translations, exactly like `object::bone_world_offsets`. Slot `i` is bone `i`;
the animation slot count and the model bone count must match exactly.

Each bone's region comes from its rest position, never its slot index:

- **leg**: at or below the lower vertical band, close enough to the axis.
- **hip**: between the vertical bands, close to the axis (pelvis and torso).
- **upper**: any bone whose horizontal radius is at least the skeleton's lateral
  threshold (arms and hands, at any height), or any bone above the upper vertical
  band that stays close to the axis (neck and head).

The bands are fractions of the skeleton's own rest `Z` extent: `low_z = min +
0.4 * (max - min)` and `high_z = min + 0.7 * (max - min)`. The lateral threshold
is `0.95 * median bone radius`, so a bone sitting on the median counts as far
instead of splitting on floating-point noise. All three numbers are relative to
the model, so no absolute world unit is hardcoded, and the output prints the
derived values plus every bone's `rest_z` and lateral radius for inspection.

Per-bone motion is the per-frame angular delta (in degrees) plus the per-frame
translation displacement; the region share is its sum over the whole clip
divided by the total. This is why the shares differ from the old proxy.

Without `--bigfile/--entry` the old **proxy** remains: lowest slot third `leg`,
middle third `hip`, highest third `upper`, angular motion only. The header says
`region=proxy`. If a model is supplied but its bone count disagrees with the
animation slot count, the tool prints
`region=unavailable reason=bone-slot-mismatch slots=<n> bones=<m>` and uses the
proxy for that file. The proxy is never labelled as a skeleton measurement.

The proxy was wrong because slot order is not body order. `HUNTER.SRA` clip 2 is
a slow idle where the head and the hanging arms move; the proxy put every moving
slot in its middle third and printed `leg 0.00 hip 1.00 upper 0.00`. The
skeleton assigns those slots to `upper` from their real rest positions and
prints `upper 1.00`.

The region basis is a measurement column only. It does not feed the state
ranking or the validation gate.

## Ranking

Each state has an eligibility filter and a total-order comparator. Ties always
break on the clip index, so the same file produces the same table on every run.

| State | Eligible when | Best first |
|---|---|---|
| `idle` | `frames >= 4` and `ang_mean <= 200` | lowest degrees/second, then `hdisp`, index |
| `walk` | `frames >= 4` and `hdisp >= 10` | highest `hdisp`, then `ang_mean`, index |
| `jump_up` | `frames <= 3` | highest `ang_mean`, then `varc`, `hdisp`, index |
| `fall` | `frames <= 2` | lowest `ang_mean`, then `hdisp`, `varc`, index |
| `land` | `frames <= 3` and `varc >= 1` | highest `varc`, then `ang_mean`, index |

The numeric ceilings (`200`, `10`, `1`) are **arbitrary** boundaries chosen from
the observed bands; they are not engine constants. `idle` uses degrees per
second rather than raw mean because a short busy clip and a long slow loop have
very different sustained speeds.

If two or more of `jump_up`, `fall` and `land` collapse onto the same top clip,
the metrics cannot split that clip into phases and all three states print `-`.
`SHORTLIST` prints the top `--top` candidates per state, with the idle candidate's
duration in seconds. Every row and every guess is labelled `heuristic`.

## Validation gate result

Running `--clip-metrics --sra-dir <OBJECT> --validate` on the shipped files
prints:

```text
CLASSIFIER VALIDATION pass=2/7 heuristic
state=idle    expected=55 top=98  result=fail (expected clip 55 is rank 41)
state=walk    expected=2  top=93  result=fail (expected clip 2 is rank 75)
state=jump_up expected=35 top=35  result=pass
state=fall    expected=36 top=6   result=fail (expected clip 36 is rank 6)
state=land    expected=37 top=122 result=fail (expected clip 37 is rank 4)
state=idle    expected=8  top=8   result=pass
state=walk    expected=1  top=17  result=fail (expected clip 1 is rank 5)
```

Only `RAZIEL.jump_up = 35` and `SLUAGH.idle = 8` are the top candidate.

The misses have concrete, structural reasons:

- **Idle and walk are not unique motion classes.** `RAZIEL.SRA` has 230 clips.
  Many are shorter, slower loops than the runtime-selected idle, and its own
  idle has more than one table (`clip 0` was a recorded variant of `55`). A
  motion table cannot know which of several near-identical loops the engine
  picked for a given state.
- **`fall = 36` is one of 33 one-frame clips.** A one-frame table has no
  consecutive pair, so its motion is zero by construction. The metric that
  "should" find the fall (lowest motion) cannot separate `36` from the other 32
  zero-motion one-frame clips; it ties and breaks on index.
- **Walk is a set, not a point.** Several clips travel horizontally and several
  loop; the runtime pick is one member of that set. The ranking names the
  strongest traveller, not the engine's selection.

Because the gate does not pass, the shortlist is a **candidate generator**, not
an identifier. Confirm every candidate in game before using it.

## First run: HUNTER.SRA

`HUNTER.SRA` has 44 slots and 10 clips. The table is reproduced from decoded
poses (values are model-space units and degrees; the file carries no labels):

| clip | frames | divisor | dur s | varc | hdisp | ang_mean | ang_max | period_lag | score | leg | hip | upper |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 10 | 222 | 0.444 | 0.0 | 60.1 | 445.1 | 579.3 | 1 | 0.23 | 0.32 | 0.07 | 0.61 |
| 1 | 7 | 716 | 1.002 | 15.0 | 0.0 | 118.6 | 172.2 | 1 | 0.53 | 0.16 | 0.27 | 0.57 |
| 2 | 14 | 907 | 2.540 | 1.7 | 2.5 | 131.1 | 236.7 | 1 | 0.50 | 0.00 | 1.00 | 0.00 |
| 3 | 6 | 346 | 0.415 | 0.0 | 11.4 | 171.1 | 210.6 | 1 | 0.44 | 0.00 | 1.00 | 0.00 |
| 4 | 7 | 350 | 0.490 | 1.3 | 7.5 | 117.1 | 218.3 | 1 | 0.53 | 0.00 | 1.00 | 0.00 |
| 5 | 9 | 212 | 0.382 | 0.0 | 212.1 | 461.5 | 884.5 | 1 | 0.22 | 0.58 | 0.42 | 0.00 |
| 6 | 15 | 100 | 0.300 | 105.0 | 163.9 | 244.4 | 417.9 | 1 | 0.35 | 0.63 | 0.37 | 0.00 |
| 7 | 14 | 138 | 0.386 | 0.0 | 275.1 | 451.8 | 717.8 | 1 | 0.23 | 0.69 | 0.31 | 0.00 |
| 8 | 2 | 900 | 0.360 | 31.3 | 11.7 | 505.5 | 505.5 | 1 | 0.21 | 0.48 | 0.52 | 0.00 |
| 9 | 5 | 100 | 0.100 | 45.9 | 0.0 | 69.3 | 88.9 | 1 | 0.66 | 0.85 | 0.15 | 0.00 |

```text
SHORTLIST idle=2(2.540s),1,4,3,9 walk=7,5,6,0,3 jump_up=- fall=- land=-
```

- **Idle: clip 2.** It is a 2.540 s, 14-frame clip with the lowest sustained
  angular speed among eligible loops (`131.1` deg/frame over `2.540 s`) and
  almost no root travel (`varc 1.7`, `hdisp 2.5`). Clip 1 is the next longest
  slow loop.
- **Walk: clip 7.** It has the largest root horizontal travel (`hdisp 275.1`)
  with strong leg-region motion (`leg 0.69`) at `451.8` deg/frame over 14
  frames. Clip 5 is the next traveller (`hdisp 212.1`, `leg 0.58`), then 6, 0
  and 3.
- **Air states: empty.** Clip 8 is the only short travelling clip and would be
  the top for `jump_up`, `fall` and `land` at once. One two-frame clip cannot be
  split into three phases, so the metrics leave all three `-` rather than name
  the same clip three times.

These are **leading candidates**, not labels. The validation gate above shows
that motion ranking alone does not reproduce the runtime's clip choice, so every
HUNTER candidate still needs in-game confirmation.

## Limits

- Metrics rank; they do not name. A shortlist member is a candidate to confirm,
  never a measured identity.
- The `.SRA` has no clip id, state byte, slot count, parent tree or rest offsets.
  The root slot is a translation-lane heuristic. Regions are skeleton-derived
  only when `--bigfile/--entry` is supplied; otherwise the output says
  `region=proxy` and uses slot thirds.
- The skeleton fixes what the region columns mean. It does **not** validate the
  classifier: the gate stayed at `2/7` after connecting it, because near-identical
  loops and the 33 identical one-frame zero-motion clips remain ambiguous.
- One-frame clips are all "still" by construction; they cannot carry motion
  semantics.
- Several near-identical loops can share one motion profile; the runtime-selected
  variant is not recoverable from motion alone.
- The thresholds are explicit and, where noted, arbitrary. Changing them changes
  the ranking; the validation gate is the guard against claiming more than the
  data supports.

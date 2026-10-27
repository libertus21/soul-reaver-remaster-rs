# pcm-content-census - block content at the correct data base

Read-only census of `.PCM` block **content** at the verified data base
(PR #47, `data_base()` in `src/level.rs`). No game bytes, absolute install
paths, coordinates, or decompiled code are recorded here. Scratch scripts lived
outside the repo and printed counts and ratios only.
Date: 2026-10-27. Box: 2 passes (census + stride/integer probes), within the
90-minute budget. Evidence class per row: **Observed** (measured here),
**Derived** (arithmetic on measured values), **Inferred** (interpretation, not
proven), **Unknown** (not established).

## Verdict

- **Invalidation (explicit):** the content conclusions in
  `docs/notes/room-geometry.md` (P14 12-byte `[u32][f32][u32]` group, P15 f32
  field, P16 leading-tag reading, and the placement lead built on them) were
  computed on the superseded `4+4N` base and are **INVALID**. Their f32 and
  group-semantics claims are **not** reproduced on the correct base (see below).
- **Surviving signal:** on the correct base, 48-byte blocks show a u16 equality
  period at lag 6 (12 bytes) at 20-34% in every probed unit. This is **not** an
  artefact of the old misalignment, so the 12-byte **stride** observation
  survives. Its **field semantics remain Unknown.**
- **Geometry:** still **NO-GO for decoding**. A concrete falsifiable probe
  exists for the surviving lag-6 period (below); no layout is proven.
- **Placement:** **NO-GO** in this change. The `[u32 id][f32][u32 flags]`
  transform reading is refuted; no replacement test passes. Community sources
  confirm room files hold object placements, which keeps placement an open
  question, not a viable slice.

## Layout check (correct base, Observed)

`data_base = 4 * roundup512(N + 1)`, base = 2048 for N < 512, 8192 for LAIR1
(N=1655), 10240 for MOVIE3 (N=2256). Offset table ends before the base in all
five units; block sizes are offset differences and sum to the data region.

## Per-unit content census (Observed)

Block sizes are offset differences and do **not** depend on the base, so the
size-class counts below are base-invariant. Byte content does depend on it.

| Unit | N | Zero blocks | Top sizes (count) | Zero words | Words in -40000..40000 | Words in 0..N | Blocks all small | Blocks all 0..N |
|---|---|---|---|---|---|---|---|---|
| LAIR5 | 62 | 0 / 62 | 4 (26), 40 (11), 48 (11) | 7.4% | 24.0% | 13.3% | 33 / 62 | 0 |
| LAIR14 | 74 | 0 / 74 | 4 (30), 40 (15), 48 (15) | 7.1% | 22.7% | 12.7% | 37 / 74 | 0 |
| CATHY44 | 326 | 0 / 326 | 4 (121), 40 (91), 48 (91) | 3.5% | 19.8% | 10.4% | 133 / 326 | 0 |
| LAIR1 | 1655 | 0 / 1655 | 4 (559), 40 (519), 48 (519) | 1.7% | 10.8% | 6.0% | 12 / 1655 | 5 |
| MOVIE3 | 2256 | 0 / 2256 | 48 (696), 4 (674), 40 (552), 76 (139), 72 (118) | 9.6% | 19.7% | 15.3% | 216 / 2256 | 34 |

Reading: no unit has an all-zero block. The "small integer" ratios are well
above chance (a uniform random u32 falls in -40000..40000 with probability
about 2e-5), so the words are not random noise. They are mixed with other
content, so a ratio is a plausibility figure, **not** a type label.

## Stride and column test (Observed)

Per-column statistics over same-size blocks (nonzero only):

- **40-byte blocks** (LAIR5 n=11, CATHY44 n=91, MOVIE3 n=552): no column is
  constant. Column 0 is small in every unit (for example 556..5128 in LAIR5),
  but it is not monotone.
- **48-byte blocks** (LAIR5 n=11, CATHY44 n=91, MOVIE3 n=696): column 0 is
  **strictly increasing with block index** in 100% of consecutive rows. This
  is an ordering fact, not yet a field meaning. It is **not** the block index
  itself (0% equal) and not an index into the unit's own N (0%).
- Other 48-byte columns are not constant and mostly outside the small range.
- Pointer-like check: in MOVIE3 48-blocks, 19% of column-0 values equal a
  relative block offset of the same unit (CATHY44 1%, LAIR5 0%). Observed
  only; **Unknown** whether this is a pointer. Next probe below.

## u16 lag test (Observed; this is the surviving "12-byte" signal)

Equality rate of u16 pairs at lag L, zero-zero pairs excluded:

| Unit | Block size | L1 | L2 | L3 | L4 | **L6** | L8 | L12 |
|---|---|---|---|---|---|---|---|---|
| LAIR5 | 48 | 0% | 0% | 1% | 0% | **20%** | 0% | 0% |
| LAIR14 | 48 | 0% | 0% | 12% | 3% | **27%** | 0% | 4% |
| CATHY44 | 48 | 0% | 0% | 2% | 1% | **34%** | 0% | 1% |
| LAIR1 | 48 | 0% | 0% | 2% | 1% | **30%** | 0% | 1% |
| MOVIE3 | 48 | 0% | 0% | 1% | 0% | **24%** | 0% | 0% |
| LAIR5 | 40 | 0% | 2% | 0% | 0% | 4% | 2% | 0% |
| CATHY44 | 40 | 0% | 5% | 0% | 0% | 1% | 9% | 0% |

A uniform random u16 pair matches with probability about 1.5e-5 (Derived), so
20-34% is far above chance. The lag-6 period is **specific to 48-byte blocks**;
40-byte blocks show no lag-6 period. What the period encodes is **Unknown**: it
could be a 6-column record, repeated values, or a shared constant pattern.

## f32 plausibility (Observed + Derived; the old f32 claim does NOT survive)

- Uniform random 32-bit patterns read as f32: 99.6% finite, and 54.5% have
  `|f| < 5000` (measured on 300,000 samples).
- The 48-byte blocks measure 51% (CATHY44), 60% (LAIR1), 74% (LAIR5), 83%
  (MOVIE3), 91% (LAIR14). The `|f| < 5000` criterion therefore sits **at or
  near chance** for most units, so it cannot separate a scalar field from
  integer or text words. The previously reported "44 of 106 small" f32 values
  carry no evidential weight.

## Integer-plausibility of 4-byte blocks (Observed)

- LAIR5 (26 blocks), LAIR14 (30), CATHY44 (121): **100%** of 4-byte blocks hold a
  value in -40000..40000 (distinct 23, 27, 117).
- LAIR1 (559): 1% small (top value repeated 5 times, large). MOVIE3 (674):
  13% small.
- The unit dependence means the 4-byte content is not a single type across the
  game. No type or name is assigned.

## Falsifiable tests and their outcomes

| Test | Result | Verdict |
|---|---|---|
| Is some block-level content all zero? | No (0 zero blocks in 5 units) | Observed |
| Does a 40- or 48-byte block show a column that is constant across records? | No | Observed |
| Does column 0 of 48-blocks equal the block index? | 0% | Refuted |
| Does column 0 of 48-blocks fall in 0..N? | 0% | Refuted |
| Does column 0 of 48-blocks equal a block offset? | MOVIE3 19%, CATHY44 1%, LAIR5 0% | Open, unit-dependent |
| Does lag-6 u16 equality exceed chance on the correct base? | Yes, 20-34% in 48-blocks | Surviving observation |
| Is the f32 `|f| < 5000` criterion evidence of a scalar field? | No: at chance (54.5%) | Refuted as evidence |

## Re-evaluated no-gos

**Geometry (room mesh, vertices, faces): NO-GO for decoding.**
- Evidence class for the surviving lag-6 period: Observed. Its meaning is
  Unknown.
- Concrete next probe (falsifiable): for 48-byte blocks, split each block into
  4 records of 6 u16 and test whether each column is monotone, bounded, or
  delta-coherent across consecutive records. Pass only if at least one column
  shows a cross-unit consistent range that is not explained by chance. Fail if
  the period appears only in one unit.
- This is a **probe**, not a viable decode. No mesh count or vertex layout
  is claimed.

**Placement (object transforms): NO-GO in this change.**
- The `[u32 id][f32][u32 flags]` group and its f32 transform reading are
  **refuted** on the correct base (f32 at chance; the group decomposition used
  the misaligned base).
- Community sources (see below) state that room scenes display object
  placements, so placement is a real open question. No surviving test on our
  data supports a placement table yet.
- Next probe (falsifiable): test whether column 0 of 48-blocks is a pointer to
  the block at that offset (follow it and compare the target block's size
  class). Pass only if the match rate is well above the 1-19% observed and holds
  across units.

## Community research (bounded, Observed sources)

- **Recombobulator** (TheSerioliOfNosgoth): `https://github.com/TheSerioliOfNosgoth/Recombobulator`.
  README: "A tool for analysing the contents of room and object files from
  Legacy of Kain: Soul Reaver"; notes that some rooms contain object lists and
  intros. **No format spec in the README.** License: **GPL-3.0** (LICENSE
  fetched from GitHub raw). Behaviour reference only; not vendored.
- **ModelEx** (TheSerioliOfNosgoth): `https://github.com/TheSerioliOfNosgoth/ModelEx`.
  README: "a Scene with all object placements displayed", and it expects
  `.drm` and `.pcm` files. **No format spec.** LICENSE is a mixed bundle
  (third-party notices and a GPL-3.0 text attributed to Only Skin Deep); the
  top-level license for ModelEx itself is **not established** from the fetched
  file. Treat as a reference only.
- The general web search returned no SR1 `.PCM` format specification.
- GitHub raw fetches for the two repos above succeeded (HTTP 200). The
  `github.com/libertus21/soul-reaver-remaster-rs` page returned **HTTP 404**
  for an unauthenticated request; not relied on.
- Other repos noted in `docs/notes/02-community-tools.md` (KAIN2, soul-re,
  Soul Spiral) were not re-fetched in this pass.

## Conflicts and risks

- `docs/notes/pcm-loader.md` (section "Consequences") states that "the 12-byte
  refutation stands". This census finds the lag-6 period **present** on the
  correct base, so that sentence is **contradicted** and must be re-checked. It
  was not edited here.
- `docs/notes/room-geometry.md` carries the invalid content claims. A banner was
  added pointing to this note. Its size-class census (`fixed48 = fixed40 + 1`)
  is base-invariant and stands.
- Stride and integer findings are plausibility-level, not proofs.

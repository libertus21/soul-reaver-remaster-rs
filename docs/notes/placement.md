# placement — what the 40/48-byte `.PCM` blocks actually encode

Read-only probe of the SR1 remaster `<install>/1/AREA` tree. No game bytes,
addresses, coordinates, or absolute install paths are recorded here. Date:
2026-10-27. Box: 3 passes, ~120 minutes, SR1 only. Scratch stayed outside the
repo and outside the game tree.

## Question

`room-geometry` recorded a later, single-unit observation: 40/48-byte blocks
looked like 12-byte `[u32 id-ish][f32 value][u32 flags]` groups, with a `0x34`
high byte on the leading word. That was the only lead toward placement or
transforms. This probe asks one narrower question: **does that layout hold
across the corpus, or was it a local coincidence?**

A documented no-go is an acceptable answer. It is the answer the corpus
supports.

## Method

Every `.PCM` was re-parsed with the shipped header model. Blocks are the gaps
between adjacent relative offsets. For all non-zero 40/48-byte blocks the probe
measured:

- absolute byte-position entropy across the corpus;
- within-block and cross-block constancy per byte phase;
- candidate strides 4/8/12/16/20/24;
- the `0x34` high-byte marker by word index and by phase;
- the f32 reading of every word (finite, subnormal, magnitude band);
- the joint `[marker][value][small flags]` rate per unit;
- per-unit name counts against 40/48 block counts.

All figures are corpus ratios over 89 units and 46,124 blocks.

## Verdict

### 1. The settled unit is 4 bytes, not 12

- **0 of 42,346** non-zero blocks violate `size % 4 == 0`. The word grid is
  universal.
- 40 B is exactly 10 words; 48 B is exactly 12 words; 4 B is one word.
- The byte-entropy profile shows a 4-byte sawtooth: the most-significant byte of
  each word has the lowest entropy. That is the signature of 32-bit values, not
  of a 12-byte row.
- The 12-byte partition is arithmetic: 48 B is 4 whole groups, but **40 B is 3
  whole groups plus a 4-byte tail**. The awkward tail is the tell that 12 is an
  imposed stride, not the storage unit.

### 2. The 12-byte group is unit-local, not global

- Mod-3 word phases are statistically **indistinguishable** across the corpus:
  cardinality ~60k each, `-1` share 0.002 each, small-int share 0.074 each, and
  f32-plausible share 0.020 / 0.029 / 0.025. An `[id][f32][flags]` record would
  make these three phases differ sharply. They do not.
- The `0x34` marker is not phase-locked: it appears at every word index 0-11.
- One unit (LAIR14) does show a local phase lock: lead-marker 0.48,
  middle-value 0.47, other phases 0.12-0.14. That is the unit the earlier
  observation came from.
- But even in LAIR14 the full triple (marker lead **and** value middle **and**
  small flags) co-occurs in only **5 of 102 groups (4.9%)**. The unit with the
  highest marker share (LAIR5, lead-marker 0.76) has **0 of 74**.
- Unit spread: lead-marker `>0.5` in 1/89 units, `0.1-0.5` in 3/89, and
  `<=0.1` in 85/89.

Reading: a 12-byte grouping exists as a **local pattern in a few units**, not as
a `.PCM` record layout. The earlier note was right about its unit and wrong as a
generalisation.

### 3. The f32 role is not established

- Corpus-wide, 93.7% of word readings are finite, but of the non-zero readings
  **75.6% are subnormal** (`<1e-3`) and 18.3% are `>1e9`. That bimodal spread is
  the classic signature of **small integers misread as floats**, not of a real
  float field.
- The float phase lock is real only in LAIR14 (middle phase 0.47 vs 0.12-0.14
  for the other phases). It does not survive the joint test above.
- No triple was shown to share a leading word while carrying three different
  values. First-three-id-equal ranges from 2.6% to 24.1% across sampled units,
  which is not a record structure.

### 4. The id-ish u32 is a rare marker, not a tag family

- The `0x34` high byte covers **1,110 / 286,780 words = 0.39%**, and appears in
  only 40/89 units.
- Its low 24 bits are **dominated by repeated-byte filler**: 261 distinct values,
  the top value covers 9.4%, and only 31.5% are printable ASCII (0.11% are
  all-uppercase). That is not a name prefix and not a small tag enum.
- The readable-looking examples in the earlier note are members of that filler
  family, not evidence of a name table.

### 5. Names have no ordinal correspondence

- 2,106 names were extracted, against 12,736 non-zero 40 B and 13,285 non-zero
  48 B blocks.
- Median `names / (fixed40 + fixed48)` is **0.089**; the 10th-90th percentile
  spans 0.025-0.222.
- Pearson correlation is 0.652 (40 B), 0.709 (48 B), 0.686 (groups), which is
  scale-only: bigger units have more of both.
- Exact ordinal equality holds in **0/89** units.

### 6. Placement and transforms: NO-GO

There is no corpus-wide layout, no proven coordinate triple, no shared-id
triple, and no name-to-block mapping. A "position + range" print would be a
guess. The honest verdict is **NO-GO** for placement and transforms on this
evidence.

## Probe ledger

| # | Probe | Result | Class |
|---|---|---|---|
| P1 | Corpus shape | 89 units, 46,124 blocks; 3,778 zero (8.2%); 42,346 non-zero | Observed |
| P2 | Word alignment | 0/42,346 non-zero blocks violate `size % 4 == 0` | Observed |
| P3 | Dominant word counts | 40 B = 10 words; 48 B = 12 words; 4 B = 1 word | Observed |
| P4 | Stride by entropy | 4-byte sawtooth; MSB of each word has the lowest byte entropy | Observed / Derived |
| P5 | Mod-3 phases | cardinality, `-1`, small-int, and f32-plausible shares are indistinguishable | Observed |
| P6 | Marker `0x34` | 0.39% of words; 40/89 units; phases 0.52% / 0.39% / 0.23% | Observed |
| P7 | Marker low bytes | 261 distinct; top 9.4%; printable 31.5%; repeated-byte filler | Observed |
| P8 | Marker index spread | present at every word index 0-11 | Observed |
| P9 | f32 corpus-wide | 75.6% of non-zero readings subnormal; 18.3% `>1e9` | Observed |
| P10 | f32 local lock | LAIR14: lead 0.48, middle value 0.47, others 0.12-0.14 | Observed |
| P11 | Local joint triple | LAIR14 5/102 = 4.9%; LAIR5 0/74 | Observed |
| P12 | Unit spread | lead-marker `>0.5`: 1/89; `0.1-0.5`: 3/89; `<=0.1`: 85/89 | Observed |
| P13 | Name correlation | median ratio 0.089; Pearson 0.652/0.709/0.686; 0/89 ordinal equality | Observed |
| P14 | 40 B tail | marker tail 5/14, 4/10, 4/19 in three sampled units - near chance | Observed |

## What stays UNKNOWN

1. What the 40/48-byte word arrays mean; the grid is settled, the field roles
   are not.
2. The unit-local marker/value pattern in LAIR14; its meaning is unknown.
3. Why some units carry `0x34` markers and most do not.
4. The 4-byte tail on 40-byte blocks: arithmetic known, meaning not.
5. The origin of the repeated-byte low-24 patterns.
6. Placement, transforms, instances, and any coordinate triple.
7. Faces, indices, normals, topology, and rendering.
8. Whether any block is compressed.
9. Any name-to-block mapping.

## Smallest visible win

Options were:

- (a) `--level-groups <path>`: a counts-only stride/group diagnostic per unit.
- (b) print positions and ranges: **NO-GO**, no proven coordinate triple.
- (c) no-go only.

(b) is out. (c) is honest but leaves the refutation unrepeatable. **(a) is GO**
when scoped as a diagnostic, not a decoder: it prints the 4-byte word grid, the
12-byte partition (whole groups plus tail words), the `0x34` marker by phase,
and a bounded f32-range predicate by phase. It makes the refutation reproducible
per unit and is falsifiable on synthetic bytes. It prints no payload, offsets,
coordinates, or type names.

## Correction to `room-geometry`

The later-observation block in `docs/notes/room-geometry.md` stays as written:
it was true for LAIR14. This note adds the corpus-wide result: the 12-byte
group is **unit-local**, not a general `.PCM` layout, and the `0x34` high byte
is a rare marker rather than a constant tag.

## Non-goals

- No coordinate, angle, scale, or transform is printed or claimed.
- No block or word receives a semantic type name.
- No name-to-block ordinal is claimed.
- No mesh, face, index, normal, or render output.
- No address, file offset, or payload dump.

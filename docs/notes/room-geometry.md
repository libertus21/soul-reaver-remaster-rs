# room-geometry — what lives inside the `.PCM` records

> **INVALIDATED (content claims).** The P14-P16 content conclusions below
> (12-byte `[u32][f32][u32]` groups, the f32 field, the leading tag, and the
> placement lead) were computed on the superseded `4+4N` base. See
> `docs/notes/pcm-content-census.md`: f32 is at chance on the correct base, and
> the group reading is refuted. The 12-byte lag-6 period survives as an
> unexplained observation. The size-class census remains valid.

Read-only probe of the SR1 remaster `<install>/1/AREA` tree. No game bytes,
absolute install paths, coordinates, or decompiled code are recorded here.
Date: 2026-10-27. Box: 3 passes, ~120 minutes.

## Question

The `area-levels` slice proved the `.PCM` header, the object-name table, and the
cross-links, then closed geometry as NO-GO. This probe asks one narrower
question: **what are the blocks that the offset table points at?** It does not
attempt a mesh, a placement table, or a render.

## Method

For every `.PCM`, the existing header model yields `N` relative offsets and a
data base of `4+4N`. Every gap between adjacent offsets is treated as one
**block**. Blocks were characterised by size, zero fraction, and 16-bit value
statistics. Several units were read in full: a small one (~62 blocks), a medium
one (~553), and a large one (~1,655). All scratch work stayed outside the repo.

## Verdict: there is no tag-based directory

- Record 0 is **4 zero bytes in 89/89** units.
- Record 1 is **all zero in 89/89**: 8 bytes in 21 units, 80 bytes in 67, and
  120 bytes in 1.

**Correction (recorded during this change):** the previous probe's reading that
records 0 and 1 form a directory is **WRONG**. Record 0 is 4 zero bytes and
record 1 is all zero across 89/89 units, so both are constant zero padding, not
a directory. The only directory in a `.PCM` is the offset table itself, and **no
block carries a type tag**.

## Classification by size clique

The census covers 46,124 blocks in 89 units (median 281 blocks per unit).

| Size | Blocks | Share | Shape |
|---|---:|---:|---|
| 4 B | 16,595 | 36.0% | scalar or zero |
| 48 B | 13,316 | 28.9% | 24 x i16 |
| 40 B | 12,810 | 27.8% | 20 x i16 |
| 28 B | 424 | 0.9% | other |
| 8 B | 408 | 0.9% | other |
| 76 B | 338 | 0.7% | other |
| 72 B | 287 | 0.6% | other |
| 16 B | 234 | 0.5% | other |

Three sizes carry **92.7%** of all blocks. 3,778 blocks (8.2%) are entirely
zero, and every one of the 89 units contains at least one. The classification
key is therefore **shape**, not meaning: `zero`, `small4`, `fixed40`, `fixed48`,
or `other:<size>`.

One dominant block exists in every unit (3,616–143,872 B). It is the **last**
block in only 1 of 89 units, so it is not a trailing append; it is a resident
payload in the middle of the block stream.

## Geometry candidates

### Candidate A — the dominant block (strongest)

- 16-bit values, 62–78% inside ±8192 in the sampled units.
- **12-byte (6 x u16) stride signature**: u16 equality at lag 6 is 0.14–0.43
  (median 0.26) across all 89 units, against ~0.03 at lags 1, 2, 3, 4, and 8.
- The 6 columns have similar magnitude distributions, so the row is not two
  clearly different triples; the field roles are not established.

Reading: the strongest geometry candidate, but the row semantics are **not**
proven. A 12-byte row could be 6 channels, 3 pairs, or 2 triples; none is
demonstrated.

### Candidate B — the 48-byte blocks (second)

- 24 x i16, with 20.3 of 24 values distinct per block.
- 72% of values fall inside ±8192 in the sampled unit.
- No clean split: 8 x (3 coords) and 12 x (2 fields) both fit the byte count,
  and neither is coherent on magnitude.

### Candidate C — the 40-byte blocks (weak)

- 20 x i16. Mirrored halves hold in only 8.4% of pairs.
- Adjacent 4-byte blocks repeat the following 40-byte head in only 12.6% of
  12,642 pairs, so there is no general (key, payload) rule.

### Not a candidate

- 4-byte blocks are scalars: 1,239 of 5,190 u32 values in the sampled 40-byte
  neighbourhood are zero; the rest have no common type code.
- Zero blocks carry no information.

No face, index, normal, transform, or placement table was located. No vertex or
face count is claimed.

## Community research

Web search returned **HTTP 403** on both queries this session. GitHub API, tree,
and raw endpoints were used instead.

| Source | License | What it establishes | Limit |
|---|---|---|---|
| https://github.com/fmil95/soul-re | MIT | PS1 decompilation; per-unit area configs and the Gex 2 engine lineage | does not document the remaster `.PCM` container |
| https://github.com/Gh0stBlade/KAIN2 | **No license file at the probed path** (root license request returns 404) | PS1 engine structures: `_SVector` = 8 B (3 x i16 + pad), `_Position` = 6 B (3 x i16), `_BSPLeaf` with face list, face count, and boxes; `_Terrain` relocation | PS1 engine, not the remaster container; the earlier "Unlicense" note is **not verified** |

The PS1 structs support the *plausibility* of 16-bit coordinate-like data
(8-byte and 6-byte vectors), but they do not prove any `.PCM` row layout. No
code was copied; only format/behavior facts were restated.

## Probe ledger

| # | Probe | Result | Class |
|---|---|---|---|
| P1 | Block census | 46,124 blocks; median 281/unit; 3,778 zero blocks (8.2%) | Observed |
| P2 | Records 0/1 | 89/89 zero; sizes 4, then 8/80/120 | Observed |
| P3 | Size cliques | 4/48/40 B = 92.7% of blocks | Observed |
| P4 | Largest block | 1 per unit; 3,616–143,872 B; last in 1/89 | Observed |
| P5 | 16-bit stride | lag-6 equality 0.14–0.43 (median 0.26) vs ~0.03 at other lags | Observed statistic / Derived stride |
| P6 | Value range | 62–78% of i16 in ±8192; 72% for a sampled 48-byte set | Derived |
| P7 | 48-byte layout | 24 x i16; 20.3/24 distinct; no clean 3-split | Observed shape |
| P8 | 40-byte layout | 20 x i16; mirrored halves 8.4% | Observed shape |
| P9 | 4 -> 40 adjacency | 12,642 pairs; head-exact 12.6% | Observed |
| P10 | Community | soul-re MIT; KAIN2 has no license file; PS1 vector sizes | Observed docs |
| P11 | Search | websearch 403; GitHub API/raw used | Observed |

## What stays UNKNOWN

1. Type tags: none exist; class labels are shape-only.
2. The dominant block's row layout and field roles; the 12-byte stride is
   statistical, not decoded.
3. 40- and 48-byte block semantics.
4. Faces, indices, normals, topology.
5. Placement and transforms.
6. Whether the dominant block is compressed.
7. `.PCM` <-> PS1 `.drm` equivalence.
8. The 3,778 zero blocks: padding, empty slots, or placeholders.
9. Why three sizes dominate.

## Smallest visible win

Candidates were:

- (a) `--level-records <path>`: per-unit block census by shape class.
- (b) extract one geometry block and print vertex/face counts.
- (c) render a room.

(c) is **NO-GO**: no proven layout. (b) is **NO-GO** for face counts and
**unproven** for vertex counts: the 12-byte stride is the best signal, but the
row semantics are unknown, so any count would be a guess. (a) is **GO**: it is
derived entirely from the offset table, is falsifiable on synthetic bytes, and
makes the shape classification visible without a semantic claim.

Chosen slice: one `blocks <unit> total=<n>` line, one line per present class in
fixed order, and one `largest <ordinal> <size>` line. No payload, no offsets, no
coordinates.

## Next probes (future changes)

1. Test the 12-byte row hypothesis directly: does a row contain a repeated
   header word or a bounded coordinate triple across many units?
2. Compare the dominant block across two units of the same room family; look
   for a shared prologue or a count word.
3. Search for a face/index array by testing u16 values against a plausible
   vertex ceiling per unit.
4. ~~Check whether the 48-byte blocks are a parallel array to the 40-byte
   blocks.~~ **Answered NO** by the later observation below: the
   `fixed48 = fixed40 + 1` count correlation holds in 26/31 `LAIR` units, but the
   block content disproves the parallel-array reading.
5. If a real PS1 `.drm` set becomes available, compare section counts against
   the block census.

## Apply result — slice `level-records` (2026-10-27)

Shipped `--level-records <path>`: a read-only per-unit block census. Accepted
on one small, one median, and the largest unit under `1/AREA`, plus one room
directory. Only counts and sizes are recorded: no game bytes, file offsets,
coordinates, or absolute install paths.

| Unit | Blocks | zero | small4 | fixed40 | fixed48 | largest |
|---|---:|---:|---:|---:|---:|---|
| `LAIR5` | 62 | 23 | 14 | 10 | 11 | 3616 B at 61 |
| `CATHY44` | 326 | 24 | 107 | 90 | 91 | 19920 B at 50 |
| `MOVIE3` | 2256 | 190 | 501 | 552 | 696 | 143872 B at 549 |
| `LAIR` room (31 units) | 12641 | — | — | — | — | — |

Every unit's class counts sum to its header block count. `other:<size>` rows
stay ascending by size, so `MOVIE3` also exposes 44-, 60-, 72-, 76-, and
104-byte shapes. On `LAIR5` the largest block is the last block (ordinal 61);
on `CATHY44` and `MOVIE3` it sits mid-stream, which matches the earlier "last
in 1/89" observation.

Wording contract: these are SIZE classes, not types. No block carries a type
tag, and no vertex or face count is claimed. Nothing here decodes geometry,
placement, transforms, or rendering. The 12-byte stride signal stays a
statistical observation, not a proven record layout.

## Later observation — 40/48-byte block internals (AFTER the playtest)

Recorded on 2026-10-27 from a follow-up inline probe that ran **after** the
accepted playtest. Evidence class: **CONFIRMED-observed**. Semantics: **UNKNOWN**.
Nothing here changes the shipped `--level-records` census, and nothing here is a
geometry, placement, or transform claim.

| # | Probe | Result | Class |
|---|---|---|---|
| P12 | 40/48 count correlation | `fixed48 = fixed40 + 1` holds in 26/31 `LAIR` units | Observed |
| P13 | Parallel-array reading | Refuted by content: the two sizes are NOT three parallel per-object arrays | Observed / Derived |
| P14 | 12-byte group stride | `[u32 id-ish][f32 value][u32 flags]`; a 48-byte block is exactly 4 groups, a 40-byte block is 3 groups plus 4 unexplained tail bytes | Observed shape |
| P15 | f32 field | 106 non-zero values; 44 with an absolute value below 5000 (e.g. -0.45, -460.35, -14.39, -14730.5, -471377.1); values repeat in runs of the same value | Observed values |
| P16 | Leading u32 | High byte `0x34`; readable low bytes (`HHD`, `CCC`) suggest an id/tag family | Observed bytes / Inferred tag |

Reading: a 12-byte `[u32 id-ish][f32 value][u32 flags]` group is a plausible
**placement/transform** shape, which is why the next slice targets placement and
transforms rather than geometry. The f32 magnitude spread and the repeated runs
are consistent with a scalar field (angle, scale, or distance), but **no field
role is proven**.

Still UNKNOWN after this observation:

1. What the id-ish u32 means, and whether the `0x34` high byte is a type/tag or
   part of a larger identifier.
2. What the f32 represents (angle, scale, a position component, or other), and
   its units.
3. What the 4 tail bytes of a 40-byte block are.
4. Whether these groups describe placement, transforms, or something else.
5. Whether the group layout generalises beyond the 40/48-byte sizes.

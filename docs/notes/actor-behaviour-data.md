# actor-behaviour-data - the shipped actor block and its behaviour entries

Bounded, read-only byte probe of the SR1 remaster object-model corpus. Behaviour
and format facts only: container-relative offsets, counts, short asset names and
small numeric ids. No game bytes, no decoded imagery, no disassembly listings, no
absolute install paths. Scratch scripts lived in `%TEMP%` only; the archives and
loose object files were opened read-only.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable corpus-wide test), **INFERRED** (one side only), **UNKNOWN**
(explicitly not established).

This note continues `actor-ai` (the runtime AI block and the data-parameterised
behaviour entry), `object-models` (the `.PCM` container and model header) and
`object-behaviour` (the name-keyed object tracker). It answers the data half of
`actor-ai` UNKNOWN #1: what the shipped models actually contain.

## Verdict

1. **There is a per-model actor block, and it sits at object data `+0x1C`.**
   The object header field at `+0x1C` points at a trailing block; every actor
   model in the corpus points at the same four-byte block signature. The block
   carries the per-model behaviour tables. (CONFIRMED)
2. **The actor family in this corpus is 21 model containers plus the player.**
   Of the 394 single-model object containers in `BIGFILE.DAT`, 19 carry the
   actor-block signature; 2 more actor-block containers exist only as loose
   `.PCM` files. `raziel` carries a distinct player block. The other 375
   `BIGFILE.DAT` single-model containers carry a different block signature and
   are not actors. This matches `actor-ai`'s "only `monster_` and `raziel__`
   carry a tick". (CONFIRMED for the signature split; the class-name mapping is
   INFERRED.)
3. **Behaviour entries are 0x20-byte records, and `+0x1B` is the per-entry
   state/action id.** The entries form a contiguous 0x20-stride array inside
   the actor block, pointed to by the block's pointer table; each entry ends
   with a `0xFFFFFFFF` sentinel at `+0x1C` (one trailing variant uses a
   different sentinel). This is the structure `actor-ai` described as "entry is
   0x20 bytes, entry index at `+0x1B`". (CONFIRMED for size, stride, offset and
   sentinel; INFERRED for the field's name.)
4. **The per-model state table is 24 bytes at actor block `+0x18`: 8 rows of 3
   one-byte clip-set ids.** The bytes group exactly into trios, matching
   `actor-ai`'s "idle trio" / "alert trio" language. The values are one-byte
   indices, not frame ranges. (CONFIRMED for the shape; INFERRED that this is
   the state-to-clip-set map.)
5. **The state-to-clip mapping is template-shared, not global and not purely
   per-actor.** Actor families share whole rows (for example `skinner` and
   `morlock` share rows 0-5 byte-for-byte; `hunter`, `wrshp`, `priests` and the
   three `vlgr` variants share rows 0-4). The last rows are per-actor. (CONFIRMED)
6. **UNKNOWN #1 is partially answered.** The table shape and the clip-id trios
   are pinned; the mapping from a row to a *named* state is not, because the
   image has no named enum. See section 8. (CONFIRMED shape, UNKNOWN labels.)

## 1. Method and scope

- Ground truth for the object container and model header is the existing
  `src/object.rs` parser (`--object-info`): `N` at entry offset 0, data base
  `4 * roundup512(N + 1)`, object header, `+0x64` record table, model header
  at `table[0]`, 8-byte vertices, 12-byte polygons, 24-byte bones, 16-byte
  materials. Nothing in that layout was re-derived.
- The probe walked the SR1 `1\BIGFILE.DAT` index (1,325 entries, 397 object
  containers, 394 single-model) and the 29 loose `.PCM` files under
  `1\OBJECT\**`. It read the object header pointer at `+0x1C`, classified the
  block it points at, then parsed the actor block. All work was in memory.
- Three passes, inside the agreed budget. No production code was changed.
- Every count below is reproducible from the shipped bytes. The note records
  only counts, offsets, short names and small ids, never raw records.

## 2. Which models are actors

The discriminator is the four-byte signature at the start of the block that the
object header `+0x1C` points at. Across the 394 single-model positions in
`BIGFILE.DAT`:

| Block signature family | Containers | Reading |
|---|---:|---|
| actor-family signature | 19 | monster/character actor data |
| player-family signature | 1 (`raziel`) | player data |
| static/effect/door signatures | 375 | non-actor object data (doors, effects, blocks, weapons, pickups, markers) |

The 19 `BIGFILE.DAT` actor containers:

```text
aluka, alukabss, hunter, kain, morlock, ronin, roninbss, skinbos,
skinner, sluagh, soul, vlgra, vlgrb, vlgrc, vwraith, wallcr,
walbosb, walboss, wrshp
```

Two more actor-block containers exist only as loose `.PCM` files and are not in
the `BIGFILE.DAT` object set:

```text
morboss, priests
```

So the actor-model set in this corpus (`BIGFILE.DAT` plus the loose `OBJECT`
`.PCM` files) is **21 monster/character containers plus `raziel`** (the player).
`soul` is in the actor family but its block is degenerate (no behaviour-entry
array); it is a system/effect object that shares the family signature, not a
combat actor. `raziel` is the only player-family model. Other `.BIG` archives
in the install (`E3.BIG`, `BONUS.BIG`, `TEXTURES.BIG`) do not use the
`BIGFILE.DAT` index shape and were out of scope.

CONFIRMED: the signature split and the two loose files. INFERRED: that the
signature maps one-to-one to the six-class table in `actor-ai` (`monster_` =
actor family, `raziel__` = player family, the other four classes = the other
signature families).

## 3. Actor block layout

The block pointer is the object-header dword at data `+0x1C`, not a model-header
field. The model header is only 0x38 bytes and is followed immediately by bones,
vertices, polygons and materials; `model + 0x2C0` and `model + 0x1A8` land
inside those arrays and are not a stable behaviour block. The `actor-ai` note's
`+0x2C0` (AI instance block) and `+0x1A8` (entry pointer inside that block) are
**runtime live-object offsets**. The file-side source is this `+0x1C` block.

The block begins at data `+0x1C` (data-base relative) and has this shape:

| Offset | Size | Content | Class |
|---|---:|---|---|
| `+0x00` | 4 | actor-family block signature | CONFIRMED |
| `+0x04` | 4 | optional sub-block pointer (0 when absent) | CONFIRMED shape |
| `+0x08` | 4 | optional sub-block pointer (0 when absent) | CONFIRMED shape |
| `+0x0C` | 4 | zero | CONFIRMED |
| `+0x10` | 4 | flags/format word; varies per actor | CONFIRMED shape / UNKNOWN meaning |
| `+0x14` | 2 | small value 0..15; per actor | UNKNOWN |
| `+0x16` | 2 | small flags/version word | UNKNOWN |
| `+0x18` | 24 | 8 rows x 3 one-byte clip-set ids | CONFIRMED shape |
| `+0x30` | 4*N | pointer table into the block's sub-tables | CONFIRMED shape |

The block length ranges roughly 0.3 KiB (degenerate system object) to 3 KiB
(`raziel` is a different family; the largest actor-family block is about 2.2
KiB). The pointer table at `+0x30` addresses the behaviour-entry array and
several animation/animation-parameter sub-tables.

The `+0x14`/`+0x16` words are plausible selectors (the note says two model
bytes choose the initial and later behaviour set), but no corpus test proves
which one selects which entry. They stay UNKNOWN.

## 4. Behaviour-entry census

The behaviour-entry array is a contiguous run of 0x20-byte records inside the
actor block, reached from the `+0x30` pointer table. Each record has:

| Offset | Size | Content | Class |
|---|---:|---|---|
| `+0x00` | 4 | per-entry scalar pair (varies) | UNKNOWN |
| `+0x04` | 4 | small parameter words | UNKNOWN |
| `+0x08` | 4 | small parameter words | UNKNOWN |
| `+0x0C` | 4 | small parameter words | UNKNOWN |
| `+0x10` | 2 | `50` in most records | UNKNOWN |
| `+0x12` | 2 | small value / `100` | UNKNOWN |
| `+0x14` | 2 | duration-like value (0..4000) | INFERRED |
| `+0x16` | 2 | zero or small flags | UNKNOWN |
| `+0x18` | 3 | zero | CONFIRMED |
| `+0x1B` | 1 | **entry state/action id** | CONFIRMED bytes / INFERRED role |
| `+0x1C` | 4 | `0xFFFFFFFF` sentinel (one trailing variant differs) | CONFIRMED |

Fourteen of the 21 actor containers yield a clean contiguous 0x20-stride array.
Seven do not: `soul`, `vlgra`, `vlgrb`, `vlgrc` and `priests` show no array under
the 0x20 rule; `walbosb` and `kain` carry entry-like records at a different
record shape (about 0x1C stride), so their entries are not counted here. Those
seven stay UNKNOWN for the array, although their 24-byte state table is present
and is counted in section 5.

### 4.1 Per-actor entry count and distinct ids

| Actor | Entries | Distinct ids | Ids (hex) |
|---|---:|---:|---|
| `skinbos` | 5 | 2 | 05, 06 |
| `morboss` | 1 | 1 | 05 |
| `roninbss` | 1 | 1 | 07 |
| `vwraith` | 3 | 3 | 09, 0a, 0b |
| `sluagh` | 3 | 3 | 0d, 0e, 0f |
| `skinner` | 9 | 5 | 18, 25, 26, 27, 28 |
| `hunter` | 4 | 4 | 1f, 20, 21, 23 |
| `walboss` | 7 | 5 | 02, 05, 06, 0b, 1b |
| `morlock` | 8 | 4 | 2b, 2c, 2d, 2e |
| `aluka` | 10 | 4 | 38, 39, 3a, 3b |
| `wallcr` | 8 | 6 | 30, 31, 32, 33, 3c |
| `ronin` | 7 | 6 | 2d, 2e, 2f, 30, 31, 32 |
| `wrshp` | 8 | 8 | 2f, 30, 31, 32, 33, 34, 35, 36 |
| `alukabss` | 1 | 1 | 02 |

Totals: **75 entry records, 37 distinct ids**. Several actors repeat an id with
different parameters (for example `skinner` uses `25`, `26`, `27`, `28` twice
each); those are entry variants, not duplicate states, so the distinct-id count
is lower than the entry count.

### 4.2 Id distribution across actors

| Id | Occurrences | Actors |
|---:|---:|---|
| 02 | 2 | `alukabss`, `walboss` |
| 05 | 6 | `morboss`, `skinbos`, `walboss` |
| 06 | 4 | `skinbos`, `walboss` |
| 07 | 1 | `roninbss` |
| 09 | 1 | `vwraith` |
| 0a | 1 | `vwraith` |
| 0b | 2 | `vwraith`, `walboss` |
| 0d | 1 | `sluagh` |
| 0e | 1 | `sluagh` |
| 0f | 1 | `sluagh` |
| 18 | 1 | `skinner` |
| 1b | 1 | `walboss` |
| 1f | 1 | `hunter` |
| 20 | 1 | `hunter` |
| 21 | 1 | `hunter` |
| 23 | 1 | `hunter` |
| 25 | 2 | `skinner` |
| 26 | 2 | `skinner` |
| 27 | 2 | `skinner` |
| 28 | 2 | `skinner` |
| 2b | 2 | `morlock` |
| 2c | 2 | `morlock` |
| 2d | 3 | `morlock`, `ronin` |
| 2e | 3 | `morlock`, `ronin` |
| 2f | 2 | `ronin`, `wrshp` |
| 30 | 4 | `ronin`, `wallcr`, `wrshp` |
| 31 | 5 | `ronin`, `wallcr`, `wrshp` |
| 32 | 3 | `ronin`, `wallcr`, `wrshp` |
| 33 | 3 | `wallcr`, `wrshp` |
| 34 | 1 | `wrshp` |
| 35 | 1 | `wrshp` |
| 36 | 1 | `wrshp` |
| 38 | 2 | `aluka` |
| 39 | 2 | `aluka` |
| 3a | 4 | `aluka` |
| 3b | 2 | `aluka` |
| 3c | 1 | `wallcr` |

The ids form a sparse global space (0x02..0x3c) partitioned in practice by
actor: `vwraith` 0x09-0x0b, `sluagh` 0x0d-0x0f, `hunter` 0x1f-0x23, `skinner`
0x25-0x28, `morlock` 0x2b-0x2e, `ronin` 0x2d-0x32, `wallcr` 0x30-0x33,
`aluka` 0x38-0x3b, `wrshp` 0x2f-0x36, bosses and NPCs in the low range. The
overlaps (`2d`/`2e`, `30`-`33`) are real and are the strongest evidence that
the id space is shared rather than per-actor.

CONFIRMED: the ids and their distribution. INFERRED: that the ids index a
shared animation/action list; the list itself was not located in the object
containers.

## 5. The per-model state table (24 bytes at +0x18)

Every actor block, including the seven without an enumerated entry array,
carries a 24-byte table at block `+0x18`. The bytes split exactly into eight
rows of three one-byte values:

| Property | Value | Class |
|---|---|---|
| Bytes | 24 | CONFIRMED |
| Grouping | 8 rows x 3 bytes | CONFIRMED |
| Stride | 3 bytes per row (24 bytes per table) | CONFIRMED |
| Value width | one byte (0x00..0x42 observed) | CONFIRMED |
| Index base | direct byte, no +1 base observed | INFERRED |
| Values are frame ranges | **no**: too narrow, and values recur across actors | INFERRED (strong) |
| Values are clip-set ids | consistent with the `actor-ai` trio language | INFERRED |
| Row -> named state | not pinned | UNKNOWN |

The task's "state -> animation table" is this table at the data level. The
runtime table that `actor-ai` reports with stride `0x10` is a different,
runtime-side table; the shipped file carries the compact 8x3 form. Do not
conflate the two.

### 5.1 Trios per actor

Decimal, eight rows in stored order:

```text
aluka     [19,21,20] [26,32,18] [5,12,7]  [14,18,28] [35,2,11] [20,3,2] [10,2,66] [18,8,1]
alukabss  [7,29,29]  [20,10,6]  [4,4,5]   [5,7,22]   [12,0,11] [20,1,1] [1,1,14]  [1,2,0]
hunter    [15,18,17] [26,20,14] [5,10,7]  [12,15,29] [23,2,11] [20,2,2] [4,3,39]  [13,8,1]
kain      [12,36,36] [26,15,11] [8,4,10]  [6,12,28]  [26,0,11] [20,1,1] [5,0,21]  [1,2,0]
morboss   [4,28,27]  [6,17,1]   [46,37,48][39,3,15]  [26,0,11] [20,1,1] [1,1,12]  [1,2,0]
morlock   [13,31,31] [25,16,12] [9,5,11]  [7,13,27]  [19,2,11] [20,3,2] [8,2,51]  [15,8,1]
priests   [15,18,17] [26,20,14] [5,10,7]  [12,15,29] [23,2,11] [20,1,1] [0,0,17]  [1,2,0]
ronin     [11,18,18] [13,16,10] [5,8,6]   [9,10,14]  [17,2,11] [20,3,2] [7,0,54]  [20,8,1]
roninbss  [12,36,36] [26,15,11] [8,4,10]  [6,12,28]  [17,0,11] [20,1,1] [1,0,20]  [1,2,0]
skinbos   [40,40,39] [30,15,8]  [31,16,43][25,8,33]  [18,0,11] [20,1,1] [5,0,22]  [1,2,0]
skinner   [13,31,31] [25,16,12] [9,5,11]  [7,13,27]  [19,2,11] [20,3,2] [9,0,55]  [19,8,1]
sluagh    [9,23,23]  [18,12,8]  [6,3,7]   [4,9,20]   [14,0,11] [20,1,1] [3,0,25]  [13,8,0]
soul      [0,0,0]    [0,0,0]    [0,0,0]   [0,0,0]    [0,0,11]  [20,1,0] [0,0,1]   [1,1,0]
vlgra     [15,18,17] [26,20,14] [5,10,7]  [12,15,29] [23,2,11] [20,2,1] [0,0,36]  [16,8,1]
vlgrb     [15,18,17] [26,20,14] [5,10,7]  [12,15,29] [23,2,11] [20,2,1] [0,0,35]  [16,8,1]
vlgrc     [15,18,17] [26,20,14] [5,10,7]  [12,15,29] [23,2,11] [20,1,1] [0,0,35]  [16,8,1]
vwraith   [20,22,21] [25,32,19] [8,4,11]  [6,10,28]  [35,0,11] [20,1,1] [3,0,25]  [14,8,0]
walbosb   [2,2,2]    [2,2,0]    [0,0,0]   [0,2,0]    [0,0,11]  [20,4,1] [2,0,11]  [1,2,1]
walboss   [5,11,10]  [14,15,0]  [0,0,0]   [0,5,0]    [0,0,11]  [20,3,1] [7,0,28]  [3,2,0]
wallcr    [19,41,41] [22,31,18] [5,12,7]  [14,19,24][35,2,11] [20,3,2] [8,0,62]  [12,8,1]
wrshp     [15,18,17] [26,20,14] [5,10,7]  [12,15,29] [23,2,11] [20,2,2] [8,1,62]  [36,16,1]
```

`raziel` has no such 24-byte table: its player block uses a different layout
(beginning with a different four-byte signature and a dense 0x10-stride
structure). Its state table is UNKNOWN in this pass.

## 6. Cross-model comparison

The trios are neither identical across the corpus nor random per file. Whole
rows are shared inside actor families:

| Row group | Actors | Rows shared |
|---|---|---|
| locomotion/combat family A | `hunter`, `vlgra`, `vlgrb`, `vlgrc`, `wrshp`, `priests` | 0-4 identical |
| family B | `skinner`, `morlock` | 0-5 identical |
| family C | `kain`, `roninbss` | 0-4 identical |
| heavy family | `wallcr`, `aluka` | rows 2 and 4 identical |
| boss/NPC tail | `kain`, `alukabss`, `roninbss`, `skinbos`, `morboss`, `priests` | row 7 `[1,2,0]` |
| wraith tail | `sluagh`, `vwraith` | row 6 `[3,0,25]`; row 7 differs by one byte |

Row 5 is the strongest shared anchor: `[20,3,2]` for `skinner`, `morlock`,
`ronin`, `wallcr`, `aluka`; `[20,1,1]` for eight other actors; `[20,2,2]` for
`hunter`/`wrshp`. That is a shared template with a per-family variant, not a
single global table.

Consequence: **the state-to-clip mapping is template-shared.** A change in a
shared row affects a whole family; the last rows (especially row 6) are the
per-actor tuning. This is exactly why `skinner` and `morlock` behave similarly
but not identically without different code.

`raziel` cannot be compared on this table; its player block is separate.

## 7. Answering `actor-ai` UNKNOWN #1

`actor-ai` asked for "the numeric state ids and the full transition table" and
said to enumerate the behaviour-entry state tables and map id -> clip set.

Now pinned:

- **Entry structure.** 0x20-byte records, id at `+0x1B`, sentinel at `+0x1C`.
  CONFIRMED.
- **Entry id sets.** 75 entries over 14 actors, 37 distinct ids, with the
  distribution in 4.2. CONFIRMED.
- **State table shape.** 24 bytes = 8 rows x 3 one-byte clip-set ids at block
  `+0x18`. CONFIRMED.
- **Clip-set families.** Rows are shared across actor families as in section 6.
  CONFIRMED.
- **State rows -> clip trios.** The eight trios per actor are pinned. INFERRED
  that they are the idle/alert/attack/etc. trio sets.

Not pinned:

- **Row -> named state.** The image has no named enum; row 0 could be idle or
  any other state. No labelled probe was run. UNKNOWN.
- **The model byte that selects the initial/later entry set.** `+0x14` and
  `+0x16` are candidates, but no corpus correlation proves either. UNKNOWN.
- **The semantics of the 0x20-entry fields.** Duration-like values are
  INFERRED; the rest are UNKNOWN.
- **`raziel`'s player state table.** Different block family; UNKNOWN.
- **The seven actors without an enumerated 0x20 array** (`soul`, `vlgra`,
  `vlgrb`, `vlgrc`, `walbosb`, `kain`, `priests`). Their state trios are
  pinned; their entry arrays are not. UNKNOWN.

So UNKNOWN #1 is **partially answered**: the numeric ids and the table shape
are pinned at the data level; the named-state labels and the selector byte are
still open. A future pass can close the labels only with a controlled runtime
probe or a targeted binary read of the state writer; it cannot be closed by
more model-byte census alone.

## 8. Evidence ledger

| Claim | Class |
|---|---|
| Object header `+0x1C` points at a trailing per-model block | CONFIRMED |
| 21 model containers carry the actor-family block signature (19 archive + 2 loose) | CONFIRMED |
| `raziel` carries a distinct player-family block | CONFIRMED |
| 375 other object containers carry non-actor block signatures | CONFIRMED |
| Actor block header shape (`+0x00`..`+0x30`) | CONFIRMED shape |
| 24-byte table at `+0x18` splits into 8 x 3 one-byte rows | CONFIRMED |
| Behaviour entries are 0x20 bytes, id at `+0x1B`, sentinel at `+0x1C` | CONFIRMED |
| 75 entries / 37 distinct ids on 14 actors | CONFIRMED |
| Ids are a sparse shared space and actors use contiguous ranges | CONFIRMED |
| Trio rows are shared inside actor families | CONFIRMED |
| Trios are the state-to-clip-set map | INFERRED |
| Ids index a global animation/action list | INFERRED |
| `+0x14`/`+0x16` are the two behaviour-set selectors | UNKNOWN |
| Row -> named state | UNKNOWN |
| `raziel` player state table | UNKNOWN |
| 0x20-array layout for `soul`/`vlgr*`/`walbosb`/`kain`/`priests` | UNKNOWN |
| Meaning of most 0x20-entry scalar fields | UNKNOWN |

## 9. Explicit UNKNOWN list

1. Which stored row is idle, alert/tracking, turn-to-face, attack/strike,
   recover, hurt/knockback or dead/removed.
2. The two model/block bytes that select the initial and later behaviour entry
   sets (`+0x14` and `+0x16` are unproven candidates).
3. The exact meaning of the 0x20-entry scalar fields other than the id.
4. Whether each trio value is an animation-clip index, an action-set index, or
   a key into another model table.
5. The `raziel` player block's state table and entry array.
6. The behaviour-entry array layout for the seven actors where the 0x20 rule
   did not find one.
7. Whether the actor block is copied verbatim into the runtime AI block at
   `+0x2C0`, or transformed at load.
8. Whether any script path overrides the per-model trios at run time.

## 10. Legal and method

Read-only byte probe. Scratch scripts and temporary JSON lived in `%TEMP%` and
were not committed. The note records counts, container-relative offsets, short
asset names and small ids only; no game bytes, no decoded imagery, no
disassembly, no absolute install paths. The archives were never copied into the
repository. `cargo test` was left at its starting baseline (321 green); no
production code was touched in this change.

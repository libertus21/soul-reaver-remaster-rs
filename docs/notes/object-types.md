# object-types - what the level-object `type` field means

Bounded, read-only Ghidra 12.1.4 pass on the SR1 remaster binary plus a
read-only corpus census of all 89 archived `.PCM` units. Behaviour notes only:
field offsets, counts, unit/object names and engine role names. No addresses,
no disassembly, no decompiled text, no raw game bytes, no absolute install
paths. The Ghidra project, scripts and scratch parser lived in `%TEMP%` only.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (code and bytes agree,
or two independent observations agree), **INFERRED** (one side only),
**UNKNOWN**.

## Verdict

1. **`type` is authored data, not a runtime state.** The level loader copies the
   source object record (`0x4C` stride) into the runtime record (`0x60` stride)
   field by field and passes `+0x26`, `+0x28` (both halves) and `+0x2C` through
   unchanged. Nothing in the load path rewrites the type. (CONFIRMED)
2. **`type` is per instance, not per name.** The same 12-byte object name is
   placed with several types in the corpus (`raziel` 2 and 4, `marker` 0/3/4/9,
   `campath` 0/1/3, and others). (CONFIRMED)
3. **The record carries a second, finer per-instance code at `+0x26`, and
   `type` is a deterministic function of it.** Across 838 objects there are 74
   distinct `+0x26` values and 25 distinct types, and **every `+0x26` value maps
   to exactly one type (zero conflicts)**. The `+0x26` value is therefore the
   per-instance archetype/prototype code and `type` is its coarse class.
   (CONFIRMED as a corpus fact; the mechanism that assigns the class is
   UNKNOWN.)
4. **The type-to-behaviour dispatch was NOT located.** No switch or jump table
   over the record's type word was found in the level-load path, and no
   comparison of the type word against the small 0..12 range was found on the
   object records. The behaviour each type selects stays UNKNOWN. This is a
   documented no-go for Part 2 of the investigation, not a negative claim that
   no dispatch exists elsewhere.
5. **A separate render-class resolver exists and is a different axis.** It
   assigns one of six engine classes (`raziel__`, `sreavr__`, `glphicon`,
   `physical`, `monster_`, `particle`) from the object's `+0x30` prototype blob
   and stores the class index at runtime `+0x04`. This is a rendering category,
   not the level-object `type`; conflating the two is the main trap in this
   area. (CONFIRMED)
6. **Type is not a 0..12 enum in general.** Fourteen of the 25 observed type
   values are greater than 12 (26, 34, 40, 81, 89, 90, 113, 129, 334, 380, 441,
   513, 931, 1376). The small values 0..12 cover the common game entities only.
   (CONFIRMED)

## 1. Data census

### 1.1 Method

The existing `--level-objects` reader was run over every loose `.PCM` unit in
the shipped `AREA` tree (89 units, 838 objects). A scratch parser in `%TEMP%`
then read the four relevant words directly (`+0x14`, `+0x26`, `+0x28`, `+0x2C`)
to separate the two halves of `+0x28` and to add the `+0x26` code. Nothing was
extracted and no game file was modified.

Read only this was: names, counts, small integers. No bytes entered the repo.

### 1.2 Overall type distribution

838 objects, 89 units, 80 distinct names.

| type | objects | type | objects |
|---:|---:|---:|---:|
| 0 | 386 | 9 | 64 |
| 1 | 74 | 12 | 2 |
| 2 | 40 | 26 | 9 |
| 3 | 36 | 34 | 1 |
| 4 | 184 | 40 | 4 |
| 5 | 1 | 81 | 2 |
| 6 | 6 | 89 | 1 |
| 7 | 10 | 90 | 3 |
| 8 | 4 | 113 | 3 |
|  |  | 129 | 2 |
|  |  | 334 | 1 |
|  |  | 380 | 1 |
|  |  | 441 | 1 |
|  |  | 513 | 1 |
|  |  | 931 | 1 |
|  |  | 1376 | 1 |

Types 10, 11 and 13..25, plus 27..33 and the other gaps, are **not used** by
any object in the shipped corpus. (CONFIRMED absence in this corpus; absence in
the format is UNKNOWN.)

### 1.3 Names that appear with more than one type

These are the interesting rows. Counts are object instances; the unit list is
abbreviated where long.

| name | type | objects | `+0x26` codes | notes |
|---|---:|---:|---|---|
| `campath` | 0 | 263 | 58, 101, 121, 162, 221 | almost all path nodes |
| `campath` | 1 | 17 | 277 | six units, not LAIR |
| `campath` | 3 | 8 | 453 | `LAIR2` only |
| `marker` | 0 | 15 | 101, 228 | `MOVIE2`, `MOVIE3`, `MRLOCK6` |
| `marker` | 3 | 6 | 499 | `MOVIE3`, `MOVIE4` |
| `marker` | 4 | 53 | 512, 552 | mostly `MOVIE*` |
| `marker` | 9 | 52 | 783 | 15 units, one flag `0x8000` |
| `raziel` | 2 | 12 | 374, 404, 405 | `ALUKA13`, `CATHY44`, `FORGE6`, `MOVIE*`, `MRLOCK*`, `UNDRCT15` |
| `raziel` | 4 | 70 | 512 | every other unit |
| `flameb` | 0 | 1 | 117 | `UNDRCT17` |
| `flameb` | 1 | 27 | 283, 338 | 14 LAIR units + `MRLOCK6` |
| `flameb` | 3 | 2 | 507 | `RETREAT3` |
| `flameb` | 6 | 3 | 636 | `UNDRCT9` |
| `lantrn` | 2 | 3 | 388 | `UNDRCT3` |
| `lantrn` | 4 | 38 | 550 | `UNDRCT1`, `UNDRCT22` |
| `lngswch` | 1 | 6 | 351 | four units |
| `lngswch` | 2 | 1 | 387 | `MRLOCK4` |
| `portal` | 0 | 2 | 157 | `MOVIE2`, `MRLOCK1` |
| `portal` | 6 | 3 | 672 | `MOVIE3`, `MRLOCK14`, `MRLOCK7` |
| `dumbub` | 1 | 1 | 344 | `UNDRCT12` |
| `dumbub` | 9 | 2 | 783 | `UNDRCT3`, `UNDRCT5` |
| `soul` | 0 | 2 | 199 | `MRLOCK7` |
| `soul` | 1 | 2 | 360 | `MOVIE3` |

All other names are single-type in this corpus.

### 1.4 The `+0x26` code to type map

The code and the type are **not independent**. Grouped by type, the 74 codes
form disjoint, monotonically increasing bands:

| type | distinct codes | code range | example names |
|---:|---:|---|---|
| 0 | 17 | 58..246 | `campath`, `marker`, `splob`, `undhost`, `flamec`, `crow`, `mfire` |
| 1 | 9 | 258..360 | `campath`, `flameb`, `updraft`, `boldasd`, `lngswch`, `soul` |
| 2 | 9 | 374..429 | `raziel`, `lantrn`, `shield`, `spout`, `walsoul`, `comblst` |
| 3 | 7 | 446..507 | `campath`, `marker`, `Ariel`, `priests`, `ronin`, `sluagh`, `valve` |
| 4 | 7 | 512..568 | `raziel`, `marker`, `steam`, `Kain`, `hunter`, `plswtch`, `vlgra` |
| 5 | 1 | 586 | `mmcrank` |
| 6 | 2 | 636, 672 | `flameb`, `portal` |
| 7 | 3 | 683..707 | `morlock`, `sugate`, `warpg` |
| 8 | 1 | 730 | `staff` |
| 9 | 2 | 783, 807 | `marker`, `dumbub`, `plugblk`, `pshblk`, `vwraith` |
| 12 | 1 | 914 | `wrpface` |
| 26 | 2 | 1317, 1328 | `banner`, `ldoore` |
| 34 | 1 | 1498 | `Morboss` |
| 40 | 1 | 1626 | `ldoora`, `ldoorb` |
| 81 | 1 | 2313 | `mrdoora`, `mrdoorb` |
| 89 | 1 | 2427 | `trifrca` |
| 90 | 1 | 2441 | `turn` |
| 113 | 1 | 2721 | `dndoor` |
| 129 | 1 | 2915 | `lairdr` |
| 334 | 1 | 4679 | `funnel` |
| 380 | 1 | 4994 | `trifrcd` |
| 441 | 1 | 5377 | `trifrcc` |
| 513 | 1 | 5802 | `trifrcb` |
| 931 | 1 | 7811 | `bring` |
| 1376 | 1 | 9496 | `blade` |

No code appears under two types. The bands are ordered exactly like the type
values, which is the signature of a **prototype/archetype table sorted by
class**: each prototype gets a code, the editor stamps the prototype's class
into `+0x28` high. (INFERRED mechanism; the code-to-class relation itself is
CONFIRMED across the corpus.)

### 1.5 What this proves about the field

- `type` cannot be derived from the name: 12 names occur with more than one
  type, and the majority are not name-wide constants.
- `type` cannot be a pure runtime state in the file: the loader copies it
  verbatim and no writer to `+0x2A` was found in the load path.
- `type` is not a small 0..12 enum: the corpus uses 25 values, ten above 12.

## 2. Binary pass

### 2.1 The object copy path (CONFIRMED)

The room source record at header slot `0x00` exposes:

- `+0x08` object count, `+0x0C` data-base-relative object-array offset;
- source records of stride `0x4C`;
- runtime records of stride `0x60`, allocated as `count x 0x60` and exposed at
  runtime room `+0x10`, then level `+0xD0` with the count at level `+0xC8`.

The copy routine reads the source record and writes the runtime record field by
field. For the fields relevant here:

| source | runtime | treatment |
|---|---|---|
| `+0x00` name | `+0x00` | copied |
| `+0x20` 3 x `i16` position | `+0x20` | copied |
| `+0x26` `u16` code | `+0x26` | copied, unchanged |
| `+0x28` `u32` (low id + high type) | `+0x28` | copied, unchanged |
| `+0x2C` `u32` flags | `+0x2C` | copied, unchanged |

Consequence: the type is decided by the **writer of the `.PCM`**, not by the
engine at load. The engine may later reinterpret it, but it does not author it.
(CONFIRMED for the copy; the later consumer is UNKNOWN.)

### 2.2 A second, unrelated class resolver (CONFIRMED, not the type)

A separate routine classifies a model object into one of six engine classes by
comparing the object's `+0x30` prototype string against a fixed class table:

```text
raziel__   sreavr__   glphicon   physical   monster_   particle
```

The routine also honours two flag bits (a forced `physical` class and a forced
`monster_` class) and writes the class index into runtime `+0x04`, or `0xFFFF`
when no class matches. This is a **render/model category** consumed by the
visual pipeline. It is not the level-object `type` and its index range (0..5
plus `0xFFFF`) does not overlap the type distribution. (CONFIRMED)

The six names are also the `_`-padded prefixes of archived object-container
names (for example `raziel` -> `raziel__`), which matches the earlier
`object-models` finding that the placed-object name resolves to a model
container. So the class resolver is "which renderer family owns this model",
not "what does this level object do".

### 2.3 The type dispatch (UNKNOWN - documented no-go)

The investigation searched the load path and its consumers for:

1. a switch or jump table over the record type word;
2. a comparison of the type word against the 0..12 range;
3. word-sized reads of the type field used as a table index.

None matched on the level-object records. The reads of `+0x2A` that were found
in the binary belong to unrelated structures (see 2.4). Therefore the mapping
from `type` to concrete behaviour is **UNKNOWN**. The absence of a hit in this
pass does not prove no dispatch exists elsewhere; the smallest next probes are
listed in section 4.

### 2.4 Field-offset caution

In a compressed transform/keyframe record (not a level object), the three `i16`
at `+0x26`, `+0x28` and `+0x2A` are a single 3-component vector, and that
record carries its own byte-sized opcode at `+0x02`. Several automated searches
for "reads of `+0x2A`" therefore returned that unrelated record family. Do not
read the level-object type from `+0x2A` without first confirming the record
shape (name at `+0x00`, position at `+0x20`, stride `0x4C`/`0x60`).

## 3. Type table (behaviour)

Legend for **behaviour**: CONFIRMED means the binary or the corpus proves the
statement; INFERRED means only name/context supports it; UNKNOWN means not
proven. The only code-level fact shared by every row is that the value is
copied verbatim from the `.PCM` (section 2.1), so no row gets CONFIRMED
behaviour from the load path alone.

| type | objects | example name(s) | behaviour | class |
|---:|---:|---|---|---|
| 0 | 386 | `campath`, `marker`, `splob`, `undhost`, `flamec`, `mfire`, `msound` | point/path/effect marker; many have no model container | UNKNOWN (name-context INFERRED) |
| 1 | 74 | `campath`, `flameb`, `updraft`, `lngswch`, `soul`, `boldasd` | trigger/volume or alternate path variant | UNKNOWN (name-context INFERRED) |
| 2 | 40 | `raziel`, `lantrn`, `shield`, `spout`, `walsoul`, `lrdial` | spawn / pickup / interactive prop | UNKNOWN (name-context INFERRED) |
| 3 | 36 | `campath`, `marker`, `Ariel`, `priests`, `ronin`, `sluagh`, `valve` | NPC or scripted prop | UNKNOWN (name-context INFERRED) |
| 4 | 184 | `raziel`, `marker`, `steam`, `Kain`, `hunter`, `vlgra` | player/NPC entity plus some props | UNKNOWN (name-context INFERRED) |
| 5 | 1 | `mmcrank` | crank mechanism | UNKNOWN |
| 6 | 6 | `portal`, `flameb` | portal or effect | UNKNOWN (name-context INFERRED) |
| 7 | 10 | `morlock`, `sugate`, `warpg` | interactive/trigger entity; `warpg` is a warp gate | UNKNOWN (warp membership CONFIRMED in `warp-level-selection`) |
| 8 | 4 | `staff` | item pickup | UNKNOWN (name-context INFERRED) |
| 9 | 64 | `marker`, `dumbub`, `plugblk`, `pshblk`, `vwraith` | blocker/marker family | UNKNOWN (name-context INFERRED) |
| 12 | 2 | `wrpface` | warp face | UNKNOWN (warp membership CONFIRMED in `warp-level-selection`) |
| 26 | 9 | `banner`, `ldoore` | cutscene prop / door | UNKNOWN (name-context INFERRED) |
| 34 | 1 | `Morboss` | boss entity | UNKNOWN (name-context INFERRED) |
| 40 | 4 | `ldoora`, `ldoorb` | door | UNKNOWN (name-context INFERRED) |
| 81 | 2 | `mrdoora`, `mrdoorb` | door | UNKNOWN (name-context INFERRED) |
| 89 | 1 | `trifrca` | puzzle prop | UNKNOWN |
| 90 | 3 | `turn` | rotating/turn mechanism | UNKNOWN (name-context INFERRED) |
| 113 | 3 | `dndoor` | door | UNKNOWN (name-context INFERRED) |
| 129 | 2 | `lairdr` | door | UNKNOWN (name-context INFERRED) |
| 334 | 1 | `funnel` | funnel/volume | UNKNOWN (name-context INFERRED) |
| 380 | 1 | `trifrcd` | puzzle prop | UNKNOWN |
| 441 | 1 | `trifrcc` | puzzle prop | UNKNOWN |
| 513 | 1 | `trifrcb` | puzzle prop | UNKNOWN |
| 931 | 1 | `bring` | item/prop | UNKNOWN |
| 1376 | 1 | `blade` | item/weapon | UNKNOWN (name-context INFERRED) |

The name-context INFERRED column is deliberately weak: it records what the
asset name and the unit family suggest, not what the binary was shown to do.
No row should be promoted to CONFIRMED behaviour without a located consumer.

## 4. Multi-type names - the answer

`type` is **a per-instance class stamped from the `+0x26` archetype code**, not
a property of the model name and not a runtime state byte.

Evidence:

- 74 distinct `+0x26` codes map to 25 types with zero conflicts; the type bands
  are ordered by code (section 1.4).
- The same name carries different codes and therefore different types
  (`raziel` 374/404/405 -> 2 vs 512 -> 4; `marker` 101/228 -> 0, 499 -> 3,
  512/552 -> 4, 783 -> 9).
- The loader copies both fields unchanged, and no separate state byte was found
  in the record: the remaining bytes are the name, two zero/offset triples,
  position, the `+0x26` code, the `+0x28` id/type dword, flags, and two
  data-base-relative pointers (`+0x30`, `+0x38`).

So the accurate model is:

```text
12-byte model name  -> selects the model container
u16 +0x26           -> per-instance archetype/prototype code
u16 +0x2A (high)    -> coarse class of that archetype = "type"
u32 +0x2C           -> placement flags
```

A name-level or "type = enum(0..12)" table is therefore wrong. Read the type
per record.

## 5. UNKNOWN (explicit)

1. What each `type` value makes the engine do (register a trigger, spawn a
   model, create a portal, run a script, do nothing). No consumer located.
2. Where the type-to-behaviour dispatch lives, if it exists at all.
3. Why the `+0x26` code and the type are redundant (cached class vs index into
   an editor table). The mechanism is INFERRED only.
4. Why ten type values exceed the small enum and what the large values (26..
   1376) mean. They may be prototype-specific classes or packed flags; both
   are unproven.
5. Whether the `+0x2C` bit `0x4000` (set/cleared by the name-registry pass in
   the prior `level-geometry` note) is related to type. Not traced here.
6. Whether the same `+0x26` code means the same thing across units (the corpus
   is consistent, but no cross-unit registry was decoded).

## 6. Next probes (ordered by cost)

1. **Object consumer.** Find the per-frame reader of the runtime array at level
   `+0xD0` (count `+0xC8`, stride `0x60`) and list which fields it reads. That is
   the shortest path to the type dispatch.
2. **Class vs code.** Check whether `+0x26` is used as an index into an editor/
   prototype table (a 74-entry or larger array) and whether that table also
   holds the class. If so, the type is a cached column and the real key is the
   code.
3. **Same-code, same-name audit.** For every distinct `+0x26` code, check that
   its members share one name and one type. This would settle whether the code
   is an archetype id or a name hash.
4. **MOVIE vs gameplay split.** The `MOVIE*` units carry many of the unusual
   multi-type placements; a per-unit comparison may separate cutscene props
   from gameplay entities.
5. **Dynamic trace (user-run).** Place a breakpoint on the runtime array walk,
   observe a `warpg` and a `marker`, and record which handler each reaches.

## 7. Legal

Behaviour notes only. Field offsets, unit/object names, counts and engine role
names. No addresses, no listings, no decompiled bodies, no game bytes, no
absolute install paths. Scratch and the Ghidra project stayed in `%TEMP%`; the
game binary and archive were opened read-only. No protection or packing was
encountered, so no line was stopped.

# placed-flags - what the placed-record `flags` word encodes

**Status:** research only. No engine change, no realm system, no bit meaning
invented. The only command added is the read-only `--flag-census` below.

Read-only census over every placed record in the shipped SR1 archive
`1/BIGFILE.DAT`. It answers one gate question: does any bit of the placed
record's `flags` word (`+0x2C`, already parsed in `src/level.rs::object_report`)
encode realm membership, visibility or activation?

**Verdict: no. `flags` is a one-of tag; only the campath bit is decoded. A
spectral-realm slice cannot rely on this word today.**

Corpus: **364 object-bearing units**, **6142 placed records**, **404
case-sensitive names**, **6 distinct `flags` values**. The total object count
reproduces the earlier `6142` in `level-transitions.md`, and the campath split
reproduces the starting point exactly (1976 campath, 4166 non-campath).

## Method

`--flag-census --bigfile <DAT> [--name <NAME>]` scans the archive index once,
parses every `.PCM` candidate once, decodes only its object array, and folds
each record's `flags` word into a `BTreeMap`-ordered aggregate. `--name`
appends one line per matching record (`name unit pos type archetype instance
flags`) so a varying bit can be traced to concrete placements. It is
deterministic, read-only, opens no window, and is covered by unit tests.

Reproduce the numbers:

```
$ soul-reaver-remaster-rs --flag-census --bigfile <1/BIGFILE.DAT>
flags-census units=364 objects=6142 names=404 distinct_flags=6
```

Game paths arrive at runtime only; no archive byte, absolute path, address or
RVA is stored in this note.

## 1. Complete `flags` distribution

| `flags` | decimal | bit | records | share |
|---|---:|---:|---:|---:|
| `0x00000000` | 0 | none | 3827 | 62.3% |
| `0x00000008` | 8 | 3 | 20 | 0.3% |
| `0x00000020` | 32 | 5 | 4 | 0.1% |
| `0x00000080` | 128 | 7 | 207 | 3.4% |
| `0x00008000` | 32768 | 15 | 108 | 1.8% |
| `0x00010000` | 65536 | 16 | 1976 | 32.2% |

**Every non-zero value is exactly one bit.** No record in the corpus sets two
bits at once, so the word behaves as a one-of tag (an enum), not as an
independent bitmask. That single fact already makes a "realm bit OR-ed together
with other flags" reading unlikely, though a loader that only ever sets one
value could still make it true.

The other 27 bits (0, 1, 2, 4, 6, 8-14, 17-31) are **never set** anywhere in the
archive.

Two names exist in case variants that the case-sensitive record keeps apart:
`campath` (1975) + `Campath` (1), and `portal` (161) + `PORTAL` (5). Both
variants agree on flags.

Recorded starting points that this pass **did not reproduce**: `wcegg` is
`0x00000000` on all 32 of its records, not `0x80`. The starting point also
listed `0x20` for no name; it is real but belongs only to `flamblk`.

## 2. Name x bit cross-tab

`constant-set` = a name whose every record carries the bit; `varied` = the same
name appears with and without it; `clear` = a name that never carries it.
404 names, so `constant-set + varied + clear = 404`.

| bit | mask | records | names | constant-set | varied | clear |
|---:|---|---:|---:|---:|---:|---:|
| 3 | `0x00000008` | 20 | 8 | 1 | 7 | 396 |
| 5 | `0x00000020` | 4 | 1 | 1 | 0 | 403 |
| 7 | `0x00000080` | 207 | 71 | 53 | 18 | 333 |
| 15 | `0x00008000` | 108 | 1 | 0 | 1 | 403 |
| 16 | `0x00010000` | 1976 | 2 | 2 | 0 | 402 |

82 distinct names carry at least one bit; 322 names are all-zero. `Kain` is the
only name that carries two different non-zero tags across its two records
(`0x08` and `0x80`), which is direct evidence that bits 3 and 7 are per-instance
tags, not per-name categories.

### Names carrying each bit (flagged/total records for that name)

- **bit 3 (`0x8`) - 8 names, all actor rows:**
  `Kain 1/2, ronin 5/66, skinbos 1/1, skinner 4/33, soul 1/268, vlgra 1/10,
  vwraith 5/37, wallcr 2/32`.
- **bit 5 (`0x20`) - 1 name:** `flamblk 4/4` (one unit).
- **bit 7 (`0x80`) - 71 names**, dominated by effects, emitters, sound points,
  props and marker/geometry names:
  `Ariel 1/1, BEAMS 1/1, Kain 1/2, Walflap 4/4, alkwnd 1/2, alukabss 1/1,
  anvil 1/2, atmosph 1/1, banvil 2/2, comblst 17/24, comcath 4/4, comet 1/1,
  comleft 1/1, comlftb 1/1, comritb 1/1, comrite 1/1, dumytub 1/1, dust 24/30,
  eaggotr 8/11, exdoor 1/1, expldc 9/9, fglass 1/1, fkefrce 2/2, flamef 1/1,
  flament 3/3, flamesk 4/4, flamesl 2/2, genva 1/4, gfire 2/2, gforce 3/3,
  gsnlght 2/3, gsound 2/2, gstone 3/3, gwater 3/3, hatstm 6/9, healthr 1/2,
  hint 1/1, nitpart 1/1, pipeb 1/2, pipec 1/2, portal 1/161, pshsce 3/3,
  pshscf 1/2, puff 1/1, reavdie 1/1, reavprt 1/1, revdieb 1/1, rooflap 4/4,
  rublea 1/1, rubleb 1/1, rvrskl 1/1, scwindw 1/1, sgbase 1/1, sgbella 1/1,
  sgbellb 1/1, sgtopht 1/1, smist 6/6, sndptsA 2/2, sndptsC 12/12,
  sndptsD 4/4, sndptsE 2/2, soulb 1/2, specrvr 1/1, stnglph 1/2, texplo 1/1,
  updraft 9/12, walsoul 8/31, wapipe 1/1, wbpipe 1/1, wcpipe 1/1,
  wrpface 14/14`.
- **bit 15 (`0x8000`) - 1 name:** `marker 108/941` (the only name that uses it).
- **bit 16 (`0x10000`) - 2 names:** `campath 1975/1975, Campath 1/1`; **0
  non-campath records** carry it.

## 3. Varying bits: concrete examples

Record identity is `unit pos type`; "kind" is from the name and the existing
ledgers (`actor-behaviour-data.md`, `object-types.md`).

### bit 3 - only actor names, minority of instances

| name | kind | flagged example | unflagged example |
|---|---|---|---|
| `vwraith` | enemy actor | `cathy55 -2340,-1036,0` type 9 `0x8` | `chrono1 3632,-1552,-17562` type 9 `0x0` |
| `ronin` | enemy actor | `conecta1 7330,-39,-777` type 3 `0x8` | `hubb2 -2064,-4217,-1036` type 3 `0x0` |
| `skinner` | enemy actor | `out1 -874,-8920,-4804` type 3 `0x8` | `push1 5724,6219,637` type 3 `0x0` |
| `soul` | actor/pickup | `train2 -145,157,-692` type 1 `0x8` | `push1 2386,3454,480` type 1 `0x0` |
| `wallcr` | breakable wall actor | `cathy40 -5487,-7813,-1096` type 8 `0x8` | `cathy36 7653,4015,3271` type 8 `0x0` |
| `Kain` | actor | `chrono1 -5456,-2325,-16352` type 4 `0x8` | `pillars10 0,0,0` type 4 `0x80` (other tag) |

All eight bit-3 names are actor-behaviour rows. The bit is never on a prop,
marker, effect or path. But it is a minority of each name's instances, so it is
a **per-instance actor tag**, not a name category.

### bit 7 - effects, emitters and props (18 names vary)

| name | kind | flagged example | unflagged example |
|---|---|---|---|
| `portal` | realm-shift portal | `pillars1 3200,-1920,0` type 6 `0x80` | `push1 2568,5731,635` type 12 `0x0` |
| `dust` | effect | `cathy19 -2102,12896,6556` type 0 `0x80` | `cathy36 8739,4412,-3471` type 0 `0x0` |
| `updraft` | effect | `cathy36 8710,4523,-3668` type 1 `0x80` | `htorm1 -4010,-8892,-1163` type 1 `0x0` |
| `walsoul` | interactive prop | `cathy55 2272,390,-862` type 2 `0x80` | `cathy55 2269,549,-880` type 2 `0x0` |
| `healthr` | pickup/effect | `pillars1 -1393,964,162` type 1 `0x80` | `aluka47 704,776,-80` type 1 `0x0` |
| `wrpface` | warp marker | 14/14 records, every unit of the warp cycle `0x80` | none |

The bit-7 set mixes material effects (`dust`, `flame*`, `gfire`, `gsound`,
`expldc`, `smist`, `sndpts*`) with warp/portal-adjacent names (`wrpface`,
`portal`) and a few spectral-sounding names (`walsoul`, `soulb`, `specrvr`). It
is **not** a coherent realm partition.

### bit 15 - `marker` only

`marker` has 941 records in 173 units; 108 carry `0x8000`, 833 carry 0. Example:
flagged `huba16 3608,-12120,2962` type 9 vs unflagged `train7 -8831,-15435,6106`
type 0. The bit is confined to one name and splits its instances, so it is a
per-marker subtype/state, not a category of the name.

## 4. H4 vs the interaction classification

The interaction slice (`docs/notes/interactions.md`) pins blocking doors and
passive switches by name. Measured flags for every one of those names:

| group | names | records | `flags` |
|---|---|---:|---|
| Blocking | `sudoor`(29), `catdoor`(2), `catdora`(1), `wrpdoor`(11), `hatch`(9), `dblgate`(5), `skgate`(3), `sugate`(8) | 68 | all `0x00000000` |
| Passive | `valve`(6), `plswtch`(11), `bellsc`(3) | 20 | all `0x00000000` |
| Excluded door | `doorh`(1) | 1 | `0x00000000` |
| Warp portal | `wportal`(9) | 9 | `0x00000000` |

No bit separates blocking from passive; every interactable is `0`. **H4
REFUTED** - `flags` does not encode a collision/physics modifier for the
door/switch set.

## 5. Realm-shift portals

Per `docs/notes/unit-warp.md`, `portal` objects are material-to-spectral realm
shifts, not level warps. All portal records (`portal` 161 + `PORTAL` 5) total
**166 in 136 units**; **165 have `flags=0`, one has `0x80`** (unit `pillars1`,
type 6, archetype 672). The uppercase `PORTAL` group is 5 records in 2 units,
all `0x00`.

`warpg` (28 records, 14 units) is `0x00`; `wrpface` (14 records, 14 units) is
`0x80` on every record - a constant warp-cycle marker, not a realm value.

So the realm-shift portals do **not** carry a realm bit. If a realm value were
stored here, the objects that cause the shift would be the first place to see
it; they are not. **No portal record distinguishes material from spectral.**

## 6. Per-hypothesis verdicts

| hypothesis | verdict | decisive numbers |
|---|---|---|
| **H1 realm bit** | **REFUTED** | Same names appear with/without bits (18 bit-7 names vary, 7 bit-3 names vary), but the flagged sets are not realm-coherent: bit 3 is actor-only yet includes material enemies (`ronin` 5, `skinner` 4, `wallcr` 2), bit 7 is dominated by material effects/geometry, and 165/166 realm-shift portals are `0`. |
| **H2 activation/visibility bit** | **UNKNOWN** | Bit 7 is consistent with triggered effects/emitters (53 names all-set, 18 mixed; many `flame*`/`g*`/`sndpts*`/`dust` names), but no runtime visibility or collision trace exists; every interactive door/switch is `0`, so it is not the door activation flag. Cannot confirm or refute. |
| **H3 category marker** | **CONFIRMED for bit 16; REFUTED for bits 3, 7, 15; UNKNOWN for bit 5** | bit 16: `campath`/`Campath` 1976/1976 records, 0 non-campath, 0 varied - a fixed name category like the known campath marker. bits 3/7/15 vary within a name (7, 18, 1 names). bit 5 is constant but only on one name (`flamblk`), so it is indistinguishable from a one-name category. |
| **H4 physics/collision modifier** | **REFUTED** | Blocking doors (68 records) and passive switches (20 records) are all `0`; no bit separates them. |

## 7. What each observed bit does and does not mean

| bit | status | what it does / does not mean |
|---:|---|---|
| 16 | **CONFIRMED (pre-existing)** | Campath path-marker category. 1976/1976 campath records, 0 others. Not realm. |
| 3 | **UNKNOWN** | Only actor names, but a minority of instances. A per-instance actor tag; **not** a name category, **not** proven realm/visibility. |
| 5 | **UNKNOWN** | Only `flamblk` (4/4, one unit). Could be a one-name category; no evidence of meaning. |
| 7 | **UNKNOWN** | 71 heterogeneous names, mostly effects/emitters/props; also `wrpface` and one `portal`. Consistent with a spawn/visibility/state tag; **not** proven, **not** a realm partition. |
| 15 | **UNKNOWN** | Only `marker` (108/941). A per-marker subtype/state; no meaning. |
| all other 27 | **ABSENT** | Never set in the archive; their existence is not even established. |

## 8. UNKNOWN: what a spectral-realm slice can and cannot rely on

Today a spectral slice **cannot** rely on any `flags` bit:

- No bit is proven to mean realm membership, visibility or activation.
- The realm-shift portals are `0` (165/166) and `warpg` is `0`; the one
  warp/portal-adjacent constant is `wrpface=0x80`, which is a warp-cycle marker.
- The only confirmed non-zero meaning is `campath=0x10000`, which is unrelated.
- Values are one-hot, so the word cannot currently be read as "realm bit plus
  other flags".

What would settle it:

1. **Runtime trace of the loader's `+0x2C` read.** Capture where the placed
   record's `flags` word is consumed at unit load and which runtime state it
   writes: a render/visibility setter (H2), an actor-activation setter, or a
   realm setter (H1). The strongest leads are bit 3 (actor-only) and bit 7
   (effects/emitters): branch on `& 0x8` and `& 0x80`.
2. **Two-realm capture of one object.** Trace one `portal`/`walsoul`/`dust`
   instance before and after a realm shift and compare the consumed value.
3. **Live visibility/collision probe.** Compare a flagged and an unflagged
   instance of the same name in the same unit to see whether the bit changes
   rendering or collision.

Until one of those lands, `flags` stays a one-of tag with one known meaning and
four unexplained values. **The spectral-realm gate is not cleared.**

## 9. Digest check

No engine path changed, so the documented digests are unchanged:

| run | digest |
|---|---|
| `under1` 60 ticks | `ede39c540a10ee39` |
| `cathy28` 60 ticks | `3e8293c0e38afac1` |

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| P1 | 364 object-bearing units, 6142 records, 404 names, 6 distinct flag values. | CONFIRMED |
| P2 | Every non-zero `flags` value is exactly one bit; no value combines bits. | CONFIRMED |
| P3 | Only bits 3, 5, 7, 15, 16 are ever set. | CONFIRMED |
| P4 | bit 16 is campath-only (1976/1976, 0 non-campath, 0 varied). | CONFIRMED |
| P5 | bit 3 appears only on the 8 actor names but varies per instance. | CONFIRMED |
| P6 | bit 7 spans 71 heterogeneous names (18 vary); bit 15 is `marker` only. | CONFIRMED |
| P7 | Realm-shift portals are 165x`0` and 1x`0x80`; no realm bit. | CONFIRMED |
| P8 | All blocking doors and passive switches are `flags=0`. | CONFIRMED |
| P9 | Bit meanings 3/5/7/15 are UNKNOWN. | UNKNOWN |

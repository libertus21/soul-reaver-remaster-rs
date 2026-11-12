# object-rotation-field - the placed-object record carries no rotation (2026-11-12)

Read-only, offline field enumeration of the SR1 level object array, run against the
explicit `/// The record carries no rotation.` claim in `src/level.rs`. Every `i16`
and `u16` at every byte offset from `0x30` to `0x4A` was scored, plus `i32`/`u32`,
all consecutive `i16` triples in that window, and the undecoded prelude
`0x0C..0x1F`. The corpus is 372 level units and 6,142 placed records; `under1`
contributes 95 records. Scratch lived in `%TEMP%`; the archive was opened
read-only and no game byte is reproduced here.

## Verdict: NOT FOUND

**No field in the `0x4C` placed-object source record is a per-object rotation.**
The `/// The record carries no rotation.` comment is correct and is left unchanged.

Apart from trivially small-range index fields (which sit near zero by
construction), the one cluster that passes a naive round-angle test is the
`+0x18` `i16` triple. It is a **cross-level / cluster translation offset**, not
three angles:
its X/Y components are level-wide constants on a 1024-unit grid, its Z component
is an arbitrary elevation offset, and all three are simultaneously non-zero in
only 119 of 6,142 records. A genuine orientation triple cannot have two of its
three components zero in 92-96% of objects.

The `0x30..0x4A` window itself holds offsets, an index/id, small counters and a
small signed triple, none of which shows a round-angle concentration above chance.

## Quick path

1. Read "Corpus and method" for the sample and the unit.
2. Read "`0x30..0x4A`: no surviving candidate" and its table.
3. Read "The only angle-shaped cluster is an offset" for the `+0x18` refutation.
4. Read "Geometry cross-check" for the wall-normal test.
5. Read "Field roles (null hypothesis)" for what the bytes actually are.

## Corpus and method

| Item | Value |
|---|---|
| Source | SR1 `BIGFILE.DAT`, read-only |
| Level units with an object array | 372 |
| Placed records | 6,142 (matches `object-models` / `level-geometry`) |
| `under1` records | 95 (66 `campath`, 26 `marker`, 1 `undhost`, 1 `raziel`, 1 `hint`) |
| Record stride | `0x4C` (76 bytes) |
| Angle unit under test | `K = 360 / 4096` degrees per unit |
| Round-angle window | within `0.5°` of a multiple of `45°` (512 units) or `90°` (1024 units) |
| Chance baseline | `1/45` per non-zero value for 45°; `1/90` for 90° |

The baseline is uniform-random over the full `i16` range. It is not valid for
small-range fields; those are called out separately below.

## `0x30..0x4A`: no surviving candidate

Aligned `i16` reads over all 6,142 records. `round45`/`round90` count non-zero
values within `±0.5°`; `exp45`/`exp90` are the chance counts for that field's
non-zero population.

| Field | distinct | min | max | zero | round45 | exp45 | round90 | exp90 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `+0x30 i16` | 632 | -29932 | 31200 | 5413 | 15 | 16.2 | 9 | 8.1 |
| `+0x32 i16` | 1 | 0 | 0 | 6142 | 0 | 0.0 | 0 | 0.0 |
| `+0x34 i16` | 1 | 0 | 0 | 6142 | 0 | 0.0 | 0 | 0.0 |
| `+0x36 i16` | 1 | 0 | 0 | 6142 | 0 | 0.0 | 0 | 0.0 |
| `+0x38 i16` | 1992 | -32724 | 32760 | 3814 | 62 | 51.7 | 31 | 25.9 |
| `+0x3A i16` | 2 | 0 | 1 | 6084 | 58 | 1.3 | 58 | 0.6 |
| `+0x3C i16` | 1 | 0 | 0 | 6142 | 0 | 0.0 | 0 | 0.0 |
| `+0x3E i16` | 1 | 0 | 0 | 6142 | 0 | 0.0 | 0 | 0.0 |
| `+0x40 i16` | 5 | 0 | 4 | 6119 | 23 | 0.5 | 23 | 0.3 |
| `+0x42 i16` | 10 | 0 | 43 | 6097 | 8 | 1.0 | 8 | 0.5 |
| `+0x44 i16` | 100 | -2285 | 6587 | 5965 | 11 | 3.9 | 10 | 2.0 |
| `+0x46 i16` | 115 | -2521 | 2309 | 5960 | 6 | 4.0 | 6 | 2.0 |
| `+0x48 i16` | 78 | -2571 | 4489 | 5999 | 6 | 3.2 | 6 | 1.6 |
| `+0x4A i16` | 1 | 0 | 0 | 6142 | 0 | 0.0 | 0 | 0.0 |

No broad-range field exceeds chance in a meaningful way. `+0x30` is below chance.
`+0x38` is `1.2x` chance. `+0x44/0x46/0x48` are `1.5-2.8x` on small non-zero
populations and, decisively, contain **zero exact 512/1024 multiple** among their
non-zero values: their few "round" hits are values `1..5`, which sit within `0.5°`
of zero by construction. Fields `+0x3A` (values `0/1`) and `+0x40` (values
`0..4`) are trivially round and carry no angle information.

`under1` is stronger still: every aligned field from `0x30` to `0x4A` is constant
zero except `+0x38`, which is the `0x78`-stride sub-object/path offset
(67 distinct values, monotonically `+120`, zero round-angle hits). There is no
room for a rotation anywhere in the `under1` window.

### Unaligned reads are a false-positive trap

Reading `i16` at odd offsets produces apparent "winners" such as `+0x0F`, `+0x13`,
`+0x1F` and `+0x37`. They are byte-fusion artifacts: an odd offset joins the zero
high byte of a small index/position field with the low byte of the next field, so
the result is `byte << 8` — a multiple of 256, half of which land on 512. The
in-range triple test confirms this: every aligned consecutive-`i16` triple has
**zero** records where all three components are simultaneously round. Only two
unaligned odd-start triples (`0x43` and `0x45`) show any all-three-round records
(12 and 2 of 6,142), which is the same byte-fusion artifact.

## The only angle-shaped cluster is an offset

The undecoded prelude triple at `+0x18/+0x1A/+0x1C` is the sole broad-range
field group that looks like three angles under the test:

| Field | non-zero | round45 | exp45 | exact ×512 | exact ×1024 |
|---|---:|---:|---:|---:|---:|
| `+0x18 i16` | 515 | 409 | 11.4 | 342 | 342 |
| `+0x1A i16` | 254 | 116 | 5.6 | 104 | 103 |
| `+0x1C i16` | 2944 | 1382 | 65.4 | 920 | 849 |

That is 21-36x chance, and it is exactly the signal a rotation hunt is looking
for. It is not orientation:

1. **X/Y are level-wide constants.** In level after level a single `(x,y)` pair
   covers 90-100% of the non-zero records: `chrono1` 130/132 at `(0,0)`,
   `skinnr6` 21/23, `pillars9` 29/32, `clfpil1`/`aluka15`/`cathy61` 100% at
   `(0,0)`. A per-object yaw/pitch cannot be constant across every object in a
   room.
2. **X/Y are a 1024-unit grid.** They take almost only `0`, `±1024`, `±2048` and
   never the `±512` (45°) values a real orientation distribution would include.
3. **Z is arbitrary.** `+0x1C` mixes `0`, `±1024`, `±2048` with non-grid values
   such as `721`, `740`, `-1745`, `389`, `2028` — an elevation offset, not an
   angle.
4. **The three are not independent angles.** All three are non-zero in only
   119 of 6,142 records; 2 land on all-three-round. The triple is essentially
   `(0, 0, z)`.
5. **The runtime already identifies it.** `level-geometry` P4/P5 and
   `object-behaviour` §1.3 record this triple as a cross-level offset consumed
   in subtraction, not an angle. The data agrees with that trace.

This is the honest null-hypothesis winner. It fails on geometry and on
independence, so it is excluded.

## Geometry cross-check

No candidate survived as a yaw, so the positive "FOUND" geometry test does not
apply. A null cross-check was still run to make sure nothing was missed: 63
door-like records (`sudoor`, `skdoor`, `wrpdoor`, `rdoor`, `ocdoor*`, `ndoor*`,
`exdoor`, …) were matched to the nearest vertical room face, and for each
candidate field the circular difference `(value·K − wall_yaw)` was tested for
concentration.

| Field | samples | mean abs deviation from best center | occupied 45° bins |
|---|---:|---:|---:|
| `+0x18` | 63 | 78.1° | 7/8 |
| `+0x1A` | 63 | 76.3° | 7/8 |
| `+0x1C` | 63 | 79.5° | 7/8 |
| `+0x30` | 63 | 76.3° | 7/8 |
| `+0x38` | 63 | 78.4° | 8/8 |
| `+0x44/0x46/0x48` | 63 | 76.3° | 7/8 |

A uniform random angle gives a mean absolute deviation of `90°` and fills all
eight bins. No candidate tracks wall orientation. The door-like records do not
carry the rotation that would align them to their wall.

Referenced blocks were also scanned: 2,328 `+0x38` sub-object blocks and 729
`+0x30` prototype blobs were searched for an orthonormal 3x3 matrix as `f32` or
`i16` Q14 in their first `0x200` bytes. **Zero hits.** The sub-object blocks
observably carry pointers, a type/id and a duplicate of the world position; no
rotation matrix was found.

## Field roles (null hypothesis)

| Offset | Size | Most likely role | Evidence |
|---|---|---|---|
| `+0x0C` | 4 | always zero | 6,142/6,142 |
| `+0x10` | 2 | object index in room | `0..207`, top values `1/2/3`; sequential per room in `under1` |
| `+0x14` | 2 | authored id | globally unique `1..6457`, all 6,142 distinct |
| `+0x18` | 6 | cross-level/cluster offset `(x,y,z)` | §"only angle-shaped cluster" |
| `+0x20` | 6 | world position | confirmed by `level-geometry` |
| `+0x26` | 2 | archetype code | prior docs |
| `+0x28` | 4 | type (high) / instance (low) | `src/level.rs` |
| `+0x2C` | 4 | flags | prior docs |
| `+0x30` | 4 | prototype/parameter blob offset | `0..55004`; points at small tagged integers |
| `+0x34` | 4 | zero | 6,142/6,142 |
| `+0x38` | 4 | sub-object block offset | `0..72644`; referenced block restates position |
| `+0x3A` | 2 | high half of `+0x38` | `0/1` |
| `+0x3C` | 2 | zero | 6,142/6,142 |
| `+0x3E` | 2 | zero | 6,142/6,142 |
| `+0x40` | 2 | small counter/param | `0..4` |
| `+0x42` | 2 | small counter/param | `0..43` |
| `+0x44/0x46/0x48` | 6 | small signed per-instance offsets/params | `-2571..6587`; no round signal, no 512 multiple |
| `+0x4A` | 2 | zero | 6,142/6,142 |

The `+0x44/0x46/0x48` role is **INFERRED**: the values behave like small
per-instance offsets or parameters, and the round-angle and geometry tests both
reject an angle interpretation, but their exact meaning is not established.

## Correction to the comment

**No correction is required.** `src/level.rs` says:

```text
/// The record carries no rotation.
```

The enumeration confirms it. The correct scope of that sentence is "no rotation
in the `0x4C` source record", not "the engine never rotates a placed object": the
runtime still needs an orientation, and this note does not locate it. Code is
left untouched because the verdict is NOT FOUND.

## UNKNOWN (explicit)

1. **Where a placed object's world orientation comes from.** It is not in the
   record. The runtime procedural/aim chain and the sub-object block remain
   candidates, but no rotation matrix was found in their first `0x200` bytes.
2. **Interior of the `+0x30` prototype/parameter blob** beyond its small tagged
   integers.
3. **Interior and full role of the `+0x38` sub-object block**; only its head
   (pointers/type/id/duplicate position) was inspected.
4. **Exact meaning of `+0x40`, `+0x42` and the `+0x44/0x46/0x48` triple.**
5. **Coverage.** One shipped build. Other builds, regional variants or unused
   object classes are unobserved.

## Legal

Read-only format note. No game bytes, no decompiled text, no listings and no
absolute install paths are recorded. Only offsets, field roles, unit constants
and corpus counts appear here. Scratch files lived in `%TEMP%`; the archive was
never written.

## Related notes

- `docs/notes/level-geometry.md` — the source-record layout and the original
  "no orientation" finding (P2, P5).
- `docs/notes/object-behaviour.md` — the runtime uses the triple as an offset,
  not an angle (§1.3).
- `docs/notes/rotation-model.md` — the confirmed `Rz(c)·Ry(b)·Rx(a)` runtime
  model and the 4096-unit turn this note tested against.
- `docs/notes/object-models.md` — where the placed-object meshes live.

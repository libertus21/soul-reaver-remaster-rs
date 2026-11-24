# campath-format - a campath placement is a local position spline, not a path-node link (2026-11-24)

Read-only, offline field study of the SR1 placed-object array, run against the
open question left by `object-behaviour` section 5.2 ("what consumes `campath`
ids") and the locomotion need stated for this slice. Every placed record in
every `BIGFILE.DAT` level unit was decoded, and the per-object block referenced
by record `+0x38` was decoded and validated. Scratch parsers lived in the
session temp directory; the archive was opened read-only and no game byte is
reproduced here. Only offsets, counts, field roles and measured distributions
appear.

This note does not change production code, does not add a CLI mode, and does
not move a digest. The two golden headless digests are unchanged
(`under1` `ede39c540a10ee39`, `cathy28` `b8c30d28e0c67130`).

## Verdict first

| # | Hypothesis | Verdict | Decisive number |
|---|---|---|---|
| H1 | Fixed record stride, and a campath record carries an integer that indexes another placed record (next node or path id). | **REFUTED** (stride half CONFIRMED) | stride `0x4C` CONFIRMED on 6,142 records; the best index candidate resolves to another campath only 41/1,976 (2.07%) and its followed hops are unbounded (median 1,911, 46% over 2,000). No field beats a random pair. |
| H2 | `flags == 0x10000` is a path-node marker bit, not a per-node path id. | **CONFIRMED** | `flags == 65536` on 1,976/1,976 campath records; bit 16 is set on 0/4,166 non-campath records. It is constant, so it groups nothing. |
| H3 | A per-node group/path id exists (a small integer repeated across the nodes of one path). | **REFUTED** | archetype, instance and `+0x10` leave the nearest-neighbour distance at the campath baseline (see section 6); `add1` places 12 nodes under one archetype with hops up to ~6,000. |
| H4 | Node-local metadata exists (orientation, dwell time, speed). | **CONFIRMED for position, timing and tangents; REFUTED for orientation and speed; dwell is not a separate field** | `+0x38` is an ordered position spline (5,394 keys, 1,948/1,948 headers valid, 5,394/5,394 deltas and tangents in range). No orientation field exists. |

The single structural finding is that a campath record points at a
**per-object position spline**: an ordered list of `(delta, position,
out-tangent, in-tangent)` keys. The keys are topology, but only *inside one
record*. There is no link, path id or group id between records.

## 1. Corpus and method

| Item | Value |
|---|---|
| Source | SR1 `BIGFILE.DAT`, opened read-only |
| Level units parsed | 372 |
| Placed records | 6,142 |
| Units that place `campath` | 233 |
| `campath` records | 1,976 |
| Requested samples | `add1` 12 (entry 267), `under1` 66 (entry 115) |
| `--group under` family | 91 = `under1` 66 + `under3` 25 |
| `--group add` family | 29 = `add1` 12 + `add6`, `add7`, `add3`, `add8` |

The brief's "under1 places 91 campaths" is the `under` family total; the single
`under1` unit places 66. The two requested units are used as spot samples, and
the verdicts are measured over all 233 units so a rule that only works on one
sample cannot pass.

Method:

1. Read the index and parse each archive entry as a `.PCM` with the same
   data-base, offset-table and room-record rules the engine already uses.
2. Decode the room's object array: `count` at room `+0x08`, data-base-relative
   array offset at room `+0x0C`, records of `0x4C` bytes.
3. Tabulate every aligned field of every record, campath versus non-campath.
4. For every field and width, test whether its value indexes another placed
   record (authored id `+0x14`, record offset, `+0x38` block offset, array
   slot). Follow the survivors and measure hop distances against a random-pair
   baseline.
5. Decode the `+0x38` block and validate the shared position-key shape against
   `spline-animation` section 2.1 across the whole corpus.

## 2. The placed record as measured

The reader in `src/level.rs` already exposes name, position `+0x20`, archetype
`+0x26`, the `+0x28` type/instance split and `flags` `+0x2C`. The remaining
bytes were enumerated over the full corpus. For a campath record they are:

| Offset | Size | Role on a campath record | Campath reading |
|---|---|---|---|
| `+0x00` | 12 | NUL-padded name | `campath` |
| `+0x0C` | 4 | unused | zero in 1,976/1,976 |
| `+0x10` | 2 | descending placement counter | 137 distinct values, never zero, no grouping power (section 6) |
| `+0x12` | 2 | unused | zero |
| `+0x14` | 2 | globally unique authored id | 1,976/1,976 distinct in the whole corpus |
| `+0x16` | 2 | unused | zero |
| `+0x18` | 6 | cross-level offset `(x,y,z)` | zero in most; 1,024/2,048-grid values otherwise |
| `+0x1E` | 2 | unused | zero |
| `+0x20` | 6 | world position `(x,y,z)` | in use |
| `+0x26` | 2 | archetype code | 23 distinct values |
| `+0x28` | 4 | type (high 16) / instance (low 16) | type 0 in 1,922/1,976; 78 instance values |
| `+0x2C` | 4 | flags | exactly `0x10000` in 1,976/1,976 |
| `+0x30` | 4 | prototype/parameter blob offset | zero in 1,976/1,976 |
| `+0x34` | 4 | unused | zero in 1,976/1,976 |
| `+0x38` | 4 | **position-spline block offset** | non-zero in 1,973/1,976 (3 are zero) |
| `+0x3C`, `+0x3E`, `+0x40`, `+0x4A` | 2 each | unused | zero in 1,976/1,976 |
| `+0x42`, `+0x44`, `+0x46`, `+0x48` | 2 each | rare per-instance params | non-zero in at most 23 records |

The field that actually differs from the rest of the object population is
`+0x38`. It is non-zero on only 355 of 4,166 non-campath records, but on almost
every campath. That asymmetry is the thread the rest of this note follows.

## 3. H2: `flags == 0x10000` is a marker bit (CONFIRMED)

Across the corpus the only flag value any campath record carries is `0x10000`,
on 1,976/1,976 records. No non-campath record sets bit 16 (0/4,166); the
non-campath flag values are `0`, `0x80`, `0x8000`, `0x8` and `0x20`. So bit 16
marks "this placed object is a campath path object" and nothing else.

Counter-evidence considered and rejected: could `0x10000` encode a path index?
No. It is one value on every node, so it cannot separate paths or nodes. It is a
class bit, not an id. (Contrast `+0x2C` bit `0x4000`, already documented as the
name-table membership flag, and bit `0x8000`, the interaction flag.)

## 4. The `+0x38` block: an ordered position spline

`+0x38` is a data-base-relative offset to a self-contained block. On every one
of the 1,948 campath blocks with a non-zero key count, the block has this
layout (offsets relative to the block start):

| Block offset | Size | Role | Validation |
|---|---|---|---|
| `+0x00` | 4 | pointer to the channel descriptor at `+0x30` | equals `block + 0x30` on 1,948/1,948 |
| `+0x04`..`+0x2F` | 44 | zero (unused channel slots) | zero throughout |
| `+0x30` | 4 | pointer to the key array at `+0x38` | equals `block + 0x38` on 1,948/1,948 |
| `+0x34` | 2 | key count (`numkeys`) | matches the block extent (section 4.2) |
| `+0x36` | 1 | stride flag | `0` (position) on 1,973/1,973 |
| `+0x37` | 1 | behaviour flag | `1` on 1,948/1,948 non-empty blocks |
| `+0x38` | `numkeys * 0x20` | position keys | see below |

Each `0x20`-byte key is exactly the source position key already documented in
`spline-animation` section 2.1:

| Key offset | Size | Role |
|---|---|---|
| `+0x00` | 2 | frame delta (`i16`) |
| `+0x02` | 6 | position x, y, z (`i16` each) |
| `+0x08` | 12 | outgoing tangent x, y, z (`i32` each) |
| `+0x14` | 12 | incoming tangent x, y, z (`i32` each) |

### 4.1 Validation

| Check | Result |
|---|---|
| Blocks decoded | 1,973 of 1,976 campaths (3 have `+0x38 == 0`) |
| Empty blocks (`numkeys == 0`) | 25 |
| Non-empty blocks | 1,948 |
| `+0x00 == block + 0x30` | 1,948/1,948 |
| `+0x30 == block + 0x38` | 1,948/1,948 |
| stride flag `+0x36 == 0` | 1,973/1,973 |
| behaviour flag `+0x37 == 1` | 1,948/1,948 |
| key total | 5,394 |
| key delta in `-100..=4096` | 5,394/5,394 |
| every tangent component magnitude within 1,000,000 | 5,394/5,394 |

The check that the layout is not a coincidence: for every non-empty block whose
successor block is present in the same unit, the distance to the next `+0x38`
offset is exactly `0x38 + 0x20 * numkeys` in 1,716/1,716 cases. The block is
laid out as a header plus a dense key array, and the count field predicts the
extent exactly.

### 4.2 The block is not campath-specific

The same `+0x38` shape occurs on non-campath records. Of the 355 non-campath
`+0x38` blocks in the corpus, 158 are non-empty and all 158 satisfy the header
(`+0x00`, `+0x30`, stride `0`, behaviour `1`); the rest are empty. Examples are
`spldum`, `srdoor` and `ocdoor` placements. So `+0x38` is a general per-object
transform spline, and campath is simply the object class that almost always has
one. This also corrects the cautious guess in `object-rotation-field` that the
`0x30`-byte sub-object element might hide a rotation matrix: the referenced
block is a *position* channel, and no rotation channel is present.

### 4.3 The runtime did not consume this in the engine

`--level-objects` prints name, position, type and flags; it does not expose
`+0x38`. The engine currently discards the spline. Whether the original runtime
plays it as a camera shot, an elevator or an actor route is not established
here (see UNKNOWN).

## 5. H1 and H3: there is no inter-record topology

Hypothesis H1's discriminating clause is "an integer that indexes another placed
record". Every aligned `u16` and `u32` field of the `0x4C` record was tested
against four target sets in the same unit: another campath's authored id
`+0x14`, another campath's record offset, another campath's `+0x38` block
offset, and a campath array slot.

| Field | Resolves to another campath id | Notes |
|---|---:|---|
| `+0x10` (best candidate) | 41/1,976 (2.07%) | 1,633/1,976 values happen to fall in `0..n_camp`, but the followed hops are unbounded |
| `+0x14` | self only | it *is* the record's own authored id |
| `+0x20`/`+0x22`/`+0x24` | <=10/1,976 | position components, chance collisions |
| `+0x26` | 0 | archetype code |
| `+0x38` | 0 | points at its own block, not another record |
| every other field | 0 | zero or chance collisions |

No field clears a chance baseline. `+0x10` is the only interesting one, and it
fails the follow test.

### 5.1 Hop histogram for the best candidate

Distances (in world units) when the candidate is followed. `+0x10 -> array`
maps the value to `campaths[value-1]`; `+0x10 -> id` maps it to a campath whose
authored id equals the value. A real path link would concentrate at small,
bounded hops.

| Bin | `+0x10 -> array` | `+0x10 -> id` | campath nearest neighbour | random campath pair |
|---|---:|---:|---:|---:|
| 0-100 | 4 | 0 | 157 | 12 |
| 100-250 | 13 | 4 | 219 | 10 |
| 250-500 | 44 | 2 | 305 | 35 |
| 500-1,000 | 118 | 5 | 476 | 141 |
| 1,000-2,000 | 333 | 11 | 420 | 257 |
| 2,000-4,000 | 522 | 9 | 263 | 381 |
| 4,000-8,000 | 421 | 10 | 74 | 342 |
| over 8,000 | 178 | 0 | 14 | 213 |
| n | 1,633 | 41 | 1,928 | 1,391 |

The followed distributions are not shifted toward small hops; they resemble the
random-pair baseline. `+0x10` is therefore a bookkeeping counter, not an index:
it does not identify a successor.

### 5.2 Chains, loops and branching

Because no link field exists, there are no chains to close, terminate or branch.
For completeness the 41 spurious `+0x10 -> id` matches were checked: each
successor has in-degree exactly one (no branching) but the mapping covers only
2% of nodes, so it is not a chain either. One node does not belong to exactly
one chain; it belongs to none.

### 5.3 Nearest-neighbour chaining is ill-defined

The brief's premise was a "guessed nearest-neighbour chain". That guess is not
merely unsupported; it is ambiguous:

| Measure | Value |
|---|---|
| campaths coincident with another campath (distance 0) | 52/1,928 |
| nearest/second-nearest ratio `< 1.5` (near-tie) | 1,061/1,828 (58%) |
| nearest/second-nearest ratio `>= 3` (clear choice) | 186/1,828 (10%) |
| nearest-neighbour hop median | 743 units |

More than half of all nodes have a near-tied second neighbour, so any
nearest-neighbour route is arbitrary at the tie.

## 6. H3: no per-node group or path id

Three small-integer fields could theoretically group nodes. None does:

| Key | Same-key nearest neighbour | Different-key nearest neighbour | Verdict |
|---|---:|---:|---|
| archetype `+0x26` | median 805 | median 1,243 | no meaningful split |
| instance `+0x28` low | median 994 | median 1,128 | no meaningful split |
| type `+0x28` high | median 755 | median 1,103 | 97% are type 0, so it cannot group |
| `+0x10` | median 6,034 | median 743 | only 12 nodes share a value, so it cannot group |

The clincher is `add1`: all 12 campaths share archetype 101 and instance 10326,
yet consecutive placements are up to ~6,000 units apart. A path id that puts all
12 nodes in one group while they are scattered cannot be a path id. `+0x10` is a
descending placement counter (it runs `n..1` down the campath run in many units,
though not in every unit), which duplicates array order and encodes no route.

## 7. What the spline keys actually are

The keys are ordered: `spline-animation` confirms the sampler advances keys by
accumulating the per-key delta. So one campath record describes one ordered
local trajectory.

| Measure | Value |
|---|---|
| `numkeys` distribution | 2:1,625, 3:157, 4:47, 5:25, 6:15, 7:12, 8:13, 9:5, 10:11, 11:7, 12:4, 13-18:17, 22-26:4, 35:1, 50:1, 81:2, 101:2, 0:25 |
| multi-key splines (`numkeys >= 2`) | 1,948 |
| stationary (every consecutive key hop is 0) | 1,101 |
| moving (total path length > 0) | 847 |
| moving splines with uniform hops (`max/min <= 1.5`) | 671 |
| moving splines with `>= 3` keys and uniform hops | 139 |
| moving path length, median / p90 / max | 1,195 / 10,203 / 48,364 units |
| moving key count, median / p90 / max | 2 / 6 / 101 |

A minority of records are unmistakably pre-sampled routes: five long uniform
splines step 11-26 units per key (`city2` two of 101 keys, `oracle15` two of 81
keys, `oracle9` one of 50 keys). Most records are short (median two keys) and
56% of multi-key splines are stationary (all keys at one point), which behaves
like a hold rather than a route. The first key's position equals the placed
position `+0x20` exactly in 980/1,948; the rest cluster near it, most often at a
`(0, +3..4, -18)` offset. Which of the two positions the runtime uses is not
established.

## 8. H4: local metadata, and what is absent

Present:

- position keys (section 4),
- per-key frame deltas (median 1, p90 120, max 1,200; the first key carries the
  record's movement duration, median 60, p90 200),
- in/out tangents for Hermite interpolation.

Absent:

- **orientation.** There is no yaw/pitch/roll, quaternion or matrix field in
  the record or the block. A campath carries a path, not a facing. (This agrees
  with `object-rotation-field`.)
- **speed.** The frame delta is a time, not a velocity; no speed field exists.
- **dwell.** The stationary splines look like holds, but there is no separate
  dwell field: a hold is just keys that do not move.

## 9. What a locomotion slice can and cannot rely on

Can rely on:

- Each campath record's `+0x38` block is an ordered, measured position spline.
  847 records actually move, and 139 multi-key moving splines have regular
  spacing. A follower can walk these keys with the stored timing and tangents.
- The layout (0x38 header, 0x20 key stride, field order) is corpus-validated and
  corroborated independently by `spline-animation`.
- `flags & 0x10000` reliably selects `campath` records.

Cannot rely on:

- Any route that strings several campath records together. No next-index,
  path id or group id exists. Do not build a nearest-neighbour chain: 58% of
  nodes have a near-tied second neighbour and the nearest hops are unbounded.
- Orientation or speed from the record. Both must come from elsewhere or be
  derived from consecutive keys at runtime.

## 10. Diagnostic: skipped, and why

A `--campath-dump` was considered and not added.

- `--level-objects` already prints every field the current reader exposes
  (name, position, type, flags), so a dump restricted to those fields duplicates
  it.
- Chain-following has no confirmed link field to follow; it would be dead code.
- The only genuinely new fact is the `+0x38` spline, and it is documented above
  with exact offsets and corpus-wide validation numbers. Encoding it as a CLI
  mode would require new `cli_catalog` entries and argv-mapping tests for a tool
  with no locomotion consumer yet.
- Keeping the slice code-free leaves both golden digests provably untouched.

If a later slice needs it, the natural shape is a read-only `--campath-dump`
sibling of `--level-objects` that prints `index pos=... id=... arch=...
keys=<n>` and, per key, `delta=... pos=...`.

## 11. UNKNOWN (explicit)

1. **What consumes the campath spline at runtime.** The record is an ordered
   position spline, but whether the original plays it as a camera shot, a moving
   platform, a door or an actor route is not established. `object-behaviour`
   section 5.2 already lists the consumer as UNKNOWN; this note narrows the data
   shape, not the consumer.
2. **Why 1,101 multi-key splines are stationary and 25 are empty.** Whether a
   stationary spline is a camera hold, an unused default, or a segment whose
   motion is supplied elsewhere is not known.
3. **The exact meaning of `+0x10`.** It descends across placements but is not a
   strict per-name reverse ordinal in every unit (it exceeds the unit's object
   count in some units), so its authoring role is not pinned. It is provably not
   a path id.
4. **Which position the runtime uses: the placed `+0x20` or the first key.**
   They coincide half the time and differ by a small offset otherwise.
5. **Whether an unshipped or edited level authors a rotation channel** in the
   `+0x38` block (none was seen; every non-empty block has a position channel).
6. **Coverage.** One shipped build. Other builds or regional variants are
   unobserved.

Evidence that would settle 1: a runtime trace of the function that reads a
placed record's `+0x38` pointer. Evidence for 3/4: a corpus of edited levels
where the editor's intent is known.

## 12. Reproduction

- Read the `BIGFILE.DAT` index and parse each entry as a `.PCM` with the engine's
  existing data-base and offset-table rules.
- Decode the room object array (`+0x08` count, `+0x0C` offset, `0x4C` stride)
  and filter `name == "campath"`.
- Decode `+0x38` as in section 4 and validate against the header and key tables.
- Follow every aligned field against the target sets in section 5 and compare
  the hop histogram with a random-pair baseline.
- Confirm the digests:
  `--play --headless --bigfile <DAT> --unit under1 --ticks 60` ->
  `ede39c540a10ee39`, and `--unit cathy28` -> `b8c30d28e0c67130`.

Scratch parsers lived in the session temp directory and are not committed.

## Legal

Read-only format note. No game bytes, no decompiled text, no listings, no
addresses, no RVAs and no absolute install paths are recorded. Only field
offsets, counts, roles and corpus measurements appear. The archive was never
written.

## Related notes

- `docs/notes/level-geometry.md` - the `0x4C` source record and the object array.
- `docs/notes/object-rotation-field.md` - the earlier no-rotation finding; this
  note identifies the block it left as a rotation candidate.
- `docs/notes/object-behaviour.md` - `campath` is not an actor; the consumer is
  the remaining UNKNOWN.
- `docs/notes/spline-animation.md` - the independently documented `0x20`
  position-key shape this block reuses.
- `docs/notes/object-types.md` - the archetype/type census.

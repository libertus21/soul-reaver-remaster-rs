# level-transitions - do portal objects carry a resolvable target unit?

Read-only census of all 372 archived `.PCM` units and a read-only record dump of
every placed `portal` object. Behaviour notes only: counts, distinct integer
values, unit names and field offsets. No game bytes, no raw record dumps, no
addresses, no absolute install paths. The scratch parser stayed in a temp
directory and the archive was opened read-only.

Date: 2026-11-24. Evidence class per claim: **CONFIRMED** (code/bytes agree, or
two independent observations agree), **INFERRED** (one side only), **UNKNOWN**.

## Verdict

**UNKNOWN. No portal object carries a resolvable target unit, so no level
transition was implemented.**

- **H1 (`portal.type` is the target unit index): REFUTED.** Across 166 portals
  there are only four `type` values (0, 3, 6, 12). A unit index would spread
  over hundreds of the 372 units; this is an enum of portal kinds, which matches
  the already-confirmed reading of `type` as a coarse per-instance class.
- **H2 (another placed-record field carries the target): REFUTED.** Every
  unprinted field was inspected on all 166 portals: `archetype`, `instance_id`,
  `flags`, and the tail `+0x30..+0x4B`. No field is a name string, no field is a
  unit index, and no field resolves to a unit. `instance_id` never coincides with
  a link-table target id (0/166).
- **H3 (linkage lives outside the record): partially true, but it does not
  pass the gate.** The per-unit link table is real and its targets resolve to
  existing units (855/855 real edges, 96.8% pooled). But it is a **unit-level,
  bidirectional connection graph with no per-portal anchor**: only 15 of 136
  portal units have a 1:1 portal-to-link cardinality, and the only other numeric
  field (`prelude[1]`) is provably the partner link's connection id, not an
  object index. Joining a specific portal to a link record would be invention.

Per the gate rule ("proceed only if a candidate resolves to existing units at a
rate far above chance, with a per-unit breakdown ... otherwise STOP and declare
UNKNOWN"), the candidate that resolves units cannot be attached to any portal
object, so the gate fails and Phase 2 is not implemented.

## Method

- `--level-reach --bigfile BIGFILE.DAT` discovers **372** units from the shipped
  SR1 archive (`start=under1 discovered=372 ... unparseable=0`).
- A read-only scratch parser in a temp directory replicated the *existing*
  reader's rules (PCM offset table, 2048-byte data base, 12-byte name +
  `+0x20` position + `+0x26` archetype + `+0x28` id/type dword + `+0x2C` flags,
  `0x4C` stride; link table at slot `0x34` bounded by `0x35`, `1,n,18,n,id` prelude,
  40-byte stride). It dumped every placed record and every link record. Nothing
  was extracted and no archive byte entered the repo.
- Corpus size: **6142** placed objects over the 372 units.

## H1 - the `portal.type` histogram

There are **166** `portal` objects in **136** units.

| `type` | portals | distinct units | `archetype` seen |
|---:|---:|---:|---|
| 0 | 55 | 50 | 157 |
| 3 | 10 | 7 | 452, 473 |
| 6 | 91 | 75 | 672 |
| 12 | 10 | 5 | 905 |

Four distinct values over 166 objects. The distinct-unit column sums to 137, not
136, because one unit (`pillar3`) carries portals of two kinds. Portals per unit:

| portals in unit | units |
|---:|---:|
| 1 | 114 |
| 2 | 15 |
| 3 | 6 |
| 4 | 1 |

An enum of four portal kinds, **not** a unit index. (CONFIRMED.)

## H2 - field-by-field audit of the placed record

The reader prints name, position, `type` and `flags`. The unprinted fields and
their distributions over all 166 portals:

| field | distinct values | behaviour | resolves to a unit? |
|---|---|---|---|
| `archetype` (`+0x26`) | 5 (157, 452, 473, 672, 905) | fixed per kind (type 3 has two) | no |
| `instance_id` (`+0x28` low) | 20, in four dense clusters | per-archetype instance counters | no |
| `flags` (`+0x2C`) | 2 (0, and 0x80 once) | constant 0 for 165/166 | no |
| `+0x30`..`+0x4B` | all zero except 6 records | see below | no |

- The 12-byte name field is always the literal `portal`; there is no name string
  anywhere else in the record.
- `instance_id` clusters (`0x2000`, `0x60DF`-ish, `0x6B48`, `0x7FFF`, `0xE7F8`)
  are archetype-instance counters. The set has **zero** intersection with the
  372 link-table target ids (0/166).
- The tail is zero for 160/166 portals. Six records set `+0x44` and/or `+0x48`
  to large integers (roughly `2.3e3` to `4.3e9`). None decodes as ASCII, none is
  an existing unit name, and none is small enough to be one of the 372 unit
  indices.

No numeric id, no name string, nothing to resolve. (CONFIRMED.)

## H3 - the separate link table, and why it does not attach to a portal

The link graph is real: **883** records across **371** units. 28 of those are the
`warpgate` pseudo-targets belonging to the documented 14-unit warp cycle; the
other **855** are real unit edges.

| test | result |
|---|---|
| real edges resolving to a discovered unit | 855 / 855 = 100% |
| pooled (including 28 `warpgate` pseudo-targets) | 855 / 883 = 96.8% |
| units where every target resolves | 357 / 371 (the 14 exceptions are exactly the warp units) |
| edges that stay in the same naming family | 717 / 855 = 83.9% |

So the **unit graph** passes a resolution test with a per-unit breakdown. What
fails is attaching an edge to a **portal object**:

- **Cardinality.** Of 136 portal units, only **15** have exactly one portal and
  one real link. 45 units have 1 portal + 2 links, 23 have 1 + 3, 13 have 1 + 4,
  and so on up to 1 portal + 9 links and 4 portals + 3 links. A unit-level rule
  cannot choose a target for a given portal.
- **Keys.** No portal field indexes the link table. `instance_id` is not in the
  target-id domain (0/166). The only apparent index match is `object_type` equal
  to a link's *ordinal*, which fires twice out of 166 and is an artifact (e.g.
  type 6 equals the sixth link).
- **`prelude[1]` is not an object index.** This is the one remaining numeric
  field, and it is explained:
  - 758 / 855 = **88.7%** of `prelude[1]` values equal the *reverse* link's
    `index`. The two numbers are paired connection ids (`A -> B, index n` is
    answered by `B -> A, index p` with `prelude[1] = n`).
  - 69% of `prelude[1]` values exceed their own unit's object count; the range is
    1..300 against object counts of 0..233. It cannot be an object-array index.
  - A brute-force join `prelude[1] == portal objidx` yields **12** hits against
    an expected **~9.1** by chance (1.3x). The cleanest-looking match (two
    portals, two links, both aligned) is fully explained by the connection-id
    symmetry, not by an object link.

Joining a portal to one of these records would invent a link, which the task
forbids. (CONFIRMED for the symmetry and the chance estimate; INFERRED that no
other join exists in this corpus.)

## Door / warp objects

| name | objects | role |
|---|---:|---|
| `warpg` | 28 | warp-gate marker (2 per warp unit) |
| `wrpface` | 14 | warp-face marker (1 per warp unit) |
| `wrpdoor` | 11 | warp door |
| `sudoor` | 29 | door |
| `catdoor` | 2 | door |
| `bellsc` | 3 | prop |
| `wportal` | 9 | warp portal |
| `sugate` | 8 | gate |

- `warpg`/`wrpface` are the membership markers for the 14-unit warp cycle. They
  carry no target: `warpg` instances are 16383/16384 and `wrpface` is 50190 with
  flag `0x80`, constant across all 14 units. The warp destination comes from the
  hardcoded 14-name cycle, not from the record (see `warp-level-selection.md`).
- On the sampled `wrpdoor`, `sudoor`, `catdoor`, `bellsc`, `wportal` and `sugate`
  records the `+0x30`, `+0x38`, `+0x3C` and `+0x40` fields are zero. None carries
  a target either.
- Portal objects are never within 512 world units of a warp object in the same
  unit (0/15 co-located cases; 151 of 166 portals have no warp object in their
  unit at all). Portals and warp gates are distinct mechanisms.

## Gate result

| hypothesis | verdict | decisive number |
|---|---|---|
| H1 `portal.type` = unit index | REFUTED | 166 portals, 4 type values |
| H2 another field = target | REFUTED | 0/166 fields resolve to a unit |
| H3 external table | fails the join | 855/855 edges resolve, but 0 per-portal anchors; 12 vs ~9.1 chance |

**UNKNOWN.** No implementation.

## What was not implemented

Nothing in the engine changed. In particular:

- no proximity link in `--play`, no `--no-transitions` flag, no
  `transition from=... to=... at=... reason=...` console line;
- no change to unit loading, collision, placed objects, actor rebuild or player
  binding;
- no change to the headless digest. Because no code path changed, the documented
  `--play` digest still verifies unchanged.

### Chosen placement rule and digest treatment

Not applicable. A placement rule ("target portal's counterpart, else the target
unit's `raziel` spawn") presupposes a portal-to-target mapping, and the gate
showed that mapping does not exist in the authored data. With no mapping there
is no transition to fold into the digest, so the digest stays a function of the
single loaded unit only.

### Explicit limits

- All four portal kinds (0, 3, 6, 12) are ignored; none is a transition trigger.
- The 14-unit warp cycle is out of scope here and remains driven by the
  hardcoded name table documented in `warp-level-selection.md`.
- A unit-level "any portal -> the unit's single real link" shortcut was rejected:
  it is ambiguous in 121 of 136 portal units and would invent a link.

## What would unblock this

Ordered by cost:

1. A runtime trace of the portal handler that reads the link record, capturing
   which link record a given portal object uses (item 1 of the open UNKNOWN list
   in `warp-level-selection.md`). The method and probe for this trace are defined
   in `docs/notes/warp-trace.md`; it is awaiting a live run.
2. A corpus unit with equal portal and link counts where the link-record order
   demonstrably follows the portal object order, plus a matching reverse edge.
3. A placed-record field, in any shipped unit, that indexes the link table.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| L1 | 166 `portal` objects in 136 units; `type` takes 4 values. | CONFIRMED |
| L2 | No unprinted placed-record field is a unit name, unit index or target id. | CONFIRMED |
| L3 | `instance_id` and link `target_id` domains are disjoint (0/166). | CONFIRMED |
| L4 | 855/855 real link edges resolve to discovered units; 96.8% pooled. | CONFIRMED |
| L5 | `prelude[1]` equals the reverse link's `index` in 88.7% of edges. | CONFIRMED |
| L6 | `prelude[1]` cannot be an object index (69% exceed object count). | CONFIRMED |
| L7 | The portal-to-link join is at chance level (12 vs ~9.1). | INFERRED |
| L8 | The portal/link association is available only at runtime, not in authored data. | UNKNOWN |

`docs/notes/warp-trace.md` defines the runtime probe that targets L8: it watches
the resident link table and reports the reader and the entry index used when a
portal is crossed, then prints that entry as a decoded unit edge. No live run has
been recorded yet, so L8 stays UNKNOWN.

## Method box and legal

One bounded read-only pass: `--level-reach` for discovery, then a scratch
read-only parser in a temp directory for the object and link dumps. No Ghidra
run this pass (the binary side was already covered by `object-types.md` and
`warp-level-selection.md`). Behaviour notes only: counts, integer values, unit
names and field offsets. No game bytes, no listings, no decompiled bodies, no
addresses or RVAs, no absolute install paths entered the repo.

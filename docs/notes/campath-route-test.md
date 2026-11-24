# campath-route-test - do moving campath splines start on placed NPCs? (2026-11-24)

Offline, read-only decisive test of the standing hypothesis that a campath
record's `+0x38` position spline is an **NPC patrol route**. The test asks one
falsifiable question: do the moving splines *start* at the positions where the
game has placed known enemies? No game session is needed; the answer comes from
the shipped archive plus the established corpus parser and the probe's key
validator, which were reused unchanged.

This note adds no production code, no CLI mode and no dependencies. It moves no
digest. Scratch analysis code lived in the session temp directory and is not
committed.

**Verdict: REFUTED.** Moving campath splines do **not** start on placed NPCs.
Across 847 moving splines, only 10 (1.9%) start within 500 world units of a known
enemy, **none** starts within 250, and the closest of all 520 moving splines in
enemy-bearing units is 250.4 units away. Static splines behave the same, so the
metric does not even separate moving from static.

## 1. The question and the operational test

The route hypothesis predicts that a moving spline's first key sits on (within
about one body) a placed enemy record. If true, the first-key-to-nearest-enemy
distance should be tiny for moving splines, much larger for random points in the
same room, and larger still for static splines. If the distances are
indistinguishable from the control, the hypothesis is refuted.

For every unit that has both campaths and placed actor records:

1. Extract each campath's spline keys in order and the campath's own placed
   position.
2. Split splines into `moving` (any two consecutive keys differ) and `static`
   (all keys identical). This reproduces the established split: 847 moving,
   1,101 static.
3. Measure the 3D Euclidean distance from the **first key** to the nearest
   placed record of a known enemy type in the same unit, and to the nearest any
   non-campath placed record.
4. Repeat for the campath's **own placed position**, which is the placement
   control: if the first key merely inherits the placement, the two distances
   must match.
5. Control: random points inside the unit's room-geometry bounding box, measured
   the same way.

Known enemy names used (case-insensitive, from the measured label set): sluagh,
hunter, skinner, vwraith, morlock, aluka, kain, wrshp, walboss, walbosb, ronin,
wallcr, alukabss, roninbss, skinbos. `raziel` and `soul` are excluded from the
enemy metric and included only in the wider "actor" metric.

World units are the shipped `i16` coordinates; the world is Z-up, so the
horizontal plane is X/Y. Both 3D and horizontal-only distances are reported, to
rule out vertical inflation.

## 2. Corpus and sanity

| Item | Value |
|---|---|
| Units parsed | 372 |
| Units that place `campath` | 233 |
| `campath` records | 1,976 |
| Splines with a valid header and >=1 key | 1,948 |
| Moving splines | 847 |
| Static splines | 1,101 |
| Units with at least one known enemy | 113 |
| Moving splines in enemy-bearing units | 520 (327 are in enemy-free units) |
| Units with at least one moving spline and one enemy | 84 |
| Room geometry parsed | 372 units |
| Placed objects inside their unit's geometry AABB | 5,884 / 6,142 (95.8%) |

The 95.8% containment is the geometry-parser sanity check: the room-geometry
AABB was read with the engine's own record rules, and essentially all placed
records fall inside it. Geometry is available for every unit, so no row is
dropped for missing bounds.

Model scale for reading the numbers: the measured humanoids span 483-686 units
vertically and 89-412 units horizontally (`object-up-axis`). A 250-unit
separation is therefore about one body radius; a 3,193-unit separation is five
to seven body heights.

## 3. Distance distributions

All distances are world units. `n` counts splines that have a target in the same
unit (a spline in an enemy-free unit has no enemy distance and is excluded from
that row only).

| Population | n | min | p10 | median | p90 | <=50 | <=100 | <=250 | <=500 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **Moving** first key -> nearest enemy | 520 | 250 | 1,068 | **3,193** | 7,933 | 0.00% | 0.00% | 0.00% | 1.92% |
| **Static** first key -> nearest enemy | 663 | 190 | 1,021 | **3,292** | 7,186 | 0.00% | 0.00% | 0.30% | 2.41% |
| Moving first key -> nearest any non-campath record | 841 | 31 | 263 | 947 | 3,017 | 0.95% | 4.28% | 9.16% | 25.21% |
| Static first key -> nearest any non-campath record | 1,096 | 13 | 255 | 780 | 1,955 | 0.73% | 1.73% | 9.40% | 32.30% |
| Moving first key -> nearest actor (enemy/raziel/soul) | 836 | 50 | 788 | 2,326 | 5,557 | 0.12% | 0.12% | 0.36% | 3.95% |
| Moving **own placed pos** -> nearest enemy | 520 | 198 | 1,074 | 3,181 | 8,032 | 0.00% | 0.00% | 0.19% | 1.92% |
| Static **own placed pos** -> nearest enemy | 663 | 176 | 1,021 | 3,281 | 7,186 | 0.00% | 0.00% | 0.30% | 2.41% |
| Control: random point in geometry AABB -> nearest enemy | 5,650 | 204 | 1,973 | 4,743 | 10,991 | 0.00% | 0.00% | 0.02% | 0.14% |
| Control: random point, floor-matched Z -> nearest enemy | 5,650 | 73 | 1,500 | 4,049 | 9,773 | 0.00% | 0.04% | 0.16% | 0.85% |

Histogram of first-key-to-enemy distance (counts):

| Bin | Moving | Static | Control 3D AABB | Control floor-Z |
|---|---:|---:|---:|---:|
| 0-100 | 0 | 0 | 0 | 0 |
| 100-250 | 0 | 2 | 1 | 7 |
| 250-500 | 10 | 14 | 7 | 39 |
| 500-1,000 | 34 | 48 | 89 | 218 |
| 1,000-2,000 | 114 | 106 | 485 | 722 |
| 2,000-4,000 | 172 | 266 | 1,728 | 1,791 |
| 4,000-8,000 | 139 | 171 | 2,093 | 1,964 |
| over 8,000 | 51 | 56 | 1,247 | 907 |
| no enemy in unit | 327 | 438 | n/a | n/a |

Horizontal-only (X/Y) distances confirm the 3D picture and are not an artifact
of height:

| Population | n | min | p10 | median | p90 | <=50 | <=100 | <=250 | <=500 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Moving first key -> nearest enemy (X/Y) | 520 | 39 | 691 | 2,665 | 7,036 | 0.19% | 0.77% | 2.12% | 6.54% |
| Static first key -> nearest enemy (X/Y) | 663 | 32 | 688 | 2,885 | 6,700 | 0.15% | 0.75% | 2.41% | 6.18% |

Four moving splines are within 100 units horizontally of an enemy (1 is within
50); the static population is identical (0.75% within 100). The vertical offset
adds a few hundred units, but even ignoring height the moving splines are
thousands of units from enemies.

## 4. Controls and what they show

- The **own-placement control** is the decisive one. The first key and the
  campath's own placed position give the same distribution: medians 3,193 vs
  3,181, within-500 1.92% for both. The first key adds no NPC association
  beyond where the campath object was already placed.
- The **random-in-geometry control** is farther on average (median 4,743 within
  the full 3D box, 4,049 when the Z is drawn from placed objects). Moving
  splines are 33% closer than the 3D control (median) and 21% closer than the
  floor-matched control. That shift is real but small and is exactly what
  placement-density predicts: campath objects are authored inside populated
  rooms, and enemies cluster in the same rooms. It is not "on an NPC".
- The **static control** destroys the discriminative claim. If moving splines
  encoded NPC routes, they should be far closer to enemies than static splines.
  They are not: static is marginally *closer* at every tight threshold
  (within 250: 0.30% vs 0.00%; within 500: 2.41% vs 1.92%). The metric does not
  separate the two populations.

## 5. Per-unit results

The effect is not just weak in the pooled number; it is inconsistent per unit.
Of the 84 units with at least one moving spline and at least one enemy:

- Moving first-key median is closer to an enemy than that unit's random-geometry
  median in **49 / 84 units (58%)** — close to a coin flip.
- Of the 29 units with at least 3 moving and at least 3 static splines, moving is
  closer than static in **15 / 29 (52%)**.
- The moving/control median ratio ranges from **0.07** (`cathy18`) to **3.18**
  (`stone3`); in 35 of 84 units the moving splines are *farther* than chance.
- **No unit** has a moving spline within 250 units of an enemy. The closest of
  all is 250.4 units.

Representative per-unit table (`mov`/`stat` are counts with an enemy in the same
unit; medians are first-key-to-nearest-enemy in world units; `ctrl` is that
unit's random-in-geometry median):

| Unit | mov n | mov median | stat n | stat median | ctrl median | mov/ctrl |
|---|---:|---:|---:|---:|---:|---:|
| `city2` | 2 | 4,483 | 3 | 7,186 | 5,779 | 0.78 |
| `oracle15` | 5 | 2,415 | 2 | 2,850 | 2,895 | 0.83 |
| `oracle9` | 1 | 766 | 1 | 2,603 | 5,309 | 0.14 |
| `oracle5` | 9 | 6,939 | 4 | 2,638 | 6,467 | 1.07 |
| `cathy55` | 55 | 1,635 | 28 | 1,829 | 4,227 | 0.39 |
| `nightb3` | 40 | 1,780 | 30 | 886 | 8,916 | 0.20 |
| `skinnr9` | 48 | 3,150 | 43 | 3,159 | 8,488 | 0.37 |
| `aluka6` | 24 | 2,410 | 35 | 2,306 | 3,389 | 0.71 |
| `chrono1` | 23 | 3,002 | 51 | 3,111 | 8,828 | 0.34 |
| `pillars1` | 26 | 3,466 | 105 | 3,548 | 9,058 | 0.38 |
| `train9` | 21 | 2,722 | 15 | 963 | 4,699 | 0.58 |
| `stone1` | 12 | 5,726 | 5 | 17,280 | 14,821 | 0.39 |
| `stone3` | 1 | 10,429 | 0 | - | 3,277 | 3.18 |
| `cathy25` | 2 | 33,275 | 3 | 2,750 | 18,298 | 1.82 |
| `cathy22` | 2 | 28,403 | 2 | 29,949 | 17,913 | 1.59 |
| `nightb5` | 3 | 15,170 | 14 | 13,510 | 11,495 | 1.32 |
| `add1` | 6 | 4,984 | 6 | 3,919 | 8,268 | 0.60 |
| `cathy18` | 3 | 1,223 | 5 | 3,388 | 16,384 | 0.07 |

The two long `city2` routes (101 keys each) start 4,483 and 3,789 units from the
nearest enemy; the two `oracle15` routes (81 keys) 2,415 and 1,841; the
`oracle9` route (50 keys) 766. None is on an NPC.

## 6. The closest matches

Even the closest matches are about one body radius away, not on the body:

| Unit | keys | first-key -> enemy | nearest enemy |
|---|---:|---:|---|
| `train9` | 2 | 250.4 | ronin |
| `aluka6` | 2 | 260.1 | alukabss |
| `train9` | 3 | 267.4 | ronin |
| `chrono1` | 2 | 308.4 | Kain |
| `nightb3` | 2 | 328.5 | roninbss |
| `skinnr9` | 2 | 358.8 | skinbos |
| `aluka6` | 2 | 423.3 | alukabss |
| `huba17` | 3 | 444.5 | skinner |
| `out1` | 2 | 495.5 | skinner |

The closest is 250.4 units, and it is a 2-key micro-spline, not a route. Only 10
of 520 moving splines are within 500 units, and they are spread over 7 units
(`train9`, `aluka6`, `chrono1`, `nightb3`, `skinnr9`, `huba17`, `out1`).

## 7. Geometry fit (horizontal extent vs room bounds)

The moving spline's horizontal (X/Y) key extent was compared with the unit's
room-geometry horizontal AABB (the engine's room record: vertex count/offset and
the `i16` vertex box).

| Measure | Value |
|---|---|
| Moving splines with geometry | 847 |
| Horizontal key AABB fully inside unit geometry AABB | 737 / 847 (87.0%) |
| Long uniform routes fully inside | 5 / 5 (`city2` x2, `oracle15` x2, `oracle9`) |
| Largest outside counts | `train3` 8, `stone1` 3, then 2 each in `under3`, `huba8`, `cathy62`, `cathy58`, `chrono17`, `oracle1`, `hubb4`, `fill3`, `clfpil3`, `city6` |

The five long uniform routes all stay inside their room's horizontal bounds, and
87% of moving splines do. This is consistent with "a patrol stays inside" but it
does **not** discriminate: a fixed camera rail, a moving platform or a door rail
also stays inside a room. The 13% that leave the bounds are concentrated in a
few units and several long outdoor (`out*`) splines, so "always inside" is not a
universal rule either.

## 8. Verdict

**REFUTED.** The operational prediction — moving splines start at placed enemy
positions — fails on every axis:

1. **No moving spline starts on an enemy.** 0 / 520 within 250 units; 10 / 520
   (1.92%) within 500; the closest of all is 250.4 units, about one body radius.
   The median is 3,193 units, five to seven body heights.
2. **The first key only inherits the placement.** The campath's own placed
   position gives the same distribution (median 3,181, within-500 1.92%).
3. **Static splines are as close or closer.** Within 250: static 0.30% vs moving
   0.00%; within 500: static 2.41% vs moving 1.92%. The metric does not
   separate moving from static, so it cannot support a routing claim.
4. **The effect is inconsistent per unit.** Moving is closer than chance in 49 /
   84 units (58%), moving is closer than static in 15 / 29 units (52%), and the
   moving/control ratio spans 0.07 to 3.18. No unit disagrees in the useful
   direction (none puts a moving spline on an enemy).
5. **The residual signal is placement density, not NPCs.** Moving splines are
   33% closer than a uniform 3D control and 21% closer than a floor-matched
   control, but still thousands of units away. That is what authored placement
   inside populated rooms produces.

The runtime context agrees with this: in the live `oracle15` session, 76 blocks
were resident and an access monitor on 7 of them saw **0 reads in 60 s** of
steady state, so the runtime does not sample these splines per frame. A
per-frame NPC locomotion route is not supported either by the data geometry or
by the live sampling.

Geometry-fit is consistent with a room-bound object (87% inside; the five long
routes inside) but is not discriminative between a patrol and a camera/platform
rail.

## 9. UNKNOWN (explicit)

1. **Which code reads `+0x38`, and when.** The live steady-state 0-read result
   points to a load-time reader, but no stack was captured. Still UNKNOWN.
2. **What the animated transform drives.** This test refutes "starts at a placed
   NPC", not every possible notion of a route. A spline could still be a camera
   rail, a moving platform, a door rail, or a patrol polyline authored
   independently of enemy spawn points. The test cannot separate those.
3. **Why 327 moving splines are in enemy-free units.** That is consistent with
   camera/platform/scripted objects, but not proven.
4. **The far-first-key placements** (`aluka27`, `chrono19`, `train5`) are real
   data; their first key is deliberately offset or the placed position is a
   separate anchor. Not resolved here.
5. **The 110 moving splines whose horizontal extent leaves the room bounds.**
   Whether those are outdoor overflow, multi-room rails, or authored beyond
   geometry is not established.
6. **Coverage.** One shipped build; other builds or regional variants are
   unobserved.

What would change the verdict: a moving spline whose first key lands within one
body radius of an enemy, in several units, while static splines do not. The
shipped corpus has none.

## 10. Reproduction

- Reuse the established corpus parser (same rules as `campath-format`: 372
  units, 1,976 records) and the probe's existing key validator. No parser was
  rewritten.
- Split splines into moving/static and measure first-key-to-nearest-enemy,
  first-key-to-any-non-campath, own-placed-to-enemy, and random points in the
  room-geometry AABB. Enemies are the case-insensitive set in section 1.
- Confirm geometry containment: 5,884 / 6,142 placed objects inside their unit's
  geometry AABB.
- The scratch driver and its JSON are not committed.

## Legal

Read-only format/behaviour note. No game bytes, no decompiled text, no listings,
no addresses, no RVAs and no absolute install paths are recorded. Only field
offsets, counts, roles and corpus measurements appear. The archive was never
written.

## Related notes

- `campath-format` - the `+0x38` position-spline decode (spline shape and the
  847-moving count this test reproduces).
- `campath-consumer` - the runtime reader method; its UNKNOWN list now points
  here for the "NPC patrol" candidate.
- `object-behaviour` - `campath` never becomes an actor.
- `object-up-axis` / `up-axis` - Z-up world and the humanoid spans used to read
  the distances.
- `level-geometry` / `room-geometry` - the room record and geometry bounds the
  fit and control use.

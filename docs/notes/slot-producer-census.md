# slot-producer-census - limbs are not AIM_MODE, but PACKED_CHANNEL is not
# established: the three-way per-slot census is UNCLASSIFIED (2026-11-12)

This slice runs the census that `docs/notes/angle-array-source.md` named as its
single next experiment. It reads the shipped object containers and the existing
runtime descriptor dump, applies the one producer selector that the code and data
actually support, and reports the rest honestly as UNCLASSIFIED rather than
inventing a mode bit. Nothing was launched, attached or instrumented here. No
engine code changed and no golden moved.

Evidence classes: **CONFIRMED** (code and data agree, or two independent
observations agree), **INFERRED** (code reading or structural inference, one side
unobserved), **UNKNOWN** (not established).

## Headline

**Limb interior slots are not shown to be PACKED_CHANNEL; the three-way census is
UNCLASSIFIED for the bulk of every body.**

- The **only** per-slot selector that is established from shipped data is the
  **aim offset at `+20`**. A non-zero offset marks an `AIM_MODE` slot; that is the
  whole of the positive result.
- On **raziel** (66 slots) that assigns **4** slots: the object-root joint (slot
  0, from the object's live aim words) and three per-slot aim joints (1, 31, 41).
  The other **62** slots are **UNCLASSIFIED**, not PACKED.
- On the contrast set, `AIM_MODE` is 5 / 35 on **skinner**, 6 / 24 on **sluagh**,
  and 5 / 37 on **morlock**; everything else is UNCLASSIFIED.
- No slot can be assigned `PACKED_CHANNEL` or `QUATERNION_TRACK`, because the
  representation flag / mode bits that select them are **not located** in the
  file-form record (the earlier mode scan was negative) and the packed-channel
  payload is **not decoded**.
- No slot is assigned `NONE`: the `+6`/`+10` "sentinel" reading used to gate
  inactive slots does **not** hold on the real records (see Contradictions).

So the answer to "do limbs get packed data or computed aim?" is: **the bulk of
the limbs is neither confirmed packed nor shown to be aim**. The census narrows
offline reconstruction; it does not expand it.

## Quick path

1. Read "Verdict" for the per-actor aggregate.
2. Read "The classification rule" for the exact fields and why it stops where it
   does.
3. Read "Raziel per-slot table".
4. Read "Contradictions" before reusing the `+6`/`+10` sentinel claim.
5. Read "What this means for offline reconstruction" and "UNKNOWN".

## Verdict

| Actor | Slots | AIM_MODE | PACKED_CHANNEL | QUATERNION_TRACK | NONE | UNCLASSIFIED |
|---|---:|---:|---:|---:|---:|---:|
| raziel (player) | 66 | 4 | 0 | 0 | 0 | 62 |
| skinner | 35 | 5 | 0 | 0 | 0 | 30 |
| sluagh | 24 | 6 | 0 | 0 | 0 | 18 |
| **total (three)** | **125** | **15** | **0** | **0** | **0** | **110** |
| morlock (extra) | 37 | 5 | 0 | 0 | 0 | 32 |

`AIM_MODE` counts include the object-root joint (slot 0), whose orientation comes
from the object's live aim words, not from a per-slot descriptor offset. The
per-slot aim sub-slice slots are raziel {1, 31, 41}, skinner {1, 14, 18, 27},
sluagh {1, 8, 10, 14, 20} and morlock {1, 14, 18, 27}.

Per-slot detail for the decisive model, raziel:

| slot | parent | kind | evidence |
|---:|---:|---|---|
| 0 | -1 | AIM_MODE | object-root joint; live object aim words (INFERRED) |
| 1 | 0 | AIM_MODE | per-slot aim offset; non-empty aim sub-slice (2 frames) |
| 2 | 1 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 3 | 2 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 4 | 1 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 5 | 4 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 6 | 5 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 7 | 6 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 8 | 7 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 9 | 8 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 10 | 5 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 11 | 10 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 12 | 11 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 13 | 12 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 14 | 1 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 15 | 14 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 16 | 15 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 17 | 16 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 18 | 17 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 19 | 18 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 20 | 18 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 21 | 17 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 22 | 17 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 23 | 17 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 24 | 16 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 25 | 24 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 26 | 24 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 27 | 24 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 28 | 15 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 29 | 28 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 30 | 29 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 31 | 30 | AIM_MODE | per-slot aim offset; non-empty aim sub-slice (1 frame) |
| 32 | 31 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 33 | 32 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 34 | 31 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 35 | 34 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 36 | 31 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 37 | 36 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 38 | 15 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 39 | 38 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 40 | 39 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 41 | 40 | AIM_MODE | per-slot aim offset; non-empty aim sub-slice (1 frame) |
| 42 | 41 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 43 | 42 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 44 | 41 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 45 | 44 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 46 | 41 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 47 | 46 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 48 | 41 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 49 | 48 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 50 | 15 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 51 | 50 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 52 | 51 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 53 | 52 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 54 | 53 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 55 | 53 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 56 | 52 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 57 | 51 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 58 | 15 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 59 | 58 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 60 | 59 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 61 | 60 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 62 | 61 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 63 | 61 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 64 | 60 | UNCLASSIFIED | no aim; packed/quat selector unlocated |
| 65 | 59 | UNCLASSIFIED | no aim; packed/quat selector unlocated |

The aim subset is not a limb-interior set. In the parent tree it is the object
root plus the first child of the root (the trunk joint, whose subtree is almost
the whole body) plus the ends of the symmetric limb chains (raziel 31 and 41 are
the ends of two seven-deep chains, with 6 and 8 descendants respectively). On
skinner, morlock and sluagh one mid-chain joint is also marked. The arms/legs
*interiors* carry no aim offset.

## The classification rule

The role chain names three producers behind a slot's angle triple
(`docs/notes/angle-array-source.md`): a **packed-channel decoder**, a
**quaternion-track sampler**, and a **mode applier** (aim/target). The census can
only key each producer on data that is actually located. This is what is known:

| Producer | Selector in the data | Located? | Class |
|---|---|---|---|
| AIM_MODE (per-slot) | non-zero aim offset at `+20`; the loader relocates it into a pointer in the expanded memory record | **Yes** | CONFIRMED (presence), INFERRED (producer) |
| AIM_MODE (object root) | the object's live aim words, read by the reverse-order constructor | Yes, in code; not a descriptor field | INFERRED for slot 0 |
| QUATERNION_TRACK | a per-slot entry in the animation record's track table (4-float keyframe pair) | Not in the 24-byte descriptor; a separate record | UNKNOWN per slot |
| PACKED_CHANNEL | per-slot per-axis packed channel (three axis bit planes; palette index or literal; frame interpolation) | Not in the 24-byte descriptor; payload not decoded | UNKNOWN per slot |
| NONE | no orientation produced for the slot | No reliable marker | NOT ASSIGNABLE |

The rule the census can defend is therefore:

```text
kind(slot) =
    AIM_MODE        if slot is the object root              (INFERRED)
    AIM_MODE        if slot.aim_offset != 0                 (CONFIRMED data, INFERRED producer)
    UNCLASSIFIED    otherwise
```

`PACKED_CHANNEL`, `QUATERNION_TRACK` and `NONE` have no data-keyed selector that
this slice can locate, so no slot is assigned to them.

### Fields the rule keys on

All offsets are in the 24-byte **file-form** descriptor record (the same record
`object::Bone` reads). Values are stated in decimal.

| Field | Offset | Observed role | Class |
|---|---:|---|---|
| bone word | `+6` | unresolved small word on active slots; 65535 also appears on active, aim-bearing and child-bearing slots | UNKNOWN |
| vertex range start | `+8` | first vertex bound of the bone | CONFIRMED |
| vertex range end | `+10` | last vertex bound; 65535 means an empty range | CONFIRMED |
| local translation | `+12`, `+14`, `+16` | signed rest-offset triple; present on every slot, so not a mode selector | CONFIRMED |
| parent | `+18` | signed parent bone index, -1 is the root | CONFIRMED |
| aim offset | `+20` | base-relative offset of the slot's aim sub-slice; 0 means absent | CONFIRMED |

The aim sub-slice structure was re-validated here on all four models: each is a
24-byte header followed by `frame_count` 20-byte frames, and every aim slot has a
**non-zero frame count** (raziel slot 1 has 2 frames; every other aim slot on all
four models has 1). No aim slot has an empty sub-slice.

### Why the rule stops here

- `docs/notes/frame-mode-composition.md` §B scanned the named descriptor words
  and found **no** field that partitions static from procedural slots.
- `docs/notes/angle-array-source.md` lists the representation flag's **scope** as
  UNKNOWN, and "which producer filled each observed slot" as UNKNOWN.
- `docs/notes/pose-writer-identity.md` records a **mode byte** dispatch in the
  per-slot evaluator, but not its file offset; the code path is known, the data
  bit is not.

The honest consequence is a two-valued census (AIM vs UNCLASSIFIED), not a
three-way split. The task's own honesty rule applies: a documented UNKNOWN is a
success, and no rule is invented to fill the gap.

## Evidence and cross-checks

| Check | Result | Class |
|---|---|---|
| Aim slots have a non-empty aim sub-slice | All do: frame counts 1 or 2 | CONFIRMED |
| Aim offsets are monotonic and gap-consistent (`24 + count * 20`) | Yes, on all four models | CONFIRMED |
| The aim subset is structural, not random | Root plus symmetric limb-chain ends (plus one mid joint on three models) | CONFIRMED (structure) |
| The non-aim slots are static/identity | **No**: the pose-motion capture shows all 66 raziel slots change rotation, `static=0` | CONFIRMED |
| Any PACKED slot's channel is non-empty | No slot was classified PACKED, so this check has no positive rows | N/A |
| A body track table exists for raziel | The runtime descriptor dump recorded **no** track-table pointer for raziel, while the `glphicon` control recorded a 2-entry table | CONFIRMED for the dump; does not prove per-slot absence |

## Contradictions and corrections

1. **The `+6`/`+10` sentinel does not mark inactive slots.** The task's premise,
   carried from `docs/notes/slot-descriptor-parse.md`, treats `+6`/`+10` = 65535
   as an inactive marker. On the real raziel record, `+10` is the bone's
   last-vertex bound, and `+6` = 65535 appears on 21 of 66 slots, including the
   aim-bearing trunk joint (slot 1) and joints with children. The sentinel reading
   must not be used to assign `NONE`. (The descriptor locator still works, because
   it anchors on the model header, not on this signal.)
2. **A naive "active and no aim means PACKED" rule would contradict itself.**
   Applying it would label 62 raziel slots PACKED, but the packed channel is not
   located, not decoded, and not shown to be non-empty. That rule is refuted by
   absent evidence, so it is not used.
3. **The body's non-aim slots are not idle.** The pose-motion capture shows every
   slot's rotation changing over the session. So the UNCLASSIFIED slots are fed by
   *something*; naming that something is exactly what the missing selector
   prevents.
4. **"No QUAT" is not a clean negative.** The raziel dump's missing track-table
   pointer is one observation; the track selector was observed firing at corpus
   level in another run. Per-slot absence of quaternion tracks is unestablished,
   not proven.

## What this means for offline reconstruction

Decodable from file data today:

- The **aim sub-slices** of the aim slots: structure, frame count and bounds are
  verified. Their per-frame field semantics are still UNKNOWN
  (`docs/notes/aim-stream-bounds.md`, `docs/notes/frame-mode-composition.md`), so
  even these are not ready for a correct rotation replay.
- The **bone tree** (`+18`) and rest translations (`+12`/`+14`/`+16`) are directly
  readable and already consumed by the existing hierarchy/pose code.

Not decodable from file data today:

- The **bulk of the body** (62 of 66 raziel slots, and the analogous majority of
  the contrast actors). The packed channel is the candidate carrier, but it is
  neither located at a selector nor decoded, and its per-slot emptiness is
  unverified. No decodable angle triple exists for these slots in the shipped
  files as parsed here.
- The **object root** orientation: it is built from live object aim words
  (position/target), i.e. runtime state, not a file value.

Net effect: the census **narrows** offline limb reconstruction. It confirms a
small authored aim subset and confirms that the rest is not yet assignable to any
offline-decodable channel. It does not show that limbs are packed, and it does not
show that they are purely procedural either.

## UNKNOWN

1. The location, scope and per-frame variability of the **mode / representation
   selector** that separates PACKED_CHANNEL from QUATERNION_TRACK and from the
   procedural mode-table/proxy routes.
2. Whether the **packed channel is populated** per slot, and its frame-table
   semantics (divisor, frame wrap, per-axis bit-plane layout, palette order).
3. The meaning of the `+6` bone word.
4. The per-slot producer of the 110 non-aim slots.
5. Whether the aim sub-slice frames are aim vectors, a general packed channel, or
   a mixture.
6. Coverage: one shipped build, four actors, no runtime log that records the mode
   and representation flag per slot.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The aim offset is the per-slot AIM selector | an aim-bearing slot that the builder skips, or a non-aim slot the aim evaluator drives | Not tested at runtime; data-side holds on all four models |
| Limb interiors are not AIM_MODE | a limb-interior slot with a non-zero aim offset | Holds: only trunk and chain-end slots carry aim |
| No slot is PACKED | a located packed channel with a non-zero per-slot frame count | Not refuted; the channel is not located |
| All non-aim slots are driven | a non-aim slot that stays at identity for the whole capture | Refuted: all 66 raziel slots change |
| `+6`/`+10` = inactive sentinel | an aim-bearing or child-bearing slot with both words at 65535 | Refuted: slot 1 is exactly that |
| The body has no quaternion track table | a raziel track-table pointer in a clean dump | One dump says none; the track selector fires elsewhere |

## Method and legal

- Static parse of the shipped object containers in the SR1 archive, read-only,
  using the repository's documented object-container and descriptor layout (and
  the repository CLI for entry discovery). Entry selection was by container name;
  descriptor and bone fields were read with the documented 24-byte layout. The aim
  sub-slice bounds and frame counts were re-derived from the container bytes.
- Cross-read of the existing runtime descriptor dump and pose-motion summary
  already checked into the repository; no game process was launched for this slice.
- No engine code changed; `cargo test` remains 618 green.
- This note records offsets, field roles, slot indices, frame counts and
  structural counts only. It contains no addresses, module-relative numbers, code
  listings, raw game bytes or absolute install paths.

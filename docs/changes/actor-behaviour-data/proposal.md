# Proposal: actor-behaviour-data (outcome: the shipped actor block is a per-model table pair)

## Intent

Close the data half of `actor-ai` UNKNOWN #1: "the numeric state ids and the
full transition table". `actor-ai` established that behaviour is
data-parameterised and that a 0x20-byte behaviour entry carries the initial
animation state at `+0x1B`, but it did not enumerate the shipped entries or the
state-to-animation table. The questions were:

- Which object models are actors at all?
- What distinct behaviour-entry ids exist in the shipped corpus, and which
  actors use them?
- Is there a state-to-animation table in the model data? What is its shape?
- Is the mapping shared across actors or per-actor?
- Can the numeric state ids be pinned to clip sets?

## Outcome

**The actor block is real and per-model.** The object header at data `+0x1C`
points at a trailing block with a fixed four-byte signature. 19 containers in
`BIGFILE.DAT` and 2 loose `.PCM` files carry the actor-family signature;
`raziel` carries a distinct player-family signature; the other 375 object
containers are non-actors. The `actor-ai` runtime offsets (`+0x2C0` for the AI
instance block, `+0x1A8` for the entry pointer) are live-object offsets, not
model-header offsets; the file-side source is this `+0x1C` block.

**Behaviour entries are 0x20-byte records with the id at `+0x1B`.** The records
form a contiguous 0x20-stride array inside the block, pointed to by the block's
`+0x30` pointer table; each record ends in a `0xFFFFFFFF` sentinel at `+0x1C`.
Fourteen actors yield a clean array: **75 entries and 37 distinct ids**. The
ids form a sparse shared space (0x02..0x3c) that actors use in mostly
contiguous ranges (`vwraith` 0x09-0x0b, `sluagh` 0x0d-0x0f, `hunter`
0x1f-0x23, `skinner` 0x25-0x28, `morlock` 0x2b-0x2e, `ronin` 0x2d-0x32,
`wallcr` 0x30-0x33, `aluka` 0x38-0x3b, `wrshp` 0x2f-0x36, bosses low).

**The state table is 24 bytes at block `+0x18`: 8 rows x 3 one-byte clip-set
ids.** The values are one-byte indices, not frame ranges, and they group into
trios exactly as `actor-ai` described the idle/alert sets.

**The mapping is template-shared, not global and not purely per-actor.**
`skinner` and `morlock` share rows 0-5 byte-for-byte; `hunter`, `wrshp`,
`priests` and the three `vlgr` variants share rows 0-4; `kain` and `roninbss`
share rows 0-4. The trailing rows are per-actor. This is the data reason two
monsters with the same code behave similarly but not identically.

**UNKNOWN #1 is partially answered.** The entry structure, id set, table shape
and clip trios are pinned. The row-to-named-state labels and the model byte
that selects the initial/later entry set are still UNKNOWN.

Full evidence: `docs/notes/actor-behaviour-data.md` (actor census, entry
distribution, trio table, cross-model families, evidence ledger, explicit
UNKNOWN list).

## What changes versus the prior notes

- `actor-ai`'s behaviour entry is no longer just "0x20 bytes chosen by a model
  byte": the shipped array, its entries and its ids are enumerated.
- `actor-ai`'s state-to-animation table (runtime stride `0x10`) now has a
  data-side counterpart: the 24-byte 8x3 table at block `+0x18`. The two must
  not be conflated; the note says which is which.
- `object-models`' "single-model container" reading is refined: the container
  carries a trailing per-model data block selected by object header `+0x1C`;
  the mesh header is not the whole model.
- The actor family is now corpus-wide and name-labelled (21 containers plus
  `raziel`), so the follow-up reader can report it instead of guessing.

## Forecast split

| PR | Content | Budget |
|---|---|---|
| A (this one) | `docs/notes/actor-behaviour-data.md` + this proposal/spec/design/tasks | ~600 lines, docs only |
| B | `--actor-census --bigfile <DAT>` in the existing viewer: one line per actor container with name, entry count, distinct ids and the eight trios; plus `--object-info` printing the actor block summary for one entry | ~220 lines Rust + ~90 lines tests, no new dependencies |
| C | Optional `--actor-census --objects-dir <path>` to cover the 29 loose `.PCM` files (`morboss`, `priests`) that `BIGFILE.DAT` does not contain | ~60 lines, reuses B |

PR B is the minimal implementable slice. It needs no runtime knowledge: every
field it prints is CONFIRMED in the note, and every failure mode is a bounds or
signature check. C is a small add-on because the loose files already parse
through the same `parse_object` path.

Spec/design/tasks are included because B is implementable and stable. They
remain a plan; no production code lands in this change.

## PR plan

- Branch: `docs/actor-behaviour-data` from `main` (head `c5342f0`).
- Commit: `docs(actor): census the shipped behaviour entries and state trios`.
- One PR, docs only, based on `main`, not merged in this session.
- Validation: `cargo test` stays 321 green (no code touched); `git status`
  shows only the new markdown files.

## Risks

- **Labels are not pinned.** The table rows cannot be named idle/attack/hurt
  from bytes alone. The note and the follow-up reader print row indices, never
  invented state names.
- **Seven actors have no clean 0x20 array.** `soul`, `vlgra`, `vlgrb`, `vlgrc`,
  `walbosb`, `kain` and `priests` are reported as "no array" or "variant
  layout", not guessed.
- **The id space is shared but its list is not located.** The reader can print
  ids, but it must not claim that id N is clip N until the animation list is
  found.
- **Legal.** The reader prints counts, offsets, short names and small ids only.
  Tests must use synthetic byte buffers; no test may read a game file or commit
  game bytes.

# object-prototype - where a placed record's class actually comes from

Bounded, read-only corpus scan of the shipped SR1 area units and object
containers, cross-checked against the existing binary notes (`object-types`,
`object-behaviour`, `actor-ai`, `actor-behaviour-data`, `object-models`).
Behaviour and format facts only: field roles, counts, short asset names. No
binary addresses, no disassembly, no game bytes, no absolute install paths.
Scratch parsers and temporary tables lived in `%TEMP%`; the game files were
opened read-only and never copied into the repository.

Date: 2026-11-06. Evidence class per claim: **CONFIRMED** (two independent
observations agree, or the corpus is exhaustive for the claim), **PARTLY KNOWN**
(one side pinned), **UNKNOWN** (explicitly not established).

This note answers the dispatch-key question left open by
`actor-state-semantics` section 9: the engine classifies the *placed instance
name*, while the original classifies the *loaded model*. It establishes what the
placed record actually carries, how the record reaches a model container, and
which key the six-row class table must be applied to.

## Verdict

1. **The unmapped placed-record bytes are instance data, not a prototype.** The
   record's unmapped window holds a small repeat index, a unique per-unit
   ordinal, and two small value/bitfield words. None of them is a name, a name
   buffer or a model code, and none is constant per object name. →
   **CONFIRMED negative; exact semantics PARTLY KNOWN / UNKNOWN**.
2. **A placed record reaches its model container by its 12-byte name.** The
   placed name *is* the container name in shipped data (`skinner` ->
   `skinner`). There is no prototype-name indirection in the placed record. →
   **CONFIRMED**.
3. **The archetype `u16` does not select the model.** The same name resolves to
   one container across different archetype codes, and the 74-code -> 25-type
   relation is an authored class stamp, not a model table. → **CONFIRMED
   negative**.
4. **No object container carries a class-prefixed name.** All 397 containers
   store the same name twice; actor containers store bare padded names
   (`skinner_`, `sluagh__`, `raziel__`). The six class literals exist only in
   the binary. The class is chosen by the model's prototype string plus a
   forced monster class flag, and the model identity that marks the actor
   family is the model's actor-block signature. → **CONFIRMED for the data;
   the class-row mapping is INFERRED, consistent with `actor-ai` section 2**.

**Consequence.** The dispatch key must be the **loaded model identity**
(prototype name, plus the actor-family kind), not the placed instance name. The
placed name is the *lookup key to the container*; it is not the class key.
Phase 2 is therefore implementable without inventing a layout.

## 1. Method

- **A. Placed corpus.** Read-only parse of every loose `.PCM` unit under
  `1\AREA` (89 files). 85 units carry a room/object array; those contain **838
  placed records** across **79 distinct names**. Every record's full `0x4C`
  bytes were read in memory; the unmapped window and the trailing pointer words
  were tabulated. No bytes were written into the repo.
- **B. Container corpus.** Read-only parse of the `BIGFILE.DAT` index (1,325
  entries) and every object container's two 12-byte embedded name fields: **397
  containers**, all with `name1 == name2`.
- **C. Binary cross-check.** The existing notes were used for the six-row class
  resolver and its forced monster/physical flags (`object-types` section 2.2)
  and for the actor-block signature split (`actor-behaviour-data` section 2).
  The scanner re-checked the signatures over the same 397 containers.
- **D. Falsification.** Each question states what would refute it and whether
  the corpus test was run.

## 2. Answers

| # | Question | Finding | Class | Evidence |
|---:|---|---|---|---|
| 1 | What lives in the placed record's unmapped 20-byte window? | Not a prototype. It is per-instance placement data: a small repeat/group index, a unique per-unit ordinal, and two small value words. | CONFIRMED negative / PARTLY exact | 838 records: the window contains no NUL-terminated name; its words vary between records that share a name; the ordinal is unique inside every one of the 85 units |
| 2 | How does a placed record resolve to its loaded model container? | By its 12-byte placed name, matched against the container's embedded name. The placed name equals the container name in shipped data. | CONFIRMED | `object-models` O13/O14 (375/378 names resolve; the three misses are marker placeholders); this pass: every actor placed name has a container of the same name, and the asset loader verifies the name after the suffix lookup |
| 3 | Does the archetype `u16` participate in model selection? | No. One name maps to one container across all of its archetype codes; the archetype is an authored class stamp. | CONFIRMED negative | `raziel` appears with archetypes 374/404/405/512 and always resolves to the one `raziel` container; `marker` appears with 783/512/101/552/499/228 and has no container; `object-behaviour` section 1.2 found no consumer of the field |
| 4 | Do the archive model containers carry a class-prefixed name? | No. All 397 store the same bare, `_`-padded name twice. | CONFIRMED | Both 12-byte name fields are byte-identical in 397/397; `skinner_`, `sluagh__`, `raziel__`, `sreavr__`, `glphicon`, `particle`; no container starts with `monster_` or `physical`; the six class literals occur only in the binary |
| 5 | Then where does the class come from? | From the **loaded model's prototype string** (its padded model name) against the six-row table, plus a forced monster class flag. The prototype string alone cannot produce `monster_` for any shipped actor, so the forced flag is the selector. The model identity that marks the actor family is the model's actor-block signature. | CONFIRMED data; forced-flag source UNKNOWN; class-row mapping INFERRED | The six class literals are binary-only; `object-types` section 2.2 (prototype comparison plus forced physical/monster flags); `actor-behaviour-data` section 2 (19 archive actors + `soul` carry the actor-family signature, `raziel` the player signature, 377 others do not) |

## 3. What the unmapped window actually contains

The placed record is `0x4C` bytes. Across 838 records the window between the
name and the position is **not** a name buffer:

| Window word | Corpus reading | Class |
|---|---|---|
| first four bytes | always zero in every record | CONFIRMED |
| next word | a small repeat/group index; it counts down across repeated placements of the same name (`boldasd` x6 reads 6,5,4,3,2,1; `crow` x2 reads 2,1) | PARTLY (role INFERRED) |
| next word | a unique per-unit record ordinal; **838/838** records are unique inside their own unit (for example a 16-record unit uses 866..881) | CONFIRMED |
| last eight bytes | small values/bitfields; they vary between records that share a name | UNKNOWN |

Refutation test: a repeated ASCII name, a constant per-name code, or a value
that survives as a string/data-base offset. None matched. The earlier
"prototype at `+0x30`" reading does not refer to this window; it refers to the
**loaded model object**, not the placed record.

## 4. Why the instance name is the wrong key

The current engine does `classify(&spawn.name)` on the placed instance name.
The corpus shows why that can never yield `monster_`:

- the placed name is the model name, and model names are bare (`skinner`,
  `sluagh`, `morlock`, ...);
- the class table's monster row is `monster_`, and no shipped container or
  placed name carries that prefix;
- therefore the prefix lookup alone returns `Unclassified` for every real
  monster, stage 12 installs no tick, and the carrier never advances.

The original does not make this mistake. It classifies the loaded model by its
prototype string (the container's padded name) and applies a forced monster
class flag for the actor family. The class is a property of the **model**, not
of the placement.

## 5. The model identity the key must use

The shipped data gives two model-identity signals:

1. **Prototype string** — the container's padded name (`raziel__`,
   `skinner_`, `glphicon`, `particle`, ...). The six-row table is a prefix
   lookup over this string. It resolves `raziel__`, `sreavr__`, `glphicon`,
   `physical` and `particle`, but not the bare actor names.
2. **Actor-block signature** — a per-model block behind the object header. It
   splits the corpus cleanly: the 19 archive actor models plus `soul` share the
   actor-family signature, `raziel` carries the player signature, and the other
   377 containers do not carry an actor block at all (202 have no block).
   `actor-ai` section 2 and `actor-behaviour-data` section 2 already tie the
   actor family to the `monster_` class row.

So the key is: **the padded prototype name**, with the `monster_` row forced
when the model carries the actor-family block. The forced flag's exact bit is
UNKNOWN; the actor-kind identity is the established proxy and is already parsed
by the engine (`ActorProfile::kind`).

## 6. Falsification ledger

| Hypothesis | Observation that would refute it | Tested? | Result |
|---|---|---|---|
| The unmapped window holds a second name / prototype buffer | printable ASCII, or a value constant per name | 838 records, all words | No ASCII, no per-name constant → REFUTED |
| The placed name resolves to the container via a prototype indirection | a record whose placed name differs from its container name | Full corpus + loader | Placed name == container name → REFUTED (no indirection) |
| The archetype selects the model | two records with the same name but different archetypes resolving to different containers | 838 records | One container per name → REFUTED |
| A container carries a class prefix | any `name1`/`name2` starting with a class row | 397 containers | None → REFUTED |
| The six-row prefix alone classifies real monsters | a shipped container/placed name starting with `monster_` | 397 containers, 838 records, binary strings | None → REFUTED |
| The actor-family signature is the forced-monster identity | actor-family models with no monster tick, or non-actors with one | Signature census + `actor-ai` | 20 actor-family models, all actor-family → INFERRED (strong) |

## 7. UNKNOWN (explicit)

- The exact bit/field that carries the forced physical and forced monster class
  flags (the notes pin the mechanism, not the storage).
- The exact semantics of the window's repeat index and the two small value
  words (orientation, flags, or group bookkeeping).
- The exact role of the low half of the type/id dword versus the unique ordinal.
- Whether any unshipped or edited level authors a literal `monster_...`
  prototype name (the shipped corpus has none).
- Whether `soul`, which shares the actor-family signature but has a degenerate
  block, ticks with the monster row in the original. The data key says it would;
  the runtime was not observed.

## 8. Consequence for the engine

`LiveObject::class` must be derived from the **loaded model**:

- the prototype key is the model's padded name (`raziel__`, `sreavr__`,
  `glphicon`, `physical`, `particle`, ...);
- an actor-family model (`ActorKind::Monster`) supplies the `monster_` row,
  because no shipped actor name carries that prefix;
- the placed instance name stays the lookup key for the container, the tracker
  and the name-keyed spawn policy; it is not the class key.

This is a change of the key passed to the existing six-row `classify` table, not
a change of the table, the hooks, the fold fields or the simulation.

## 9. Reproduction

- Placed corpus: read-only parse of the 89 `1\AREA` `.PCM` units (85 with an
  object array) and the `BIGFILE.DAT` index (1,325 entries, 397 object
  containers). Scratch parsers lived in `%TEMP%`.
- Deterministic headless run (read-only, runtime only): see the apply summary
  for the before/after `skinnr1` and `under1` digests.

No game bytes, decompiled listings, binary addresses or absolute install paths
are recorded here.

## 10. Addendum (2026-11-07) - the forced bits are now exact

`actor-state-writer` section 2 resolved the storage this note listed as UNKNOWN:
the class comes from a 32-bit class-flags word captured at load-complete, and
the bits are **18 for `physical`** and **19 for `monster_`**, physical first.
The engine now classifies the loaded model with that exact rule; it no longer
uses the actor-family kind as a proxy. The padded prototype string is only the
fallback when neither bit is set, and that comparison is exact, not a prefix.


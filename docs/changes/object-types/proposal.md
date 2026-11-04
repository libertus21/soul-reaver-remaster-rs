# Proposal: object-types (outcome: type is an authored per-instance class; behaviour dispatch UNKNOWN)

## Intent

Decode what the level-object `type` field (high 16 bits of the `+0x28` dword in
the `0x4C` source record) means. The premise was that it is a small 0..12
behaviour enum; the question was whether that holds, how multi-type names are
possible, and which behaviour each value selects.

## Outcome

**`type` is authored data, not a runtime state.** Across 89 units and 838
objects there are 25 distinct type values; ten are greater than 12. The same
object name appears with several types because each placement carries a
different per-instance `+0x26` archetype code, and `type` is a deterministic
class of that code:

- 74 distinct `+0x26` codes map to 25 types with **zero conflicts**;
- the code bands are ordered exactly like the type values (a prototype table
  sorted by class);
- `raziel` is code 374/404/405 -> type 2 and code 512 -> type 4; `marker` is
  101/228 -> 0, 499 -> 3, 512/552 -> 4, 783 -> 9.

A name-level or "type = enum(0..12)" table is therefore wrong; the field must be
read per record.

**The behaviour dispatch was not located.** The level loader copies
`+0x26`, `+0x28` and `+0x2C` verbatim from the `.PCM` source record into the
runtime `0x60` record; no writer to the type word was found in the load path and
no switch/jump table over it was found in the load path or in the consumers
searched. Therefore the type-to-behaviour mapping is UNKNOWN. This is a
documented no-go for the binary half, not a claim that no dispatch exists.

**A separate render-class resolver was found and must not be confused with the
type.** It assigns one of six model classes (`raziel__`, `sreavr__`, `glphicon`,
`physical`, `monster_`, `particle`) from the object's `+0x30` prototype blob and
stores the index at runtime `+0x04`. That is a rendering axis, not the
level-object type.

Full evidence: `docs/notes/object-types.md` (census, code-to-type map,
multi-type table, type table with evidence classes, UNKNOWN list).

## What changes versus the prior notes

- `docs/notes/level-geometry.md` P2/P4 describe `+0x28` high as the type but do
  not separate it from `+0x26`. This pass adds the `+0x26` code and proves the
  two are redundant, which is the explanation for multi-type names.
- `docs/notes/level-geometry.md` P3 ("type is not simply a per-name constant")
  is refined: the type is per instance and is a class of `+0x26`; it is not a
  name lookup.
- The 48-entry "name registry" wording in `level-geometry` P4 is not used here.
  The registry this pass actually decoded is the six-entry model class table,
  which is a different thing.

## Forecast split

This is a docs-only slice. No production code is proposed in this change. If a
follow-on reader slice is approved, the smallest useful unit is:

| PR | Content | Budget |
|---|---|---|
| A (this one) | `docs/notes/object-types.md` + this proposal | ~250 lines, docs only |
| B (optional) | `--level-objects` prints `code=<+0x26>`; add a `--level-object-types` census that prints the `+0x26 -> type` map and counts | ~120 lines code + tests |

PR B is deferred because the semantic mapping is UNKNOWN. A spec that promised
"type 7 registers a trigger" would encode an unproven claim. PR B only exposes
raw fields and counts, so it can be specified safely. Spec/design/tasks are
intentionally omitted until PR B is scheduled.

## PR plan

- Branch: `docs/object-types` from `main`.
- Commit: `docs(level): decode the object type field and its archetype code`.
- One PR, docs only, no merge in this session.
- Validation: `cargo test` stays 321 green (no code touched); `git status`
  shows only the two new markdown files.

## Risks

- Type semantics may never be recoverable if the consumer is dispatched by name
  or by a table outside the load path. The investigation already accepts that
  outcome; the census and the code-to-type relation stand regardless.
- PR B could tempt callers into treating the raw type as a behaviour enum. The
  reader must label it `raw type` and print the code beside it.
- The `+0x26` code is only understood as an archetype/prototype id by inference;
  a future pass could show it is a hash or an offset. The proposal does not
  depend on that interpretation: it only relies on the confirmed code-to-type
  function.

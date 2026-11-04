# Proposal: object-behaviour (outcome: the dispatch key is the object name, not the archetype code)

## Intent

Find the per-frame consumer of the runtime placed-object table (level `+0xD0`,
count `+0xC8`, stride `0x60`) and characterize what each object does. The open
questions from `object-types` were: is the dispatch keyed by `+0x26`, by `type`,
by the render class, by the name, or by a function pointer in the record; and
how do `warpg`, `wrpface`, `campath` and `marker` actually behave.

## Outcome

**There is no per-frame dispatcher over the placed table.** The table is a
placement/lookup database. Every direct reader is a load-time pass, an id
lookup or a name lookup. Nothing iterates it per frame and nothing switches on
`+0x26` or `type`.

**The dispatch key is the 12-byte object name.** A name selects:

- the model container and the three archive paths (`.sra`, `.crm`, `.pcm`);
- one of **48 fixed object-tracker slots** (stride `0x38`, state `+0x18`:
  empty/loading/ready), which is the real per-object runtime;
- a small set of hardcoded engine roles: the ten-name always-spawn set, the
  ten-name actor roster, the eight-name boss/special set, the 22-name E3 asset
  override, and exact names such as `raziel`, `MORLOCK`, `warpgate`, `cliff1`
  and `movie*`.

A secondary key is the authored **id `+0x14`**, used by script/entity target
lookup and by event-program operand relocation. The render class at `+0x04`
stays a renderer selector and is not a behaviour key.

**Ground truth:**

| Case | Result | Class |
|---|---|---|
| `warpg`, `wrpface` | Load-time warp markers; no model container; never tracked; the warp system is reached through the link/level path and the `warpgate` link target | CONFIRMED |
| `marker` | Inert: no model container, never tracked, only the load-time `0x4000` name-table flag pass touches it | CONFIRMED |
| `campath` | Not an actor: no model container, never tracked. Path/waypoint data consumed through ids and the always-spawned `paths` object. The exact camera consumer was not located | CONFIRMED (no actor) / INFERRED (camera) |

**Runtime state does not live in the placed record** beyond the flag word
`+0x2C`. It lives on the tracker (`+0x18` state) and on the live object (the
`0x6D00` animation/state block, the render class at `+0x04`, and per-object
counters). This is the answer to the multi-code naming: the `+0x26` code and the
`type` are authoring metadata, preserved by the loader and not consumed at
runtime.

Full evidence: `docs/notes/object-behaviour.md` (reader inventory, record-family
table, name-keyed behaviour table, ground-truth validation, runtime-state
answer, explicit UNKNOWN list).

## What changes versus the prior notes

- `object-types` left the type-to-behaviour dispatch as a documented no-go. This
  pass converts that into a positive mechanism: the runtime keys on the name,
  not on the type or code.
- `level-geometry` P4's "second pass uses the `+0x18` triple" is refined: the
  destination-pointer pass runs on the **link/transition array** (owner `+0x50`,
  stride `0x60`, flags `+0x1E`, destination pointer `+0x28`), not on the placed
  table. The placed table's `+0x28` stays an id/type dword.
- The two 48-entry tables are named and separated: the object tracker
  (stride `0x38`, name-keyed, instance state) and the level-slot table
  (stride `0x48`, status `+0x04`, level pointer `+0x08`, name `+0x10`).

## Forecast split

This is a docs-only slice. No production code is proposed in this change.
A production reader would have to encode engine name lists, which is fragile
and not useful for the viewer. The safe follow-on, if approved, is limited to
raw evidence the repo already parses:

| PR | Content | Budget |
|---|---|---|
| A (this one) | `docs/notes/object-behaviour.md` + this proposal + a README limit update | ~320 lines, docs only |
| B (optional, deferred) | `--level-objects` prints `id=<+0x14>` and `code=<+0x26>` beside the name, and a `--level-object-roles` census groups names by "has model container" vs "marker" | ~120 lines code + tests |

PR B is deferred because any behaviour column beyond "has a model container"
would encode engine-internal name lists. A spec that promised "type 7 registers
a trigger" would encode a claim this pass disproved. Spec/design/tasks are
intentionally omitted until PR B is scheduled.

## PR plan

- Branch: `docs/object-behaviour` from `main` (head `b5266b6`).
- Commit: `docs(level): key object behaviour on the object name`.
- One PR, docs only, based on `main`, not merged in this session.
- Validation: `cargo test` stays 321 green (no code touched); `git status`
  shows only the new markdown files plus the README limit line.

## Risks

- The negative result ("no per-frame placed-table consumer") is bounded by the
  scans performed. A debug-only or tooling-only consumer could exist; the note
  states the scope rather than claiming global absence.
- The `campath` -> camera link is INFERRED. The note keeps it INFERRED instead
  of promoting it to a confirmed handler.
- The name lists are engine behaviour, not format. They are recorded in the note
  as evidence, but they must not leak into a data reader as a behaviour enum.
- The 48-slot tracker limit may be exceeded in unusual states; the engine fails
  loud, and the note records that rather than assuming an unlimited pool.

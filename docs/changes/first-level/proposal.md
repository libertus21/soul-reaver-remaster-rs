# Proposal: first-level (outcome: the new-game opening unit is named; docs only)

## Intent

Answer one question without re-deriving the startup state machine: which
room/level does the engine load when a new game starts?

## Outcome

The new-game branch seeds the current-unit name with the literal `under1`, and
the archive path builder requests `area\under\bin\under1.pcm`. The default
archive holds one level whose own name table contains `under1`; extracted
read-only, it parses to 1,515 offset-table records, 137 name strings, 1,468
vertices and 2,922 faces with zero out-of-range indices.

This is not the loose `UNDRCT1.PCM`: that unit calls itself `undrct1` and has
different counts (2,235 records, 66 name strings, 3,689 vertices, 6,765 faces).

Full evidence, trace shape and the explicit UNKNOWN list:
`docs/notes/first-level.md`.

## What changed versus prior assumptions

- The initial room is not a table entry and not a save default: it is a
  hard-coded literal in the new-game branch. A save can override it.
- The two name-list blobs are not involved in the first-room selection.
- The loose `AREA\...\BIN\<NAME>.PCM` tree is not automatically the runtime
  source; the traced request addresses the default archive namespace.

## Scope of this change

Docs only. No code, no new reader arm.

## Next probes (ordered by cost)

1. Dynamic trace of one new game to record the first archive path actually
   requested (the only way to confirm runtime order).
2. Locate the save field that carries the current unit name and its default.
3. Identify when the loose `undrct` data set is selected, if ever.

## Optional tiny slice

If a runnable check is wanted later, a read-only helper could scan the default
archive for the entry whose name table contains a requested unit and print its
record/geometry counts. That is a verification tool, not a claim about runtime
selection; it should stay out of this docs-only PR.

## Non-goals

Game bytes, addresses, listings, decompiled code, install paths, spawn-point or
save-format decode.

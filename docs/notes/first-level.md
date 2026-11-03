# First unit loaded by a new game

## Answer

A new game loads the unit **`under1`**, not `undrct1`.

- The new-game branch writes the literal unit name `under1` into the
  current-unit name buffer.
- The level-request path hands that buffer to the archive path builder, which
  composes `area\<prefix>\bin\<unit>.pcm`. The prefix is the name up to the
  first digit, so the request becomes `area\under\bin\under1.pcm`.
- The default archive contains exactly one level whose own name table contains
  `under1`. Extracted read-only and parsed with the already-verified reader:
  **1,515 offset-table records, 137 name strings, 1,468 vertices, 2,922 faces,
  0 out-of-range indices**.
- The loose `AREA\UNDRCT\BIN\UNDRCT1.PCM` is **a different unit**: it declares
  itself `undrct1` and parses to 2,235 records, 66 name strings, 3,689 vertices
  and 6,765 faces. Do not answer `undrct1` for the new-game opening.

Evidence class: **CONFIRMED** for the literal name, the path shape, and the
identity of the archive unit (code plus a literal plus a real parse). The claim
that the player reaches this branch through the visible Start Game action is
**INFERRED** from the static branch shape; nothing was executed.

## Trace shape (roles only)

1. The startup state machine has a dedicated new-game branch that seeds the
   current-unit name buffer with `under1`.
2. The same branch distinguishes an empty pending unit from a saved unit:
   empty means "start at the default"; a stored name is copied into the same
   buffer instead.
3. The menu-to-game transition enters the intro hand-off state and then the
   game-init state.
4. Game init passes the name buffer to the load-by-name request.
5. Load-by-name finds or allocates a load slot, asks the archive path builder
   for the `.pcm` path, and queues the asynchronous reader.
6. Independently, an in-game path also calls the loader with the same literal
   `under1`, which corroborates the starting unit.
7. A menu handler for the start/continue action compares the current unit name
   against `under1`, consistent with `under1` being the opening unit.

## Where the name comes from

The name is not read from a progression table or from the two name-list blobs.
It is a hard-coded default literal in the new-game branch. A save overrides it
by copying its stored unit name into the same small current-unit buffer (seven
bytes used by the default). The exact field offset inside the save structure is
UNKNOWN.

## Cross-check against real data

| Unit | Source | Records | Name strings | Vertices | Faces | Index check |
|---|---|---:|---:|---:|---:|---|
| `under1` | default archive entry | 1,515 | 137 | 1,468 | 2,922 | 0 out of range |
| `undrct1` | loose `AREA\UNDRCT\BIN\UNDRCT1.PCM` | 2,235 | 66 | 3,689 | 6,765 | 0 out of range |

Both parse with the verified reader. The different counts show they are not the
same room under two names.

## UNKNOWN (explicit)

1. The exact save-field offset that overrides the default unit name.
2. Runtime order: the state sequence is read from code, not measured.
3. How the archive maps a path to an opaque name hash; the unit was identified
   by its self-declared name, not by computing the hash.
4. When the loose `undrct` data set is used, if ever; no traced path requests it.
5. The spawn point and initial object state inside `under1`.
6. Behaviour when `under1` is absent from the archive; the loader has a
   failure message, but no run was observed.

## Method and legal

Read-only Ghidra 12.1.4 headless pass, project and decompiler output in `%TEMP%`
only. The archive entry was read into `%TEMP%` for a read-only parse. This note
records behaviour paraphrases, short literals and counts only: no listings, no
decompiled code, no raw addresses, no game bytes, no install paths.

# warp-level-selection - how a warp/portal picks its destination unit

Read-only Ghidra 12.1.4 pass and read-only archive scan of the SR1 remaster.
Behaviour notes only: role names, short level/asset names, counts and a few field
offsets. No addresses, no disassembly, no decompiled text, no game bytes, no
absolute install paths. The Ghidra project, scripts and decompiler output lived
in `%TEMP%` only; game files were opened read-only.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (code and bytes agree,
or two independent observations agree), **INFERRED** (one side only),
**UNKNOWN**.

## Verdict

1. A warp gate does **not** read its destination from the save state and does
   **not** build one from a numeric `area<N>` value. It selects from a
   **hardcoded, fixed-stride table of 14 level names** in the module's
   initialized data.
2. The selected entry is a **global warp index**. The index is seeded from the
   current unit's position in the same table, changed by exactly `+1` or `-1`
   with wraparound modulo 14, and reset to the first entry at game
   initialisation.
3. The 14 table entries are exactly the 14 units that carry `warpg` (type 7)
   and `wrpface` (type 12) objects and the two `warpgate,1` / `warpgate,2`
   link records. The warp network is one 14-node cycle.
4. **None of the 30 unreferenced island names is in the table.** A warp cannot
   reach any of them, and no warp edge leaves the 14-cycle.
5. Ordinary (non-warp) portals are different: their destination is the
   portal/link record's **target name string**, which comes from the level data,
   not from a module table.
6. **The "30 unreachable islands" premise does not survive a second look.** The
   current link reader rejects uppercase names and then misaligns the table
   prologue; the engine itself compares names case-insensitively. With the
   engine's comparison, only `push1` remains in-degree zero, and most of the 30
   have real inbound link records. That is not a warp path, but it changes the
   reachability answer.

## Mechanism (roles only)

### The 14-name table

- Fourteen 24-byte name slots in initialized data. Entry 0 is `under3`; entry
  13 is `chrono17`.
- A single global index selects the slot. Every read/write of that index is
  modulo 14.

### Index lifecycle

- Game initialisation resets the index to 0.
- On arrival in a warp unit, the index is set to that unit's slot number (a name
  lookup in the 14-slot table).
- The warp interaction adjusts the index by `-1` or `+1` with wraparound. Two
  small routines do exactly one of these each.
- The interaction is a selection UI: left/right changes the index, confirm
  triggers the load.

### Load

- The loader compares the current unit name with the selected table slot.
- It passes the selected slot (the literal level name) to the shared
  load-by-name routine, the same routine ordinary level requests use.
- The warp unit's object data is not consulted for the destination name. The
  object data only marks the level as a warp level (`warpg`/`wrpface` records)
  and supplies the two `warpgate,N` link records.

### Normal portals

- The portal/link handler copies the target string beside the portal record and
  compares it with `warpgate`.
- If it is `warpgate`, the destination is the `table[index]` path above.
- Otherwise the same string is the destination unit name and a neighbouring
  integer is the destination level id; both are written into the pending
  level-change state.
- Therefore an ordinary portal's destination name is a **level-data string**.
  It need not appear in the module at all.

## The 14-entry warp table (level names only)

```text
under3, clfpil3, huba8, out10, city6, cathy58, cathy62,
stone15, add6, aluka43, fill3, hubb4, oracle1, chrono17
```

Cross-check: exactly these 14 units contain `warpg`/`wrpface` objects, and
exactly these 14 carry both `warpgate,1` and `warpgate,2` link records. No other
unit carries either marker.

## Are any of the 30 reachable through the warp mechanism?

**No.** Table membership is disjoint from the 30. The warp graph is a single
14-cycle, and the only index arithmetic is `+/-1` modulo 14. A warp cannot name
a unit outside the table.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| W1 | Warp selection uses a 14-slot, 24-byte-stride literal name table in initialized data. | CONFIRMED |
| W2 | The selection index is modulo 14 and is reset to entry 0 at game initialisation. | CONFIRMED |
| W3 | The index changes only by `+1` or `-1` with wraparound; two separate routines implement the two directions. | CONFIRMED |
| W4 | The load path compares the current unit name with the selected slot and passes the selected literal to the shared load-by-name routine. | CONFIRMED |
| W5 | The 14 table names are exactly the 14 units that carry `warpg`/`wrpface` objects. | CONFIRMED |
| W6 | All 14 carry `warpgate,1` and `warpgate,2` link records; no other unit does. | CONFIRMED |
| W7 | A non-warp portal uses a level-data target name plus a target level id. | CONFIRMED (code) / INFERRED (record origin) |
| W8 | None of the 30 names is in the warp table. | CONFIRMED |
| W9 | The 14-cycle is the complete warp network for the observed archive. | INFERRED |

## Cross-check: the island baseline is parser-limited

This finding is independent of the warp mechanism, but it changes the
"unreachable islands" answer, so it is recorded here.

- The engine's name comparison is **case-insensitive** (it folds ASCII
  upper-case to lower-case before comparing). CONFIRMED.
- Link names in the archive are **mixed case**, for example `Aluka1,50` next to
  `aluka41,11`. CONFIRMED.
- The current link reader accepts only a lower-case first letter and scans the
  prologue in 4-byte steps. On a table whose first record is capitalised, it
  matches a 4-byte-shifted *suffix* (`a1,50` from `Aluka1,50`), stops there, and
  then walks every record 4 bytes off. Targets become truncated tokens
  (`a1`, `a41`, `luka3`, ...) that match no unit, so the edges are dropped.
  CONFIRMED for the sampled units.
- Raw-byte spot checks find real, canonical 40-byte link records inside the
  declared table range for six of the 30:

  | source unit | stored link names (relevant targets) |
  |---|---|
  | `aluka3` | `Aluka1,50`, `aluka41,11`, `aluka7,62`, `Aluka2,93` |
  | `aluka15` | `Aluka3,95`, `Aluka3,94`, `aluka48,72`, `aluka32,73`, `aluka2,49` |
  | `aluka34` | `Aluka1,9`, `chapel4,08`, `aluka41,51` |
  | `aluka4` | `Aluka39,81`, `Aluka28,59`, `aluka5,55` |
  | `chrono1` | `chrono19,100` |
  | `pillars1` | `pillars10,99` |

  That names six of the 30: `aluka2`, `aluka41`, `aluka5`, `aluka7`,
  `chrono19`, `pillars10`. CONFIRMED.
- Re-running the in-degree with case-insensitive names and a prelude-gated
  alignment leaves exactly one true island: `push1`. Under the same corrected
  graph, 28 of the 30 are reachable from `under1`; `chrono19` has inbound links
  but is not reachable from `under1`, and `push1` is the only in-degree-zero
  unit. INFERRED (the six above are spot-verified in bytes; the remaining edges
  come from the corrected parse).

Consequence: absence of a name from the module is **not** evidence of cut
content, because portal destinations are level-data strings. The binary-literal
test and the link-graph test were measuring different things.

## UNKNOWN (explicit)

1. Runtime use of the link table for player transitions. The table is decoded
   and behaves like the level graph, but this pass did not execute a transition.
2. Archive path/hash case handling for capitalised link names. The engine's name
   comparison is case-insensitive, but the archive lookup itself was not traced.
3. The exact runtime layout of the portal record (name, target id, index).
   Handler behaviour is clear; the record's origin is inferred.
4. The mapping of `warpgate,1` versus `warpgate,2` to index `-1` versus `+1`.
5. Whether `push1` and `chrono19` are reached by a non-warp mechanism (script,
   save, trigger).
6. Whether the 14-cycle is the complete warp network in every game state
   (debug modes excluded).

## Next probes (ordered by cost)

1. **Fix the link reader** (DONE): case-insensitive name comparison and a
   prelude-gated prologue scan. `--level-reach` from `under1` now reports
   `334 reachable / 38 unreachable / 1 island`; `push1` is the only island. See
   `level-reach.md` for the corrected counts.
2. **Dynamic trace (user-run)**: stand on a warp face, log the selected table
   slot and the load-request name, and confirm the slot-to-path handoff at
   runtime.
3. **Portal record origin**: trace the portal handler's caller to pin the
   runtime layout and the `warpgate,N` normalisation.
4. **Outlier audit**: check `push1` and `chrono19` for script or save entry
   points.

## Method box

Two bounded passes inside the agreed budget: (1) re-pin the warp strings and
follow references to the warp state machine, index helpers and load-by-name
routine; (2) read-only archive scan of object arrays and link tables, plus a
raw-byte cross-check of the link names and a corrected in-degree/BFS run.
Ghidra analysis and scripts stayed in `%TEMP%`.

## Legal

Behaviour notes only. Role names, short level/asset names, counts and field
offsets. No addresses, no listings, no decompiled bodies, no game bytes, no
absolute install paths. Scratch stayed in `%TEMP%`; the game binary and archive
were opened read-only. No protection or packing was encountered.

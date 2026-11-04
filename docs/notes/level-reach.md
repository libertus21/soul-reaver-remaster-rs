# level-reach - archived level graph reachability

Read-only, in-memory scan of one `BIGFILE.DAT`. No extraction, no game bytes,
no absolute install paths. This note records the discovery rule, the output
shape and the measured counts of the opening graph.

## Tool

```
soul-reaver-remaster-rs --level-reach --bigfile <BIGFILE.DAT> [--start <UNIT>]
```

`--start` defaults to `under1`, the new-game opening unit. The command is
headless: it never opens a window.

`--level-links` now accepts the same archive forms as the other level arms:

```
soul-reaver-remaster-rs --level-links --bigfile <BIGFILE.DAT> --entry <INDEX>
soul-reaver-remaster-rs --level-links --bigfile <BIGFILE.DAT> --group <PREFIX>
```

## Discovery rule

- Every BIGFILE entry is read into memory and tested for the `.PCM` header
  shape: a sane record count, an ascending offset table starting `0, 4`, and a
  padded data base inside the entry.
- A candidate is parsed with the same reader used by the loose-file arms.
- The index entry's fourth dword is the uppercase last four characters of the
  unit name. It is used to confirm the header name, or to select the matching
  scanned name when the header tail disagrees.
- A candidate that fails the full parser is counted and printed as
  `unparseable`; it is never dropped.
- The link table at header slot `0x34` is bounded by slot `0x35`. Some archived
  units place a short non-link prologue before the 40-byte records, so the first
  record-shaped entry wins; the walk then stops at the first record whose
  12-byte name is not `<target>,<digits>`.

## Real result (SR1 archive)

Start `under1`, opening unit of a new game:

```
reach start=under1 discovered=372 reachable=225 unreachable=147 unparseable=0
```

- 372 archived `.PCM` units discovered.
- 225 reachable from `under1` through outgoing link targets.
- 147 discovered but not reachable (including unparseable units; none here).
- 24 reachable name groups and 40 unreachable name groups. A group is the unit
  name with digits stripped, e.g. `cathy62` -> `cathy`.

The same run reports per-group counts for both sets, so they can be compared
directly. Example reachable groups include `cathy 61`, `huba 18`, `skinnr 17`,
`city 15`, `out 13`, `tower 12` and `train 11`. Example unreachable groups
include `aluka 32`, `chrono 19`, `stone 14`, `oracle 13`, `nighta 12`,
`nightb 10`, `add 8` and `tompil 5`.

`--level-links --bigfile <DAT> --entry 115` (the archived `under1`) prints:

```
links under1 count=1
  link under2 2 target_id=-9233 prelude=1,1,18,1,-9233
```

## What "unreachable" does and does not prove

It proves that BFS over the decoded outgoing link records, starting at the
chosen unit, does not arrive at that unit. It does **not** prove the unit is cut
content, unused, or unreachable in the running game. Other load paths exist
that this tool does not model: saves, scripted unit changes, trigger volumes,
warps and any loader that names a unit directly. An unreachable group is a
candidate for inspection, not a verdict.

## Limitations

- Only link-table edges are followed. A target name that is not one of the
  discovered units is ignored; the tool does not report unresolved targets.
- The start unit must be present; otherwise the command exits non-zero.
- The link parser is read-only and strict; a malformed prologue or a name that
  does not match the record grammar ends that unit's walk.
- Counts describe this archive and this parser revision, not the runtime graph.

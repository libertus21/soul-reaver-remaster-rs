# level-reach - archived level graph reachability

Read-only, in-memory scan of one `BIGFILE.DAT`. No extraction, no game bytes,
no absolute install paths. This note records the discovery rule, the output
shape and the measured counts of the opening graph.

## Tool

```
soul-reaver-remaster-rs --level-reach --bigfile <BIGFILE.DAT> [--start <UNIT>] [--names]
```

`--start` defaults to `under1`, the new-game opening unit. The command is
headless: it never opens a window. `--names` adds one `island-name <unit>` line
per island after the grouped island counts; without it the output is unchanged.
The flag lives on the legacy arm only.

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
  record-shaped entry wins. A record is shaped when its prelude is `1`, `n`, `18`,
  `n`, `id` (`n` repeats at fields 1 and 3) and its 12-byte name is
  `<identifier>,<digits>` in either case. The walk then stops at the first record
  that fails either test.
- Link targets keep their stored case in `--level-links`, but graph work folds
  them to ASCII lower case before comparing, because discovered unit names are
  lower case and the engine compares names case-insensitively.

## Output sections

```
reach start=... discovered= reachable= unreachable= unparseable=
  reachable groups=N
  reachable <group> <count>
  unreachable groups=N
  unreachable <group> <count>
  islands total=N
  island <group> <count>
  island-name <unit>
  inbound-unreachable total=N
  inbound-unreachable <group> <count>
  unparseable <entry>
```

- The first block is the start-dependent BFS result and is unchanged.
- `islands` lists units whose inbound degree is zero.
- `island-name` lists those units individually, sorted, and only with `--names`.
- `inbound-unreachable` lists units that have at least one inbound link but
  were not reached from `--start`.
- Groups strip digits from the unit name, e.g. `cathy62` -> `cathy`.

## In-degree rule

Reachability from one seed is not a cut-content signal: a unit can be fully
reachable from its own entry point while looking orphaned from `under1`. The
in-degree tells a different story.

- The inbound degree of a unit is the number of **distinct other** discovered
  units whose link tables name it.
- A self-link does not count. A target that is not a discovered unit is
  ignored, exactly as in the BFS.
- `islands` = inbound degree 0: no other unit links to it, so there is no
  arrival path at all.
- `inbound-unreachable` = inbound degree greater than 0 and not reachable from
  `--start`: another unit can enter it, but the chosen seed cannot.
- The in-degree is start-independent by construction. The island set must be
  identical for every `--start`; only the reachable and inbound-unreachable
  sets move.
- For parsed units, `islands + inbound-unreachable = unreachable` whenever the
  start unit has an inbound link, as in both runs below. A start unit with
  in-degree 0 is still listed as an island so the set stays start-independent.

## Real result (SR1 archive)

Start `under1`, opening unit of a new game:

```
reach start=under1 discovered=372 reachable=334 unreachable=38 unparseable=0
  ...
  islands total=1
  inbound-unreachable total=37
```

- 372 archived `.PCM` units discovered; all parse (0 unparseable).
- 334 reachable from `under1`, 38 not reachable.
- 1 island (inbound degree 0) and 37 inbound-but-unreachable units.
- `1 + 37 = 38`, the expected partition of parsed unreachable units.
- Unreachable groups: `chrono 19`, `city 2`, `oracle 13`, `push 1`, `stone 3`.
- The single island is `push1`. The other 37 unreachable units have inbound
  links from units the `under1` walk never reaches.

Island groups with counts:

```
push 1
```

Inbound-but-unreachable groups with counts:

```
chrono 19, city 2, oracle 13, stone 3
```

Start `oracle10`:

```
reach start=oracle10 discovered=372 reachable=366 unreachable=6 unparseable=0
  ...
  islands total=1
  inbound-unreachable total=5
```

- The reachable set grows from 334 to 366.
- The island total and every island group are byte-for-byte identical to the
  `under1` run. That is the start-independence sanity check.
- Unreachable groups: `city 2`, `push 1`, `stone 3`; inbound-but-unreachable:
  `city 2`, `stone 3`. `1 + 5 = 6`.

`--level-links --bigfile <DAT> --entry 115` (the archived `under1`) prints:

```
links under1 count=1
  link under2 2 target_id=-9233 prelude=1,1,18,1,-9233
```

### Parser correction (2026-11-04)

The earlier `225 / 147 / 38` baseline was parser-limited, not a property of the
archive. The reader accepted only a lower-case first letter, so a table whose
first record named a capitalised target (for example `Aluka1,50` or
`Aluka3,95`) was rejected at the true record boundary. The prologue scan then
advanced four bytes and accepted a shifted lower-case suffix such as `a1,50`;
every later record was read four bytes off and its edge was dropped.

The reader now requires the observed prelude invariant (`1`, `n`, `18`, `n`,
`id`) before accepting a name, so a shifted window fails even when its truncated
name still matches the grammar. Names are matched case-insensitively for graph
work and keep their original case in `--level-links`. With the corrected parse
the `under1` run is `334 / 38 / 1`; the only true island is `push1`.

## What "unreachable" does and does not prove

It proves that BFS over the decoded outgoing link records, starting at the
chosen unit, does not arrive at that unit. It does **not** prove the unit is cut
content, unused, or unreachable in the running game. Other load paths exist
that this tool does not model: saves, scripted unit changes, trigger volumes,
warps and any loader that names a unit directly. An unreachable group is a
candidate for inspection, not a verdict.

An island is stronger evidence than an unreachable unit: nothing in the scanned
link graph enters it, so a cut level is a plausible explanation. A non-island
unreachable unit, such as `oracle` or `chrono` from `under1`, already has an
arrival path and must not be treated as cut.

## Limitations

- Only link-table edges are followed. A target name that is not one of the
  discovered units is ignored; the tool does not report unresolved targets.
- The start unit must be present; otherwise the command exits non-zero.
- The link parser is read-only and strict; a record must satisfy both the
  prelude invariant and the mixed-case name grammar, otherwise the walk ends.
- Counts describe this archive and this parser revision, not the runtime graph.

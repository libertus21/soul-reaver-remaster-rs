# island-crosscheck - the 38 islands against the SR1 module

Cross-check of the `--level-reach` island set against the strings present in the
SR1 game module (`sr1.dll`). Read-only on the game data; no bytes are copied
into the repo and no retail offsets are recorded here.

## Method

- The name list is the tool's own output, not a hand parser:
  `--level-reach --bigfile <BIGFILE.DAT> --names` on the real SR1 archive.
  The run matched the documented baseline: 372 discovered, 225 reachable,
  147 unreachable, 0 unparseable, 38 islands.
- The module was scanned once, in memory, for each name as an ASCII byte
  sequence. A hit counts only when both neighbouring bytes are outside
  `[A-Za-z0-9_]`; this is what keeps `aluka3` from matching inside `aluka34`
  and `stone1` from matching inside `stone13`.
- Hit counts are per name; the scan is case-sensitive because unit names are
  lowercase tokens. A second case-insensitive pass was used only as a sanity
  check and is called out where it changes the reading.
- Context classes below come from short printable neighbourhoods around the
  first hit (a few characters), not from disassembly.

## Per-name result

`REFERENCED` means the exact lowercase token appears in the module.
`ABSENT` means it does not.

| unit | hits | verdict | context class |
|---|---:|---|---|
| aluka1 | 1 | REFERENCED | contiguous level-name list |
| aluka2 | 0 | ABSENT | - |
| aluka24 | 0 | ABSENT | - |
| aluka29 | 0 | ABSENT | - |
| aluka3 | 0 | ABSENT | - |
| aluka30 | 0 | ABSENT | - |
| aluka31 | 0 | ABSENT | - |
| aluka32 | 1 | REFERENCED | fixed-slot (24-byte) name array |
| aluka33 | 0 | ABSENT | - |
| aluka37 | 0 | ABSENT | - |
| aluka38 | 0 | ABSENT | - |
| aluka39 | 0 | ABSENT | - |
| aluka40 | 0 | ABSENT | - |
| aluka41 | 0 | ABSENT | - |
| aluka44 | 1 | REFERENCED | warp-adjacent short-name pool |
| aluka45 | 1 | REFERENCED | contiguous level-name list |
| aluka46 | 0 | ABSENT | - |
| aluka5 | 0 | ABSENT | - |
| aluka6 | 1 | REFERENCED | fixed-slot (24-byte) name array |
| aluka7 | 0 | ABSENT | - |
| chrono19 | 0 | ABSENT | - |
| gastuna1 | 1 | REFERENCED | contiguous level-name list |
| gastunb1 | 0 | ABSENT | - |
| litrnda1 | 0 | ABSENT | - |
| litrndb1 | 0 | ABSENT | - |
| nighta1 | 1 | REFERENCED | fixed-slot (24-byte) name array |
| nighta10 | 0 | ABSENT | - |
| nighta12 | 0 | ABSENT | - |
| nighta2 | 0 | ABSENT | - |
| nighta4 | 1 | REFERENCED | contiguous level-name list |
| nighta5 | 0 | ABSENT | - |
| nighta6 | 0 | ABSENT | - |
| nighta7 | 0 | ABSENT | - |
| nighta8 | 0 | ABSENT | - |
| out16 | 0 | ABSENT | - |
| pillars10 | 0 | ABSENT | - |
| pisairb1 | 0 | ABSENT | - |
| push1 | 0 | ABSENT | - |

Referenced: 8 of 38 (`aluka1`, `aluka6`, `aluka32`, `aluka44`, `aluka45`,
`gastuna1`, `nighta1`, `nighta4`). Absent: 30 of 38. Every hit count is 1.

## Group prefixes

The island groups are 11 prefixes. Exact lowercase prefix hits:

| prefix | hits | reading |
|---|---:|---|
| aluka | 1 | base-name table entry |
| chrono | 1 | base-name table entry |
| gastuna | 1 | base-name table entry |
| gastunb | 0 | no exact prefix anywhere |
| litrnda | 1 | base-name table entry |
| litrndb | 1 | base-name table entry |
| nighta | 1 | base-name table entry |
| out | 2 | 1 base-name entry + 1 English "out" in a memory message |
| pillars | 1 | base-name table entry |
| pisairb | 1 | base-name table entry |
| push | 0 | lowercase absent; capitalised `Push 1`..`Push 10` labels exist |

The module contains a contiguous table of base group names (`under`, `train`,
`cliff`, ... `aluka`, `out`, `pillars`, ... `nighta`). `gastunb` is not in it.
Capitalised `Push N` menu labels are a different token from the lowercase unit
`push1`.

## Context classes

- **contiguous level-name list**: a run of short level names packed with
  null/alignment padding, e.g. `fill1`, `aluka1`, `fill2`, `gastuna1`,
  `suntnl1`, `huba2`. The names read as a roster of unit names.
- **fixed-slot (24-byte) name array**: a repeated fixed-stride array mixing
  groups (`train7`, `cliff1`, `aluka32`, `nighta1`, `chrono1`, `cathy55`) with
  duplicate entries. A table, not a path template.
- **warp-adjacent short-name pool**: `aluka44` sits in a packed pool of short
  level/actor codes next to a `warp unit` diagnostic format string. The
  warp reading is inferred from the neighbourhood, not proven.
- **base-name table**: one entry per level family, no digits, adjacent to
  other family names. It explains why `chrono`, `out`, `pillars` and
  `pisairb` are present while their numbered units (`chrono19`, `out16`,
  `pillars10`, `pisairb1`) are not.
- **procedural loader templates**: the module contains short format strings
  such as `\kain2\area\%s\bin\%s.pcm`, `\kain2\area\%s\bin\%s.crm` and
  `\kain2\object\%s\%s.pcm`. A saved state or script can supply the `%s`
  values, so a unit name need not exist as a literal string in the module.

## What this does and does not prove

It proves:

- 8 of the 38 inbound-degree-zero units have their exact lowercase name
  present somewhere in the module, in name-roster contexts.
- The module knows the base group names for 9 of the 11 island groups, and
  has no exact lowercase `gastunb` or `push` prefix.
- The module already builds unit paths procedurally from `%s`, so literal
  name presence is not the only way a unit can be loaded.

It does **not** prove:

- That an ABSENT name is cut content. The loader may compose
  `area\<name>\bin\<name>.pcm` from a saved state, a script variable or a
  base name plus a numeric suffix. The missing numbered units next to present
  base names (`chrono` vs `chrono19`) fit that pattern.
- That a REFERENCED name is reachable, used or loadable at runtime. A roster
  or table entry only shows the name is known to the module.
- Anything about runtime reachability. The island set comes from the decoded
  link graph; saves, warps, scripted changes and direct loads are outside it.
- That the scan is exhaustive. It searches exact ASCII tokens only; encoded,
  compressed or procedurally generated names are invisible to it.

## Reproduction

```
soul-reaver-remaster-rs --level-reach --bigfile <BIGFILE.DAT> --names
```

Scan the SR1 game directory's `sr1.dll` read-only with an ASCII boundary
search for each listed name. No offsets are required or recorded.

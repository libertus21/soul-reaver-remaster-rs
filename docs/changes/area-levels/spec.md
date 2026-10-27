# Spec: area-levels

`cargo run -- --level-info <PATH>` reads one SR1-remaster `.PCM` unit, or a
room directory containing `.PCM` units, and prints a read-only inventory: the
container record count, the object-name table, the portal/door links, and the
PMF/PNF sidecar shape. It never decodes geometry, placement, textures, or
audio, and it never opens a window. Malformed input fails LOUD with exit 2;
bad invocation fails with exit 1; a refused path fails with exit 3. Every
existing CLI form keeps its exact prior behavior.

## Quick path

1. Run `cargo run -- --level-info "<install>\1\AREA\LAIR"` (relative name in
   docs; the real path is runtime-only).
2. stdout shows one `level` line per unit and one `link` line per link.
3. User acceptance records four relative paths: a linked room, a linkless
   room, a sidecar-present room, and a sidecar-absent room.

## Output contract

One line per unit, sorted by unit name:

```text
level <unit> pcm_records=<N> names=<M> links=<K> pmf=<present|absent> segments=<S> rate_field=<R> pnf=<E>
```

Then, in file order, zero or more link lines:

```text
  link <target> <index>
```

`unit` is the own name found in the `.PCM` name table (lowercase).
`names=` counts every accepted name, duplicates preserved; `links=` counts
names of the form `<target>,<index>`. When no sidecar exists the PMF fields
print as `segments=0 rate_field=0 pnf=0`. No ids, offsets, hex, coordinates,
or file bytes are printed; `<index>` is a small ordinal, not an address.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shape: exactly two args, `--level-info` then one path. Zero, one, three or more args, or an unknown flag, are a LOUD usage error + exit 1. Existing modes are unchanged |
| R2 | Path kinds: a `.pcm` file (case-insensitive) or a directory. A directory is accepted when it contains `BIN\*.PCM`, or when it directly contains `*.PCM` (so both `...\LAIR` and `...\LAIR\BIN` work). Any other path kind is refused with a clear message + exit 3. A missing or unreadable path is an I/O failure + exit 2 |
| R3 | PCM model: read `u32 record_count`; it MUST be in `2..=MAX_RECORDS` (a fixed sanity ceiling). The next `record_count` `u32` values are relative offsets; `offset[0] == 0`, `offset[1] == 4`, and the list MUST be strictly ascending. The data base is `4 + 4 * record_count`; `base + offset[last]` MUST be within the file. Any violation is `Err` + exit 2, no panic, no unbounded allocation |
| R4 | Name table: scan the file for printable runs; accept a run as a name only when it is 3..=20 bytes and matches `^[a-z][a-z0-9]*(,[0-9]{1,2})?$`. The file MUST contain a name equal to its own stem (case-insensitive); otherwise exit 2. `names=` is the accepted count, duplicates preserved |
| R5 | Link model: a name containing `,` splits into target and index; target is printed as found, index is parsed as an unsigned ordinal in `0..=99`. A target is not required to exist as a `.PCM` (the census contains special targets) |
| R6 | Sidecar: for unit `X`, look for `X.PMF` and `X.PNF` in the same directory (case-insensitive). Neither present → `pmf=absent`. One present without the other → exit 2. Both present: `FMPa` magic, length at least `0x2C`, segment count at `0x08` in `1..=MAX_SEGMENTS`, rate field at `0x28`; `FNPa` magic, length at least 20, entry count at `0x0A`, `len == 20 + 2 * count`; ids at `0x06` MUST match and counts MUST match. Any violation → exit 2 |
| R7 | Output: exactly the contract above. Units sorted by name; links in file order. Only counts, unit names, link targets, and small ordinals. No ids, offsets, hex, coordinates, or bytes |
| R8 | Non-goals: geometry, collision, vertex/face decode, object placement, transforms, textures, rendering, audio playback, writing, extraction, export, SR2, Ghidra. No module outside `src/level.rs` and the new `main.rs` arm may change |
| R9 | Legal/tests: tests use SYNTHETIC byte buffers only; no test reads a game file; no game bytes, absolute paths, coordinates, hashes, or dumps are committed; the whitelist `.gitignore` stays |
| R10 | No regression: `--mesh`, `--pose`, `--bigfile`, `--textures`, `--audio`, the file path, and the directory path keep their prior messages and exit codes |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | At least one unit parsed and printed (0 links and an absent sidecar are valid) |
| 1 | Usage error: wrong argument shape, unknown flag, missing path |
| 2 | Malformed container or I/O: bad count/offsets, truncated table, missing own name, missing/unreadable path, no units in a directory, missing/extra sidecar, bad sidecar magic, sidecar id/count mismatch |
| 3 | Refused path: not a `.pcm` file and not a directory |

## Acceptance scenarios

- [ ] **Linked room directory:** `--level-info` on a room with links exits 0;
      every unit prints a `level` line, at least one has `links>0`, and each
      link has a `  link <target> <index>` line.
- [ ] **Linkless unit:** a `.PCM` with no link names exits 0 with `links=0`
      and no link lines.
- [ ] **Sidecar present:** a unit with PMF/PNF prints `pmf=present`, the
      PMF segment count, the rate field, and the PNF entry count; the two
      counts are equal.
- [ ] **Sidecar absent:** a unit with neither file exits 0 with
      `pmf=absent segments=0 rate_field=0 pnf=0`.
- [ ] **Own-name guard:** a synthetic buffer whose only name differs from its
      stem exits 2; a buffer with the stem name passes.
- [ ] **Header refusals:** count below 2, count above the ceiling, offsets not
      starting `0, 4`, a descending pair, a table past EOF, and a last record
      past EOF each exit 2 with a one-line stderr message and no panic.
- [ ] **Sidecar refusals:** a lone PMF, a lone PNF, a bad `FMPa`/`FNPa`
      magic, a PNF length that contradicts its count, and an id or count
      mismatch each exit 2.
- [ ] **Invocation refusals:** no args, `--level-info` alone, two paths, an
      unknown flag, a `.pmf`/`.sra` path, a missing path, and a directory with
      no `.PCM` each produce the documented exit code (1, 2, or 3) and no
      partial output.
- [ ] **No regression:** all prior CLI forms print exactly what they printed
      before this change.
- [ ] **Unit level:** `cargo test` covers R3–R6 both arms, the exit matrix,
      sorting, duplicate-name counting, and the output line shape, without
      touching a game file.
- [ ] **Legal:** `git status --short` shows only intended source/doc files;
      no extracted bytes, absolute game paths, coordinates, or hashes.

## Next step

See `design.md` for the probe ledger, evidence classes, and UNKNOWNs;
`tasks.md` for the forecast and PR plan.

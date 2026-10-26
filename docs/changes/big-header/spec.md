# Spec: big-header

`cargo run -- --textures <file> --tlist` parses the `.BIG` header from the first four bytes, validates it, prints a header report, and exits 0 when the header is valid. Invalid headers — truncated file, wrong header size, implausible count — fail loud with a DISTINCT message and exit 1, and never print a success report. The entry table stays UNKNOWN, and `--tshow` stays a loud exit 1. Every other CLI mode keeps its prior behavior.

## Quick path

1. Run `cargo run -- --textures "<any .BIG from the install>" --tlist` (path at runtime only, never committed).
2. Confirm stdout reports a 512-byte header and an entry count; confirm exit code 0.
3. Point it at a truncated or wrong-size file and confirm a distinct stderr message and exit 1.
4. Run `--tshow 0` on a valid file and confirm the loud UNKNOWN entry-table error, exit 1, no window.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shapes stay unchanged: one positional path, `--bigfile ... --list/--show`, `--textures <path> --tlist`, `--textures <path> --tshow <sel>`, and the usage errors. Missing path, unknown verb, or wrong arg count is a LOUD error + exit 1 |
| R2 | The `.BIG` header SHALL be read from exactly two little-endian `u16` fields: header size at bytes `0..2` and entry count at bytes `2..4`. No other byte participates in the parse |
| R3 | Header size MUST equal 512. Any other value is a LOUD error naming the found value and the expected 512, exit 1 |
| R4 | Entry count MUST be within `1..=32768`. Zero or greater than 32768 is a LOUD "implausible entry count" error naming the value and the sanity range, exit 1 |
| R5 | A file shorter than the 512-byte header MUST fail as a LOUD truncation error naming the observed length and the expected header size, exit 1. A short read during the 4-byte prefix read is also a LOUD error, exit 1 |
| R6 | On a valid header, `--tlist` SHALL print a report to stdout carrying: the 512-byte header size, the parsed entry count, a coarse size class, the existing probe window classes, and the line `entry table: UNKNOWN (location not established; no stride inferred)`. It SHALL exit 0. Exact byte lengths MUST NOT be printed |
| R7 | The entry table MUST remain explicitly UNKNOWN in output, code, and docs: no index offset, no stride, no entry lines, and no inferred entry fields. The existing table-hunt result stays false/UNKNOWN — it is evidence, not a parser |
| R8 | `--tshow <sel>` SHALL remain a LOUD error + exit 1 naming the selector, with no window and no entry-data read, whether or not the header is valid. A valid header changes only the reported reason (index location UNKNOWN), never the exit code |
| R9 | All other modes — single file, directory gallery, `--bigfile --list`, `--bigfile --show`, mesh modes — keep their exact prior behavior and exit codes |
| R10 | No game bytes, install paths, hashes, offsets, or content dumps are committed. Tests use SYNTHETIC bytes only in temp dirs; the real files are never touched by tests |

## Message contracts

| Condition | Stream | Message (shape) | Exit |
|---|---|---|---|
| Valid header | stdout | `BIG header: 512-byte header, <count> entries` + `entry table: UNKNOWN (location not established; no stride inferred)` | 0 |
| File shorter than 512 | stderr | `truncated .BIG header: file holds <len> bytes, fewer than the 512-byte header` | 1 |
| Header size != 512 | stderr | `not a .BIG container: header size word <found>, expected 512` | 1 |
| Count outside `1..=32768` | stderr | `implausible .BIG entry count: <count> (sanity range 1..=32768)` | 1 |
| `--tshow` on a valid header | stderr | `no entry table identified for selector '<sel>': index location UNKNOWN (no stride inferred)` | 1 |
| Bad/missing args | stderr | usage lines (unchanged) | 1 |

## Acceptance scenarios

- [ ] **Valid header (`--tlist`):** on each of the four real containers, stdout reports `512-byte header` and that container's parsed count, the UNKNOWN entry-table line is present, and the exit code is 0. Planning probe recorded counts 1287 / 29 / 268 / 178 for TEXTURES / MOVIES / BONUS / E3.
- [ ] **Truncated file:** a 100-byte synthetic file exits 1 with the truncation message; a 2-byte synthetic file does the same (not a panic, not a partial report).
- [ ] **Wrong header size:** a synthetic file whose first `u16` is 256 exits 1 with the wrong-size message naming 256 and 512.
- [ ] **Implausible count:** synthetic count `0` and `32769` each exit 1 with the implausible-count message; no report is printed.
- [ ] **`--tshow` loud:** with a valid synthetic header, `--tshow 0` exits 1 with the selector message; with an invalid header it exits 1 with the corresponding header error. No window opens in either case.
- [ ] **Entry table still UNKNOWN:** no scenario lists entries, and no code path computes an index offset or stride.
- [ ] **Modes unchanged:** `cargo run -- <dds>`, `cargo run -- <dir>`, and both `--bigfile` forms behave exactly as before.
- [ ] **Unit level:** `cargo test` covers valid parse, all three rejection classes, summary content (including the no-exact-length rule), and the four CLI exit codes — on synthetic bytes only.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; no game bytes or install paths appear in artifacts or commits.

## Next step

See `design.md` for the exact parsed byte ranges, the decisions kept UNKNOWN, and the alternatives rejected; `tasks.md` for the ordered work units and review forecast.

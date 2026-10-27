# Tasks: area-levels

Read-only diagnostic slice. No geometry, no placement, no render. `test_command`
for every apply phase: `cargo test`. Tests use synthetic bytes only. The
planning bundle (this file plus proposal/spec/design/notes) is committed on the
planning branch and is **not** counted in the apply forecast below.

## Review Workload Forecast

The split is explicit: production and docs are counted separately, and the
test figure is the first instinct multiplied by 1.5.

| Bucket | Lines | Note |
|---|---|---|
| Production | **300** | `src/level.rs` ~235, `main.rs` ~55, wiring ~10 |
| Test code | 200 → **300** | first instinct 200, x1.5 |
| Docs (README + note addendum) | **50** | README "What works/missing", note appendix |
| **TOTAL** | **650** | 150 lines of headroom under the 800 session budget |

- **Per-PR ceiling:** 400 authored lines (repo rule). 650 exceeds it, but the
  session review budget is **800**, and 650 fits with 150 lines of headroom.
- **PR split plan:** **single PR**. It contains `src/level.rs` (header, name
  table, sidecar), the `main.rs` arm, the synthetic tests, and the README/note
  lines. Tests travel with the behavior they prove.
- **Stop rule:** if production exceeds 330 authored lines or tests exceed 330,
  stop and split: PR 1 = `src/level.rs` + `parse_pmf`/`parse_pnf`/`parse_pcm` +
  the single-file CLI + unit tests; PR 2 = directory scan + sidecar wiring +
  CLI tests + docs (PR 2 stacks on PR 1). Do not carry overage into a second
  commit of the same PR.
- **Why not a chain up front:** one module, one CLI arm, no render path, no new
  crate, no uncertain geometry semantics. The risky work (geometry and
  placement) is explicitly out of scope.

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Medium
```

(`stacked-to-main` applies only if the stop rule forces the two-PR split; with
`Chained PRs recommended: No` no chain is created up front.)

### Suggested work units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | PCM/PMF/PNF parsers in `src/level.rs` | PR 1 (single) | `cargo test level` | N/A — pure functions on buffers | delete `src/level.rs` + its tests |
| 2 | `--level-info` single-file arm | PR 1 (single) | `cargo test level_info` | `cargo run -- --level-info <one real .PCM>` | revert the `main.rs` arm |
| 3 | Directory scan + sidecar wiring | PR 1 (single) | `cargo test level_info` | `cargo run -- --level-info <room dir>` | revert the scan branch |
| 4 | README + note appendix | PR 1 (single) | `cargo test` | N/A — docs | revert the two doc hunks |

## Phase 1 — Foundation

- [ ] 1.1 Create `src/level.rs` with `MAX_RECORDS`, `MAX_SEGMENTS`,
      `MAX_NAME_LEN`, `LevelError`, and the public structs from the design.
- [ ] 1.2 Implement `parse_pmf`: magic, length floor, id, count range, rate
      field; refuse with `BadSidecarMagic` or `CountCeiling`.
- [ ] 1.3 Implement `parse_pnf`: magic, length floor, count, `len == 20+2n`;
      refuse a contradiction.
- [ ] 1.4 Add `#[cfg(test)]` builders for synthetic PCM/PMF/PNF buffers.

## Phase 2 — Core parser

- [ ] 2.1 `parse_pcm`: read `N`, bounds-check `4+4N`, read the offset list,
      require `0, 4`, strict ascent, and last record inside the file.
- [ ] 2.2 Extract names with the lexical rule; count all accepted names and
      split `<target>,<index>` into `Link`; preserve file order and duplicates.
- [ ] 2.3 Require the expected stem in the name list; otherwise
      `MissingUnitName`.
- [ ] 2.4 Keep the parser allocation-bounded: one offset `Vec`, names capped by
      the file length, no payload copy.

## Phase 3 — Path resolution and CLI

- [ ] 3.1 `read_path`: `.pcm` file → one unit; directory → its `BIN` if present,
      else itself; sort units by name; `NoUnits` when the scan is empty.
- [ ] 3.2 Sidecar lookup: sibling `X.PMF`/`X.PNF` case-insensitive; both, one,
      or neither; `SidecarIncomplete` on a lone file; id/count checks.
- [ ] 3.3 Add the `--level-info <path>` arm in `src/main.rs`; exactly two args
      or a loud usage error + exit 1.
- [ ] 3.4 Path gate: not `.pcm` and not a directory → exit 3, before reading.
- [ ] 3.5 Print the `level` line and link lines from the spec; map `LevelError`
      to exit 1/2/3 with a one-line stderr message; print nothing on failure.

## Phase 4 — Tests (synthetic only)

- [ ] 4.1 PCM header: valid, count 1, count above ceiling, `0/4` violation,
      descending pair, table past EOF, last record past EOF.
- [ ] 4.2 Names: own-name pass, own-name fail, link split, duplicate count,
      over-long name ignored, uppercase noise ignored.
- [ ] 4.3 Sidecars: valid pair, absent pair, lone PMF, lone PNF, bad magic,
      PNF length contradiction, id mismatch, count mismatch.
- [ ] 4.4 CLI: no args, flag alone, two paths, unknown flag, `.pmf` path,
      empty directory, one-file happy path, directory happy path; assert the
      exact exit code and that a failure prints no `level` line.
- [ ] 4.5 Regression: `--mesh`, `--pose`, `--bigfile`, `--textures`, `--audio`,
      bare file, bare directory keep their prior messages and exit codes.

## Phase 5 — Docs and acceptance

- [ ] 5.1 README "What works / What's missing": `.PCM` inventory and link
      graph work; geometry, placement, and render still missing.
- [ ] 5.2 Append the apply result to `docs/notes/area-levels.md` (counts only;
      no coordinates, no absolute paths, no bytes).
- [ ] 5.3 User acceptance: one linked room, one linkless room, one
      sidecar-present room, one sidecar-absent room; record relative names,
      output lines, exit codes. If any file contradicts the header model,
      STOP and document the failed check; do not reinterpret a record as
      geometry.
- [ ] 5.4 Verify: `cargo test` green; `git status --short` only whitelisted
      files; no game bytes, absolute paths, coordinates, or hashes.

## Apply gate

- [ ] Merged only if `cargo test` is green, the 0/1/2/3 exit matrix is proven,
      and the four acceptance paths print the documented shape.
- [ ] No claim of geometry, object placement, transforms, playback, or
      rendering appears in any commit, README line, or output string.

# Tasks: room-geometry

Read-only census slice. No geometry, no placement, no render. `test_command` for
every apply phase: `cargo test`. Tests use synthetic bytes only. This planning
bundle is committed on `feature/room-geometry-plan` and is **not** counted in the
apply forecast.

## Review Workload Forecast

| Bucket | Lines | Note |
|---|---|---|
| Production | **120** | `src/level.rs` ~85, `main.rs` ~35 |
| Test code | 90 -> **135** | first instinct 90, x1.5 |
| Docs (README + note addendum) | **35** | one status line, one acceptance block |
| **TOTAL** | **290** | 110 lines under the 400 per-PR ceiling |

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Low
```

- **PR split:** single PR. The classifier, the CLI arm, the tests, and the docs
  are one reviewable unit; tests travel with the behavior they prove.
- **Stop rule:** if production exceeds 150 or tests exceed 175, stop and split
  PR 1 = sizes + classifier + unit tests; PR 2 = CLI + integration tests + docs.
  Do not carry overage into a second commit of the same PR.

### Suggested work units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | Block sizes + classifier in `src/level.rs` | PR 1 | `cargo test level` | N/A - pure functions | delete the new types and `block_sizes` field |
| 2 | `--level-records` arm | PR 1 | `cargo test level_records` | `cargo run -- --level-records <one real .PCM>` | revert the `main.rs` arm |
| 3 | Census tests | PR 1 | `cargo test census` | N/A - synthetic fixtures | delete the test module entries |
| 4 | README + note addendum | PR 1 | `cargo test` | N/A - docs | revert the two doc hunks |

## Phase 1 - Foundation

- [ ] 1.1 Add `block_sizes: Vec<u32>` to `LevelUnit`; fill it in `parse_pcm`
      while the ascending table is already being validated.
- [ ] 1.2 Add `BlockClass` and `classify(block: &[u8])`; precedence is `zero`
      first, then `small4`/`fixed40`/`fixed48`, then `Other(size)`.
- [ ] 1.3 Keep `Sidecar`, `Link`, and the `--level-info` line untouched.

## Phase 2 - Census

- [ ] 2.1 Add `BlockCensus` and `census(&LevelUnit)`; counts are exclusive and
      must sum to `records`.
- [ ] 2.2 Compute `largest` as `(ordinal, size)`; ties go to the lowest ordinal.
- [ ] 2.3 Add `record_lines(&LevelUnit)` printing `blocks <unit> total=<n>`,
      the class lines in fixed order, and `largest <ordinal> <size>`.
- [ ] 2.4 Do not print payload bytes, file offsets, coordinates, or transforms.

## Phase 3 - CLI

- [ ] 3.1 Add the `--level-records <path>` arm in `src/main.rs`; exactly one
      non-flag argument or a usage error + exit 1.
- [ ] 3.2 Reuse `level::read_path`, the exit matrix (0/1/2/3), and the
      no-partial-output rule; print all units before the first line.
- [ ] 3.3 Add the usage line; leave every other mode's text unchanged.

## Phase 4 - Tests (synthetic only)

- [ ] 4.1 Class precedence: all-zero 4/40/48 -> `zero`; non-zero 4/40/48 ->
      their class; 72 -> `other:72`.
- [ ] 4.2 Sum invariant: class counts equal the header's block count on a mixed
      fixture; `other` rows are ascending by size.
- [ ] 4.3 Largest: unique, tied (lowest ordinal wins), and when the largest is
      also a 40/48 block (its class count still includes it).
- [ ] 4.4 CLI: no args, flag alone, two paths, unknown flag, `.pmf` path, empty
      directory, single file, directory; assert exit codes and no `blocks` line
      on failure.
- [ ] 4.5 Regression: `--level-info` output unchanged; `--mesh`, `--pose`,
      `--bigfile`, `--textures`, `--audio` keep their messages and exit codes.

## Phase 5 - Docs and acceptance

- [ ] 5.1 README: point "what works / what's missing" at the block census;
      geometry, placement, and render stay missing.
- [ ] 5.2 Append the apply result to `docs/notes/room-geometry.md` (counts
      only; no bytes, offsets, coordinates, or absolute paths).
- [ ] 5.3 User acceptance: one small, one medium, one large unit; record
      relative names, the census lines, and exit codes. If a unit contradicts
      the size model, STOP and document the failed check.
- [ ] 5.4 Verify: `cargo test` green; `git status --short` only whitelisted
      files; no game bytes, absolute paths, coordinates, or hashes.

## Apply gate

- [ ] Merged only if `cargo test` is green, the 0/1/2/3 matrix is proven, and
      the three acceptance units print the documented shape.
- [ ] No claim of geometry, vertex/face count, placement, transform, or render
      appears in any commit, README line, or output string.

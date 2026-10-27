# Tasks: placement

Read-only diagnostic slice. No coordinate, transform, placement, or render
decode. `test_command` for every apply phase: `cargo test`. Tests use synthetic
bytes only.

This planning bundle (proposal, spec, design, tasks, findings note) is committed
on `feature/placement-plan` and is **not** counted in the apply forecast.

## Review Workload Forecast

| Bucket | Lines | Note |
|---|---|---|
| Production | **100** | `src/level.rs` ~75, `src/main.rs` ~25 |
| Test code | 85 -> **128** | first instinct 85, x1.5 |
| Docs (README + note addendum) | **45** | one status line, one acceptance block, one correction pointer |
| **TOTAL** | **273** | 127 lines under the 400 per-PR ceiling |

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Low
```

- **PR split:** single PR. The census, the CLI arm, the tests, and the docs are
  one reviewable unit; tests travel with the behavior they prove.
- **Stop rule:** if production exceeds 130 or tests exceed 160, stop and split
  PR 1 = `GroupCensus` + `group_lines` + unit tests; PR 2 = CLI + integration
  tests + docs. Do not carry overage into a second commit of the same PR.

### Suggested work units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | `GroupCensus` + `group_census` in `src/level.rs` | PR 1 | `cargo test level::tests::group` | N/A - pure functions | delete the new type and function |
| 2 | `--level-groups` arm | PR 1 | `cargo test level_groups` | `cargo run -- --level-groups <one real .PCM>` | revert the `main.rs` arm |
| 3 | Census tests | PR 1 | `cargo test group_census` | N/A - synthetic fixtures | delete the test module entries |
| 4 | README + notes | PR 1 | `cargo test` | N/A - docs | revert the doc hunks |

## Phase 1 - Foundation

- [ ] 1.1 Add `GroupCensus` with the fields in the design; every field is a
      pure count.
- [ ] 1.2 Add `group_census(&LevelUnit)`; iterate non-zero 40/48-byte blocks
      only; count words, `whole12`, `tail_words`, marker by phase, value by
      phase, and `unaligned`.
- [ ] 1.3 Keep `BlockClass`, `census`, and `record_lines` untouched.

## Phase 2 - Group diagnostic

- [ ] 2.1 Count `words` as `size / 4` for each non-zero 40/48-byte block.
- [ ] 2.2 Count `whole12 = size / 12` and `tail_words = (size % 12) / 4`.
- [ ] 2.3 Count `marker34` when `word >> 24 == 0x34`; split by
      `word_index % 3`.
- [ ] 2.4 Count `value_f32_range` when the word is a finite `f32` with
      `1e-3 <= |value| <= 1e6`; split by phase.
- [ ] 2.5 Count `unaligned` for non-zero blocks whose size is not a multiple
      of 4; those blocks contribute no words.
- [ ] 2.6 Add `group_lines(&LevelUnit)` printing the exact spec shape and
      ordering; never print payload bytes, offsets, coordinates, or type names.

## Phase 3 - CLI

- [ ] 3.1 Add the `--level-groups <path>` arm in `src/main.rs`; exactly one
      non-flag argument or a usage error + exit 1.
- [ ] 3.2 Reuse `level::read_path`, the exit matrix (0/1/2/3), and the
      no-partial-output rule; print all units before the first line.
- [ ] 3.3 Add the usage line; leave every other mode's text unchanged.

## Phase 4 - Tests (synthetic only)

- [ ] 4.1 Partition: 40 B -> `whole12=3`, `tail_words=1`; 48 B ->
      `whole12=4`, `tail_words=0`.
- [ ] 4.2 Marker: `0x34` at word 0 of a 48-byte block -> `marker34=1`,
      `phase0=1`, other phases 0.
- [ ] 4.3 Value predicate: `1.5f32` counts; `1e-40f32` does not; NaN and
      Infinity do not; `0.0` does not.
- [ ] 4.4 Unaligned: a synthetic non-zero 6-byte block increments
      `unaligned` and contributes no words.
- [ ] 4.5 Zero blocks are excluded from every count.
- [ ] 4.6 Line shape and ordering exactly match the spec.
- [ ] 4.7 CLI: no args, flag alone, two paths, unknown flag, `.pmf` path, empty
      directory, single file, directory; assert exit codes and no `groups` line
      on failure.
- [ ] 4.8 Regression: `--level-info` and `--level-records` output unchanged;
      `--mesh`, `--pose`, `--bigfile`, `--textures`, `--audio` keep their
      messages and exit codes.

## Phase 5 - Docs and acceptance

- [ ] 5.1 README: point "what works / what's missing" at the group census;
      placement and transforms stay missing.
- [ ] 5.2 Append the apply result to `docs/notes/placement.md` (counts only; no
      bytes, offsets, coordinates, or absolute paths).
- [ ] 5.3 Add a one-line correction pointer at the end of
      `docs/notes/room-geometry.md` to the new note. Do not delete the original
      later-observation block; it was true for its unit.
- [ ] 5.4 User acceptance: one group-like unit and one non-group unit; record
      relative names, the `groups` lines, and exit codes. If a unit contradicts
      the word-grid model, STOP and document the failed check.
- [ ] 5.5 Verify: `cargo test` green; `git status --short` only whitelisted
      files; no game bytes, absolute paths, coordinates, or hashes.

## Apply gate

- [ ] Merged only if `cargo test` is green, the 0/1/2/3 matrix is proven, and
      both acceptance units print the documented shape.
- [ ] No claim of placement, transform, coordinate, vertex/face count, or
      render appears in any commit, README line, or output string.
- [ ] The shipped tool is described as a **diagnostic**, not a decoder, in the
      README and the note.

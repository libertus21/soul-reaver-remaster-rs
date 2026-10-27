# Tasks: sra-animation

Read-only diagnostic slice. No pose, no playback. `test_command` for every
apply phase: `cargo test`. Tests use synthetic bytes only. The planning
bundle (proposal/spec/design/notes) is committed on the planning branch and
is **not** counted in the apply forecast below.

## Review Workload Forecast

The two earlier slices missed by 3.7x and 3.9x because test lines were
under-budgeted. The test figure below is the first instinct multiplied by
1.5, and production/docs are counted separately.

| Bucket | Lines | Note |
|---|---|---|
| Production | **155** | `src/sra.rs` ~130, `main.rs` ~25 |
| Test code | 120 → **180** | first instinct 120, x1.5 |
| Docs (README + note addendum) | **30** | README "What works/missing" line, note appendix |
| **TOTAL** | **365** | 35 lines of headroom under the 400 ceiling |

- **Per-PR ceiling:** 400 authored lines (repo rule). 365 leaves 35.
- **Session review budget:** 800. A single PR at ~365 fits with room, so
  there is no chain.
- **PR split plan:** **single PR**. It contains `src/sra.rs` (header,
  probes, sparkline), the `main.rs` arm, the synthetic tests, and the
  README/note lines. Tests travel with the behavior they prove.
- **Stop rule:** if production exceeds 190 authored lines or tests exceed
  200, stop and split: PR 1 = `src/sra.rs` + refusals + unit tests; PR 2 =
  `--sra-info` arm + sparkline + CLI tests + README (PR 2 stacks on PR 1).
  Do not carry overage into a second commit of the same PR.
- **Why not a chain:** one module, one CLI arm, no render path, no new
  crate, no uncertain semantics to explore during apply. The risky work
  (binding a curve to a joint) is explicitly out of scope and belongs to a
  future change, not to this PR.

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Medium
```

(`stacked-to-main` applies only if the stop rule forces the two-PR split;
with `Chained PRs recommended: No` no chain is created up front.)

### Suggested work units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | Header + probes in `src/sra.rs` | PR 1 (single) | `cargo test sra` | N/A — pure functions on buffers | delete `src/sra.rs` + its tests |
| 2 | `--sra-info` arm + sparkline | PR 1 (single) | `cargo test sra_info` | `cargo run -- --sra-info <one real .SRA>` | revert the `main.rs` arm |
| 3 | README + note appendix | PR 1 (single) | `cargo test` | N/A — docs | revert the two doc hunks |

## Phase 1 — Module skeleton and refusals

- [ ] 1.1 Create `src/sra.rs` with `HEADER_FLOOR = 2048`, `MAX_DECLARATIONS`,
      `SraInfo`, `SraError`, and `pub fn parse(&[u8])`.
- [ ] 1.2 Implement the header rule `max(2048, 8 + 4*(N+1))`; refuse a
      length below the floor+16, header past EOF, and `N` above the ceiling.
- [ ] 1.3 Read the `N`-entry table; refuse a non-strictly-ascending entry and
      a non-zero terminator.
- [ ] 1.4 Add `#[cfg(test)]` builders for synthetic buffers (correct header,
      variable `N`, payload suffix).

## Phase 2 — Structural probes

- [ ] 2.1 `hierarchy_probe`: one pass over the payload; `Candidate` only for
      a run of at least 128 consecutive `u16` values in `1..=127`.
- [ ] 2.2 `curve_segments`: one pass over aligned `f32`; run = at least 8
      finite values in `0.0..=360.0`, strictly increasing, step `< 30.0`.
- [ ] 2.3 Keep the longest segment for rendering; cap stored samples
      (bounded memory) and document the cap.
- [ ] 2.4 `sparkline`: map a value range onto the 8 glyphs
      `.:-=+*#@`; a flat range must render one repeated glyph, not panic.

## Phase 3 — CLI and output contract

- [ ] 3.1 Add the `--sra-info <path>` arm in `src/main.rs`; exactly two args
      or a loud usage error + exit 1.
- [ ] 3.2 Extension gate: non-`.sra` path → message + exit 3, before reading.
- [ ] 3.3 Print the exact summary line from spec R8, then `curve: <sparkline>`
      or `curve: none`. No window, no GPU init.
- [ ] 3.4 Map `SraError` to exit 2 with a one-line stderr message; never
      panic on any input shape.

## Phase 4 — Tests (synthetic only)

- [ ] 4.1 Header tests: floor below 2 KiB, exact rule, `N = 0`, ceiling,
      truncation at the last table word.
- [ ] 4.2 Table tests: ascending, one descending pair, missing terminator,
      header past EOF.
- [ ] 4.3 Probe tests: 128-value `u16` run → `candidate`; one `200` inserted
      → `none`; ramp of 20 → 1 segment; one >30 step → 2 segments.
- [ ] 4.4 CLI tests: zero/multiple args, `--sra-info` alone, `.srm` path,
      1 KiB buffer, and the happy path stdout shape; assert exit codes
      0/1/2/3 and no window.
- [ ] 4.5 Regression: existing `--mesh` / `--pose` / `--bigfile` usage
      errors keep their prior messages and exit codes.

## Phase 5 — Docs and acceptance

- [ ] 5.1 README "What works / What's missing": `.SRA` is a diagnostic
      census; animation still missing; no hierarchy and no clip table.
- [ ] 5.2 Append the apply result to `docs/notes/sra-animation.md`
      (counts only; no coordinates, no paths).
- [ ] 5.3 User acceptance: run on one large character file, one prop, one
      stub; record relative names, the summary lines, and exit codes. If
      any file contradicts the header model, STOP and document the failed
      check; do not reinterpret a curve as a bone rotation.
- [ ] 5.4 Verify: `cargo test` green; `git status --short` only whitelisted
      files; no game bytes, absolute paths, or coordinates.

## Apply gate

- [ ] Merged only if `cargo test` is green, the exit-code matrix is proven
      for all four codes, and the census acceptance shows the header model
      on all three sample files.
- [ ] No claim of hierarchy, clips, posing, or playback appears in any
      commit, README line, or output string.

# Tasks: f4-launcher-cli

Ordered work units. `test_command` for apply: `cargo test`.

## Task 1 — Pure parser module (code + tests)

- [ ] Create `src/launcher_cli.rs` with `LauncherArgs`, `Game`, `EgsEnv`,
      `LauncherCliError`, and `pub fn parse(args: &[String]) -> Result<...>`.
- [ ] Implement `=`-split and space-separated value consumption; collect unknowns.
- [ ] Add unit tests: no-op flags, value flags, missing/invalid value, unknown
      token, mixed argv.
- [ ] Verify: `cargo test` green; parser has no IO/env/process calls.

## Task 2 — Viewer no-op wiring

- [ ] In `src/main.rs`, strip `--nosteam`/`-nocheck`/`-skipintro` from `args`
      before dispatch.
- [ ] Add tests asserting identical exit code/output for stripped vs unstripped.
- [ ] Verify: `cargo test` green; existing CLI tests unchanged.

## Task 3 — Hygiene proof + single PR

- [ ] `git status --short` shows only intended files; no listings/DBs/game data.
- [ ] Land as SINGLE PR to `main` (<=400 authored lines).
- [ ] Verify: `cargo test` green; PR diff contains no game data.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~260-340 (parser ~120 + tests ~90 + main.rs ~15 + docs ~120) |
| Per-task ceiling | Each task <=150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~340 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main |
| Work-unit mapping | Task 1 -> commit 1 (parser+tests), Task 2 -> commit 2 (no-ops), Task 3 -> PR (hygiene) |
| `delivery_strategy` | single-pr (per chained-pr gates: forecast fits <=400) |
| `test_command` | `cargo test` |

## Apply gate

- [ ] Parser + tests committed, `cargo test` green, no-ops wired, `git status`
      clean of unintended files, single PR <=400 authored lines.

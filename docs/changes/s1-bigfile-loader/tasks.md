# Tasks: s1-bigfile-loader

Ordered work units. Each is independently committable; verification travels with its unit. `test_command` for the apply phase: `cargo test`.

## Task 1 — Re-pin loader path + behavior notes (docs only)

- [ ] Reuse the TEMP Ghidra project; re-pin by STRING XREF at runtime: both error formats, the `CreateFileW` open cluster, the seek/size/read cluster. Record the fresh function labels + xref counts; never hardcode survey FUN@ names.
- [ ] Write `docs/notes/s1-bigfile-loader-behavior.md`: open→seek→read→error call order, per-step failure conditions, stage→message mapping (spec O1/O2/O3 rows confirmed or marked UNKNOWN), termination observable.
- [ ] If DRM/anti-cheat/packing blocks this line: STOP, report per spec R6 (none expected — survey: none).
- [ ] Verify: second pair of eyes can re-run the xref from the strings alone; `cargo test` green (no code touched).

## Task 2 — Confirm parity mapping (docs only)

- [ ] Fill the spec parity table from Task 1 notes: one confirmed/UNKNOWN verdict per failure case (missing file, truncated table, entry past EOF, seek failure, short read).
- [ ] Any row still UNKNOWN after the bounded pass stays UNKNOWN and is EXCLUDED from Task 3 — record the exclusion, do not guess.
- [ ] Verify: every spec table row traces to a notes-file observation; `cargo test` green.

## Task 3 — Message/exit-code alignment (targeted code)

- [ ] In `src/bigfile.rs`, change ONLY `BigfileError` Display prefixes per the confirmed mapping (design § Details): load-failure prefix for pre-open failures, read-failure prefix for post-open failures, detail kept as suffix. `BadSelector`/`NotDds`, parser, range checks, CLI arms UNCHANGED.
- [ ] Keep the loud-nonzero contract: every failure still stderr + exit 1 via the existing `main.rs` wrapper.
- [ ] Verify: `cargo test` green; manual `--bigfile` runs on synthetic missing/truncated inputs print the new prefixes with exit 1.

## Task 4 — Synthetic-bytes regression tests (tests with code)

- [ ] Extend `mod tests` in `src/bigfile.rs` with the existing synthetic-container pattern: one test per parity-table failure (missing file, truncated table, past-EOF entry, short read) asserting the Display prefix; CLI-level `ExitCode(1)` assertions in `main.rs` `cli_tests` style where cheap.
- [ ] No game files touched; no game bytes/paths/hashes committed.
- [ ] Verify: `cargo test` green; new tests fail if the Task 3 prefixes are reverted (true regression pins).

## Task 5 — Hygiene proof + single PR

- [ ] `git status --short` shows only intended files (behavior notes, `src/bigfile.rs`, tests, these plan docs); `git check-ignore` re-proof that Ghidra DBs/listings stay excluded; binaries untouched.
- [ ] Land as a SINGLE PR to `main` (≤400 authored lines); overflow, if any, filed as a scoped follow-up with its own budget — never silently absorbed.
- [ ] Verify: `cargo test` green; PR diff contains no listings, DBs, or game data.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~250-350 (plan docs ~170 now + behavior notes ~80-110 + code ~10-30 + tests ~40-70) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low-Medium (forecast ~350 max vs 400 budget; Task 4 is the flex — drop CLI-level asserts first if the cap tightens) |
| Decision needed before apply | No — proceed as SINGLE PR to main |
| Work-unit mapping | Task 1 -> commit 1 (notes), Task 2 -> commit 2 (mapping), Task 3 -> commit 3 (prefixes), Task 4 -> commit 4 (tests), Task 5 -> PR (hygiene proof) |
| Rebase point for apply | Updated `main` at `40cab52`; this plan branch carries docs only, so apply branches fresh from `main` |
| `delivery_strategy` | single-pr (per chained-pr gates: forecast fits ≤400) |

## Apply gate

- [ ] Behavior notes committed with confirmed/UNKNOWN mapping, code implements exactly the confirmed mapping, `cargo test` green, `git status --short` shows only intended files, single PR ≤400 authored lines.

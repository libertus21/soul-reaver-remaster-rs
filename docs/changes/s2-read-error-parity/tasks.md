# Tasks: s2-read-error-parity

Ordered work units. Each is independently committable; verification travels with its unit. `test_command` for the apply phase: `cargo test`.

## Task 1 — Re-pin + edge trace + behavior-notes addendum (docs only)

- [ ] Reopen the TEMP Ghidra project read-only (`-noanalysis`); re-pin by STRING XREF at runtime: both error formats, open cluster, seek/size/read cluster. Never hardcode survey or S1 function labels.
- [ ] Trace call edges: callees of each cluster function + callers of each error-half function; record every cluster→error-half caller→callee pair (max 2 probe passes, 45 min wall box).
- [ ] Append the addendum to `docs/notes/s1-bigfile-loader-behavior.md`: fresh labels + counts, observed edges, per-row PROVEN/HELD verdict, termination re-check. Unresolved rows record an explicit no-edge result.
- [ ] If DRM/anti-cheat/packing blocks this line: STOP, report per spec R6 (none expected).
- [ ] Verify: second pair of eyes can re-run xref + edge queries from the strings alone; `cargo test` green (no code touched).

## Task 2 — Confirm per-row mapping (docs only)

- [ ] Fill the spec parity table from the Task 1 addendum: one PROVEN/HELD verdict per row (truncated count, truncated table, past-EOF, seek-fail, short-read, termination).
- [ ] Any row still HELD after the box stays HELD and is EXCLUDED from Task 3 — record the exclusion, do not guess. If NOTHING resolves, S2 narrows to a docs-only slice (addendum + holds) and stops before Task 3.
- [ ] Verify: every spec table row traces to an addendum observation; `cargo test` green.

## Task 3 — Message alignment for PROVEN rows (targeted code)

- [ ] In `src/bigfile.rs`, change ONLY `BigfileError` Display prefixes per the proven mapping (design § Details): read-failure prefix for proven post-open rows, detail kept as suffix. HELD rows, `BadSelector`/`NotDds`, parser, range checks, CLI arms UNCHANGED.
- [ ] Keep the loud-nonzero contract: every failure still stderr + exit 1 via the existing `main.rs` wrapper.
- [ ] Verify: `cargo test` green; manual `--bigfile` runs on synthetic inputs print the new prefixes with exit 1.

## Task 4 — Synthetic-bytes regression tests (tests with code)

- [ ] Extend `mod tests` in `src/bigfile.rs` with the existing synthetic-container pattern: proven rows assert the new `Can't read from bigfile:` prefix; held rows assert current text (hold pins). CLI-level `ExitCode(1)` assertions where cheap.
- [ ] New tests fail if Task 3 prefixes are reverted (true regression pins). No game files touched; no game bytes/paths/hashes committed.
- [ ] Verify: `cargo test` green.

## Task 5 — Hygiene proof + single PR

- [ ] `git status --short` shows only intended files (addendum, `src/bigfile.rs`, tests, these plan docs); `git check-ignore` re-proof that Ghidra DBs/listings stay excluded; binaries untouched.
- [ ] Land as a SINGLE PR to `main` (≤400 authored lines); anything unresolved is filed as a scoped follow-up with its own budget — never silently absorbed.
- [ ] Verify: `cargo test` green; PR diff contains no listings, DBs, or game data.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~280-380 (plan docs ~140 now + addendum ~60-90 + code ~10-30 + tests ~50-80) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Medium (forecast ~380 max vs 400 budget; flex: drop CLI-level asserts first, then narrow to resolved rows only) |
| Decision needed before apply | No — proceed as SINGLE PR to main; box expiry narrows scope automatically, no user call needed |
| Work-unit mapping | Task 1 -> commit 1 (addendum), Task 2 -> commit 2 (mapping), Task 3 -> commit 3 (prefixes), Task 4 -> commit 4 (tests), Task 5 -> PR (hygiene proof) |
| Rebase point for apply | Updated `main` at `cfcdac3`; this plan branch carries docs only, so apply branches fresh from `main` |
| `delivery_strategy` | single-pr (per chained-pr gates: forecast fits ≤400) |

## Apply gate

- [ ] Addendum committed with proven/held verdicts + named edges, code implements exactly the proven mapping, `cargo test` green, `git status --short` shows only intended files, single PR ≤400 authored lines.

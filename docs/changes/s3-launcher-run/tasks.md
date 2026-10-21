# Tasks: s3-launcher-run

Ordered work units. Each is independently committable; verification travels with
its unit. `test_command` for the apply phase: `cargo test`.

## Task 1 — SRX.exe import-table parse + re-pin (docs only)

- [ ] Create a new TEMP Ghidra project under `%TEMP%\opencode\ghidra-srx`;
  import SRX.exe read-only; run auto-analysis (fast, 989 KB).
- [ ] Parse the import table: enumerate external symbols with library + name +
  ordinal; record any `sr1.dll` library entry and any `run` import (name or
  ordinal).
- [ ] Re-pin the launcher literal by string xref (never hardcode survey RVAs);
  re-pin `sr1.dll`'s `run` export and `bigfile.dat` in the existing probe
  project (read-only, `-noanalysis`).
- [ ] Trace `LoadLibrary*` / `GetProcAddress` call sites; for each, list the
  referenced strings at the call site (launcher literal / `run`).
- [ ] If DRM/anti-cheat/packing blocks this line: STOP, report per spec R6 (none
  expected).
- [ ] Verify: a second pair of eyes can re-run the import-table + xref queries;
  `cargo test` green (no code touched).

## Task 2 — Linkage verdict + engine startup contract (docs only)

- [ ] Write `docs/notes/s3-launcher-run-behavior.md`: fresh labels + counts, the
  import entry or load call sites, per-claim CONFIRMED/UNKNOWN verdict (L1-L5),
  and the engine startup contract paraphrase.
- [ ] Any claim still UNKNOWN after the box stays UNKNOWN with an explicit
  no-evidence record; never guess.
- [ ] Verify: every verdict traces to a named import entry or call site;
  `cargo test` green.

## Task 3 — Hygiene proof + single PR

- [ ] `git status --short` shows only intended files (behavior notes plus these
  plan docs); `git check-ignore` re-proof that Ghidra DBs/listings stay excluded;
  binaries untouched.
- [ ] Land as a SINGLE PR to `main` (<=400 authored lines); anything unresolved is
  filed as a scoped follow-up with its own budget — never silently absorbed.
- [ ] Verify: `cargo test` green; PR diff contains no listings, DBs, or game
  data.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~220-320 (plan docs ~180 now + behavior notes ~60-110; docs-only, no code) |
| Per-task ceiling | Each task <=150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (docs-only; forecast ~320 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main; box expiry narrows scope automatically |
| Work-unit mapping | Task 1 -> commit 1 (import parse + re-pin), Task 2 -> commit 2 (verdicts + contract), Task 3 -> PR (hygiene proof) |
| Rebase point for apply | Updated `main` at `8d5874a`; this plan branch carries docs only, so apply branches fresh from `main` |
| `delivery_strategy` | single-pr (per chained-pr gates: forecast fits <=400) |

## Apply gate

- [ ] Behavior notes committed with per-claim CONFIRMED/UNKNOWN verdicts + named
  import entry or call site (or no-evidence record), `cargo test` green,
  `git status --short` shows only intended files, single PR <=400 authored lines.

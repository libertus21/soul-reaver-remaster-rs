# Tasks: s4-run-internals

Ordered work units. Each is independently committable; verification travels
with its unit. `test_command` for the apply phase: `cargo test`.

## Task 1 — Re-pin + ordered call-site walk (docs only)

- [ ] Open the TEMP engine project read-only, `-noanalysis`. If the marker
  file is missing, re-import into a fresh TEMP project and note it; never
  write to the game dir.
- [ ] Re-pin by export name: `run`, export count, direct-callee count. No
  address is copied into any artifact.
- [ ] Walk `run`'s CALL instructions in ascending address order; record
  call-site ordinal, callee role label, instruction count, callee count.
- [ ] Per callee: defined-string refs (short printable quotes) and
  thunk-resolved external names. Fix the operand-range import read if the
  result is zero; that zero is a probe artifact, recorded as such.
- [ ] DRM/anti-cheat/packing on this line: STOP and report (none expected).
- [ ] Verify: a second pair of eyes reproduces the counts and the ordered list;
  `cargo test` green (no code touched).

## Task 2 — Stage table + subsystem entry points (docs only)

- [ ] Write `docs/notes/s4-run-internals-behavior.md` (<=65 authored lines):
  ordered stage table with provisional name, callee role label, anchors, and
  evidence class (CONFIRMED edge / CONFIRMED anchor / INFERRED / UNKNOWN), plus
  the fixed sentence separating static call-site order from runtime order.
- [ ] Add a subsystem entry-point table: one owning function per subsystem, its
  first call edge, and its anchoring evidence; anything unanchored stays
  UNKNOWN with its no-evidence result (for example `device_api_xrefs=0`).
- [ ] Record the default-archive handoff as step zero and its reach into the
  archive open path, re-confirmed by re-pin.
- [ ] Verify: every row cites a call site and/or anchor; no row is invented
  from plausibility; `cargo test` green.

## Task 3 — State surfaces + follow-up slices + not-established list (docs only)

- [ ] Add the state-surface table (archive selection, texture overrides,
  text/localization, audio bank, front end, bonus, movies), each tied to the
  xref-anchored table or message family; field layouts and write order stay
  UNKNOWN.
- [ ] Add the follow-up slice list (S5-S9 plus the deferred dynamic trace) with
  a one-line reason each; nothing is absorbed into S4.
- [ ] Add the "deliberately not established" list and the re-run steps
  (second pair of eyes), plus the legal block (read-only binaries, TEMP
  projects outside the repo, no bytes/paths/hashes/offsets).
- [ ] Record box usage: probe passes used, wall time, and every row demoted to
  UNKNOWN on expiry.
- [ ] Verify: reviewer can replay the walk and get the same table; `cargo test`
  green.

## Task 4 — Hygiene proof + single PR (docs only)

- [ ] `git status --short` shows only intended files (behavior note plus these
  plan docs); `git check-ignore` re-proof that Ghidra DBs/listings stay
  excluded; binaries untouched.
- [ ] Land as a SINGLE PR to `main` (<=400 authored lines); anything unresolved
  is filed as a scoped follow-up with its own budget.
- [ ] Verify: `cargo test` green; PR diff contains no listings, DBs, or game
  data; no retail offsets or addresses anywhere in the diff.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~380-394 (plan docs ~329 now + behavior note <=65; docs-only, no code) |
| Per-task ceiling | Each task <=150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low-Medium (~394 max vs 400 budget; behavior note capped at 65 lines) |
| Decision needed before apply | No — single PR to main; if the note runs past 65 lines, trim anchor detail, never widen scope |
| Work-unit mapping | Task 1 -> commit 1 (re-pin + ordered walk), Task 2 -> commit 2 (stage table + entry points), Task 3 -> commit 3 (surfaces + follow-ups), Task 4 -> PR (hygiene proof) |
| Rebase point for apply | Updated `main` after this plan merges; this plan branch carries docs only, so apply branches fresh from `main` |
| `delivery_strategy` | single-pr (per chained-pr gates: forecast fits <=400) |
| Runtime harness | N/A — docs-only slice, no executable behavior added |

## Apply gate

- [ ] Ordered call-site evidence exists for every stage row; every name has a
  class; every UNKNOWN has a no-evidence record; box usage recorded;
  `cargo test` green; `git status --short` shows only intended files; single PR
  <=400 authored lines; no retail addresses, listings, or game bytes.

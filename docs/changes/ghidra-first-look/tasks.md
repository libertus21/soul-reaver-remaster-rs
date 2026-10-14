# Tasks: ghidra-first-look

Ordered work units. Each is independently committable; verification travels with its unit. `test_command` for the apply phase: `cargo test` (no-op baseline — docs-only slice, proves nothing broke).

## Task 1 — Time-boxed headless auto-analysis + command log

- [ ] Reuse TEMP project (`%TEMP%\opencode\ghidra-probe\probe`); run default auto-analysis on the imported `sr1.dll`, time-box 30 min. On overrun: keep import-only DB, narrow to string-xref scripting, record the cap.
- [ ] Append every command + wall time to the survey's "Reproducibility" section (import 26.1 s already logged in spec).
- [ ] If DRM/anti-cheat/packing blocks analysis: STOP that line, report per spec R4 (none observed in Phase 1).
- [ ] Verify: command log re-runnable by a second pair of eyes; `cargo test` green.

## Task 2 — F1: bigfile loader candidates with string evidence

- [ ] Xref `Can't read from bigfile: %s` (@`0x1a57b0`) + archive-name cluster (@`0x200958`–`0x200984`) to code; name the loader function(s) as behavior paraphrases (open→seek→read→error), cross-checked against `CreateFileW`/`ReadFile`/`SetFilePointerEx` call sites. No listings.
- [ ] Confirm or correct each Phase 1 file offset; mark anything unconfirmed UNKNOWN.
- [ ] Verify: second-eyes spot check of ≥2 evidence strings with an independent strings tool; `cargo test` green.

## Task 3 — F2/F3: entry bootstrap + global-state layout notes

- [ ] F2: from entry `0x131ff0` + the single export (resolve name or record UNKNOWN) + `LoadLibraryExW`/`GetProcAddress` surface, paraphrase how `SRX.exe` reaches game code.
- [ ] F3: anchor major `.data` globals via data xrefs — layout only, no invented semantics.
- [ ] F4 stays optional (launcher CLI) — do it only if playtest needs demand it.
- [ ] Verify: `cargo test` green; notes stay behavior-level on re-read.

## Task 4 — Survey doc + hygiene proof + follow-up proposals

- [ ] Write `docs/notes/sr1-ghidra-survey.md`: binary-role table, string-evidence inventory, candidate function list (F1–F3 outcomes), follow-up slice proposals, reproducibility log.
- [ ] Hygiene gate: `git check-ignore` re-proof for `*.gpr`/`*.rep`; `git status --short` shows only intended `*.md` files; no game bytes/listings committed.
- [ ] Verify: `cargo test` green; `git status --short` clean of non-whitelisted files.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~250-330 (plan docs ~230 now + survey ~100-150 in apply; almost no code) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~330 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main |
| Work-unit mapping | Task 1 -> commit 1 (log), Tasks 2+3 -> commit 2 (findings), Task 4 -> commit 3 (survey + hygiene proof) |
| Rebase point for apply | Updated `main` at `9ee1c81`; this plan branch carries docs only, so apply branches fresh from `main` |

## Apply gate

- [ ] Survey committed, command log complete with wall times, spot check done, `cargo test` green, `git status --short` shows only intended `*.md` files, single PR ≤400 authored lines.

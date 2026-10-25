# Archive: f4-launcher-cli — DONE (pure parser + compatibility no-ops)

The launcher command-line surface found in `SRX.exe` strings is replicated as a
pure Rust parser with synthetic-argv unit tests, and three flags are exposed in
the viewer as documented no-ops. No engine code was changed, no binary was
invoked from tests, and every flag effect remains `UNKNOWN` (static evidence
only). PR #21 is merged into `main`; the contained slice branch is deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `094b132` (merge of PR #21).
2. `cargo test` — 102/102 green on synthetic bytes and argv only.
3. Read `docs/changes/f4-launcher-cli/spec.md` for the args table and the
   packed-string evidence token behind each flag.

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| Unknown argument | `-foo` → `error: no such file or directory: -foo`, exit 1 (user-confirmed). The parser collects unknown tokens instead of rejecting them; the viewer still fails loud on an unrecognized positional. |
| No-op flags | `--nosteam -nocheck -skipintro` stripped correctly (no output, exit code unaffected); `--nosteam --bigfile <missing>` → `Can't load big file named`, exit 1 (user-confirmed). |
| Regression suite | `cargo test` 102 passed / 0 failed (verify-confirmed). |
| Args discovered | 18 launcher flags (`nosteam`, `noegs`, `nolegal`, `novsync`, `dev`, `ldpi`, `egs_*`, `width`/`height`, `AUTH_PASSWORD`, `epic*=`, `-1`/`-2`); effects all UNKNOWN; `-nocheck`/`-skipintro` absent in this build → no-ops for compatibility. |
| Scope hold | Parser + no-op wiring only — no engine code changes; no binary invocation in tests. |

## Details

| Topic | Outcome |
|---|---|
| PR | [#21 — feat(launcher-cli): pure parser + compatibility no-op flags](https://github.com/libertus21/soul-reaver-remaster-rs/pull/21), state MERGED (`mergedAt` 2026-10-21T04:13:02Z), base `main`, head `feature/f4-launcher-cli`, 6 files, +529/−2. |
| Commits on `main` | `24af201` plan proposal/spec/design/tasks → `8ccf47b` pure parser + unit tests → `73bd719` strip compatibility no-op flags before dispatch → `094b132` PR #21 merge. |
| Delivered | `src/launcher_cli.rs`: `parse(&[String]) -> Result<LauncherArgs, LauncherCliError>` (pure: no IO, no env, no process spawn), plus `LauncherArgs`, `Game`, `EgsEnv`, and a manual `Display`/`Error` impl (no new crates). Both `-flag=value` and space-separated values are consumed; unknown tokens are collected, not rejected. `src/main.rs` strips `--nosteam`/`-nocheck`/`-skipintro` (single- and double-dash forms) before dispatch. |
| Deliberately not built | Engine-side effect of any flag, Steam/Epic integration, resolution/window changes, runtime execution, any binary invocation in tests. |
| Branch cleanup | `feature/f4-launcher-cli`: remote already deleted by the PR merge (fetch pruned it); local branch verified fully contained in `main` and deleted. `feature/f4-launcher-cli-plan` (remote): NOT deleted — strict ancestry containment fails (head `26a43f5` predates the rebased planning commit `24af201`), although its docs content is identical to `main`. Left for manual cleanup. |
| Follow-ups (TBD by user) | Wire the remaining parser-only flags into behavior only if a future slice requires them. Dynamic-trace slice for the still-HELD post-open rows and the still-UNKNOWN contract rows. S4 `run` internals. None is scoped by this archive. |
| Legal | Behavior notes only; no listings, no disassembly output, no Ghidra DBs, no game bytes/paths/hashes committed. Evidence uses short packed-string quotes already present in the committed spec. No DRM encounters observed on this line. |

## Checklist

- [x] PR #21 MERGED and `main` contains all slice commits
- [x] Playtest evidence recorded verbatim (unknown arg, no-op strip, 102/102 suite, args table)
- [x] Contained slice branch deleted after containment check; uncontained plan branch kept with reason
- [x] README "What works" gains the F4 line; "What's missing" keeps the next binary slice open (dynamic trace / S4)
- [x] No game bytes, paths, hashes, or offsets added by this archive

## Next step

User picks the next binary slice: the dynamic-trace candidate for the still-HELD
post-open rows and the still-UNKNOWN contract rows, or S4 (`run` internals) —
each with its own proposal → spec → tasks → code → playtest cycle under the same
slice discipline.

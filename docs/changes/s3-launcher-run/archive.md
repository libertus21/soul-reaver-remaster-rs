# Archive: s3-launcher-run — DONE (docs-only)

Launcher-to-engine handshake for SR1 is closed as a docs-only slice: SRX.exe has
no static import of `run` from `sr1.dll` (L1 NOT CONFIRMED), but dynamically
loads the engine library via `LoadLibraryA` with the `1\sr1.dll` literal and
resolves `run` via `GetProcAddress` in the same function (L2 CONFIRMED, so L3
CONFIRMED dynamic), and `run` (also exported as ordinal 1) references the
default archive name `bigfile.dat` within its first bytes (L4 CONFIRMED). The
engine startup contract beyond entry stays PARTIAL/UNKNOWN. No production code
changed. PR #19 is merged into `main`; contained slice branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `89b224e` (merge of PR #19).
2. `cargo test` — 88/88 green on synthetic bytes (no code touched).
3. Read `docs/notes/s3-launcher-run-behavior.md` for the handshake and the
   engine startup contract paraphrase.

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| L1 static import of `run` | NOT CONFIRMED — import-table parse: `sr1_library_symbol_count=0`, `sr1_external_function_count=0`, `ordinal_import_count=0`; the only `run`-named import is `STEAM_API64.DLL!SteamAPI_RunCallbacks` (unrelated). |
| L2 dynamic load + resolve | CONFIRMED — a `LoadLibraryA` call site in the launcher-bootstrap function references `1\sr1.dll`; a `GetProcAddress` call site in the SAME function references `run`. |
| L3 linkage is L1 or L2 | CONFIRMED (dynamic) — L2 CONFIRMED; L1 absent. |
| L4 `run` is the engine entry receiving the default archive name | CONFIRMED — engine export `run` (also ordinal 1) references `bigfile.dat` within its first bytes; re-pin reproduces the survey. |
| L5 engine startup contract | PARTIAL — handshake method is dynamic (L2) and the default-archive handoff is observable (L4); what `run` does after entry is UNKNOWN (static only; nothing executed). |
| Regression suite | `cargo test` 88 passed / 0 failed (user-confirmed; no code touched). |
| Scope hold | DOCS-ONLY — no production code changes; behavior notes only. |

## Details

| Topic | Outcome |
|---|---|
| PR | [#19 — docs(s3-launcher-run): confirm launcher-to-engine handshake](https://github.com/libertus21/soul-reaver-remaster-rs/pull/19), state MERGED (`mergedAt` 2026-10-21T03:01:02Z), base `main`, head `feature/s3-launcher-run`, 5 files, +317/-0. |
| Commits on `main` | `a6efb60` plan proposal/spec/design/tasks → `9a81c7c` record launcher import parse and re-pin → `6433a35` record linkage verdicts and engine startup contract → `89b224e` PR #19 merge. |
| Branch cleanup | `feature/s3-launcher-run` and `feature/s3-launcher-run-plan`: verified fully contained in `main`, deleted locally and from origin. Contained leftover archive branch deleted the same way (local + origin): `docs/archive-s2`. Explicitly kept (out of scope / not contained): `origin/dev`, `origin/dev-3.0`, `origin/Visor-flesh-3d-galeria` (long-lived/other lines); `origin/dev-2-0` and `origin/feature/texture-gallery-plan` (NOT contained in `main`). |
| Scope kept | SR1 only. L1 NOT CONFIRMED; L2/L3/L4 CONFIRMED; L5 PARTIAL. Adjacent observation (same launcher-bootstrap function, SR1 focus kept): the launcher also resolves a second engine procedure name `exec` from the same library, and repeats the load-and-resolve shape for the sibling engine library; neither changes the L1–L5 verdicts. No production-line changes: no code touched. `cargo test` 88/88 green. |
| Follow-ups (TBD by user) | F4 launcher CLI candidate (`-nosteam`, `SR1_*` IDs) — own slice. Dynamic-trace slice candidate for the still-UNKNOWN rows (post-open read/error halves and what `run` does after entry) — own slice. S4 `run` internals — own slice. None is scoped by this archive. |
| Legal | Behavior notes only; no listings, no Ghidra DBs, no game bytes/paths committed by this slice. Evidence uses import counts, call-site pairs, export names, and short string quotes already present in the committed survey/notes. No DRM encounters to report (none observed on this line). |

## Checklist

- [x] PR #19 MERGED and `main` contains all slice commits
- [x] Playtest evidence recorded verbatim (L1–L5 verdicts; 88/88 suite; docs-only scope hold)
- [x] Contained slice branches deleted after containment check; uncontained / out-of-scope branches explicitly kept with reason
- [x] README "What works" gains the S3 line; "What's missing" keeps the next binary slice open (F4 / dynamic trace / S4)
- [x] No game bytes, paths, hashes, or offsets added by this archive

## Next step

User picks the next binary slice among F4 (launcher CLI), the dynamic-trace candidate for the still-HELD post-open rows and the still-UNKNOWN contract rows, or S4 (`run` internals) — each with its own proposal → spec → tasks → code → playtest cycle under the same slice discipline.

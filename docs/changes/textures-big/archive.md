# Archive: textures-big — DOCUMENTED NO-GO

`TEXTURES.BIG` is a CUSTOM layout, not SR1PC, and the bounded table-hunt found no entry table — so this slice lands as a documented no-go: a custom-layout region classifier plus loud `--tlist` / `--tshow` arms (summary + exit 1, nothing listed, nothing decoded), synthetic-only tests, and a pointer at the next candidate. PR #10 is merged into `main`; contained feature branches are deleted. SR1 ONLY — no SR2-side work was done or scoped.

## Quick path

1. `git checkout main && git pull` — HEAD is `38b4a09` (merge of PR #10).
2. `cargo test` — 49/49 green on synthetic bytes; `cargo build` shows zero warnings.
3. `cargo run -- --textures <TEXTURES.BIG> --tlist` (your own install path at runtime) — prints the CUSTOM layout summary to stdout and exits 1 loud naming the next candidate; `--tshow <entry>` fails loud the same way with no window.

## Playtest evidence

| Item | Value |
|---|---|
| `--tlist` on the real TEXTURES.BIG | CUSTOM summary + exit 1 (user-confirmed, twice). |
| `--tshow 0` on the real TEXTURES.BIG | Loud no-table error + exit 1, no window (user-confirmed). |
| Single-file regression | Stone door renders (user-confirmed, verbatim "se vio la runa"). |
| Headless gates from verify | 49/49 tests, zero warnings, legal clean. |
| Outcome | Documented no-go (no entry table in file; custom layout verified, probes bounded ≤1.4KB read-only). |

## Details

| Topic | Outcome |
|---|---|
| PR | [#10 — feat(textures): probe TEXTURES.BIG custom layout, loud --tlist/--tshow no-go arms](https://github.com/libertus21/soul-reaver-remaster-rs/pull/10), state MERGED (`mergedAt` 2026-10-12T01:51:39Z), base `main`, head `feature/textures-big` |
| Commits on `main` | `19fc769` plan docs (proposal/spec/design/tasks) · `ea70073` Task 1: region classifier, `describe`, `--tlist` no-table arm + tests · `50d6986` Task 2: bounded table-hunt reporting documented no-table + tests · `e4ee7a7` Task 3: `--tshow` loud no-go arm + README slice docs · `3dd7c54` plan-docs merge into the feature branch · `31a3b37` merge-back of `main` into the feature branch · `38b4a09` PR #10 merge |
| Branch cleanup | Deleted locally and from origin after verifying fully contained in `main` (`git merge-base --is-ancestor`): `feature/textures-big`, `feature/textures-big-plan`, `docs/archive-gallery`, `docs/archive-bigfile`, `docs/archive-bc7-slices`. Explicitly kept: `feature/texture-gallery-plan` is NOT contained in `main` (local + remote kept); `origin/dev` is a long-lived branch, out of scope for slice cleanup (kept). |
| Scope kept | SR1 ONLY. `1\TEXTURES.BIG` probed read-only with bounded windows; `src/bigfile.rs` correctly rejects it (`NotBigfile` — count-at-0 exceeds the sanity limit); no table identified, so nothing listed, nothing decoded, nothing extracted. The SR2-side sibling is named ONLY as the next candidate in loud errors and docs — `2\BIGFILE.DAT` scoping is OUT, untouched by this slice. |
| Follow-ups | Chosen next: 3D-models milestone (own scoping probe, same slice discipline). Other SR1 `.BIG` files (`E3.BIG`, `BONUS.BIG`, `MOVIES.BIG`) remain unprobed — each needs its own probe before any claim. sRGB correctness pass, resize handling, and full mipmap-chain UI stay deferred. |
| Legal | No game bytes added by this archive; evidence uses counts and exit codes only (no hashes, offsets, or paths of game data); game data paths via CLI args at runtime only, never committed |

## Checklist

- [x] PR #10 MERGED and `main` contains all slice commits
- [x] Playtest note recorded (`--tlist` CUSTOM summary + exit 1 twice; `--tshow 0` loud exit 1 no window; single-file stone-door regression renders; user acceptance as no-go)
- [x] Headless gates recorded (49/49 tests, zero build warnings, loud-exit cases, legal clean)
- [x] `tasks.md` boxes reconciled to landed reality (Tasks 1–3 checked with no-go notes)
- [x] Contained branches deleted after containment check; uncontained / out-of-scope branches explicitly kept with reason
- [x] README "What works" marks textures-big as documented no-go; "What's missing" closes TEXTURES.BIG and queues 3D models next
- [x] No game bytes, hashes, offsets, or paths of game data added by this archive

## Next step

Plan the 3D-models milestone: proposal → spec → tasks with its own scoping probe first. Do not reopen `TEXTURES.BIG` without a new entry-table hypothesis and a fresh probe budget.

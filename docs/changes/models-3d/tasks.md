# Tasks: models-3d

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

## Task 1 — `src/srm.rs`: SRM parser (positions + indices) with unit tests

- [ ] Add `SrmError` (`Io` / `NotSrm` / `InvalidSrm(String)` — spec R-scenarios gate message + exit 1, not the type name) + `Mesh { positions, indices }` + `parse(bytes)` / `load(path)` per `design.md` (magic check, opaque header skips with pre-read range checks, counts sanity incl. `faceCount % 3 == 0`, `u64` overflow-safe range validation, 32-byte vertex stride positions, `u16` triplets, both sentinel arms).
- [ ] Unit tests on SYNTHETIC bytes only (hand-built minimal `.SRM` buffers in temp dirs via `tempfile`; no game files): magic accept/reject; header-skip arithmetic to exact EOF; both sentinel arms; huge-count + past-EOF rejection (no big allocation, no panic); exact position/index extraction; truncated-tail rejection; `Display` messages carry the failed check + expected-vs-found sizes.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — `render.rs` mesh pipeline + `--mesh` CLI arm

- [ ] Add `show_mesh(positions, indices)` pipeline per `design.md` (vertex/index buffers, `Depth24Plus`, auto-rotate uniform ~0.6 rad/s, fixed perspective, derivative flat-shading, culling OFF, CPU auto-fit with degenerate rejection). `show_rgba8` / gallery paths UNTOUCHED (spec R4).
- [ ] Wire `--mesh <path>`: exactly one file arg → `srm::load` → counts line to stdout → window (title per file name); any parse failure → loud stderr + exit 1, no window. Esc + window close exit 0. Existing file/dir/`--bigfile`/`--textures` arms UNTOUCHED (spec R1).
- [ ] Unit tests on SYNTHETIC bytes only: `--mesh` missing-path / bad-magic / truncated arms exit 1 loud (CLI-level, no window, no GPU); auto-fit math (centering/scale, degenerate rejection) as pure-function tests.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 3 — README + playtest evidence + legal gate

- [ ] Update `README.md` "What works" for `--mesh` (format verdict, positions+indices-only rule, flat-shaded auto-rotate, in-memory guarantee, playtest file by relative name) without adding game paths/coordinates/offsets.
- [ ] Playtest (acceptance, user): `--mesh` on the real playtest `.SRM` — rotating solid on screen + Esc exits 0; record file + observation + exit code. Regression: single-file DDS still renders.
- [ ] Legal: `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes; no extracted bytes, no game paths/coordinates/offsets/content dumps committed (counts output stays on stdout, never in docs).
- [ ] Verify: `cargo test` passes; playtest recorded; `git status --short` clean of non-whitelisted files.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~260-340 (Task 1 ~100-130 incl. tests, Task 2 ~120-150 incl. pipeline + arm, Task 3 ~40-60 incl. README; no generated files; `Cargo.lock` untouched — zero new crates) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~340 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main (chain strategy stays deferred: chaining is not needed; this slice does NOT trigger the first real use of chaining) |
| Work-unit mapping | Task 1 -> commit 1 (parser + tests), Task 2 -> commit 2 (mesh pipeline + `--mesh` arm + tests), Task 3 -> commit 3 (README + playtest evidence; tests travel with their behavior per work-unit-commits) |
| Rebase point for apply | Updated `main` at `09a56cb` (merge of PR #11 archive-textures-big); this plan branch carries docs only, so apply branches fresh from `main` |

## Apply gate

- [ ] `cargo test` green, `--mesh` playtest note recorded (rotating solid + Esc exit 0), `git status --short` clean of non-whitelisted files, single PR to main.

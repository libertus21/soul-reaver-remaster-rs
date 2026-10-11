# Tasks: texture-gallery

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

## Task 1 — Listing + navigation core: `collect_dds_entries` + `step_index` (with unit tests)

- [ ] Add `collect_dds_entries(dir) -> Result<Vec<PathBuf>, ViewerError>` (in `dds.rs` or a NEW `gallery.rs` — apply picks the smaller diff): `read_dir`, keep files with case-insensitive `.dds` extension, sort ascending by file name; empty result is a loud error (message names the directory).
- [ ] Add pure `step_index(current: usize, len: usize, dir: Next/Prev) -> usize` with clamp-at-ends (no wrap).
- [ ] Unit tests on SYNTHETIC temp dirs/files only (no game files; `tempfile` dev-dependency allowed): sorted order incl. mixed-case `.DDS`; non-`.dds` files and subdirs excluded; empty dir → loud error; `step_index` clamps at both ends; single-entry list never moves.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — `render.rs`: gallery window (`show_gallery` + keyboard + re-upload + title)

- [ ] Add `pub fn show_gallery(entries: &[PathBuf]) -> Result<(), String>`: same `State` construction as `show_rgba8` for the first DECODABLE entry (skip loop with `warning: skipping {filename}: {e}` per failure; zero successes → `Err` naming the directory); `WindowEvent::KeyboardInput` handles ArrowRight/ArrowLeft (Pressed, non-repeat) via `step_index` + skip loop + re-upload (new texture/view/bind group, pipeline reused) + `set_title` + stdout line; Esc and CloseRequested exit cleanly.
- [ ] Keep `show_rgba8` signature and behavior UNTOUCHED (single-file path reuses it verbatim).
- [ ] No new GPU tests (playtest-verified per project convention); keep `cargo test` green.
- [ ] Verify: `cargo test` passes; `git status --short` whitelist check.

## Task 3 — `main.rs`: file-vs-dir dispatch + playtest evidence + README

- [ ] Dispatch on the CLI arg: existing file → slice-3 single-file arm UNCHANGED (die-on-first-error); existing directory → gallery arm (`collect_dds_entries` → `show_gallery`, gallery errors → stderr + `ExitCode(1)`); neither → loud error + nonzero exit. Usage message documents both forms.
- [ ] Update `README.md` "What works" for the gallery (keys, title format, skip policy, single-file unchanged).
- [ ] Playtest (acceptance, user): `cargo run -- <real TEX dir>` — browse ≥10 textures across both directions, confirm title/console agreement, note at least one skip line if a bad file is hit (or record "no skips observed"); `cargo run -- <one real DDS>` confirms single-file unchanged. Record dir (install-relative only) + browsed range + seen result + date in the commit/PR body per archive-report convention.
- [ ] Legal: `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes; no game bytes/paths committed.
- [ ] Verify: `cargo test` passes; playtest recorded; `git status --short` clean of non-whitelisted files.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~240-320 (Task 1 ~90-120 incl. tests, Task 2 ~100-130, Task 3 ~50-70 incl. README; no generated files; `Cargo.lock` touched only if `tempfile` dev-dep is added — excluded from count, noted if present) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~320 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main (chain strategy stays deferred: chaining is not needed) |
| Work-unit mapping | Task 1 -> commit 1, Task 2 -> commit 2, Task 3 -> commit 3 + playtest evidence (tests travel with their behavior per work-unit-commits) |
| Rebase point for apply | Updated `main` at `45ec595` (archive of slices 2+3); this plan branch carries docs only, so apply branches fresh from `main` |

## Apply gate

- [ ] `cargo test` green, gallery + single-file playtest notes written (dir/file + what was seen), `git status --short` clean of non-whitelisted files, single PR ≤400 authored lines.

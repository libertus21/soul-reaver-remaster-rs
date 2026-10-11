# Tasks: dds-texture-viewer

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

## Task 1 — `dds.rs`: parse + decode + LOUD errors (with unit tests)

- [ ] Add `src/dds.rs`: `DecodedImage { width, height, rgba8 }`, `ViewerError { Io, NotDds, UnsupportedFormat(u32), Decode }`, `load_rgba8(path)`.
- [ ] Unit tests on synthetic bytes only (no game files): valid DDS magic accepted, bad magic -> `NotDds`, DX10 format extraction, unsupported format -> `UnsupportedFormat` naming the value.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — `render.rs`: wgpu fullscreen quad window

- [ ] Add `src/render.rs`: wgpu `State` (surface/adapter/device/queue/texture/pipeline), trivial WGSL fullscreen quad, `show_rgba8(width, height, pixels)`.
- [ ] Keep it stretch-to-window; no resize sophistication, no sRGB pass (deferred per design).
- [ ] Verify: `cargo test` passes (compiles; no GPU needed for the test run); `git status --short` whitelist check.

## Task 3 — `main.rs`: CLI wiring + event loop

- [ ] Rewrite `src/main.rs`: one CLI arg (DDS path), usage-to-stderr + nonzero exit when missing; map every `ViewerError` to a clear stderr message + `ExitCode(1)`; open window via `render`, close exits 0.
- [ ] Update `README.md` "What works / What's missing" for the slice.
- [ ] Verify: `cargo test` passes; `cargo run -- <real DDS>` shows the texture (playtest note: file + seen result recorded in the commit/PR body); `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~250-330 (dds ~120 + tests, render ~120, main ~40, README/proposal follow-ups ~30; no generated files) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~330 max vs 400 budget; `Cargo.lock` excluded from count if regenerated, noted if present) |
| Decision needed before apply | No — proceed as SINGLE PR to main |
| Work-unit mapping | Task 1 -> commit 1, Task 2 -> commit 2, Task 3 -> commit 3 (tests travel with their behavior per work-unit-commits) |

## Apply gate

- [ ] `cargo test` green, playtest note written (file + what was seen), `git status --short` clean of non-whitelisted files, single PR ≤400 authored lines.

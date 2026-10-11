# Tasks: bc7-texture-support

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

## Task 1 — `dds.rs`: BC7 header math + block slice + LOUD shape errors (with unit tests)

- [ ] Add consts (`WIDTH/HEIGHT/MIPCOUNT/DATA offsets`, `DXGI_BC7_UNORM = 98`, block size/dim), `CompressedBc7`, `LoadedTexture`, `load_bc7`, `load_texture` dispatcher, and `ViewerError::InvalidBc7(String)` + its `Display` arm. `load_rgba8` untouched.
- [ ] Unit tests on synthetic bytes only (no game files): 98 dispatches to `load_bc7` with the exact expected slice; w/h not multiples of 4 -> `InvalidBc7`; truncated payload -> `InvalidBc7` naming expected vs found; 99/bogus values still -> `UnsupportedFormat` naming the value; non-DX10 bytes still take the `load_rgba8` path.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — `render.rs`: compressed-texture upload path (`show_bc7`)

- [ ] Parameterize `State::new` with `format` + `required_features`; add the `TEXTURE_COMPRESSION_BC` adapter check (loud `Err` naming the feature when absent); make the pixel-length assert format-aware (`block_copy_size`/`block_dimensions` for BC7); add thin `show_bc7(width, height, blocks)`. WGSL shader, bind-group layout, and `show_rgba8` signature unchanged.
- [ ] No new tests needed beyond compilation (GPU path is playtest-verified per project convention); keep `cargo test` green.
- [ ] Verify: `cargo test` passes (compiles; no GPU needed); `git status --short` whitelist check.

## Task 3 — `main.rs`: dispatch BC7 vs RGBA8 + playtest evidence

- [ ] Switch `main` from `load_rgba8` to `load_texture`; match `LoadedTexture::{Rgba8, Bc7}` to `show_rgba8`/`show_bc7`; `InvalidBc7` flows through the existing stderr + `ExitCode(1)` arm. No new exit plumbing.
- [ ] Playtest: `cargo run -- <dxgiFormat-98 DDS>` (e.g. `1\TEX\0_0.DDS`) shows the texture matching the reference viewer; `cargo run -- <legacy DXT1 DDS>` renders as in slice 1 (no regression). Record file + seen result in the commit/PR body (install-relative filename only, per archive-report convention).
- [ ] Verify: `cargo test` passes; both playtests recorded; `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~250-320 (dds ~130 incl. tests, render ~110, main ~30, README follow-up ~20; no generated files; `Cargo.lock` excluded from count if regenerated, noted if present) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~320 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main |
| Work-unit mapping | Task 1 -> commit 1, Task 2 -> commit 2, Task 3 -> commit 3 (tests travel with their behavior per work-unit-commits) |

## Apply gate

- [ ] `cargo test` green, BC7 + DXT1 playtest notes written (file + what was seen), `git status --short` clean of non-whitelisted files, single PR ≤400 authored lines.

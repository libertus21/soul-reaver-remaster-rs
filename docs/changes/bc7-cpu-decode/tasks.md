# Tasks: bc7-cpu-decode

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test` (with `cargo build` warning-free as a co-gate per spec R7).

## Task 1 — `dds.rs`: CPU decode `decode_bc7_to_rgba8` + golden/structural unit tests + dead-wrapper removal

- [ ] Add `bcdec_rs = "=0.2.0"` to `Cargo.toml` (`Cargo.lock` updated via `cargo build`, committed).
- [ ] Add pure `pub fn decode_bc7_to_rgba8(c: &CompressedBc7) -> DecodedImage` (per-block `bcdec_rs::bc7` loop decoding straight into a `w*h*4` buffer with pitch `width*4`; design has the exact call shape).
- [ ] Delete dead wrappers `load_rgba8(path)` and `load_bc7(path)`; migrate their test call sites to `load_texture` (identical assertions) and delete the wrapper-only test `bc7_path_wrapper_loads_blocks`. No `#[allow(dead_code)]` anywhere.
- [ ] New unit tests on synthetic bytes only (no game files): structural (4x4 single-block + 8x8 multi-block → exact `w*h*4` bytes, dims preserved); golden (one fixed synthetic 16-byte block, bytes documented in-test, → recorded 64-byte expectation under the pinned decoder); invalid-block (all-zero block → opaque black per D3D11 rule).
- [ ] Verify: `cargo test` passes; `cargo build` shows no `warning:`; `git status --short` shows only intended files.

## Task 2 — `render.rs` + `main.rs`: supersede the native-upload path, route BC7 through `show_rgba8`

- [ ] `render.rs`: delete `show_bc7` and the `format`/`required_features` plumbing in `State::new`/`show` (back to the slice-1 RGBA8-only shape; the `TEXTURE_COMPRESSION_BC` check disappears with the only caller that needed it). WGSL shader and window behavior unchanged.
- [ ] `main.rs`: match `LoadedTexture::Bc7(c)` → `decode_bc7_to_rgba8(&c)` → `show_rgba8` (same call shape as the DXT1 arm); module docs updated (no more "natively-uploaded compressed texture" claim). `load_texture` dispatch and all error arms unchanged.
- [ ] No new tests (GPU path is playtest-verified per project convention); keep `cargo test` green.
- [ ] Verify: `cargo test` passes; `cargo build` shows no `warning:`; `git status --short` whitelist check.

## Task 3 — Playtest evidence (BC7 on THIS machine + DXT1 regression) + legal whitelist

- [ ] Playtest 1 (acceptance): `cargo run -- <dxgiFormat-98 DDS>` (e.g. `1\TEX\0_0.DDS`) on THIS machine opens the window; texture visibly matches the reference viewer. Record file (install-relative name only) + seen result + date in the commit/PR body per archive-report convention.
- [ ] Playtest 2 (no regression): `cargo run -- <legacy DXT1 DDS>` renders exactly as in slices 1–2; record same way.
- [ ] Legal: `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes; no game bytes/paths committed; decoder license already recorded in `design.md` (+ future `THIRD-PARTY-NOTICES.md` noted).
- [ ] Verify: `cargo test` passes; both playtests recorded; `git status --short` clean of non-whitelisted files.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~200-280 (dds ~120-150 incl. tests, render ~−90 net deletion, main ~±15, Cargo.toml + lock ~+5, README follow-up ~20; `Cargo.lock` regeneration excluded from the authored count, noted if present) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice (deletions don't add review load) |
| 400-line budget risk | Low (forecast ~280 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main |
| Work-unit mapping | Task 1 -> commit 1, Task 2 -> commit 2, Task 3 -> commit 3 + evidence (tests travel with their behavior per work-unit-commits) |
| Rebase point for apply | Updated `main` at `de8c044` (merge of PR #3, slice-2 native path); this plan branch carries docs only, so apply branches fresh from `main` |

## Apply gate

- [ ] `cargo test` green, `cargo build` zero warnings, BC7 + DXT1 playtest notes written (file + what was seen), `git status --short` clean of non-whitelisted files, single PR ≤400 authored lines.

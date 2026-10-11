# Design: bc7-cpu-decode

Same binary, same window, one fewer GPU code path: file bytes in, CPU-decoded RGBA8 out, pixels on screen through the twice-playtest-proven upload. The slice-2 header math is reused verbatim; the slice-2 native-upload path is deleted.

## Quick path

`main.rs` (`load_texture` dispatch, unchanged) -> `dds.rs` (`load_bc7_from_bytes` header math reused + NEW `decode_bc7_to_rgba8`: per-block `bcdec_rs::bc7` loop) -> `render.rs` (existing `show_rgba8`, the ONLY upload path left) -> pixels.

## Details

| Topic | Decision |
|---|---|
| Decoder crate | DECIDED: `bcdec_rs` **0.2.0**, exact pin (`bcdec_rs = "=0.2.0"`), `Cargo.lock` committed. Sources: [docs.rs crate page](https://docs.rs/crate/bcdec_rs/latest) (version 0.2.0, 2025-01-10, zero dependencies) and its [Cargo.toml.orig](https://docs.rs/crate/bcdec_rs/0.2.0/source/Cargo.toml.orig) (`license = "MIT"`, repo `ScanMountGoat/image_dds`). |
| Why this crate | Four converging reasons: (1) API fits the existing flow exactly — `pub fn bc7(compressed_block: &[u8], decompressed_block: &mut [u8], destination_pitch: usize)` decodes one 16-byte block to RGBA8 ([docs.rs `bc7`](https://docs.rs/bcdec_rs/latest/bcdec_rs/fn.bc7.html)), i.e. it consumes slice-2's `CompressedBc7.blocks` with no reshaping. (2) Zero dependencies, `no_std`, safe pure Rust — no C toolchain needed on Windows, minimal supply-chain surface. (3) Fuzzed against bindings to the original C `bcdec` for arbitrary input blocks with identical behavior (crate docs) — the closest thing to a correctness oracle without a GPU. (4) Already-established negative: `image` 0.25 decodes only BC1/BC2/BC3 (slices 1–2), so no existing dependency can do this job. No suitable-crate fallback is needed — one was found. |
| License compatibility | MIT (permissive). The project is a private rewrite that never redistributes game assets; the crate is only linked, never vendored or modified. MIT's sole redistribution condition (preserve the copyright notice) is satisfied by attribution: recorded here and to be added to a future `THIRD-PARTY-NOTICES.md` when that file is created. No copyleft, no asset encumbrance. |
| Version pinning | Exact pin `=0.2.0` (not caret): decode output must be bit-stable — the golden unit test (below) would silently change meaning under an auto-upgraded decoder. `Cargo.lock` is already committed and travels with the change. |
| `src/dds.rs`: new function | `pub fn decode_bc7_to_rgba8(c: &CompressedBc7) -> DecodedImage` — PURE (no I/O, no GPU): allocate `w*h*4` bytes; for each block `(bx, by)`, source offset `(by*blocks_wide+bx)*16`, destination origin `(bx*4, by*4)`, call `bcdec_rs::bc7(&blocks[o..o+16], &mut rgba[dst..], (width*4) as usize)` decoding directly into the output buffer with row pitch `width*4`. Sound because `load_bc7_from_bytes` guarantees w/h are nonzero multiples of 4, so every 4x4 tile fits exactly. |
| `src/dds.rs`: header math | UNCHANGED: `load_bc7_from_bytes` (offsets, `DXGI_BC7_UNORM = 98`, block consts, `InvalidBc7` validation) is reused verbatim — its unit tests stay green untouched and remain the shape gate. |
| Native-upload path | DECIDED: SUPERSEDE (remove), not keep-as-preferred. Justification — simplest correct wins: (1) it can never be verified on this hardware (the playtest that motivated this slice), and unverifiable GPU code is a liability; (2) one BC7 path halves the render surface vs a capability-checked dual path with zero visible difference in a single-quad viewer (native upload saves GPU memory nobody here is short of); (3) `show_rgba8` is proven by TWO playtests — routing BC7 through it minimizes new GPU code to exactly zero lines. Slice-2 code remains recoverable from git history (`main` at `de8c044`). Concretely: delete `show_bc7` + the `format`/`required_features` plumbing in `State::new`/`show` (render.rs returns to its slice-1 RGBA8-only shape); `main.rs` matches `LoadedTexture::Bc7(c)` → `decode_bc7_to_rgba8(&c)` → `show_rgba8`. |
| Mip strategy | CONFIRMED: base-mip-only, consistent with slices 1–2. The fullscreen quad samples mip 0 exclusively; lower mips stay on disk by construction. |
| Warnings cleanup | DECIDED: removal, no `#[allow]`. Delete the two now-confirmed-dead wrappers `load_rgba8(path)` and `load_bc7(path)` (both bypass the `load_texture` dispatcher that `main` actually uses); migrate their 4 test call sites to `load_texture` (same assertions — `NotDds`/`UnsupportedFormat`/`Io` behavior is dispatcher-identical) and delete the wrapper-only test `bc7_path_wrapper_loads_blocks`. `cargo build` must show no `warning:` — spec R7 gates on it. |
| Tests | Unit tests in `dds.rs` on SYNTHETIC bytes only: (1) structural — single-block (4x4) and multi-block (8x8) decodes yield exactly `w*h*4` bytes with dims preserved; (2) golden — one fixed synthetic 16-byte block (bytes documented in-test) decodes to a recorded 64-byte expectation (recorded during apply under the pinned decoder; the exact pin makes it a regression gate, NOT a correctness proof); (3) invalid-block — the all-zero block decodes to opaque black per the Direct3D 11 rule the crate implements (documented oracle behavior, stable under the pin). Pixel CORRECTNESS itself is proven by the user playtest (spec S1), honestly stated — unit tests guard shape + regressions. No GPU needed for `cargo test`. |
| Dependencies | ONE new: `bcdec_rs = "=0.2.0"`. `wgpu`/`winit`/`image`/`pollster` versions unchanged. |

## Deliberately NOT built

- Capability-checked dual path (native upload preferred, CPU fallback) — rejected above; full mip-chain upload + mip selection UI; `dxgiFormat` 99 / sRGB pass (follow-up); BC6H/float formats; encoder-side crates (`texpresso`, `rusty_dds` — decode-only need, rejected as oversized); `bevy_ecs`, audio, BIGFILE, gameplay.

## Checklist

- [ ] Reviewer can trace file-to-screen in one paragraph (table above) and name the single remaining upload function (`show_rgba8`).
- [ ] Reviewer can state why `bcdec_rs` (API fit + zero deps + fuzzed vs C oracle + MIT) and why exact-pinned, each in one sentence.
- [ ] Reviewer can state why the native path was removed rather than kept (unverifiable here, zero visible benefit, history-preserved).
- [ ] `cargo build` shows zero warnings without any `allow(dead_code)`; correctness proof is the playtest, regression guard is the pinned golden test — no circular claims.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

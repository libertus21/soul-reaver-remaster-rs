# Design: bc7-texture-support

Same binary, same two modules, one new data flow beside the old one: file bytes in, compressed blocks onto the GPU, pixels on screen. The slice-1 RGBA8 path is extended, never rewritten.

## Quick path

`main.rs` (dispatch on `dxgiFormat`) -> `dds.rs` (`load_bc7`: header math + block slice, or existing `load_rgba8`) -> `render.rs` (`show_bc7`: `Bc7RgbaUnorm` texture upload, or existing `show_rgba8`) -> pixels.

## Details

| Topic | Decision |
|---|---|
| `src/dds.rs`: new consts | `WIDTH_OFFSET = 16`, `HEIGHT_OFFSET = 12`, `MIPCOUNT_OFFSET = 28` (standard DDS header), `BC7_DATA_OFFSET = 148` (4 magic + 124 header + 20 DX10 ext), `DXGI_BC7_UNORM = 98`, `BC7_BLOCK_BYTES = 16`, `BC7_BLOCK_DIM = 4` |
| `src/dds.rs`: new types | `CompressedBc7 { width, height, blocks: Vec<u8> }` (base-mip blocks only); `enum LoadedTexture { Rgba8(DecodedImage), Bc7(CompressedBc7) }`; dispatcher `load_texture(path)` reads the file once, branches on `dxgi_format_of`: `Some(98)` -> `load_bc7`, else -> existing `load_rgba8` (so `UnsupportedFormat` behavior is preserved verbatim for everything else) |
| `src/dds.rs`: `load_bc7` | Validate magic (`NotDds` as today); read w/h as LE u32; require w,h > 0 and multiples of 4 (BC block requirement — wgpu `block_dimensions` is (4,4)), else `InvalidBc7`; `blocks = ((w+3)/4) * ((h+3)/4)`, `expected = 148 + blocks*16`; require file length >= expected, else `InvalidBc7`; copy `bytes[148 .. 148+blocks*16]` (for 512x512: 262,144 bytes). Mip count is parsed but only asserted >= 1; lower mips are skipped by construction |
| `src/dds.rs`: new error | ONE new variant: `ViewerError::InvalidBc7(String)` — message names the offending values (e.g. `invalid BC7 texture: expected 262292 bytes for 512x512 base mip, file has 100`). `Display` renders it; main maps it to stderr + `ExitCode(1)` like every other variant |
| Base-mip-only vs full chain | DECIDED: base mip only (`mip_level_count: 1`). Justification: the fullscreen quad samples mip 0 exclusively, so lower mips are dead weight in this slice; full-chain upload (6 `write_texture` calls + per-mip extents) buys nothing visible and belongs with the mipmap-selection UI non-goal. Size math above proves the base mip is self-sufficient |
| wgpu format variant | DECIDED: `TextureFormat::Bc7RgbaUnorm`. Three converging reasons: (1) the observed value 98 IS `DXGI_FORMAT_BC7_UNORM` (99 is the sRGB sibling, which stays loud per R5); picking Srgb would mislabel the file's declared transfer function. (2) Consistency: slice 1 uploads decoded bytes as `Rgba8Unorm` with no transfer function anywhere — UNORM keeps both paths visually comparable. (3) Scope: choosing Srgb would BE the deferred sRGB correctness pass through the back door. `Bc7RgbaUnormSrgb` is an explicit follow-up with that pass |
| `src/render.rs`: device | Check `adapter.features().contains(Features::TEXTURE_COMPRESSION_BC)` BEFORE `request_device`; if absent, return loud `Err` naming `TEXTURE_COMPRESSION_BC` (spec R7). If present, request the device with `required_features: Features::TEXTURE_COMPRESSION_BC` instead of `empty()`. (Source: `TextureFormat::Bc7RgbaUnorm` docs — "must be enabled to use this texture format", docs.rs `wgpu` 30.0.1, `TextureFormat` page, re-verified 2026-10-11) |
| `src/render.rs`: upload | `Device::create_texture` with `format: Bc7RgbaUnorm`, `mip_level_count: 1`, same `TEXTURE_BINDING \| COPY_DST` usage; `queue.write_texture` with `bytes_per_row: Some(blocks_wide * 16)` (= 2048 for 512-wide — a multiple of wgpu's 256-byte row-pitch alignment, so no padding path needed), `rows_per_image: Some(height)` (texel rows, matching the existing RGBA8 call shape), `size` = full w/h extent. (Source: `Queue::write_texture` signature — `data` must be in the texture's format — docs.rs `wgpu` 30.0.1, `Queue` page, re-verified 2026-10-11) |
| `src/render.rs`: sharing | `State::new` gains `format: TextureFormat` + `required_features: Features` params; the RGBA8 length assert becomes format-aware (`Rgba8Unorm` -> w*h*4 exact; `Bc7RgbaUnorm` -> `block_copy_size` x block count exact, computed via `format.block_copy_size`/`block_dimensions`, docs.rs `wgpu` 30.0.1). Bind-group layout (`texture_2d<f32>`, filterable float) and the WGSL shader are UNCHANGED — BC7 float formats sample as float. New thin wrapper `show_bc7(width, height, blocks)` beside `show_rgba8`; `show_rgba8` keeps its exact signature (no-regression R4) |
| `src/main.rs` | Replace the `load_rgba8` call with `load_texture`; `match` the enum: `Rgba8(img)` -> `show_rgba8` (identical call as today), `Bc7(c)` -> `show_bc7`. `InvalidBc7` Display output flows through the existing `eprintln!("error: {e}")` + `ExitCode(1)` arm — no new exit plumbing |
| Tests | Unit tests in `dds.rs` on SYNTHETIC bytes only (build a 148-byte header + N*16 fake blocks in-memory; no game files): 98 dispatches to `load_bc7` with correct slice; non-multiple-of-4 dims -> `InvalidBc7`; truncated payload -> `InvalidBc7` naming sizes; 99 (and bogus values) still -> `UnsupportedFormat(99)`; DXT1-shaped bytes still take the `load_rgba8` path. No GPU needed for `cargo test` |
| Dependencies | NONE new — `wgpu 30.0.1` already exposes `Bc7RgbaUnorm` + `TEXTURE_COMPRESSION_BC`; `image` stays for the legacy path only |

## Deliberately NOT built

- Full mip-chain upload + mip selection UI, `Bc7RgbaUnormSrgb` / `dxgiFormat` 99 support (follow-up with the sRGB pass), CPU-side BC7 decode (a `bc7`-family crate is NOT added), BC6H/float formats, 3D/sliced BC (`TEXTURE_COMPRESSION_BC_SLICED_3D` not requested), `bevy_ecs`, audio, BIGFILE, gameplay.

## Checklist

- [ ] Reviewer can trace file-to-screen for BOTH paths in one paragraph (table above).
- [ ] Reviewer can state why UNORM (not sRGB) and why base-mip-only, each in one sentence.
- [ ] Unsupported formats still have exactly one code path: loud error naming the value.
- [ ] `show_rgba8` signature and slice-1 behavior are byte-identical (regression impossible by construction).

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

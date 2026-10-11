# Design: dds-texture-viewer

One binary, two small modules, one data flow: file bytes in, pixels on screen. Nothing else is built.

## Quick path

`main.rs` (args + window loop) -> `dds.rs` (parse + decode to RGBA8) -> `render.rs` (wgpu upload + fullscreen quad) -> pixels.

## Details

| Topic | Decision |
|---|---|
| `src/main.rs` | CLI arg handling (usage + LOUD errors), winit event loop, wires `dds` decode output into `render`; `pollster` blocks on adapter/device init |
| `src/dds.rs` | NEW: `load_rgba8(path) -> Result<DecodedImage, ViewerError>`; validates `DDS ` magic, reads DX10 `dxgiFormat`, decodes base mip via `image`; `ViewerError` covers `Io`, `NotDds`, `UnsupportedFormat(u32)`, `Decode` — every variant renders a clear stderr message + nonzero exit |
| `src/render.rs` | NEW: owns wgpu `State` (surface, adapter, device, queue, texture, pipeline); `show_rgba8(width, height, pixels)` opens the winit window and draws a fullscreen textured quad with a trivial WGSL shader; sRGB subtleties deliberately deferred |
| Data flow | `read file -> validate magic -> parse DX10 header -> image-decode base mip -> RGBA8 buffer -> wgpu texture upload -> fullscreen quad -> present` |
| Dependencies | `winit 0.30.13`, `wgpu 30.0.1`, `image 0.25.10`, `pollster 1.0.1` (already pinned in `Cargo.toml`); no new crates in this slice |
| Error policy | No guessing, no silent fallback: any unknown format or bad input is `ViewerError` -> stderr + `ExitCode(1)`; panics only for programmer bugs, never for user input |
| Tests | Unit tests in `dds.rs` on synthetic header bytes (magic ok/bad, DX10 format extraction, unsupported-format error); no game files in tests; visual correctness left to the playtest note per AGENTS.md (strict TDD false for slice 1) |

## Deliberately NOT built

- BIGFILE/TEXTURES.BIG parsing, HD gallery textures, mipmap selection UI, sRGB correctness pass, resize handling beyond stretched quad, `bevy_ecs`, audio, maps, gameplay, Ghidra-derived structs.
- Cargo feature trimming (`default-features = false`) — deferred trim noted in `03-dds-viewer-direction.md`.

## Checklist

- [ ] Reviewer can trace file-to-screen in one paragraph (table above).
- [ ] Reviewer can state why each module exists and what would break if removed.
- [ ] Unsupported formats have exactly one code path: loud error.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

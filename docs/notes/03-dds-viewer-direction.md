# DDS viewer direction (decided 2026-10-11, not yet implemented)

## Decision

Slice `dds-texture-viewer` will use **wgpu + winit + image (+ pollster)**
directly. No Bevy for this slice. Current `Cargo.toml` pins:
`winit 0.30`, `wgpu 30`, `image 0.25` (includes `dds` support),
`pollster 1` (for blocking adapter/device init in a tiny binary).

## Tradeoff

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Bevy (full engine) | Least code to open a window + sprite; matches IW4L/benilla references; `bevy_ecs` reusable later | Huge compile graph for "show one texture"; inherits the ECS-perf trap Guide 03 warns about; overkill for slice 1 | Rejected for slice 1; revisit when game logic needs an ECS |
| wgpu + winit + image | Minimal dependency surface for the job; wgpu is the same renderer IW4L uses (incl. D3D9→WGSL path for later); forces us to parse the real DDS/DX10 header ourselves (good discipline) | More boilerplate (surface, adapter, device, texture upload, render pass) | **Chosen**: smallest thing that proves real game bytes reach the screen |
| Lighter blits (pixels/minifb/eframe) | Even less code | Extra abstraction to remove later; wgpu knowledge transfers to the engine slice | Fallback only if wgpu init fails on the user's machine |

## Scope guard for the slice (for the upcoming proposal/spec)

- Read ONE real `.DDS` from the user's install at RUNTIME via a CLI path
  argument (default: `1\TEX\0_0.DDS`-style file the user points at).
  Never copy the texture into the repo; never hardcode absolute game
  paths in committed code (use args/env + documented example only).
- Parse the DDS header + DX10 extended header with `image`/`ddsfile`
  semantics; support the observed BC-compressed DX10 variant first.
  Unknown `dxgiFormat` values must error loudly, not guess.
- Open a winit window, upload to a wgpu texture, draw a fullscreen quad.
  Success = the texture is visibly correct next to the same file in any
  standard DDS viewer.
- Follow-ups (NOT this slice): `.BIG` index reading, gallery HD textures,
  mipmaps, sRGB correctness pass, `bevy_ecs` introduction.

## Open trims (recorded, not done)

- `image` default features pull avif/exr/webp etc. A later commit may set
  `default-features = false, features = ["dds", "png"]` to shrink the
  graph; kept default for bootstrap so the skeleton compiles untouched.

# soul-reaver-remaster-rs

Unofficial fan-project Rust rewrite of Legacy of Kain: Soul Reaver 1&2
Remastered (2024, Aspyr). Not affiliated with or endorsed by Aspyr or
Crystal Dynamics. Written with AI coding-agent assistance.

The engine reads data from the player's OWN installed copy of the game
at runtime. This repo contains only our code — never game assets,
extracted files, decompiled code, or database files.

## Quick path

1. Install the game (GOG/Steam build) and verify it runs.
2. `cargo test` — runs the suite (DDS header/error unit tests on
   synthetic bytes; no game files in tests).
3. `cargo run -- <path-to-a-DDS>` — opens a window showing that
   texture (pass your own install path at runtime; never committed).

## Details

| Topic | Decision |
|---|---|
| Stack | Rust (cargo 1.99.0), wgpu + winit + image for slice 1; `bevy_ecs` later if game logic needs it (see `docs/notes/03-dds-viewer-direction.md`) |
| Game data | Read-only at runtime from the user's install; paths via CLI args/env only, never committed |
| Legal guardrails | Whitelist `.gitignore` (verified every commit); single-player/offline owned copy only; credit community work in `THIRD-PARTY-NOTICES.md` once reuse starts |
| Process | Small chained slices (≤400 lines each); proposal → spec → tasks → code → playtest report |

## Checklist

- [x] Slice 1 `dds-texture-viewer`: load one real `.DDS` from the install, show it in a window
- [ ] `THIRD-PARTY-NOTICES.md` created on first community-code reuse
- [ ] README "What works" updated every slice

## What works

- `dds-texture-viewer`: `cargo run -- <path-to-a-DDS>` decodes one
  `.DDS` (DX10 extended header via the `image` crate, base mip to
  RGBA8) and shows it as a stretched fullscreen quad in a winit/wgpu
  window. Bad magic, unsupported `dxgiFormat`, and unreadable paths
  fail loud (stderr + nonzero exit, no guessing).
- `bc7-texture-support`: `cargo run -- <path-to-a-BC7-DDS>`
  (`dxgiFormat` 98) uploads the raw base-mip blocks to a
  `Bc7RgbaUnorm` wgpu texture (no CPU decode) and shows them in the
  same window. Malformed BC7 input fails loud via `InvalidBc7`; any
  other undecodable `dxgiFormat` (including 99) still fails loud via
  `UnsupportedFormat`; a GPU without `TEXTURE_COMPRESSION_BC` fails
  loud naming the feature.

## What's missing

- [ ] BC7 (`dxgiFormat` 98) textures: currently fail loud with
      `UnsupportedFormat(98)` — planned as slice 2 `bc7-texture-support`.
- [ ] Visual playtest confirmation against a reference DDS viewer
      (done by the user per slice; see the change PR).
- [ ] Everything after that (BIGFILE index, models, audio, gameplay) — unscoped

## Next step

Read `AGENTS.md`, then `docs/notes/` (survey + community tools + viewer
direction). The next change is the proposal/spec for `dds-texture-viewer`.

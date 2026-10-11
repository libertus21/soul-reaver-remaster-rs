# soul-reaver-remaster-rs

Unofficial fan-project Rust rewrite of Legacy of Kain: Soul Reaver 1&2
Remastered (2024, Aspyr). Not affiliated with or endorsed by Aspyr or
Crystal Dynamics. Written with AI coding-agent assistance.

The engine reads data from the player's OWN installed copy of the game
at runtime. This repo contains only our code — never game assets,
extracted files, decompiled code, or database files.

## Quick path

1. Install the game (GOG/Steam build) and verify it runs.
2. `cargo test` — runs the suite (empty at bootstrap).
3. `cargo run -- <path-to-a-DDS>` — planned DDS viewer entry point
   (not implemented yet; see "What works / What's missing").

## Details

| Topic | Decision |
|---|---|
| Stack | Rust (cargo 1.99.0), wgpu + winit + image for slice 1; `bevy_ecs` later if game logic needs it (see `docs/notes/03-dds-viewer-direction.md`) |
| Game data | Read-only at runtime from the user's install; paths via CLI args/env only, never committed |
| Legal guardrails | Whitelist `.gitignore` (verified every commit); single-player/offline owned copy only; credit community work in `THIRD-PARTY-NOTICES.md` once reuse starts |
| Process | Small chained slices (≤400 lines each); proposal → spec → tasks → code → playtest report |

## Checklist

- [ ] Slice 1 `dds-texture-viewer`: load one real `.DDS` from the install, show it in a window
- [ ] `THIRD-PARTY-NOTICES.md` created on first community-code reuse
- [ ] README "What works" updated every slice

## What works

- (nothing yet — bootstrap only)

## What's missing

- [ ] `dds-texture-viewer`: parse DDS/DX10 header, upload to wgpu, render in a winit window
- [ ] Everything after that (BIGFILE index, models, audio, gameplay) — unscoped

## Next step

Read `AGENTS.md`, then `docs/notes/` (survey + community tools + viewer
direction). The next change is the proposal/spec for `dds-texture-viewer`.

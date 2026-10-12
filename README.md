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
- [x] `THIRD-PARTY-NOTICES.md` created on first community-code reuse
      (`bcdec_rs`, slice `bc7-cpu-decode`)
- [ ] README "What works" updated every slice

## What works

- `dds-texture-viewer`: `cargo run -- <path-to-a-DDS>` decodes one
  `.DDS` (DX10 extended header via the `image` crate, base mip to
  RGBA8) and shows it as a stretched fullscreen quad in a winit/wgpu
  window. Bad magic, unsupported `dxgiFormat`, and unreadable paths
  fail loud (stderr + nonzero exit, no guessing).
- `bc7-cpu-decode`: BC7 textures via CPU decode (playtest-accepted):
  `cargo run -- <path-to-a-BC7-DDS>` (`dxgiFormat` 98) CPU-decodes the base-mip blocks to RGBA8 (pinned
  `bcdec_rs` 0.2.0, MIT — see `THIRD-PARTY-NOTICES.md`) and shows them
  through the same `Rgba8Unorm` upload as slice 1, so it works on GPUs
  WITHOUT `TEXTURE_COMPRESSION_BC`. Malformed BC7 input fails loud via
  `InvalidBc7`; any other undecodable `dxgiFormat` (including 99) still
  fails loud via `UnsupportedFormat`. Supersedes the slice-2 native
  BC7 upload (kept in git history), which this machine's adapter cannot
  run.
- `texture-gallery`: `cargo run -- <directory>` browses that
  directory's direct-child `*.DDS` files (case-insensitive extension,
  sorted ascending by file name, no recursion) one at a time in the
  existing window through the existing decode + upload path (lazy
  decode, no pre-scan). ArrowRight steps forward, ArrowLeft steps back
  (clamped at the ends, key auto-repeat ignored); the window title and
  one console line both show `<index>/<total> <filename>` (1-based;
  `index/total` counts list entries). Esc or window close exits 0.
  Gallery skip policy: an undecodable entry prints
  `warning: skipping <filename>: <error>` to stderr and browsing
  continues; an empty or all-undecodable directory exits 1 with no
  window. Single-file mode is unchanged.

## What's missing

- [ ] Texture gallery (NEXT milestone): browse multiple textures instead
      of one-file-per-run.
- [ ] sRGB correctness pass, resize handling, full mipmap-chain UI —
      still deferred; `dxgiFormat` 99 (BC7 sRGB) still fails loud.
- [ ] Visual playtest confirmation against a reference DDS viewer
      (done by the user per slice; see the change PR).
- [ ] Everything after that (BIGFILE index, models, audio, gameplay) — unscoped

## Next step

Read `AGENTS.md`, then `docs/notes/` (survey + community tools + viewer
direction). The next change is the proposal/spec for `dds-texture-viewer`.

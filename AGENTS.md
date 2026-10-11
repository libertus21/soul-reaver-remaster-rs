# AGENTS.md — soul-reaver-remaster-rs dev journal

All technical artifacts in this repo are in English. No game bytes, no
invented structs: if a format detail is unsure, read the real file or
mark it UNKNOWN in `docs/notes/`.

## Hard rules (every session)

1. GAME DATA IS READ-ONLY: `D:\Juego\planescape\Soul Reaver I-II`
   (game 1: `...\1\`). Never write there, never copy game bytes here.
2. Whitelist `.gitignore` stays: only `*.rs`, `Cargo.toml`, `Cargo.lock`,
   `*.md`, `.gitignore`, `.gitattributes` may be committed. Run
   `git status --short` before every commit and abort on surprises.
3. Never commit: `*.dds`, `*.big`, `*.dat`, `*.ogg`, `*.ogv`, extracted
   files, decompiled code, Ghidra DBs, retail offsets/addresses.
4. Ghidra is DEFERRED until community formats/tools are exhausted.
5. Conventional Commits, no AI attribution trailers.
6. Keep `README.md` "What works / What's missing" current per slice.
7. Delivery: force-chained slices, ≤400 changed lines each.
8. Test command: `cargo test`. Slices that touch rendering also need a
   manual playtest note (what file, what was seen) in the commit/PR body.

## Stack

Rust 2024 edition, cargo 1.99.0. Slice 1: `winit 0.30` + `wgpu 30` +
`image 0.25` (dds) + `pollster 1`. Strict TDD: false for slice 1 —
the acceptance is visual (texture on screen matches a reference viewer),
backed by header-parse unit tests where feasible.

## Session log

- 2026-10-11 — SDD init + exploration (bootstrap): guides read, install
  surveyed read-only, community tools tabled, wgpu-over-Bevy decided,
  skeleton committed. Next: proposal/spec for `dds-texture-viewer`.

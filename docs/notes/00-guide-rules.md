# Guide rules affecting this project

Source: https://github.com/trevaintdead/ai-game-modding-guides
(`guides/00-start-here.md`, `guides/03-rust-rewrites-and-ports.md`,
`guides/06-rules-legal-and-publishing.md`, read 2026-10-11).
Full guides are reference material; only the rules that bind THIS project
are recorded here. This file is a dev-journal note, not code.

## Roles (00)

- The agent writes the code; the user playtests and reports back.
- The agent cannot see or feel the game, so every slice ends with a
  user playtest report before the next slice starts.
- Work in small slices: recon, one working slice, playtest, commit, repeat.
  "A finished thing that does almost nothing teaches you more than an
  ambitious thing you abandon on day two."

## Rust rewrite shape (03)

- The new engine reads models/textures/maps/sounds from the player's OWN
  installed copy at runtime. The repo contains only our code.
- Study order before decompiling: (1) community format docs/tools first,
  (2) source releases if any, (3) decompiling the executable only as a
  last resort. Ghidra is DEFERRED for this project until community
  formats/tools are exhausted (user decision).
- If unsure of a struct/offset/format, read the real file or mark it
  UNKNOWN. Never invent plausible layouts; a made-up offset compiles,
  runs, and is wrong in ways that cost hours.
- Keep a "what works / what's missing" list in the README and update it
  per slice. Keep a dev log (`AGENTS.md` / `docs/notes/`) so a fresh
  session can recover context.
- Reference structures: IW4L, gang-beasts-rust, benilla (README shape,
  extractor-or-runtime-read pattern, whitelist `.gitignore`).

## Rendering stack note (03)

- Bevy is the common choice (all-code engine, good Rust compiler errors)
  but projects report hard-to-fix frame-rate trouble when everything goes
  through the ECS system graph. The documented escape is: keep `bevy_ecs`
  for game logic, swap the renderer/app shell for something else.
- IW4L uses wgpu for rendering on top of Bevy and translates original
  Direct3D 9 shader bytecode to WGSL.
- Decision for slice 1 (dds-texture-viewer) is recorded in
  `03-dds-viewer-direction.md`: wgpu + winit directly, no Bevy yet.

## Legal rules (06) — mandatory

- Single-player/offline game the user owns only. No DRM, anti-cheat, or
  online work. This title has no anti-cheat; nothing here changes that.
- NEVER commit game assets, extracted files, decompiled code, or Ghidra
  DBs. Players supply their own copies.
- Whitelist `.gitignore` from day one (ignore `*`, re-allow only our
  source/manifests/docs). Verified with `git status` before every commit.
- Prefer observation (black-box: files on disk, headers, GPU captures)
  over decompilation; observation involves no copying.
- Do NOT ship decompiler output: rename `FUN_`/`sub_` names, keep retail
  addresses/offsets out of the repo. Identical bugs/dead code are treated
  as evidence of copying — reimplement, do not clone quirks.
- Credit every community project used or learned from; keep licenses in
  `THIRD-PARTY-NOTICES.md` once reuse starts.
- State in the README: unofficial fan project, no affiliation, AI-assisted.
- If a rights holder asks for removal/change: comply.

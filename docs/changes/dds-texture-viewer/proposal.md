# Proposal: dds-texture-viewer (slice 1)

Render ONE real game texture on screen. This is the smallest end-to-end proof that our engine can read the player's install and put real game bytes on the GPU.

## Quick path

1. `cargo run -- <path-to-a-DDS>` opens a window showing that texture.
2. User compares it by eye against any standard DDS viewer — match means success.
3. Verified with `cargo test`, a playtest note, and a `git status` whitelist check.

## Details

| Topic | Decision |
|---|---|
| Intent | Prove runtime-read of the owned install + wgpu render path with a single loose `TEX/*.DDS` file |
| Stack | `winit 0.30` + `wgpu 30` + `image 0.25` (DDS) + `pollster 1`, per `docs/notes/03-dds-viewer-direction.md`; no Bevy in this slice |
| Input contract | DDS path via CLI arg (or env) at runtime only; absolute game paths never committed; no bytes copied into the repo |
| First format target | Observed DX10-extended DDS (`dxgiFormat` 98, 512x512, 6 mipmaps — see spec); decode via `image`, upload to wgpu, fullscreen quad |
| Failure policy | LOUD: clear stderr error + nonzero exit on non-DDS input or unsupported `dxgiFormat`; never guess a format |
| Non-goals | No BIGFILE/archive parsing, no audio, no models/maps/gameplay, no mipmaps UI, no sRGB correctness pass, no `bevy_ecs`, no Ghidra |
| First-slice boundary | One texture, one window, close button; everything else is a follow-up slice |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (one DDS on screen).
- [ ] Reviewer can state what is out of scope (BIGFILE, audio, gameplay).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed.

## Next step

Read `spec.md` for acceptance scenarios, then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).

# Proposal: bc7-cpu-decode (slice 3)

Decode BC7 (`dxgiFormat` 98) blocks on the CPU to RGBA8 and render through the proven slice-1 path — so files like `1\TEX\0_0.DDS` display on ANY GPU, including this machine's adapter, which provably lacks `TEXTURE_COMPRESSION_BC`.

## Quick path

1. `cargo run -- <BC7 DDS>` opens a window showing that texture (same window as slice 1, CPU-decoded RGBA8 upload — no compressed GPU format involved).
2. User compares it by eye against a reference DDS viewer — match means success.
3. Verified with `cargo test`, `cargo build` with zero warnings, a playtest note, and a `git status` whitelist check.

## Details

| Topic | Decision |
|---|---|
| Intent | Unblock the 7,318 BC7 game textures on hardware that can never run the slice-2 native-upload path: playtest 2026-10-11 proved this machine's adapter lacks `TEXTURE_COMPRESSION_BC`, a HARDWARE block, not a code bug (Engram `sdd/bc7-texture-support/apply-progress`, observation #110) |
| Approach | Reuse slice-2 header math (`load_bc7_from_bytes` → blocks + dims, unchanged), decode each 16-byte block to RGBA8 on the CPU with the `bcdec_rs` crate (MIT, pinned — see `design.md`), upload via the playtest-proven `show_rgba8` |
| Native-upload path | SUPERSEDED (removed, not kept): it is unverifiable on this hardware, doubles the GPU code surface, and buys nothing visible for a single-quad viewer; slice-2 code stays in git history at `de8c044` if a future slice needs it |
| Mip strategy | Base mip only, consistent with slices 1–2 (fullscreen quad samples mip 0 exclusively) |
| Failure policy | LOUD, unchanged: unknown formats still `UnsupportedFormat(value)`; malformed BC7 still `InvalidBc7`; stderr + nonzero exit, no guessing |
| Warnings cleanup | Included: the 2 `dead_code` warnings (`load_rgba8`/`load_bc7` unused wrappers, confirmed on `main` via `cargo build`) are REMOVED, not silenced — zero warnings is an acceptance gate |
| Non-goals | Full mipmap-chain UI, sRGB correctness pass, BC7-sRGB files (`dxgiFormat` 99 — stays loud), audio, BIGFILE, gameplay, Ghidra |
| First-slice boundary | One BC7 texture visible on THIS machine next to the still-working DXT1 path; everything else is a follow-up slice |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (BC7 DDS on screen via CPU decode, on a GPU without BC support).
- [ ] Reviewer can state what is out of scope (native-upload path — removed; mip UI, sRGB pass, format 99).
- [ ] Reviewer can state why CPU decode instead of fixing the native path (hardware block — no code change can add a missing GPU feature).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed, game paths via CLI args only, decoder license recorded in `design.md`.

## Next step

Read `spec.md` for acceptance scenarios, then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).

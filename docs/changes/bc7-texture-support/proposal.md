# Proposal: bc7-texture-support (slice 2)

Render BC7 (`dxgiFormat` 98) textures by uploading compressed blocks natively to the GPU — files like `1\TEX\0_0.DDS` display instead of failing loud with `UnsupportedFormat(98)`.

## Quick path

1. `cargo run -- <BC7 DDS>` opens a window showing that texture (same window as slice 1, new compressed upload path).
2. User compares it by eye against a reference DDS viewer — match means success.
3. Verified with `cargo test`, a playtest note, and a `git status` whitelist check.

## Details

| Topic | Decision |
|---|---|
| Intent | Unblock the 7,318 BC7 game textures (per archive-report context): slice 1 fails loud on them because `image` 0.25 decodes only BC1/BC2/BC3 |
| Approach | Parse the DDS header ourselves (dims + base-mip size), slice the raw BC7 blocks out of the file, upload them to a `Bc7RgbaUnorm` wgpu texture with `TEXTURE_COMPRESSION_BC` — no CPU decode, no new crates |
| Format variant | `Bc7RgbaUnorm` (UNORM, not sRGB) — the observed value 98 is `BC7_UNORM`; matches slice 1's transfer-function-free `Rgba8Unorm` path; sRGB pass stays a non-goal (design answers this fully) |
| Mip strategy | Base mip only (`mip_level_count: 1`); lower mips are never sampled by the fullscreen quad; full chain deferred with the mipmap UI non-goal |
| Failure policy | LOUD, unchanged: every other unknown/unsupported format still fails with `UnsupportedFormat(value)` + nonzero exit; new `InvalidBc7` variant only for malformed BC7 shape (bad dims, truncated payload) |
| Non-goals | Full mipmap-chain UI, sRGB correctness pass, BC7 sRGB files (`dxgiFormat` 99 — stays loud, follow-up), CPU-side BC7 decode, audio, BIGFILE, gameplay, Ghidra |
| First-slice boundary | One BC7 texture on screen next to the still-working DXT1 path; everything else is a follow-up slice |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (BC7 DDS on screen via native GPU upload).
- [ ] Reviewer can state what is out of scope (mip UI, sRGB pass, format 99, CPU decode).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed, game paths via CLI args only.

## Next step

Read `spec.md` for acceptance scenarios, then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).

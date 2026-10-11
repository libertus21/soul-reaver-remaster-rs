# Archive: bc7-cpu-decode (slice 3) — DONE

Slice 3 is complete and playtest-accepted: BC7 (`dxgiFormat` 98) blocks decode on the CPU to RGBA8 (pinned `bcdec_rs` 0.2.0, MIT) and render through the proven `show_rgba8` path, so BC7 textures display on ANY GPU — including this machine's adapter, which provably lacks `TEXTURE_COMPRESSION_BC`. PR #4 is merged into `main`; feature branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `fbaf430` (merge of PR #4).
2. `cargo test` — CPU-decode structural/golden unit tests on synthetic bytes, green; `cargo build` shows zero warnings.
3. `cargo run -- <path-to-a-BC7-DDS>` (your own install path at runtime) — window shows the decoded texture.

## Playtest evidence

| Item | Value |
|---|---|
| File | `1000_0.DDS` (filename + install-relative dir only; absolute path is runtime-only, never committed) |
| Observed | Renders detailed BC7 stone door/panel with runes (matches reference expectation of real content) |
| File | `FONT0.DDS` (filename + install-relative dir only) |
| Observed | Renders full font atlas incl. L1/L2/R1/R2 glyphs |
| File (regression) | `ALUKABSS_13_E.DDS` DXT1 flame unchanged |
| Observed (regression) | DXT1 path renders exactly as in slices 1–2 — no regression |
| Earlier | `0_0.DDS` white screen proven NON-BUG (flat placeholder texture, RGB 248) |
| Date | 2026-10-11 |
| Verdict | User-accepted — CPU BC7 decode visually proven |

## Details

| Topic | Outcome |
|---|---|
| PR | [#4 — feat(bc7): decode BC7 on the CPU, render via RGBA8 path](https://github.com/libertus21/soul-reaver-remaster-rs/pull/4), state MERGED (`mergedAt` 2026-10-11T04:20:26Z), base `main`, head `feature/bc7-cpu-decode` |
| Commits on `main` | `44a5401` planning docs (proposal/spec/design/tasks) · `eca3cd0` feat(dds) CPU BC7 decode with pinned `bcdec_rs` · `f0555d5` refactor(render) supersede native BC7 upload, route BC7 through `show_rgba8` · `9be593c` docs(legal) `bcdec_rs` attribution and current BC7 behavior · `d6f2e17` planning-docs merge · `fbaf430` PR #4 merge |
| Branch cleanup | Remote `feature/bc7-cpu-decode` and `feature/bc7-cpu-decode-plan` verified fully contained in `main` (`git branch --contains`) and deleted from origin |
| Decoder | `bcdec_rs = "=0.2.0"` exact pin (bit-stable decode output; golden test is a regression gate), zero dependencies, MIT — attribution in `THIRD-PARTY-NOTICES.md` |
| Supersede | Slice-2 native-upload path (`show_bc7` + `format`/`required_features` plumbing) REMOVED, not kept: unverifiable on this hardware, zero visible benefit in a single-quad viewer; recoverable from history at `de8c044` |
| Zero warnings | The 2 `dead_code` warnings (`load_rgba8`/`load_bc7` unused wrappers) were REMOVED, not silenced — `cargo build` is warning-free as a spec gate |
| Link-env fix | `LNK1318` was stale PDBs, fixed by deleting `target\debug\deps\*.pdb` (build artifact, no repo move needed) |
| Deferred (unchanged) | Full mipmap-chain UI, sRGB correctness pass, `dxgiFormat` 99 (still loud), audio, BIGFILE, gameplay, Ghidra |

## Checklist

- [x] PR #4 MERGED and `main` contains all slice commits
- [x] Playtest notes recorded (files + seen results + date + user acceptance)
- [x] Remote feature branches deleted after containment check
- [x] README "What works" marks CPU BC7 decode done; gallery milestone queued under "What's missing"
- [x] No game bytes, absolute game paths, or offsets added by this archive

## Next step

Plan the texture-gallery milestone (chosen next): browse multiple textures instead of one-file-per-run. sRGB/resize/mipmap-chain stay deferred unless the gallery proposal pulls them in. Native-BC7 upload remains recoverable from history at `de8c044`.

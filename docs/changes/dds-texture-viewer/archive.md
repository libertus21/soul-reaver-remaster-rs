# Archive: dds-texture-viewer (slice 1) — DONE

Slice 1 is complete and playtest-accepted: `cargo run -- <path-to-a-DDS>` decodes one legacy DXT1 DDS to RGBA8 and shows it in a winit/wgpu window. PR #1 is merged into `main`; feature branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `7ab2d92` (merge of PR #1).
2. `cargo test` — DDS header/error unit tests on synthetic bytes, green.
3. `cargo run -- <path-to-a-DDS>` (your own install path at runtime) — window "Soul Reaver DDS viewer" shows the texture.

## Playtest evidence

| Item | Value |
|---|---|
| File | `1\TEX\ALUKABSS_13_E.DDS` (filename + install-relative dir only; absolute path is runtime-only, never committed) |
| Observed | Window titled "Soul Reaver DDS viewer" opened, showing the flame-like orange sprite on black |
| Date | 2026-10-11 |
| Verdict | User-accepted (visual match confirmed by the user) |

## Details

| Topic | Outcome |
|---|---|
| PR | [#1 — feat(dds-viewer): render one game texture in a wgpu window](https://github.com/libertus21/soul-reaver-remaster-rs/pull/1), state MERGED (`mergedAt` 2026-10-11T03:37:33Z), base `main`, head `feature/dds-texture-viewer` |
| Commits on `main` | `b77c07b` planning docs (proposal/spec/design/tasks) · `a57dcb1` feat(dds) parse-decode module + unit tests · `7e2cfd5` feat(render) wgpu fullscreen-quad window · `a8a724e` feat(cli) DDS-to-window wiring + loud CLI errors · `0570e26` fix(render) adapter limits for low-capability GPUs · `47dc364` planning-docs merge · `7ab2d92` PR #1 merge |
| Branch cleanup | Remote `feature/dds-texture-viewer` and `feature/dds-texture-viewer-plan` verified fully contained in `main` (`git branch --contains`) and deleted from origin |
| Scope kept | Legacy DXT1 path only; sRGB correctness and resize handling deferred by design (stretched quad) |
| Known limitation | The 7318 BC7 textures currently fail loud with `UnsupportedFormat(98)` — by design, per the error policy (stderr + nonzero exit, no guessing) |

## Checklist

- [x] PR #1 MERGED and `main` contains all slice commits
- [x] Playtest note recorded (file + seen result + date + user acceptance)
- [x] Remote feature branches deleted after containment check
- [x] README "What works" marks slice 1 done; BC7 stays under "What's missing"
- [x] No game bytes, absolute game paths, or offsets added by this archive

## Next step

Plan slice 2 `bc7-texture-support`: native BC7 upload path so the 7318 BC7 (`dxgiFormat` 98) textures render instead of failing loud. sRGB/resize stay deferred unless the slice-2 proposal pulls them in.

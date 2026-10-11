# Archive: bc7-texture-support (slice 2) — DONE (superseded by slice 3)

Slice 2 is complete and archived: BC7 (`dxgiFormat` 98) textures gained a native GPU upload path (`Bc7RgbaUnorm` + `TEXTURE_COMPRESSION_BC`), with the slice-1 DXT1 path untouched. PR #3 is merged into `main`. The native-upload path was later SUPERSEDED by slice 3 (`bc7-cpu-decode`), which removed it — the code stays recoverable from git history. Playtest verdict: DXT1 regression PASS; BC7 loud-failed exactly as designed on hardware without BC support.

## Quick path

1. `git checkout main && git pull` — HEAD is `fbaf430` (merge of PR #4; includes the PR #3 merge `de8c044`).
2. `cargo test` — BC7 header-math/block-slicing unit tests on synthetic bytes, green.
3. `cargo run -- <path-to-a-DDS>` (your own install path at runtime) — DXT1 renders as in slice 1; BC7 behavior is now owned by slice 3.

## Playtest evidence

| Item | Value |
|---|---|
| File (regression) | `ALUKABSS_13_E.DDS` (filename + install-relative dir only; absolute path is runtime-only, never committed) |
| Observed (regression) | DXT1 regression PASS visual — flame identical to slice 1 |
| File (BC7) | `0_0.DDS` (`dxgiFormat` 98) |
| Observed (BC7) | Hardware-blocked on this GPU: loud fail per spec R7 (`TEXTURE_COMPRESSION_BC` absent), correct behavior — NOT a code bug |
| Date | 2026-10-11 |
| Verdict | User-accepted (DXT1 unchanged; BC7 loud fail is the spec-R7 designed outcome on a BC-less adapter) |

## Details

| Topic | Outcome |
|---|---|
| PR | [#3 — feat(bc7): render BC7 textures via native GPU upload](https://github.com/libertus21/soul-reaver-remaster-rs/pull/3), state MERGED (`mergedAt` 2026-10-11T03:58:19Z), base `main`, head `feature/bc7-texture-support` |
| Commits on `main` | `51cd37c` planning docs (proposal/spec/design/tasks) · `4157d78` feat(dds) BC7 header math, block slicing, loud shape errors · `2f6486f` feat(render) compressed-texture upload path with BC7 entry point · `db521c6` feat(app) BC7-vs-RGBA8 dispatch with playtest docs · `ebd3776` planning-docs merge · `de8c044` PR #3 merge |
| Branch cleanup | Remote `feature/bc7-texture-support` and `feature/bc7-texture-support-plan` verified fully contained in `main` (`git branch --contains`) and deleted from origin |
| Scope kept | Base mip only (`mip_level_count: 1`); `Bc7RgbaUnorm` (UNORM, not sRGB — value 98 is `BC7_UNORM`); one new error variant `InvalidBc7`; unknown formats (incl. 99) still fail loud via `UnsupportedFormat` |
| Supersede note | Slice 3 removed the native-upload path (`show_bc7` + `format`/`required_features` plumbing) because this machine's adapter provably lacks `TEXTURE_COMPRESSION_BC` — unverifiable GPU code is a liability. Recoverable from history at `de8c044` if a future slice needs it |
| Link-env fix | `LNK1318` during this slice was stale PDBs, fixed by deleting `target\debug\deps\*.pdb` (build artifact, no repo move needed) |
| Deferred (unchanged) | Full mipmap-chain UI, sRGB correctness pass, `dxgiFormat` 99, CPU-side BC7 decode (became slice 3), audio, BIGFILE, gameplay, Ghidra |

## Checklist

- [x] PR #3 MERGED and `main` contains all slice commits
- [x] Playtest note recorded (file + seen result + date + user acceptance)
- [x] Remote feature branches deleted after containment check
- [x] No game bytes, absolute game paths, or offsets added by this archive
- [x] Supersede by slice 3 recorded with the history recovery point (`de8c044`)

## Next step

Slice 3 `bc7-cpu-decode` is also archived (see `docs/changes/bc7-cpu-decode/archive.md`). The chosen next milestone is the texture-gallery milestone; sRGB/resize/mipmap-chain stay deferred.

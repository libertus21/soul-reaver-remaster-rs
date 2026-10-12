# Archive: texture-gallery (slice 4) — DONE

Slice 4 is complete and playtest-accepted: `cargo run -- <directory>` browses that directory's direct-child `*.DDS` files one at a time in the existing window, stepping with ArrowRight / ArrowLeft (clamped, no wrap), with the window title and one console line both showing `i/n name`. PR #6 is merged into `main`; contained feature branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `eccd80a` (merge of PR #6).
2. `cargo test` — 23/23 green on synthetic temp dirs/files; `cargo build` shows zero warnings.
3. `cargo run -- <TEX-dir>` (your own install path at runtime) — window opens on the first decodable texture; arrows step, Esc exits.

## Playtest evidence

| Item | Value |
|---|---|
| Gallery browsing | PASS visual in real TEX dir (user-confirmed 2026-10-11/12): opens at 1/8066, arrows navigate both directions, title/console show `i/n name`, Esc exits. |
| Headless gates from verify | 23/23 tests, zero build warnings, empty-dir/no-args loud exit 1, real dir startup `1/8066 0_0.DDS`. |
| Process learning | Interrupted apply runs leave resumable partial state (Task-1 commit + dirty tree); resume-by-inspection worked on 3rd attempt. Users sometimes run cargo from the game dir — playtest instructions must start with cd-to-repo + branch check. |
| Known | Test-profile `dead_code` warnings don't count against the zero-warning build gate. |

## Details

| Topic | Outcome |
|---|---|
| PR | [#6 — feat(gallery): browse DDS directory with clamped arrow navigation](https://github.com/libertus21/soul-reaver-remaster-rs/pull/6), state MERGED (`mergedAt` 2026-10-12T00:57:41Z), base `main`, head `feature/texture-gallery` |
| Commits on `main` | `156af74` planning docs (proposal/spec/design/tasks) · `837be4d` Task 1 listing + clamped navigation with unit tests · `8ffb073` Task 2 gallery window with clamped arrow navigation and file-vs-dir dispatch · `4564c65` Task 3 gallery keys, title format, and skip-policy docs · `eccd80a` PR #6 merge |
| Branch cleanup | `feature/texture-gallery`: verified fully contained in `main` (`git merge-base --is-ancestor`), deleted locally and from origin. Slice-1 leftovers: `feature/dds-texture-viewer`, `feature/dds-texture-viewer-plan` deleted locally (no remote remained); `docs/archive-dds-viewer` verified contained, deleted locally and from origin. `feature/texture-gallery-plan` NOT deleted (local + remote kept): commit `9ab5c39` carries identical docs content but is not an ancestor of `main` (`merge-base --is-ancestor` fails), so it is not fully contained — deleting it would drop a distinct commit object. |
| Scope kept | Direct children only (no recursion), case-insensitive `.dds`, lexicographic file-name sort; lazy decode (no pre-scan, `index/total` counts list entries); skip-loud in gallery mode (`warning: skipping <filename>: <error>`), die-loud in single-file mode; arrows-only keys with auto-repeat ignored; window size follows first shown texture |
| Deferred (unchanged) | sRGB correctness pass, resize handling, full mipmap-chain UI, `dxgiFormat` 99 (still loud); audio, models, gameplay, Ghidra |
| Legal | No game bytes, absolute game paths, or offsets added by this archive; evidence uses relative filenames only |

## Checklist

- [x] PR #6 MERGED and `main` contains all slice commits
- [x] Playtest note recorded (dir + browsed range + seen result + date + user acceptance)
- [x] Headless gates recorded (23/23 tests, zero build warnings, loud-exit cases)
- [x] Contained branches deleted after containment check; uncontained plan branch explicitly kept with reason
- [x] README "What works" marks the gallery done; "What's missing" queues the next milestone
- [x] No game bytes, absolute game paths, or offsets added by this archive

## Next step

The chosen next milestone is the BIGFILE.DAT milestone: index and read textures from the archive instead of loose files. sRGB/resize/mipmap-chain stay deferred unless the BIGFILE proposal pulls them in.

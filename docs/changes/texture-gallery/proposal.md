# Proposal: texture-gallery (slice 4)

Turn the one-file viewer into a directory gallery: `cargo run -- <TEX-dir>` lists `*.DDS` sorted and shows one texture at a time in the existing window, stepping with the keyboard. This is the smallest change that makes the 8,066 loose `TEX/*.DDS` files browsable without leaving the proven decode + render path.

## Quick path

1. `cargo run -- <TEX-dir>` opens the existing window on the first decodable texture.
2. ArrowRight / ArrowLeft step forward / back through the sorted list; the window title and a console line show `index/total + filename`.
3. Verified with `cargo test` (synthetic temp dirs/files only), a user playtest browsing the real `TEX` dir, and a `git status` whitelist check.

## Details

| Topic | Decision |
|---|---|
| Intent | Make the viewer's existing decode (`image` + CPU BC7) and render (`show_rgba8`) path reusable across a whole directory, so the user can review game textures one by one instead of relaunching per file |
| Input contract | The gallery directory arrives via CLI arg at runtime only; absolute game paths never committed; no bytes copied into the repo (same legal rule as slices 1–3) |
| Navigation scheme | ArrowRight / ArrowLeft ONLY (no N/P alternative — design justifies this; one scheme keeps the event handling and the playtest to a single path) |
| Failure policy | Gallery mode NEVER dies on a bad file: undecodable entries are skipped with a loud stderr line and browsing continues (this supersedes die-on-first-error for gallery mode). Single-file mode keeps its current die-on-first-error behavior verbatim — design states both |
| Non-goals | Thumbnails grid, search/filter, recursive subdirectories, mipmap selection, sRGB correctness pass, BIGFILE support, audio, gameplay, Ghidra |
| Slice boundary | One window, one texture at a time, keyboard stepping, loud skips; everything else is a follow-up slice |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (browse a directory of DDS files one at a time in the existing window).
- [ ] Reviewer can state what is out of scope (grid, search, recursion, mips, sRGB, BIGFILE).
- [ ] Reviewer can state the two failure policies and which mode each belongs to (skip-loud in gallery, die-loud in single-file).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed, game paths via CLI args only.

## Next step

Read `spec.md` for acceptance scenarios, then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).

# Spec: texture-gallery

`cargo run -- <dir>` opens the existing winit window showing the first decodable `*.DDS` in that directory (sorted by file name); ArrowRight / ArrowLeft step through the list, the window title and a console line show `index/total + filename`, and Esc or window close exits 0. `cargo run -- <file>` keeps the exact slice-3 single-file behavior.

## Quick path

1. Run `cargo run -- "<install>\1\TEX"` (any directory; path is runtime-only, never committed).
2. The window opens on the first decodable texture; ArrowRight steps forward, ArrowLeft steps back.
3. Compare several textures by eye against a reference DDS viewer — they match (user playtest note records dir + browsed range + result).

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI takes exactly one argument. A missing argument prints usage to stderr and exits nonzero (unchanged). An argument that is neither an existing file nor an existing directory is a LOUD error (message + nonzero exit) |
| R2 | Directory mode: collect direct-child entries whose extension is `.dds` (case-insensitive), files only, sorted ascending by file name (lexicographic on the file name; natural-number ordering is a non-goal). Subdirectories are never traversed |
| R3 | Navigation shows exactly one texture at a time through the EXISTING decode path (`load_texture` → CPU BC7 or `image` RGBA8) and the EXISTING upload (`show_rgba8`-family). No new decoder, no new GPU format |
| R4 | Navigation keys are ArrowRight (next) and ArrowLeft (previous) ONLY. At the ends the index clamps (no wrap): pressing past the last/first entry stays put. Key auto-repeat is ignored so one press is exactly one step |
| R5 | On every shown texture, the window title is set to `<index>/<total> <filename>` (1-based index) AND one console line prints the same string to stdout. Both carry the same content so playtest notes can quote either |
| R6 | Gallery skip policy (supersedes die-on-first-error IN GALLERY MODE ONLY): any entry that fails to load (unknown format, corrupt, unreadable) prints a loud stderr line naming the file and the error (`warning: skipping <filename>: <error>`) and browsing continues with the next decodable entry in the step direction |
| R7 | Single-file mode is UNCHANGED: `cargo run -- <file>` loads exactly that file and any failure is a loud stderr error + nonzero exit, as in slices 1–3 |
| R8 | Esc exits the gallery with code 0; window close (CloseRequested) exits 0 (unchanged). No game bytes, paths, or offsets committed; whitelist `.gitignore` verified per commit; tests use synthetic temp dirs/files only |

## Acceptance scenarios

- [ ] **Happy path (gallery):** `cargo run -- <real TEX dir>` opens the window on the first decodable texture; ArrowRight/ArrowLeft move one texture per press; title and console line agree (`3/8066 1000_1.DDS`-style) at every step.
- [ ] **Empty dir:** pointing at a directory containing zero `*.DDS` files prints a loud stderr error naming the directory and exits 1 with no window.
- [ ] **Mixed good/bad files:** a directory with decodable files plus corrupt/unknown-format files is browsable end to end; each bad file produces exactly one loud stderr skip line naming the file, and no bad file kills the session or opens a blank window.
- [ ] **All-bad dir:** a directory whose entries ALL fail to decode prints a loud stderr error and exits 1 with no window (same exit contract as the empty dir).
- [ ] **Esc:** pressing Esc at any point exits 0; closing the window exits 0.
- [ ] **Single-file unchanged:** `cargo run -- <one real DDS>` renders exactly as in slice 3, and `cargo run -- <corrupt file>` still exits nonzero with a loud error (no skip, no gallery).
- [ ] **Unit level:** `cargo test` covers listing (sorted order, extension filter, empty-dir error) and navigation clamping on synthetic temp dirs/files without touching game files.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; `*.dds` can never appear.

## Next step

See `design.md` for modules, the key-scheme justification, and the dual error policy; `tasks.md` for the ordered work units.

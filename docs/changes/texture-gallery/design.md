# Design: texture-gallery

Same binary, same decode, same upload: a directory is just a sorted list of paths fed one at a time through the slice-3 pipeline, with keyboard events swapping which decoded image the window shows. The only new logic is listing, index movement, and per-step texture replacement.

## Quick path

`main.rs` (file-vs-dir dispatch, unchanged single-file arm) -> NEW `collect_dds_entries` (sorted file list) -> `render.rs` (NEW `show_gallery`: `show_rgba8`-style window + ArrowRight/ArrowLeft/Esc handling + per-step re-upload and `set_title`) -> pixels.

## Details

| Topic | Decision |
|---|---|
| Listing (`src/dds.rs` or NEW `src/gallery.rs`) | `pub fn collect_dds_entries(dir) -> Result<Vec<PathBuf>, ViewerError>`: `read_dir`, keep entries whose extension equals `dds` case-insensitively AND whose file type is a file, sort ascending by file name (lexicographic; natural-number sort explicitly rejected — unpredictable labels like `1000_10` vs `1000_2` are acceptable in a debug browser). Empty result is a LOUD error (new `ViewerError` variant or a dedicated message — apply picks the smaller diff; spec R-scenarios gate the message + exit 1, not the type name) |
| Lazy decode (no pre-scan) | Entries decode ON DEMAND when first shown. A full pre-decode of ~8,066 files (each ~350 KB BC7, CPU-decoded) would stall startup for minutes; lazy keeps open time identical to slice 3. `index/total` therefore counts LIST entries, including not-yet-attempted ones — stated here so the playtest cannot misread it |
| Skip loop | Both initial show and every navigation step run: attempt load in the step direction, `eprintln!("warning: skipping {filename}: {e}")` per failure, continue. If the loop exhausts the list with zero successes, loud error + exit 1 (spec all-bad scenario). Skips never open a blank window: the currently shown texture stays until a decodable replacement exists |
| Key scheme: arrows ONLY | DECIDED: `Key::Named(NamedKey::ArrowRight)` / `ArrowLeft`, matched on `WindowEvent::KeyboardInput` with `state == Pressed` and `repeat == false`. Justification in three parts: (1) layout-independence — `NamedKey` is identical on QWERTY/AZERTY/etc., while N/P arrive as `Key::Character` and move on some layouts; (2) one scheme halves the event-matching code and the playtest matrix vs supporting both; (3) arrows are the file-browser convention and sit next to Esc-free navigation without consuming letter keys that a future search/filter slice will want. Esc is `Key::Named(NamedKey::Escape)` → `event_loop.exit()` (exit 0) |
| Clamp, no wrap | Pure helper `step_index(current, len, dir) -> usize` clamps at `0` / `len-1`. Wrap-around on an 8,066-file list would teleport the user across thousands of files on one over-press; clamping is predictable and unit-testable without a GPU |
| Texture swap | `State` gains a re-upload method: create a new `Rgba8Unorm` texture + view + bind group from the newly decoded RGBA8 bytes and replace the stored ones (device/queue/pipeline/sampler layout reused). Window size follows the FIRST shown texture (slice-1 behavior); later textures stretch exactly like slice 1 — per-texture resize is a non-goal |
| Title + console | After each successful show: `window.set_title(format!("{i}/{n} {name}"))` (1-based) plus `println!("{i}/{n} {name}")`. Title is the in-window proof, stdout is the quotable playtest record |
| Dual error policy | Gallery arm: skip-loud + continue (spec R6). Single-file arm: byte-for-byte the slice-3 path — `load_texture` error → stderr + `ExitCode(1)` (spec R7). The two arms share `load_texture`; ONLY the error handling differs, branched once in `main` |
| Tests | Unit tests on SYNTHETIC temp dirs/files only (new `tempfile` dev-dependency is ALLOWED — apply decides; it keeps tests hermetic with zero shipped code): listing order + extension filter (incl. `.DDS` uppercase) + empty-dir error; `step_index` clamping; skip-loop direction (good/bad/good synthetic trio lands on the right file). No GPU needed for `cargo test`; pixel correctness stays a user playtest per project convention |
| Dependencies | ZERO new shipped crates. `winit`/`wgpu`/`image`/`pollster`/`bcdec_rs` versions unchanged |

## Deliberately NOT built

- Thumbnails grid, search/filter UI (letter keys deliberately left free for this), recursive subdirectory walk, natural-number sorting, prefetch/decode-ahead thread, per-texture window resize, mipmap selection, sRGB pass, `dxgiFormat` 99, BIGFILE support, `bevy_ecs`, audio, gameplay.

## Checklist

- [ ] Reviewer can trace dir-to-screen in one paragraph (table above) and name the single shared decode entry point (`load_texture`).
- [ ] Reviewer can state why arrows-only in one sentence (layout-independent, one code path, reserves letters for search).
- [ ] Reviewer can state both error policies and which `main` arm each belongs to (skip-loud gallery, die-loud single-file).
- [ ] `index/total` counts list entries (not yet-decoded ones included) — no playtest surprise.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

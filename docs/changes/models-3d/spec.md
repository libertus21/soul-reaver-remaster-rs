# Spec: models-3d

`cargo run -- --mesh <file>` parses one SR1-remaster `.SRM` file (positions + triangle indices only), prints its counts, and shows it as a flat-shaded auto-rotating solid in a window; closing the window or pressing Esc exits 0. Corrupt input fails LOUD with exit 1. `cargo run -- <file>`, `cargo run -- <dir>`, both `--bigfile` forms, and both `--textures` forms keep their exact prior behavior.

## Quick path

1. Run `cargo run -- --mesh "<install>\1\OBJECT\ALKLENS_1.SRM"` (any `.SRM` path; runtime-only, never committed).
2. A window opens showing the mesh rotating (flat-shaded solid); Esc or window close exits 0.
3. User playtest records file + what was seen (recognizable solid, rotation direction, Esc exit code).

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI keeps its current forms UNCHANGED: one positional arg (file/dir), `--bigfile <path> --list/--show`, `--textures <path> --tlist/--tshow`, and the loud usage error. `--mesh <path>` is a NEW flag form taking exactly one existing `.SRM` file. Unknown/missing flag arguments are a LOUD error (usage + nonzero exit) |
| R2 | `--mesh` parses ONLY what the slice needs: magic check (`SRM\x01`), shader/material/bone headers consumed as opaque byte ranges per the documented layout, `u32` vertex/face counts with sanity limits, then positions (`3 x f32` LE per 32-byte vertex stride) and triangle indices (`u16` LE triplets, `faceCount / 3` triangles). Normals, tangents, UVs, weights, materials, and bone data are SKIPPED, never interpreted |
| R3 | `--mesh` prints one counts line to stdout (`N vertices, M triangles` — counts only, never coordinates) and renders the mesh. A file whose bytes do not match the layout (bad magic, truncated header, insane counts, short vertex/face data, trailing-shape mismatch) is a LOUD stderr error + exit 1 naming the file and the first failed check — no window, no partial mesh |
| R4 | Rendering uses a NEW mesh pipeline in `render.rs` reusing the existing window/event-loop concepts: vertex + index buffers, depth-tested solid triangles, fixed perspective camera, slow auto-rotation, uniform base color with derivative-based flat shading. `show_rgba8` / gallery behavior unchanged. Close/Esc exits 0 |
| R5 | Positions are auto-fit (center + uniform scale to a unit cube on the CPU at load): unit/scale/orientation of file coordinates is UNKNOWN, so the slice normalizes framing instead of assuming it. Rotation is automatic (no orbit input in this slice); Esc and window close are the only inputs |
| R6 | Backface culling is OFF for this slice (file winding is carried as observed from the community parser and visually confirmed at playtest, not trusted). Depth testing is ON (occlusion must look solid, not see-through) |
| R7 | Corrupt/truncated input fails LOUD with exit 1: every malformed shape prints a clear stderr message; no guessing, no partial render presented as success |
| R8 | No game bytes, paths, coordinates, hashes, offsets, or content dumps committed; whitelist `.gitignore` verified per commit; tests use SYNTHETIC bytes only (hand-built minimal `.SRM` buffers in temp dirs — the real files are never touched by tests) |

## Acceptance scenarios

- [ ] **Happy path (`--mesh`):** `--mesh` on the real playtest `.SRM` exits 0 path (window opens, counts line on stdout); user playtest confirms a rotating solid mesh on screen + Esc exits 0, and records file + observation + exit code.
- [ ] **Solid, not wireframe/blank:** the render shows filled triangles with visible faces (flat-shaded), not an empty window, not a point cloud, not see-through soup. Depth occlusion reads correctly by eye.
- [ ] **Corrupt/truncated:** pointing `--mesh` at a non-SRM file, a truncated `.SRM`, or a missing path prints a loud stderr error and exits 1; no window opens.
- [ ] **Insane counts:** a synthetic header claiming millions of vertices (or a vertex/face range past EOF) fails loud + exit 1, never a huge allocation or a panic.
- [ ] **Modes unchanged:** `cargo run -- <one real DDS>` renders exactly as before, `cargo run -- <real TEX dir>` browses exactly as before, and both `--bigfile` / `--textures` forms behave exactly as archived (no flag-form regressions).
- [ ] **Unit level:** `cargo test` covers magic accept/reject, header-skip arithmetic, counts sanity, position/index extraction on synthetic buffers, truncated-data rejection, and error-message rendering — without touching game files, without a GPU.
- [ ] **Legal:** `git status --short` shows only intended source/doc files; no extracted bytes, no game paths/coordinates/offsets/content dumps committed.

## Next step

See `design.md` for the probe ledger, the UNKNOWN gaps, and module decisions; `tasks.md` for the ordered work units.

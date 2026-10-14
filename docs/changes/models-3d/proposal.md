# Proposal: models-3d

GO: the SR1 remaster mesh format is identified and byte-exact on real files — loose `.SRM` files under `1\OBJECT\` parse end-to-end under the community-documented layout (magic `SRM\x01`, shader/material/bone headers, 32-byte vertices, `u16` triangles), with both probed files landing exactly on EOF. This slice parses positions + indices from ONE `.SRM` (no skeleton/UV/materials) and draws it as a flat-shaded auto-rotating solid in the existing wgpu window (`--mesh <path>`, Esc exits 0). Tests use synthetic mesh bytes only.

## Quick path

1. `cargo run -- --mesh <one-SRM-file>` prints the mesh counts and opens a window showing the mesh rotating (user playtest); corrupt input fails loud, exit 1.
2. Verified with `cargo test` (synthetic bytes only), a user playtest on a real `.SRM`, and a `git status` whitelist check.
3. Read `spec.md` for acceptance scenarios (with exit codes), then `design.md` and `tasks.md`. Apply follows with a SINGLE PR (forecast in `tasks.md`).

## Details

| Topic | Decision |
|---|---|
| Intent | Render the first real 3D geometry: prove the `.SRM` layout reads positions + triangles correctly by putting pixels on screen through our own parser and our own pipeline |
| Format verdict (planning probe, read-only) | GO — `.SRM` under `1\OBJECT\` matches the `io_scene_xrm` SRM layout byte-for-byte: magic, 44-byte shader records, 32-byte material records, `(32+extra) x 48`-byte bone block, optional 4-byte sentinel, 128 bone-flag bytes, `u32` vertex/face counts, 32-byte vertices, `faceCount/3` reversed `u16` triangles. Two real files parse to exact EOF (see `design.md` ledger) |
| Minimal slice | NEW `src/srm.rs` (positions + indices only; everything else skipped as opaque byte ranges) + `render.rs` EXTENDED with a mesh pipeline (vertex/index buffers, depth, derivative flat-shading, auto-rotate, fixed camera) + `--mesh <path>` CLI arm. No skeleton, no UV, no materials, no textures on the mesh |
| Reuse rule | `dds.rs` / `bigfile.rs` / `textures.rs` untouched; `render.rs` `State`/event-loop concepts extended, not rewritten (new pipeline + `show_mesh` entry point beside `show_rgba8`) |
| Input contract | The mesh path arrives via `--mesh <path>` at runtime only; absolute game paths never committed; no game bytes in repo or on disk outside process memory (same legal rule as all prior slices) |
| Failure policy | LOUD everywhere (mirrors `bigfile-index`): bad magic, truncated header, insane counts, short vertex/face data, and unknown selectors each print a clear stderr error + exit 1. No guessing, no partial mesh presented as success |
| Non-goals | Skeleton/animation (`.SRA`), level/room files (`.SRL`, `AREA/`), legacy `.PCM`, materials/textures on the mesh, export-to-disk, lighting editors, `bevy_ecs`, SR2 side, Ghidra |
| Slice boundary | Parse-one + render-one + docs + playtest. A second mesh format surprise at apply (layout mismatch on the playtest file) lands as a documented no-go naming what mismatched — never a silent reinterpretation |
| Playtest file | One small loose `.SRM` (`ALKLENS_1`, 86 vertices / 56 triangles — relative name only, resolved against the user's own install at runtime) |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (parse one `.SRM`'s positions + triangles, render it rotating flat-shaded, Esc exits 0).
- [ ] Reviewer can state why this is GO and not a no-go (two real files parse to exact EOF under the cited community layout).
- [ ] Reviewer can state the failure policy (loud exit 1 on any malformed input; no guessing).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed or extracted, game paths via CLI args only, counts/codes/field-shapes only in artifacts, community sources cited with licenses (no code copied).

## Next step

Read `spec.md` for acceptance scenarios (with exit codes), then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).

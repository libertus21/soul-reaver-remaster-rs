# Design: models-3d

Same binary, same window concepts, but a NEW geometry path: `1\OBJECT\*.SRM` files are loose remaster meshes in a community-documented layout, so apply builds the smallest parser the observed bytes support (positions + indices only) and a new flat-shaded mesh pipeline beside the texture blit. Nothing is ever extracted to disk; tests run on synthetic buffers.

## Quick path

`main.rs` (NEW `--mesh <path>` arm; file/dir/`--bigfile`/`--textures` arms untouched) -> NEW `src/srm.rs` (`parse` magic + opaque header skips + counts + positions + indices, all pure + loud errors) -> `render.rs` (NEW `show_mesh` pipeline: vertex/index buffers + depth + auto-rotate uniform + derivative flat-shading; `show_rgba8` untouched) -> pixels.

## Format verdict (planning probe, read-only, 2026-10-12): GO

Loose `.SRM` files under `1\OBJECT\` match the community SRM layout for the SR1/SR2 remasters (non-Defiance branch: single LOD, 44-byte shader records, 32-byte vertices). Both fully-parsed probe files land EXACTLY on EOF under this layout — no residual bytes, no overrun. This is the first slice whose format probe is byte-exact, so the slice is GO (not a no-go with a fallback).

Per-vertex stride re-derivation (recorded so apply never re-guesses): `vec3f` pos (12) + `3x u8` tangent + 1 constant byte (4) + `3x u8` normal (3) + material index (1) + `3x u8` bone indices (3) + `u8` U (1) + `3x u8` weights (3) + `u8` V (1) + 4 skipped bytes = **32 bytes**. Faces are `faceCount / 3` triangles of `3 x u16` LE (winding carried as observed, culling OFF per spec R6).

## Spec source (credit + license)

| Item | Value |
|---|---|
| Remaster SRM layout (PRIMARY) | `io_scene_xrm`, `srm_parser.py` (`SRM.parse_model_file`) + `readers.py` (`Reader`: LE `struct` unpacks — `vec3f` = `3f`, `vec3us` = `3H`, `vec3ub` = `3B`) — `https://github.com/TheDodylectableX/io_scene_xrm/blob/main/srm_parser.py` (also fork `https://github.com/Hengle/io_scene_xrm`). Covers Soul Reaver I-II `.SRM` import/export. LICENSE: repo carries NO license file (verified via API file listing, 2026-10-12) — treat as unlicensed: FACTS ONLY, clean-room, NO code copied; reusing its code later MUST resolve licensing first |
| Remaster SRM semantics (materials/UV/weights/shaders) | Tomb Raider Forums SRM modding thread — `https://www.tombraiderforums.com/showthread.php?t=230457` (2025: material prefix + flag = present-map bits D1/N2/S4/E8; UVs are 1 byte each; max 3 weights per vertex; shader type + params + opaque/alpha/additive index groups; 65535-triangle export limit). Forum terms apply; cited as behavior notes, nothing copied |
| Original-game models (BACKGROUND ONLY) | ModelEx — `https://github.com/TheSerioliOfNosgoth/ModelEx` (views/exports SR1/SR2 `.drm`/`.pcm` extracted via Soul Spiral to Collada; C#). LICENSE file = mixed MIT (Apparat/AssimpNet/SlimDX) + BSD (Assimp) + GPLv3 (Only Skin Deep) — GPL-affected: facts only, NO code copied. Applies to the 1999 builds, NOT the remaster `.SRM` layout |
| Original-game room/object analysis (BACKGROUND ONLY) | Recombobulator — `https://github.com/TheSerioliOfNosgoth/Recombobulator` (analyses SR1 room/object `.pcm` files; C#). LICENSE file present (35,146 bytes, GPLv3 size-class; exact terms unverified — code not reused). Applies to 1999 `.pcm`, not remaster `.SRM` |
| Original-game conversion path (NOT remaster) | Soul Reaver to Blender Conversion Toolchain — `https://thelostworlds.net/Software/Soul_Reaver_to_Blender_Conversion_Toolchain.html` (ModelEx + Recombobulator over Soul-Spiral-extracted 1999 bigfiles). Cited so a later slice does not re-search it; not applicable to loose remaster `.SRM` |
| Remaster editability proof | Nexus SR2 Tweaked Character Models — `https://www.nexusmods.com/legacyofkainsoulreaver1and2remastered/mods/40` (mod page terms; proves remaster meshes are community-editable via Blender; install = overwrite in game folder, which we never do) |
| Cross-check readers (BACKGROUND ONLY) | LOK Noesis plugins — `https://github.com/TheSerioliOfNosgoth/LOK-NoesisPlugins` (independent model-format readers for original formats; check repo license before any reuse; not needed for this slice) |
| Write-path tooling (OUT OF SCOPE) | AsunderNoMore — `https://github.com/TheSerioliOfNosgoth/AsunderNoMore` (packs assets into new bigfiles; we only read — cited for completeness per the survey) |

## Real-file grounding (read-only probes, 2026-10-12)

Total read: ~6.9 KB across 5 files (two full small `.SRM` files parsed in memory + three 16-byte magic windows) plus directory listings + file sizes. No writes, nothing copied out of the install. Counts/codes/field-shapes only — no coordinates, names, hashes, or dumps recorded.

| Probe | Offsets + lengths | Observed (shape only) |
|---|---|---|
| P1 magic + shader window `[0..64]` ×3 | `1\OBJECT\ALKLENS_1.SRM`, `1\OBJECT\MASHER_2.SRM`, `1\OBJECT\ALUKABSS.SRM` (relative names only) | All start with magic `SRM\x01`; all report `shaderCount = 1`, `shaderType = 0`, zeroed 16-byte shader params; per-shader 6×`u32` triplets carried opaque (shapes: small offsets/lengths consistent with in-file ranges, never interpreted) |
| P2 full-layout parse, exact-EOF check | `MASHER_2.SRM` (1904 bytes total) | 1 shader + 1 material (flag 0) + `extraBoneCount = 0` + NO sentinel + 128 flag bytes + `vertexCount = 4`, `faceCount = 6` + `4 x 32`-byte vertices + `2 x 6`-byte triangles = 1904/1904 EXACT |
| P3 full-layout parse, exact-EOF check | `ALKLENS_1.SRM` (4916 bytes total) | 1 shader + 3 materials (all flag 7) + `extraBoneCount = 0` + NO sentinel + 128 flag bytes + `vertexCount = 86`, `faceCount = 168` + `86 x 32`-byte vertices + `56 x 6`-byte triangles = 4916/4916 EXACT |
| P4 size census (no content reads) | `1\OBJECT\` file listing | Smallest `.SRM` files are 1904 bytes (10 files at that size); mid-size ~5 KB; large ~600 KB (sub-600 KB class). Playtest pick is the 86-vertex file (small but non-degenerate); the 4-vertex file bounds the degenerate floor |
| P5 sibling-format magics `[0..16]` | `1\OBJECT\ALUKA.SRA`, `1\OBJECT\ACLALU01.SRL`, `1\OBJECT\BLADE\BLADE.PCM` | `.SRL` magic is `SRL\x02` (distinct mesh-adjacent format, OUT of scope); `.SRA` starts with a count-like `u32` word, no text magic (UNKNOWN, out of scope — likely skeleton side); `BLADE.PCM` starts with a compression-like prefix (UNKNOWN, out of scope — legacy object path, not remaster `.SRM`) |
| P6 dir structure (no content reads) | `1\OBJECT\` listing | 666 entries: per-object subdirs (mostly near-empty or single-legacy-file) + loose `.SRM`/`.SRA`/`.SRL` files at top level. Meshes live LOOSE, not in a container — no archive step needed |

## UNKNOWNs + fallback plan

| Gap | Status | Fallback (documented in the apply PR, no new probing without a ledger entry) |
|---|---|---|
| Bone-matrix / flag-bytes semantics | UNKNOWN, carried opaque | Skipped as byte ranges; skeleton animation is a later milestone, never guessed here |
| Shader-triplet semantics (opaque/alpha/additive offsets) | UNKNOWN, carried opaque | Skipped as byte ranges; flat-shaded solid needs no material info |
| Sentinel presence rule (absent on both probes) | UNKNOWN | Parser handles BOTH (peek `0x80000000` → skip 4, else rewind — same rule as the community parser); unit-tested synthetic both ways |
| Face winding / front-face order | UNKNOWN, carried as observed | Culling OFF (spec R6); playtest confirms a solid; wrong-winding follow-up only if the playtest shows inside-out faces |
| Position units / orientation / scale | UNKNOWN | Auto-fit to unit cube at load (spec R5); no axis assumption committed |
| Normals/tangents/UVs/weights/materials | UNKNOWN-to-this-slice (documented upstream) | Offsets skipped, never interpreted; flat shading derives face normals from positions in-shader |
| `.SRA` / `.SRL` / `.PCM` / `AREA/` room meshes | UNKNOWN, out of scope | Each needs its own scoping probe before any claim (same discipline as `E3.BIG` etc.) |
| Large-file shape (600 KB-class `.SRM`) | UNVERIFIED beyond P1 magic window | Parser is count-driven so size is irrelevant to correctness; playtest stays on the small file; large-file render is a follow-up slice |

## Details

| Topic | Decision |
|---|---|
| New module (`src/srm.rs`) | `pub struct Mesh { positions: Vec<[f32; 3]>, indices: Vec<u16> }` + `pub fn parse(bytes: &[u8]) -> Result<Mesh, SrmError>` (pure, no I/O; whole-file buffer handed in) + `pub fn load(path) -> Result<Mesh, SrmError>` (reads file, calls `parse`). Header skips: `4 + 4 + shaderCount x 44 + 4 + materialCount x 32 + 4 + (32+extra) x 48 + optional 4 + 128 + 8`, every advance range-checked BEFORE the read (mirrors `bigfile.rs` discipline). Counts sanity: `vertexCount <= 65535 x 3`-ish bound + `offset + len <= file_len` overflow-safe (`u64` arithmetic); `faceCount % 3 == 0` required |
| Error type | NEW `SrmError`: `Io` / `NotSrm` (bad magic) / `InvalidSrm(String)` (truncated header, insane counts, short vertex/face data — message names expected-vs-found sizes + the failed check). Every variant → stderr + `ExitCode(1)`; no window on error |
| `--mesh` output | Counts line to stdout (`N vertices, M triangles`), window titled per file name; close/Esc exits 0. Existing arms byte-for-byte unchanged |
| Render (`show_mesh`) | NEW pipeline in `render.rs` beside the blit: vertex buffer (`3 x f32` pos), index buffer (`u16`), `Depth24Plus` depth texture, uniform buffer (rotation `mat4` + static perspective, updated per frame at ~0.6 rad/s Y), fragment flat-shading via `dFdx`/`dFdy` cross-product face normal + single directional light + ambient, uniform base color, `cull_mode: None`. Reuses `State`-shaped window setup + Esc/close handling from the gallery app concepts; `show_rgba8` path zero changes |
| Auto-fit | CPU at load: AABB center + uniform `1/maxExtent` scale into a unit cube; degenerate (all-identical positions) is `InvalidSrm`, not a divide-by-zero |
| Tests | Unit tests on SYNTHETIC bytes only (hand-built minimal `.SRM` buffers: magic accept/reject, both sentinel arms, counts sanity incl. huge-count rejection, exact position/index extraction, truncated-tail rejection, error-message rendering). No GPU needed; pixel correctness stays a user playtest |
| Dependencies | ZERO new shipped crates; `winit`/`wgpu`/`image`/`pollster`/`bcdec_rs` unchanged (rotation/perspective math hand-rolled — no `glam`/`cgmath` for one `mat4`) |

## Deliberately NOT built

Skeleton/animation, UVs/materials/textured mesh, wireframe toggle, orbit/mouse camera, multi-mesh or directory-browse mode, `.SRA`/`.SRL`/`.PCM` parsing, `AREA/` rooms, export-to-disk, lighting/material editors, `bevy_ecs`, SR2 side, Ghidra.

## Checklist

- [ ] Reviewer can trace file-to-screen in one paragraph (table above) and name the new vs reused entry points (`srm::parse` NEW; `show_mesh` NEW pipeline; `show_rgba8` untouched).
- [ ] Reviewer can state every probed path + byte range (P1–P6 table) and the total read budget (~7 KB, read-only).
- [ ] Reviewer can state the per-vertex stride derivation (32 bytes, field-by-field) and both exact-EOF arithmetic lines.
- [ ] Reviewer can state every UNKNOWN and its fallback (opaque skips, sentinel both-arms, culling OFF, auto-fit, follow-up slices).
- [ ] Index ranges are validated overflow-safe before any read; file bytes never touch disk outside process memory.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

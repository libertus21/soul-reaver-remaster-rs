# Spec: animation

`cargo run -- --mesh <FILE.SRM> --pose <bone>:<degrees>` parses one
extra=0 SR1-remaster mesh, rotates exactly one decoded bone's weighted
vertices, prints the moved-vertex count, and shows the deformed mesh in
the existing auto-rotating window; Esc or window close exits 0. Corrupt
input, an undecodable model class, or a bad pose spec fails LOUD with exit
1. Every prior `--mesh`, `--mesh-textured`, `--mesh-all`, `--bigfile`,
`--textures`, audio, file, and directory form keeps its exact prior
behavior.

## Quick path

1. Run `cargo run -- --mesh "<install>\1\OBJECT\RAZIEL_1.SRM" --pose 7:45`
   (relative model name in docs; the real path is runtime-only).
2. A window opens showing the flat-shaded mesh auto-rotating with the
   chosen bone's weighted region rotated 45 degrees; stdout reports the
   bone, angle, and moved-vertex count.
3. User playtest records model relative name, command, what was seen, and
   Esc exit code. Repeat with `--pose 7:0` to confirm the rest mesh is
   unchanged.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI: `--mesh <FILE>` (one arg) keeps its current behavior. NEW second shape: `--mesh <FILE> --pose <bone>:<degrees>` (exactly three args, second arg literally `--pose`). Any other shape, unknown flag, missing value, or extra trailing arg is a LOUD usage error + exit 1 |
| R2 | Bone-block decode: after the material block, read `u32 extra`; then `extra` 48-byte records, each twelve f32 LE (the decoded HD matrix class); then exactly 32 records of four `vec3` f32 LE = 128 joint positions; then the optional `0x80000000` sentinel; then 128 flag bytes. `extra` above the sanity ceiling is `InvalidSrm`, never a huge allocation. The parser stores matrices as raw 12-float records (semantics UNKNOWN) and the 128 joints in order |
| R3 | Per-vertex bone fields: three `u8` bone indices at offsets 20..22 and three `u8` weights at 24..26 inside the existing 32-byte stride (offsets 19/23/27 stay material/U/V as today). Every bone index must be below 128 or the file is `InvalidSrm`. Weight triples are used as observed; this slice does not renormalize and does not require a particular sum |
| R4 | Pose spec: `<bone>:<degrees>`, one colon, integer bone in 0..=127, finite degrees with `abs(degrees) <= 360`. Empty parts, extra colons, non-integers, NaN/infinity, and out-of-range values are LOUD errors + exit 1 |
| R5 | Pose semantics: the target bone must have at least one vertex with a non-zero weight. The parser stores joints raw; before pose math each joint is mapped to mesh space by negating Z (`[x, y, -z]`), the mapping established on the extra=0 humanoid (design P5). Head = mesh-space joint for that index. Tail = the adjacent joint in the same four-slot record (next slot when the row is 0..2, previous slot when the row is 3). Rotation axis = normalized cross of `(tail - head)` with the model up axis `+Z`; a degenerate direction falls back to `+X`. Each vertex with weight `w` on the target bone moves by `p + (w / 255) * (R * (p - head) - (p - head))`; summed duplicate indices are allowed; zero-weight vertices are unchanged. Pose is applied in mesh space, then the existing auto-fit frames the posed mesh |
| R6 | Undecodable class refusal: if `extra > 0`, `--pose` prints a clear error naming the `extra` HD matrix class as UNKNOWN and exits 1 without opening a window. The rest `--mesh` form still renders such a file |
| R7 | Output: `--mesh` prints `N vertices, M triangles` plus `skeleton: K matrices, 128 joints` (counts only). `--pose` adds `pose bone B, D deg, V vertices moved` before the window. Counts only, never coordinates or bytes |
| R8 | Failure policy: malformed magic/header, truncated bone block, `extra` ceiling, bone index >= 128, undecodable class, no-weighted-vertex bone, bad pose spec, and non-finite target joint/angle each print a loud stderr message and exit 1 with no window and no partial pose. No guessing, no silent fallback |
| R9 | Non-goals: skeleton overlay/wireframe, hierarchy/parenting, animation clips/keyframes, `.SRA`/`.SRL`, `extra > 0` matrix interpretation, export, material/lighting editors, SR2 side, Ghidra |
| R10 | Legal/tests: tests use SYNTHETIC bytes only (hand-built `.SRM` buffers in temp dirs); no game files touched by tests; no game bytes, absolute paths, coordinates, hashes, or content dumps committed; whitelist `.gitignore` stays |

## Acceptance scenarios

- [ ] **Rest path unchanged:** `--mesh "<real .SRM>"` on an extra=0 model
      opens the same auto-rotating flat-shaded mesh as before, now with the
      `skeleton: K matrices, 128 joints` suffix; Esc exits 0.
- [ ] **Pose happy path:** `--mesh "<real extra=0 humanoid>.SRM"
      --pose 7:45` exits 0 after Esc; stdout names bone 7, 45 degrees, and
      a positive moved-vertex count; user playtest confirms the weighted
      region visibly moves with the mesh still intact and auto-rotating.
- [ ] **Zero angle:** `--pose 7:0` renders the rest mesh (no visible
      displacement) and still exits 0.
- [ ] **Undecodable class:** `--pose` on a real `extra > 0` model prints
      the HD-matrix UNKNOWN message and exits 1; no window.
- [ ] **Bad bone:** `--pose 999:45`, `--pose 127:45` (no weights), and
      `--pose 7:45` on a synthetic mesh with no weights for bone 7 each
      exit 1 with a clear message; no window.
- [ ] **Bad spec:** `--pose 7`, `--pose :45`, `--pose 7:`, `--pose 7:45:0`,
      `--pose x:45`, `--pose 7:nan`, `--pose 7:361` each exit 1 loud.
- [ ] **Arg shapes:** `--mesh` with 0, 2, or 4+ args exits 1; `--pose`
      without the flag word exits 1; existing modes keep their archived
      behavior.
- [ ] **Malformed bone block:** synthetic truncation in the matrix or
      joint block, an `extra` count past the ceiling, or a bone index of
      128+ exits 1 loud; no panic, no huge allocation.
- [ ] **Unit level:** `cargo test` covers matrix/joint extraction, exact
      ordering, both sentinel arms, per-vertex bone index/weight offsets,
      index bound, spec parsing, rotation/axis math, partial weights,
      pivot invariance, duplicate indices, zero-weight and other-bone
      vertices, and every refusal message without a GPU or game files.
- [ ] **Legal:** `git status --short` shows only intended source/doc files;
      no extracted bytes, no absolute game paths, no coordinates.

## Next step

See `design.md` for the probe ledger, evidence classes, UNKNOWNs, and
module decisions; `tasks.md` for the three-PR chain and forecast.

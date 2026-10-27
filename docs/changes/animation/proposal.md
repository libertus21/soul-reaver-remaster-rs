# Proposal: animation

GO for a one-bone pose: the `.SRM` bone block decodes into `extra` 3x4 f32
matrices followed by 32 records of four `vec3` joints (128 joint slots),
and the existing vertex stride already carries three bone indices and
three weights per vertex. This slice adds `--mesh <FILE> --pose
<bone>:<degrees>`: one chosen bone's weighted vertices rotate around its
decoded joint, and the existing flat-shaded mesh path shows the deformed
result. Hierarchy, animation clips, and the `extra > 0` matrix semantics
stay UNKNOWN and are explicitly refused, not guessed.

The smallest visible win is a mesh that visibly deforms under one manually
rotated bone. Drawing the bind-pose joint chains is feasible from the same
data but is deferred so the risky render work does not ride with the first
deformation proof.

## Quick path

1. `cargo run -- --mesh "<one extra=0 humanoid>.SRM" --pose 7:45` prints
   the moved-vertex count and opens the auto-rotating mesh with that bone
   rotated 45 degrees (user playtest).
2. `cargo test` proves the decode, the pose math, and every refusal on
   synthetic bytes only.
3. The playtest records file + command + observation + exit code. If the
   deformation is not plausible, this change stops as a documented no-go.

## Details

| Topic | Decision |
|---|---|
| Intent | Give the existing `.SRM` mesh path one real skeletal deformation using only data the file already carries: joint positions, per-vertex bone indices, per-vertex weights |
| Format verdict (planning probe, read-only) | GO for one-bone pose. 349/349 loose `.SRM` files under `1\OBJECT\` parse to exact EOF with `(32 + extra) * 48` bone bytes. Across the 7 files with `extra > 0`, all 128 extra records are orthonormal 3x4 affine matrices; the remaining 32 records are 128 `vec3` joint positions. Every vertex's three weights sum to 255 and every bone index is below 128 (max 67). On an `extra = 0` humanoid, 65 plausible joints align with the mesh (median nearest-vertex distance about 21 units on a roughly 500-unit tall model) |
| What is NOT in the `.SRM` | No parent links, no bone names, no keyframes, no clip table. The `.SRA` files are the animation candidate and are out of this slice |
| Minimal slice | NEW pure `src/pose.rs` (`parse_spec`, `apply`) + `srm.rs` decode of the bone block and per-vertex bone fields + a new `--pose` arm in `main.rs`. `render.rs` is untouched: `show_mesh` already auto-fits and auto-rotates |
| Reuse rule | `srm.rs` extended, not rewritten; `render.rs` / `dds.rs` / `bigfile.rs` / `textures.rs` untouched; no new crates; no new GPU pipeline |
| Input contract | Mesh path and pose spec arrive at runtime only; absolute game paths never committed; no game bytes in the repo |
| Failure policy | LOUD everywhere: malformed/truncated bone block, bone index >= 128, `extra > 0` (HD matrix alignment UNKNOWN), bone with no weighted vertices, malformed pose spec, non-finite angle/joint, and wrong arg shapes each print a clear stderr error + exit 1. No window, no partial pose |
| Non-goals | Skeleton overlay, bone hierarchy, animation clip playback, `.SRA`/`.SRL`, `extra > 0` matrix interpretation, export-to-disk, material/lighting editors, `bevy_ecs`, SR2 side, Ghidra |
| Slice boundary | Decode + one-bone pose + tests + docs + one user playtest. A layout surprise at apply is a documented no-go naming the failed check, never a silent reinterpretation |
| Playtest file | One `extra = 0` humanoid `.SRM` (`RAZIEL_1.SRM`, relative name only), resolved against the user's own install at runtime |

## Explicit no-go condition

This change is GO only while all four hold:

1. The bone block shape holds on the census: `extra` matrices followed by
   32 x 4 joint positions, exact EOF.
2. On the chosen `extra = 0` humanoid, the joint positions align with the
   mesh within a stated tolerance (median nearest-vertex distance at most
   5% of the model's largest bbox extent).
3. Every vertex bone index is below 128 and at least one target bone has
   weighted vertices.
4. The user playtest shows a plausible one-bone deformation (the moved
   region follows the chosen joint; no exploded mesh, no invisible move).

If (2), (3), or (4) fails, STOP. Document the failed check and name the
next candidate (`.SRA` plus Ghidra, both deferred), rather than falling
back to an invented hierarchy or a centroid pivot.

## Checklist

- [ ] Reviewer can state the one deliverable without re-reading (one bone
      rotates, mesh deforms, Esc exits 0).
- [ ] Reviewer can state why this is GO and not a no-go (census + joint
      alignment + index bound + weight sum; playtest is the remaining gate).
- [ ] Reviewer can state what is NOT decoded (hierarchy, clips, `extra > 0`
      matrices, flag semantics).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes
      committed, relative names only, synthetic test bytes only, community
      sources cited with licenses and no code copied.

## Next step

Read `spec.md` for acceptance scenarios and exit codes, then `design.md`
for the probe ledger and the code shape, then `tasks.md` for the forecast
and the three-PR chain.

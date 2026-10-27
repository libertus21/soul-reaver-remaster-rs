# Design: animation

The mesh path already parses `.SRM` positions, UVs, material indices, and
triangles. This change decodes the one remaining geometry-relevant block
(the bone block) and applies exactly one bone transform on the CPU before
handing positions to the existing `show_mesh` pipeline. `render.rs` does
not change. The file supplies the joint positions and the per-vertex
weights; the file does NOT supply a hierarchy or clips, so the slice
deliberately stops at a single-bone pose.

## Quick path

`main.rs` (`--mesh <FILE> --pose <bone>:<deg>` arm) -> `srm::load`
(matrix records + 128 joints + per-vertex bone indices/weights) ->
`pose::parse_spec` -> `pose::apply` (axis + rotation + weighted blend) ->
`render::show_mesh` (unchanged auto-fit, auto-rotate, flat shading) ->
pixels. Every other CLI arm is untouched.

## Format verdict: GO for one-bone pose, NO-GO for hierarchy/clips

The block is not a hierarchy. It is a fixed pool of 128 joint positions
plus per-vertex weights. That is enough for (a) a joint point/chain
overlay and (b) a one-bone linear-blend deformation, but not for an
articulated rig or clip playback. The slice chooses (b) because it reuses
the existing render path and proves the mapping with pixels.

## Probe ledger (planning, read-only, 2026-10-26)

Box: three passes, no writes to game data, no game bytes copied into the
repo, scratch only in the session temp directory. Pass 1 = census and
block arithmetic across every loose `.SRM`; pass 2 = block shape,
per-vertex fields, and alignment/mapping checks on a small extra=0
humanoid; pass 3 = community tools and `.SRA` scoping. No Ghidra, no
`.SRA` decode, no writes.

| Probe | Scope | Observation (shape only) | Evidence class |
|---|---|---|---|
| P1 | 349 loose `.SRM` files under `1\OBJECT\` | 349/349 parse to exact EOF under `4 + 4 + shaders*44 + 4 + materials*32 + 4 + (32+extra)*48 + [4 sentinel] + 128 flags + 8 counts + vertices*32 + faces*2`. `extra` distribution: 342 x 0, 3 x 26, 1 x 17, 1 x 25, 1 x 7, 1 x 1 | Observed |
| P2 | The 7 files with `extra > 0` (128 matrix records total) | Every one of the first `extra` 48-byte records has a near-orthonormal 3x3 basis and an arbitrary 3-float translation (3x4 affine row-major shape). 128/128 pass; the following 32 records do not | Derived (strong) |
| P3 | The 32 records after the matrices, all files | Each 48-byte record is four `vec3` f32 = 128 joint slots in file order. Plausible (finite, magnitude > 1 unit) slots range 0..65 per file, median 1; 164 static-prop files have none, while the extra=0 humanoid has 65 forming 17 chains with at least two points | Derived + inferred grouping |
| P4 | Vertex stride, all files | Three `u8` bone indices at 20..22 and three `u8` weights at 24..26; material at 19, U at 23, V at 27. Every vertex weight triple sums to 255; every bone index is below 128; the observed maximum is 67 | Observed |
| P5 | extra=0 humanoid, mesh-vs-joint alignment | Raw joints must have Z negated to overlay the mesh: 65 plausible joints, median nearest-vertex distance about 21 units on a roughly 500-unit tall model, max about 252. Other axis conventions are worse. High-weight vertex clusters sit roughly 42..145 units from their slot joint, consistent with bone length but not proof of anatomy | Derived + inferred mapping |
| P6 | `.SRA` siblings | 227 files; first word is count-like, no text magic; the remaster Blender addon has no `.SRA` parser. `.SRA` stays the animation candidate, out of scope | Observed + inferred |
| P7 | Community tools | See the table below. The only remaster tool that reads the bone block treats each 48-byte record as four bone positions and builds unparented bones; its own comments say the structure is unknown. No remaster animation importer found | Observed |

No raw coordinates, file offsets beyond the published block shape, hashes,
or bytes are recorded here.

## What is established vs UNKNOWN

| Claim | Status | Basis / fallback |
|---|---|---|
| `(32 + extra) * 48` bone bytes, matrices first, then 128 joints | Established | P1 + P2; exact-EOF arithmetic on all 349 files |
| 128 joint slots, four per 48-byte record | Established | P3; parse shape |
| Per-vertex 3 indices + 3 weights at the documented offsets | Established | P4; existing stride derivation |
| All bone indices below 128, weights sum to 255 | Established on the census | P4; parser still validates the index bound and uses weights as bytes |
| Z-negated joints align with an extra=0 humanoid | Established for the playtest class | P5; `extra > 0` is refused for pose |
| Slot index -> weighted region mapping | Inferred, playtest-gated | P5; joint-to-cluster distances are plausible, but no bone names exist |
| Bone hierarchy / parents | UNKNOWN | None in the `.SRM`; the community addon creates unparented bones |
| Animation clips / keyframes | UNKNOWN, out of scope | `.SRA` candidate; no decoder; Ghidra deferred |
| 128 flag-byte semantics | UNKNOWN | 0/1 values; counts correlate with used bones on some extra=0 models but are not index-aligned. Ignored by this slice |
| `extra > 0` matrices (HD class) | Shape established, semantics UNKNOWN | P2; stored raw, never applied; `--pose` refuses this class |
| Per-model orientation / anatomy / bone names | UNKNOWN | Slice scopes the visible win to one extra=0 humanoid and speaks of "bone N", never "left arm" |
| Whether the 128 joints are the old or HD skeleton | UNKNOWN | Community note mentions old + HD; no proof |

## Community research (exact URLs + licenses)

| Tool | URL | License | What it actually reads | Verdict for this slice |
|---|---|---|---|---|
| io_scene_xrm (remaster Blender addon) | https://github.com/TheDodylectableX/io_scene_xrm | No LICENSE file; GitHub API reports no license (unlicensed) | Remaster `.SRM` models; parses the 48-byte records as four `vec3` positions per record and builds an unparented armature. Comments state the bone structure is unknown and "supposedly only carries location vectors". No `.SRA` parser | PRIMARY format facts only, clean-room. No code copied. Its four-positions-per-record reading matches P3; our matrix/joint split is a data-derived refinement (P2), not a copy |
| ModelEx | https://github.com/TheSerioliOfNosgoth/ModelEx | LICENSE file present; GitHub classifies `NOASSERTION` (mixed terms) | Original SR1/SR2 `.drm` / `.pcm` models, not remaster `.SRM`; no remaster animation | Background only, not this slice |
| Recombobulator | https://github.com/TheSerioliOfNosgoth/Recombobulator | GPL-3.0 | Original SR1 `.pcm` rooms/objects; has original-game animation structs (`SAnim`, etc.), not remaster `.SRA` | Background only; GPL, no code copied |
| AsunderNoMore | https://github.com/TheSerioliOfNosgoth/AsunderNoMore | MIT | Packs SR1 PC assets into a new `bigfile.dat`; no skeleton or animation | Not applicable |
| Tomb Raider Forums SRM thread | https://www.tombraiderforums.com/showthread.php?t=230457 | Forum terms | SRM material/UV/weight behavior notes; no skeleton decode | Prior-slice background only |

No remaster `.SRA` parser, animation importer, or documented bone
hierarchy was found. That is why the slice stops at one bone and names
`.SRA` + Ghidra as the next candidate if more is needed.

## Details

| Topic | Decision |
|---|---|
| `src/srm.rs` | Extend `Mesh` with `bones: BoneBlock { matrices: Vec<[f32; 12]>, joints: Vec<[f32; 3]> }`, `bone_indices: Vec<[u8; 3]>`, `bone_weights: Vec<[u8; 3]>`. Parse helper reads 12 f32 per matrix and 4 x vec3 per joint record. `MAX_EXTRA_BONES = 4096` sanity ceiling (real max 26; a fixed 4096 keeps the allocation small while never rejecting a real file). Reject `extra` over the ceiling and any vertex bone index `>= 128` as `InvalidSrm`. Keep the sentinel peek and 128 flag skip as today. Exact-EOF check stays |
| `src/pose.rs` (NEW) | Pure module, no I/O: `PoseSpec { bone: usize, degrees: f32 }`, `parse_spec(&str) -> Result<PoseSpec, String>`, `mesh_space_joints(&[[f32; 3]]) -> Vec<[f32; 3]>`, `apply(positions, bone_indices, bone_weights, mesh_joints, spec) -> Result<PoseOutcome, String>`. `PoseOutcome { positions: Vec<[f32; 3]>, moved_vertices: usize }`. Validates lengths, finite head, target weights, and axis fallback. Hand-rolled cross/normalize/rotation; no new crate |
| Joint frame | The parser stores joints raw in file order. `mesh_space_joints` maps each to `[x, y, -z]` before pose math; this is the P5 mapping. Vertices are already mesh space, so head/tail/axis and vertex positions share one frame |
| Axis | Head = `mesh_joints[bone]`; tail = adjacent slot in the same four-slot record (next for rows 0..2, previous for row 3). `axis = normalize(cross(tail - head, +Z))`; if the direction is degenerate, use `+X`. This makes a limb swing instead of an arbitrary twist |
| Blend | For each vertex, sum the weights whose index equals the target bone (duplicate indices allowed). `t = sum / 255`. `p' = p + t * (R * (p - head) - (p - head))`. Zero-weight vertices are copied unchanged. This is one-bone linear blend skinning, not a rigid-body split, so seams stay continuous |
| Framing | Pose is applied in mesh space. The existing `show_mesh` auto-fit frames the posed positions. The model may rescale slightly when the bbox grows, but the deformation stays correct and no joint overlay needs a shared fit transform in this slice |
| CLI | `main.rs` matches `[path]` as today and `[path, "--pose", spec]` as NEW. `srm::load`, then refuse `extra > 0`, parse spec, `mesh_space_joints`, apply, print `pose bone B, D deg, V vertices moved`, call `render::show_mesh`. All other arms untouched |
| Rest counts | `--mesh` counts line gains `skeleton: K matrices, 128 joints` so the decode is visible without a window; counts only |
| Tests | `srm.rs`: synthetic builders with zero and non-zero `extra`, matrix/joint ordering, both sentinel arms, bone index/weight extraction, index-128 rejection, `extra` ceiling, truncation in each block. `pose.rs`: spec parsing, zero angle, known 90-degree rotation, pivot invariance, partial weight, duplicate indices, other-bone vertices unchanged, out-of-range/no-weight/non-finite refusals, degenerate axis fallback. CLI: wrong arg shapes and refusal-before-window paths. No GPU, no game files |
| Dependencies / render | ZERO new crates; no `render.rs` change; no line/point pipeline; `show_rgba8`, textured mesh, gallery, bigfile, textures, audio untouched |

## Deliberately NOT built

Skeleton point/chain overlay, bone hierarchy or parenting, animation clip
playback, `.SRA`/`.SRL` parsing, `extra > 0` HD matrix interpretation,
bone names, export-to-disk, lighting/material editors, `bevy_ecs`, SR2
side, Ghidra. The overlay is the natural next slice because P3/P5 already
support the joint geometry; it is separated to keep the render risk out of
the first deformation proof.

## Checklist

- [ ] Reviewer can trace file-to-pixels in one paragraph and name the new
      entry points (`srm::BoneBlock`, `pose::apply`, the `--pose` arm).
- [ ] Reviewer can state the block shape and the exact evidence class for
      every claim in the ledger.
- [ ] Reviewer can state what is UNKNOWN and how the slice refuses it
      (`extra > 0`, hierarchy, clips, flags).
- [ ] Reviewer can state the pose math (head/tail, axis, partial weight)
      and why it is one-bone LBS.
- [ ] Reviewer can state the legal/test boundaries: synthetic bytes only,
      relative names only, no game bytes or absolute paths.

## Next step

Implement in the three-PR order in `tasks.md`; the forecast there is
deliberately pessimistic and the chain is fixed in the plan, not at apply
time.

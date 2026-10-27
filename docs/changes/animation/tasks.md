# Tasks: animation

Ordered work units, delivered as a fixed three-PR chain. Each PR is
independently committable, keeps tests/docs with its behavior, and stays
at or below the 400-line per-slice ceiling. `test_command` for every apply
phase: `cargo test`. Tests use synthetic bytes only.

## PR 1 - `feat(srm): decode the bone block and per-vertex bone fields`

- [ ] Extend `src/srm.rs`: `BoneBlock { matrices: Vec<[f32; 12]>, joints:
      Vec<[f32; 3]> }`, `Mesh.bones`, `Mesh.bone_indices: Vec<[u8; 3]>`,
      `Mesh.bone_weights: Vec<[u8; 3]>`.
- [ ] Parse the block in order: `u32 extra` -> `extra` x 48-byte matrix
      records (12 f32 LE) -> 32 x 48-byte joint records (4 x vec3 f32 =
      128 joints) -> optional `0x80000000` sentinel -> 128 flags. Add a
      small `MAX_EXTRA_BONES` ceiling and reject `extra` above it. Reject
      any vertex bone index `>= 128`. Keep the exact-EOF check.
- [ ] Read the per-vertex bone fields at the existing stride: indices at
      20..22, weights at 24..26; do not disturb material 19 / U 23 / V 27.
- [ ] Add `skeleton: K matrices, 128 joints` to the `--mesh` counts line
      (counts only).
- [ ] Unit tests on synthetic bytes: zero and non-zero `extra`; matrix
      and joint ordering; both sentinel arms; exact joint count 128;
      bone index/weight extraction; index-128 rejection; `extra` ceiling;
      truncation in matrix block, joint block, and vertex block; no
      panic or huge allocation.
- [ ] Update `docs/notes/srm-bone-block.md` with the final decode ledger
      and README "What works" line for the decoded counts.
- [ ] Verify: `cargo test` green; `git status --short` only intended
      files; no game bytes or absolute paths.

## PR 2 - `feat(pose): add pure one-bone deformation math`

- [ ] Add `src/pose.rs` (pure, no I/O): `PoseSpec`, `parse_spec`,
      `mesh_space_joints` (Z negation per design P5), `apply`,
      `PoseOutcome { positions, moved_vertices }`.
- [ ] `parse_spec`: one colon, integer bone 0..=127, finite degrees,
      `abs(degrees) <= 360`; every malformed shape is a loud `Err(String)`.
- [ ] `apply`: length checks; finite head; target bone must have at least
      one non-zero weight; tail is the adjacent joint in the same
      four-slot record; axis = `normalize(cross(tail - head, +Z))` with a
      `+X` fallback for a degenerate direction; partial-weight LBS
      `p + (w/255) * (R(p-head) - (p-head))`; duplicate target indices
      sum; zero-weight and other-bone vertices unchanged; count moved
      vertices.
- [ ] Hand-rolled cross/normalize/rotation helpers; no new crate.
- [ ] Unit tests: spec accept/reject matrix; `mesh_space_joints` negates
      Z and preserves order; zero angle returns identical positions; known
      90-degree rotation around a known axis; pivot invariance; partial
      weight; duplicate indices; other-bone vertices unchanged;
      out-of-range bone, no-weight bone, non-finite joint, and
      degenerate-axis fallback; moved-vertex count.
- [ ] Verify: `cargo test` green; `git status --short` only intended
      files.

## PR 3 - `feat(anim): wire --pose to the mesh path and record playtest`

- [ ] Extend `run_mesh` in `main.rs`: accept exactly `[path]` (rest,
      unchanged) and `[path, "--pose", spec]` (NEW). Any other shape is a
      loud usage error + exit 1.
- [ ] On the pose arm: `srm::load`; refuse `extra > 0` with the HD-matrix
      UNKNOWN message; `pose::parse_spec`; `pose::apply`; print
      `pose bone B, D deg, V vertices moved`; call the existing
      `render::show_mesh` with the posed positions. No `render.rs` change.
- [ ] CLI tests (no window): wrong arg shapes, bad spec, extra > 0,
      out-of-range/no-weight bone, and malformed synthetic bone block all
      exit 1 with the expected message; rest `--mesh` path unaffected.
- [ ] Update `README.md` "What works / What's missing" with the one-bone
      pose, the extra=0 limitation, and the fact that hierarchy/clips are
      still missing.
- [ ] User playtest: `--mesh "<real extra=0 humanoid>.SRM" --pose 7:45`
      (and `7:0` control) -> mesh deforms plausibly, auto-rotates, Esc
      exits 0. Record relative file name, command, observation, exit code
      in the PR body. If not plausible, STOP and document the no-go; do
      not invent a pivot or hierarchy.
- [ ] Verify: `cargo test` green; playtest recorded; `git status --short`
      clean of non-whitelisted files; no game bytes or absolute paths.

## Review Workload Forecast

The two prior slices missed their forecasts by 3.7x and 3.9x because test
lines were under-budgeted. This forecast is deliberately pessimistic:
the **first-instinct test estimate was multiplied by 1.5** and the result
is the number below. Production and docs are counted separately. The PR
split is fixed here, not chosen at apply time.

| PR | Production lines | Test lines (first instinct -> x1.5) | Docs/README lines | Total |
|---|---|---|---|---|
| PR 1 - bone-block decode | 90 | 170 -> **255** | 25 | **370** |
| PR 2 - pose math | 130 | 150 -> **225** | 0 | **355** |
| PR 3 - CLI + playtest | 70 | 80 -> **120** | 35 | **225** |
| **Total** | **290** | **600** | **60** | **950** |

- **Per-PR ceiling:** 400 authored lines (repo rule). Every slice above is
  under it with 30+ lines of headroom; PR 1 is the closest at ~370.
- **Session review budget:** 800 lines. The forecast total (~950) exceeds
  it, so this is a chain of **three PRs from the start**, not a single PR.
  PR 1 + PR 2 together are ~725; adding PR 3 crosses the budget. Review
  the chain in order, or split the review across sessions.
- **PR split plan:** PR 1 = decode only (`srm.rs` + counts + tests +
  note). PR 2 = pure `pose.rs` math + tests, no CLI. PR 3 = `--pose` wiring
  + CLI/refusal tests + README + playtest. No file is edited by two PRs
  except `README.md` (PR 1 counts line, PR 3 pose section) and the note
  (PR 1 create, PR 3 playtest appendix if needed).
- **Stop rule:** if PR 1 lands above 400 authored lines, do not merge it
  and do not start PR 2; split the PR 1 tests into a follow-up commit
  first. Do not carry the overage into PR 2.
- **Why not one PR:** ~950 authored lines, two new behaviors, parser
  limits, rotation math, CLI refusals, and a playtest. A single review
  would exceed the 800-line session budget and repeat the under-budgeted
  test failure mode.
- **Work-unit mapping:** PR 1 -> commit 1 (decode + tests + note); PR 2 ->
  commit 2 (pose math + tests); PR 3 -> commit 3 (CLI + README + playtest).
  Tests travel with the behavior they prove.

## Apply gate

- [ ] PR 1 decode merged only if `cargo test` is green, all 128 joints are
      present, every bone index is < 128, and the exact-EOF behavior is
      unchanged.
- [ ] PR 2 pose math merged only if the pure tests prove the rotation,
      partial weight, pivot, and every refusal without a GPU.
- [ ] PR 3 merged only after the user playtest records a plausible
      deformation and Esc exit 0. If the playtest fails, mark the change
      no-go, keep the decode PR, and name `.SRA` + Ghidra as the next
      candidate.
- [ ] `git status --short` clean of non-whitelisted files at every merge;
      no game bytes, absolute paths, coordinates, or hashes in any PR.

# Archive: animation — PARTIAL

**Stage banner: PARTIAL — decode + one-bone math delivered; full-body pose blocked by absent hierarchy.**

The `.SRM` bone block is decoded and validated; the pure one-bone deformation
math is implemented; and `--mesh <FILE.SRM> --pose <bone>:<degrees>` renders
deformed positions. That part works. What does NOT work is coherent full-body
posing: `.SRM` carries no bone hierarchy, so rotating a slot moves only its own
weighted vertices and leaves every other slot fixed. The mesh tears. The
limitation is structural, not a math defect — the file simply does not contain
the parent/child relation that posing needs. PR #39 is merged into `main` at
`d1cd70f`; the change touched `src/srm.rs`, `src/pose.rs`, `src/mesh_entry.rs`,
`src/main.rs`, `README.md` and the change docs only.

## Quick path

1. `git checkout main && git pull` — HEAD is `d1cd70f` (merge of PR #39).
2. `cargo test` — 208/208 green on synthetic bytes only.
3. `cargo run -- --mesh <relative-name>.SRM --pose 7:0` — rest control; prints
   `pose bone 7, 0 deg, 0 vertices moved`, mesh unchanged.
4. `cargo run -- --mesh <relative-name>.SRM --pose 7:45` — 940 vertices moved;
   mostly plausible, one leg edge looks odd at the cluster seam.
5. `cargo run -- --mesh <relative-name>.SRM --pose 12:45` — 5,215 vertices
   moved; the result is clearly broken/scattered. This is the honest boundary
   of the slice.

## Delivered and working

| Item | Evidence |
|---|---|
| Bone-block decode | `extra` 48-byte HD 3x4 matrix records, then 128 joint slots in 32 four-`vec3` records, then the optional sentinel and 128 flags. 349/349 loose `.SRM` files parse to exact EOF; every file decodes 128 joints |
| Per-vertex bone fields | Three `u8` indices at 20..22 and three `u8` weights at 24..26; weights sum to 255; indices stay below 128 (observed max 67) |
| Validation | Orthonormal matrix bases within `1e-3` (worst real file `2.2e-5`), index bound, weight-sum invariant, `MAX_EXTRA_BONES` 4096 ceiling, truncation stays loud |
| Deformation math | Pure one-bone linear blend skinning in mesh space: `p' = sum(w_i * (M_i * p)) / sum(w_i)`. Joints are Z-negated per design P5; axis derives from the adjacent slot; degenerate direction falls back to `+X`; zero angle is bit-exact rest |
| CLI | `--mesh <FILE> --pose <bone>:<degrees>` prints `pose bone B, D deg, V vertices moved`, then shows the deformed mesh through the unchanged auto-rotating flat-shaded path. No `render.rs` change |
| Refusals | Malformed spec, bone outside `0..=127`, unweighted slot, `extra > 0` (matrix/joint relation UNKNOWN), `--pose` with another mode, and truncated SRM all fail loud before a window |
| Gates | 208/208 tests, zero warnings, repository private, legal clean |

## NOT delivered

| Item | Why it is blocked |
|---|---|
| Coherent full-body posing | `.SRM` has no hierarchy: 128 independent slots, no parent indices, no child lists, no per-slot bind transforms, no keyframes, no clips. A rotated slot's children do not follow |
| Articulated limb/body animation | Requires the parent/child relation and/or clip data that the file does not supply |
| Clip playback (`.SRA`/`.SRL`) | 227 `.SRA` siblings are still undecoded; no community parser found |
| `extra > 0` HD matrix semantics | Shape decoded, relation to the 128 joints UNKNOWN; `--pose` refuses this class rather than guessing |
| Joint/chain overlay, bone names | Out of scope for this slice |

## Accepted playtest evidence (verbatim)

| Run | User observation |
|---|---|
| `7:0` (control) | normal rest mesh |
| `7:45` (940 vertices) | looks good except one leg edge looks odd |
| `12:45` (5,215 vertices) | clearly broken/scattered |

Model: the extra=0 humanoid named `RAZIEL_1.SRM` in the PR body (relative
name only; the real path is resolved against the user's own install at
runtime). The 0-degree control is the important negative control: it proves
the rest path is untouched before any rotation is judged.

## Structural explanation: why `7:45` and `12:45` behave differently

There is no hierarchy in the `.SRM`, so the pose has no way to propagate:

1. A "bone" in this slice is a weighted vertex cluster, not a parent in a
   chain. `pose::apply` rotates exactly the vertices whose weight points at
   the target slot; every other slot keeps its rest position.
2. `7:45` moves a small cluster (940 vertices). The rotated region is
   internally consistent, so the result reads as a local deformation. The one
   odd leg edge is the seam where the rotated cluster meets neighbours that
   did not move.
3. `12:45` moves the largest single cluster (5,215 vertices). A large region
   rotates away from adjacent regions that cannot follow, so the silhouette
   scatters. The more the slice "looks like a limb", the worse the tear.
4. Therefore this is a structural limit, not a defect in the blend. The math
   does exactly what the data supports; the data has no parent/child edges to
   carry a chain.

The honest sentence for the README: **animation is NOT solved** — the bone
decode and one-bone deformation are real, but full-body posing needs data the
`.SRM` does not carry.

## Process wins worth recording

- **The 3-PR chain held the 400-line ceiling.** Authored lines were 339
  (PR #37), 323 (PR #38) and 370 (PR #39) — every PR under 400, with PR #39
  the closest. The aggregate (~1,032) exceeded the 800-line session review
  budget, which is exactly why the chain was fixed at three PRs up front
  instead of chosen during apply.
- **Tests-as-their-own-line forecast rule worked.** The two prior slices
  missed their forecasts by 3.7x and 3.9x because test lines were
  under-budgeted. Here tests were budgeted as a first-class line in
  `tasks.md`; the forecast (~950) landed within about 9% of actual (~1,032),
  and no PR repeated the multiplier miss.
- **Zero-angle exactness was caught by real data, not by unit tests alone.**
  The first wired version reported 8,503 moved vertices at `7:0` because
  renormalised weights shifted rest coordinates by a few f32 ulp. A
  magnitude-relative epsilon kept the exact rest position and made the moved
  count truthful (`0` at zero angle, `940` for bone 7 at 45 degrees).
- **Refusal-before-window held.** Every bad class exited 1 with no GPU window,
  including `extra > 0` on a real file and `--pose` combined with `--tshow`.

## Details

| Topic | Outcome |
|---|---|
| PR chain | [#37 — feat(srm): decode the SRM bone block](https://github.com/libertus21/soul-reaver-remaster-rs/pull/37) (MERGED `2026-10-27T03:23:42Z`, +835/−18, 9 files), [#38 — feat(pose): add pure one-bone deformation math](https://github.com/libertus21/soul-reaver-remaster-rs/pull/38) (MERGED `2026-10-27T03:38:49Z`, +323, 2 files), [#39 — feat(anim): wire --pose into the mesh path](https://github.com/libertus21/soul-reaver-remaster-rs/pull/39) (MERGED `2026-10-27T04:04:37Z`, +339/−31, 3 files) |
| Commits on `main` | `67b89c7` decode the bone block → `8d94b65` report skeleton counts → `c6a0589` pose math → `669a7c3` keep zero-angle rest exact → `f43f2c5` wire `--pose` → `18afcf4` README → `d1cd70f` merge of PR #39 |
| Format facts (census) | 349 loose `.SRM` files; 349/349 exact EOF; `extra` distribution 342 × 0, 1 × 1, 1 × 7, 1 × 17, 1 × 25, 3 × 26; 128 joints per file; 7/7 `extra > 0` files refuse pose |
| Pose math | One-bone LBS in mesh space; joint Z negation (P5); head = target slot, tail = adjacent slot in the same four-slot record; axis = `normalize(cross(tail - head, +Z))` with `+X` fallback; duplicate target indices sum; zero-weight and other-slot vertices unchanged; moved count uses a magnitude-relative epsilon |
| Deliberately not built | Hierarchy/parenting, clip playback, `.SRA`/`.SRL` decode, `extra > 0` matrix interpretation, joint overlay, bone names, export, new crates |
| Playtest gate | The slice passes its own gate (deformation visible, rest path unchanged, Esc exits 0) but FAILS the intent of "pose the character". The archive records the partial outcome rather than relabeling it as success |
| Process warnings (recorded) | (1) A one-bone rotation of a large cluster can look worse than a small one; the playtest correctly exposed this. (2) `--pose` is a diagnostic of the decode, not an animation feature. (3) Full-body work must start from `.SRA`, not from larger angles on the same slots |
| Branch cleanup | Deleted only branches whose content is fully in `main`; see the report below |
| Follow-ups | (1) Scope `.SRA` (227 files): counts, magic, structural invariants, clip/hierarchy evidence. (2) Joint/chain overlay as a visual debug aid. (3) `extra > 0` matrix/joint relation. (4) Ghidra only if `.SRA` static work stalls |
| Legal | Format and behavior facts only. No game bytes, no extracted files, no coordinates, no hashes, no absolute game paths. Tests use synthetic bytes in temp directories; the real model is a read-only playtest input resolved at runtime |

## Branch cleanup report

Containment was checked with `git merge-base --is-ancestor <ref> main`. For a
branch that is not an ancestor, content was compared with
`git diff <ref> main -- <paths the branch touched>`; an empty diff (or a diff
that only adds main-side content) means the branch has nothing `main` lacks.

| Branch | State | Action |
|---|---|---|
| `feature/animation-decode` | ancestor of `main` (tip `8d94b65`) | Deleted local + remote |
| `feature/animation-pose` | ancestor of `main` (tip `c6a0589`) | Deleted local + remote |
| `feature/animation-cli` | ancestor of `main` (tip `18afcf4`) | Deleted local + remote |
| `feature/animation-plan` | not an ancestor (planning commit `6544c88`); all five planning files are in `main`, and the only delta is the post-plan decode ledger that `main` added to `docs/notes/srm-bone-block.md` | Deleted local + remote |

## Checklist

- [x] PR #39 MERGED and `main` contains the full chain (HEAD `d1cd70f`)
- [x] Stage banner states PARTIAL in plain words; no claim that animation works
- [x] Delivered vs NOT delivered split is explicit
- [x] Playtest evidence recorded verbatim (`7:0`, `7:45`, `12:45`)
- [x] Structural cause stated: no hierarchy in `.SRM`, so children cannot follow
- [x] The `7` vs `12` difference explained from the same structural cause
- [x] `.SRA` (227 files) named as the next candidate
- [x] Process wins recorded: 3-PR ceiling (339/323/370) and the tests-line forecast rule
- [x] Contained branches deleted after containment checks
- [x] README "What works" and "What's missing" updated with the same honesty
- [x] Note updated with a Limitation section tied to the observed behavior
- [x] No game bytes, coordinates, hashes, or absolute game paths added by this archive

## Next step

Plan `.SRA` scoping: 227 sibling files, a count-like first word, no text magic,
no community parser. Start with a read-only census (counts, size classes,
first-word shape, structural invariants) and only then decide whether the
files carry clips, hierarchy, or both. Do not extend the one-bone pose until
the data supplies a parent/child relation; larger angles on independent slots
will only tear the mesh harder.

# object-up-axis - the object-model local vertical is Z

Date: 2026-11-09. Read-only corpus probe against the player's own copy plus the
production fix. Behaviour and counts only: no game bytes, no decompiled text and
no install paths live in this repository. The probe ran in memory and was
removed before commit.

## Verdict

**CONFIRMED: the object-model local up axis is Z (index 2), `+Z` up.** The
archived object containers and the loose `.SRM` meshes share this vertical, and
it is the same vertical as the confirmed Z-up world (`docs/notes/up-axis.md`).

Consequences:

- The **object viewers** (`--object-mesh`, `--object-mesh --textures`,
  `--mesh`, `--mesh-textured`, `--mesh-all`) now render with the Z-up camera
  basis. Before this change they used a Y-up basis, which laid a Z-up humanoid
  on its back.
- **Placement needs no rotation.** Because `OBJECT_UP == UP` and
  `OBJECT_UP_SIGN == UP_SIGN`, a placed model is already upright under the
  existing translation. `--level-render --objects` and the engine scene stay a
  pure translation, now proven by a test instead of assumed.
- The shared definition lives in `src/engine/axis.rs`
  (`OBJECT_UP`, `OBJECT_UP_SIGN`); `render::SceneUp::object_local()` derives its
  choice from those constants. No second literal exists on the render path.

## Method

A temporary `#[ignore]` test decoded every object container in the archive
through the existing `object::parse_object` parser and measured the rest pose
(`object::rest_positions`, which applies the 24-byte bone translation chain).
It reported, per model: the rest-pose bounding-box span on each local axis, the
span of the accumulated bone-chain offsets, and the dominant face-normal axis
of every non-degenerate triangle. It then parsed four loose `.SRM` humanoids
through `srm::load` as an independent asset-family cross-check. The probe was
reverted before commit; the numbers below are its output.

Corpus size: **394 distinct archived object models** parsed; **310** have at
least 4 vertices and 4 polygons and were used for the corpus tally.

## Evidence

### 1. Aspect ratio: standing humanoids are tallest along local Z

A standing figure is taller than it is wide. Every measured humanoid is longest
along index 2 in the rest pose.

| Model | Vertices | Polygons | Bones | Span X | Span Y | **Span Z** | Longest |
|---|---:|---:|---:|---:|---:|---:|---|
| `raziel` | 299 | 640 | 66 | 180 | 370 | **506** | Z |
| `morlock` | 178 | 334 | 37 | 412 | 230 | **686** | Z |
| `skinner` | 173 | 317 | 35 | 198 | 114 | **568** | Z |
| `sluagh` | 135 | 272 | 24 | 210 | 170 | **483** | Z |
| `ronin` | 164 | 286 | 24 | 297 | 241 | **550** | Z |
| `kain` | 335 | 652 | 49 | 235 | 191 | **659** | Z |
| `hunter` | 207 | 330 | 44 | 186 | 119 | **495** | Z |
| `vlgra` | 230 | 284 | 44 | 145 | 89 | **513** | Z |
| `ariel` | 206 | 348 | 43 | 131 | 89 | **534** | Z |

9/9 humanoids: local Z is the long axis. The two horizontal spans are the
arm span and the body depth, which is the expected shape of a T-pose humanoid.

### 2. Bone-chain direction: the chain extends along local Z

The accumulated bone translation (`bone.local` plus every ancestor) is widest on
Z for the same 9/9 models. The root sits at the feet/hips and the extremities
extend along the vertical, exactly as an upright armature implies.

| Model | Bone-chain span X | Span Y | **Span Z** | Longest |
|---|---:|---:|---:|---|
| `raziel` | 146 | 328 | **475** | Z |
| `morlock` | 273 | 140 | **659** | Z |
| `skinner` | 163 | 48 | **508** | Z |
| `sluagh` | 177 | 42 | **431** | Z |
| `ronin` | 191 | 193 | **479** | Z |
| `kain` | 208 | 133 | **656** | Z |
| `hunter` | 125 | 67 | **458** | Z |
| `vlgra` | 125 | 67 | **458** | Z |
| `ariel` | 131 | 79 | **628** | Z |

### 3. Normal distribution: Z is the vertical, not the facing axis

For every non-degenerate triangle the dominant absolute normal axis was tallied.
On the world, Z dominated because rooms are full of horizontal floors and
ceilings (`up-axis.md`). A standing character has the opposite signature: its
large surfaces face horizontally (chest/back, arms, legs) and only a few face
vertically (top of the head, feet), so the **vertical axis is the one dominated
by the fewest normals**.

| Model | Dominant X | Dominant Y | **Dominant Z** |
|---|---:|---:|---:|
| `raziel` | 347 | 204 | **89** |
| `morlock` | 144 | 142 | **48** |
| `skinner` | 145 | 141 | **31** |
| `sluagh` | 118 | 112 | **42** |
| `ronin` | 130 | 112 | **44** |
| `kain` | 300 | 278 | **74** |
| `hunter` | 149 | 123 | **58** |
| `vlgra` | 134 | 100 | **50** |
| `ariel` | 155 | 163 | **30** |

9/9: Z is the least-dominant normal axis. The near-horizontal set (the engine's
own `n_A^2 >= 3 * (other squares)` test) is likewise smallest on Z for every
model (e.g. `raziel` near-horizontal counts X 163 / Y 101 / **Z 37**). This
rules out the old Y-up assumption: Y is strongly dominant, so Y cannot be the
vertical of a figure whose large surfaces face horizontally.

### 4. Independent asset-family cross-check (`.SRM`)

The loose remaster `.SRM` meshes are a separate asset family from the archived
object containers. Four humanoids were parsed directly and show the same shape
and the same Z-up convention. `RAZIEL_1.SRM` even reproduces the archived
silhouette closely (514 tall versus 506).

| `.SRM` | Vertices | Triangles | Span X | Span Y | **Span Z** | Longest |
|---|---:|---:|---:|---:|---:|---|
| `RAZIEL_1.SRM` | 20,239 | 36,000 | 184 | 359 | **514** | Z |
| `MORLOCK_1.SRM` | 10,242 | 16,226 | 404 | 191 | **685** | Z |
| `KAIN_1.SRM` | 17,131 | 29,500 | 256 | 201 | **664** | Z |
| `SKINNER_1.SRM` | 8,128 | 13,421 | 197 | 134 | **565** | Z |

### 5. Non-character control set

The full corpus (310 models with at least 4 vertices and 4 polygons) has
longest-axis counts **X 102 / Y 57 / Z 151**. That tally is not a verdict on its
own: most of the corpus is small props, doors, lights and effects whose bounding
box is shaped by how the prop is modelled, not by a body plan. They are excluded
from the character claim rather than forced to agree.

One sprawling creature, `vwraith` (1,112 long on X, 796 on Z), is X-longest and
X-longest in its bone chain; it is not a bipedal humanoid and is excluded as
non-character for this check. Its orientation still follows the same local frame;
the classifier here is simply not applicable to it.

## Why no placement rotation

`src/engine/axis.rs` holds the single definition:

- `OBJECT_UP = 2`, `OBJECT_UP_SIGN = 1`;
- `UP = 2`, `UP_SIGN = 1`.

The two are equal, so a placed model's local up already points along world up.
`level::append_objects` and `engine::backend_wgpu` add the record's integer world
position to each model vertex and apply no rotation. That is correct, and it is
now asserted by
`level::tests::a_placed_model_keeps_its_local_up_aligned_with_the_world_up`.

If a future asset family stores a different local vertical, the fix is a
documented transform driven by `OBJECT_UP`, not a sign flip hidden in a viewer.

## Viewer fix

`render::SceneUp::object_local()` used to return `YUp` because the axis was
UNKNOWN. It now derives its answer from `engine::axis::OBJECT_UP` and returns
`ZUp`. Every object viewer takes `SceneUp` as a required parameter, so the change
reaches `--object-mesh`, `--object-mesh --textures`, `--mesh`,
`--mesh-textured` and `--mesh-all` through one named constructor.

## Determinism

The change is render-only for the viewers and does not alter the placement
transform (which was already a translation and remains one). The headless
digests are unchanged:

| Unit | 60-tick digest | Before | After |
|---|---|---|---|
| `under1` | `ede39c540a10ee39` | same | same |
| `skinnr1` | `398d1870a463fd7a` | same | same |

No golden moved. The read-only capabilities' text output is unchanged: the
`--object-info` lines are byte-identical before and after (the change touches no
text-producing code path).

## Tests

- `engine::axis::tests::the_object_local_up_axis_matches_the_world_up_axis`
  pins `OBJECT_UP == UP` and the sign match.
- `object::tests::a_humanoid_fixture_is_tallest_along_the_object_local_up_axis`
  asserts the classifier picks `OBJECT_UP` on a standing fixture, not a literal.
- `render::mesh_tests::level_and_object_local_camera_rigs_both_use_z_up` proves
  both object cameras are Z-up and `SceneUp::object_local() == SceneUp::ZUp`.
- `level::tests::a_placed_model_keeps_its_local_up_aligned_with_the_world_up`
  proves a placed model keeps local `+Z` on world `+Z`.

## UNKNOWN (unchanged)

The local **facing** axis (front/back) is not pinned: the normal data shows it is
one of X or Y but does not separate the two. No rotation about the vertical is
applied or claimed; the level record carries no orientation
(`docs/notes/level-geometry.md`, P5).

# legacy-viewers-up-axis - which viewer renders with which world up

Date: 2026-11-09. The world convention is already confirmed and fixed
(`docs/notes/up-axis.md`, `src/engine/axis.rs`: `UP = 2`, `+Z` up). This note
records how the **legacy viewers** choose their camera up axis. The object-model
local up axis was proven afterwards (`docs/notes/object-up-axis.md`): it is also
Z-up, so every viewer now uses the Z-up basis.

## Verdict

The renderer supports an explicit up axis: `render::SceneUp` (`YUp` / `ZUp`).
Every legacy viewer entry point takes it as a required parameter, so the choice
is visible at the call site. There is no hidden default on the render path.

| Legacy path | Data domain | Up | Call site |
|---|---|---|---|
| `--level-render <PATH>` | room/level world space | **ZUp** | `run_level_render` -> `SceneUp::world()` |
| `--level-render ... --textures` | room/level world space | **ZUp** | `run_level_render_textured` -> `SceneUp::world()` |
| `--level-render ... --objects [--textures]` | mixed (world rooms + placed models) | **ZUp** | same two call sites |
| `--object-mesh --bigfile <DAT> --entry <I>` | object-model local space | **ZUp** | `run_object_mesh` -> `SceneUp::object_local()` |
| `--object-mesh ... --textures` | object-model local space | **ZUp** | `run_object_mesh_textured` -> `SceneUp::object_local()` |
| `--mesh <SRM>` | `.SRM` object-model local space | **ZUp** | `run_mesh` -> `SceneUp::object_local()` |
| `--mesh-textured <SRM>` | `.SRM` object-model local space | **ZUp** | `run_mesh_textured` -> `SceneUp::object_local()` |
| `--mesh-all <DIR>` | `.SRM` object-model local space | **ZUp** | `show_mesh_gallery` (`SceneUp::object_local()`) |
| engine `--play` | room/level world space | **ZUp** | already correct (`src/engine/play.rs`) |

## Why the object viewers now use Z-up

The shipped room data and the object-model local space are both **confirmed
Z-up** (`docs/notes/object-up-axis.md`): 9/9 humanoids are tallest along local Z
and their bone chains extend along Z, and the `.SRM` humanoids agree. The object
viewers must therefore use the Z-up basis.

`SceneUp::object_local()` derives its answer from
`engine::axis::OBJECT_UP`, so a future axis change updates one named constant,
not a scattered sign flip. Keeping the render layer defaultless is what makes
the next regression hard: `CameraRig` cannot create an `OrbitCamera` without a
`SceneUp`.

## The mixed case: `--level-render --objects`

`--level-render --objects` merges two things into one vertex/index buffer:

- the room geometry, which is **confirmed Z-up world space**;
- one real placed-object model per placed record, or a cube marker when the
  name does not resolve. Placed models are appended with a **translation to the
  record's world position and no rotation** (`level.rs::append_objects` /
  `append_objects_textured`; the record carries no orientation).

The scene is therefore world space: it is dominated by, and anchored to, the
confirmed Z-up room. The required camera up is **ZUp** for the whole scene.
Both the room and the placed models are Z-up, so the placed models are upright
under their translation (`docs/notes/object-up-axis.md`). The placement path is
still a translation with no rotation; that is now a proven-correct transform,
not an assumption.

## Determinism

The camera choice is render-only. It never writes simulation state and never
touches the canonical fold, so the headless digests are unchanged:

- `--play --headless --bigfile <DAT> --unit under1 --ticks 60` =
  `ede39c540a10ee39`;
- `--play --headless --bigfile <DAT> --unit skinnr1 --ticks 60` =
  `398d1870a463fd7a`.

No golden moved. The text output of every read-only capability is
byte-identical before and after; the change only reaches the interactive window.

## Tests

- `render::mesh_tests::level_and_object_local_camera_rigs_both_use_z_up` builds
  both rigs and asserts every orbit and fly camera reports `+Z` up, and that
  `SceneUp::object_local() == SceneUp::ZUp`.
- `render::mesh_tests::initial_fly_camera_applies_the_scene_up_choice` proves
  the flag reaches the fly eye for both the world and object choices.
- The existing `z_up_cameras_use_world_z_as_up_and_rise_along_it` and
  `z_up_looking_at_faces_the_target_with_world_z_up` still pin the camera math.

## UNKNOWN (unchanged)

The local **facing** axis (front/back) is not pinned; see
`docs/notes/object-up-axis.md`. The vertical is confirmed Z-up.

# legacy-viewers-up-axis - which viewer renders with which world up

Date: 2026-11-09. The world convention is already confirmed and fixed
(`docs/notes/up-axis.md`, `src/engine/axis.rs`: `UP = 2`, `+Z` up). This note
records how the **legacy viewers** now choose their camera up axis, and why the
object-model viewers deliberately do not follow the world.

## Verdict

The renderer supports an explicit up axis: `render::SceneUp` (`YUp` / `ZUp`).
Every legacy viewer entry point takes it as a required parameter, so the choice
is visible at the call site. There is no hidden default on the render path.

| Legacy path | Data domain | Up | Call site |
|---|---|---|---|
| `--level-render <PATH>` | room/level world space | **ZUp** | `run_level_render` -> `SceneUp::world()` |
| `--level-render ... --textures` | room/level world space | **ZUp** | `run_level_render_textured` -> `SceneUp::world()` |
| `--level-render ... --objects [--textures]` | mixed (world rooms + placed models) | **ZUp** | same two call sites |
| `--object-mesh --bigfile <DAT> --entry <I>` | object-model local space | **YUp** | `run_object_mesh` -> `SceneUp::object_local()` |
| `--object-mesh ... --textures` | object-model local space | **YUp** | `run_object_mesh_textured` -> `SceneUp::object_local()` |
| `--mesh <SRM>` | `.SRM` object-model local space | **YUp** | `run_mesh` -> `SceneUp::object_local()` |
| `--mesh-textured <SRM>` | `.SRM` object-model local space | **YUp** | `run_mesh_textured` -> `SceneUp::object_local()` |
| `--mesh-all <DIR>` | `.SRM` object-model local space | **YUp** | `show_mesh_gallery` (`SceneUp::object_local()`) |
| engine `--play` | room/level world space | **ZUp** | already correct (`src/engine/play.rs`) |

## Why the object viewers stay Y-up

The shipped room and object data is Z-up, but an **object model's own local up
axis is not established** (`docs/notes/object-models.md`, section 6, item 9:
where a placed object's world orientation is composed is UNKNOWN; the level
record carries a position only, and the vertices are model-local). Rotating
those vertices into a guessed frame would launder an unproven assumption into
the viewer.

The chosen option is **(b)**: an explicit, typed parameter at every entry point,
with `SceneUp::object_local()` documented as the object-model default (`YUp`).
It is stronger than leaving an implicit Y-up default because each viewer now
*states* its domain, and a future proof of the object axis changes one named
constructor instead of hunting for hidden camera state. Keeping the render
layer defaultless is what makes the next regression hard: `CameraRig` cannot
create an `OrbitCamera` without a `SceneUp`.

## The mixed case: `--level-render --objects`

`--level-render --objects` merges two things into one vertex/index buffer:

- the room geometry, which is **confirmed Z-up world space**;
- one real placed-object model per placed record, or a cube marker when the
  name does not resolve. Placed models are appended with a **translation to the
  record's world position and no rotation** (`level.rs::append_objects` /
  `append_objects_textured`; the record carries no orientation).

The scene is therefore world space: it is dominated by, and anchored to, the
confirmed Z-up room. The required camera up is **ZUp** for the whole scene.
Using Y-up here is exactly the old bug that rolled the room 90 degrees. Any
misorientation of a placed model is a separate object-model issue (the local
axis is still UNKNOWN) and must not be "fixed" by rolling the world camera:
that would trade a known-correct room for a guess.

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

- `render::mesh_tests::level_camera_rig_uses_z_up_and_object_local_rig_stays_y_up`
  builds the level rig and asserts both the orbit and fly cameras report `+Z`
  up (and the object rig stays `+Y`).
- `render::mesh_tests::initial_fly_camera_applies_the_scene_up_choice` proves
  the flag reaches the fly eye.
- The existing `z_up_cameras_use_world_z_as_up_and_rise_along_it` and
  `z_up_looking_at_faces_the_target_with_world_z_up` still pin the camera math.

## UNKNOWN (unchanged)

The object-model (`.SRM` and archived object container) local up axis. Until it
is proven, object viewers keep `YUp` and this note stays the reminder.

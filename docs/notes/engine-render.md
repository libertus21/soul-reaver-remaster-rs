# Engine render scene and backend (T9)

T9 is the first slice that produces something visible. It splits rendering into
two pieces that meet at a CPU contract:

- `engine::scene` builds a `RenderScene` from the live list and the loaded
  level. It never imports `wgpu` or `winit`.
- `engine::render_backend` defines `RenderBackend` and the headless
  `NullBackend`.
- `engine::backend_wgpu` implements the trait with wgpu, merging one CPU mesh
  per frame and reusing the existing textured shader/pipeline.

`engine::play` owns the window loop, the camera modes and the headless tick
loop. `main` maps a texture-page failure to exit code 2.

## Scene contract

`RenderScene` carries:

- `camera: CameraView` - a column-major MVP already in render space;
- `transform: SceneTransform` - the unit-extent normalisation;
- `items: Vec<DrawItem>` - draw order, each with `mesh`, integer `world`
  translation, `visible` and `class`;
- `meshes: Vec<MeshData>` - shared source meshes with scene-wide material
  indices;
- `material_pages: Vec<Option<u16>>` - material 1 is the flat fallback
  (`None`), later entries are `TEXTURES.BIG` pages in first-use order.

Handles are stable: the room is handle 0, live-object names get `1..` in
first-use live order, and duplicate names share one handle. The level mesh is
the room geometry; live objects are resolved by name through the existing
object parser and textured-mesh builder. An unresolved name gets a stable empty
handle rather than changing the handle numbering.

## Determinism

Scene construction is a pure function of the live state and the static assets:

- the live arena is walked in live order, never in hash order;
- name-to-handle and page-to-material maps are `BTreeMap`s used for lookup
  only, never iterated to produce order;
- the material table is filled in first-use order;
- no wall clock, RNG or `Instant` participates.

`the_same_live_state_builds_the_same_scene_twice` builds the same state twice
and compares the whole scene (items, handles, meshes, material pages). It fails
if the live walk becomes unordered; the PR evidence records the temporary
`HashMap` perturbation and its failure.

The scene does not change the canonical `WorldState` fold. **All seven existing
determinism goldens are untouched by T9.** The new check is a separate,
independent gate in `scene.rs` tests.

## Reuse, not a fork

The room mesh, object meshes, page decoding and camera math come from the
existing asset layer and `src/render.rs`:

- `level::read_entry_room_mesh_textured` / `read_entry_room_mesh`;
- `object::ModelResolver::textured`;
- `object_texture::{PageBank, scene, flat_textures}`;
- `render::{fit_bounds, OrbitCamera, FlyCamera, view_proj, mat_bytes,
  open_core, upload_rgba_texture}`;
- `render::{textured_uniform_layout, textured_texture_layout,
  build_textured_pipeline}`.

The last three functions were extracted from the viewer's textured setup so the
viewer and the engine call one pipeline definition. The extraction is
mechanical: the shader source, vertex layout, depth state, blend state and
texture layout are identical, and the viewer's argument handling and output are
unchanged.

## Fail-fast texture pages

`SceneAssets::load` decodes every room page before a window exists. The play
driver then builds the initial scene and decodes every page it references,
including the player model. A missing or out-of-range page is a
`RunError::Texture` (exit code 2) reported before the event loop is created.

## Runtime coverage

Verified in the development environment:

- headless `under1` run completes the fixed tick loop through `NullBackend`;
- a two-page synthetic `TEXTURES.BIG` with the real archive exits 2 with the
  exact out-of-range page, before any window;
- the four legacy CLI argument-parsing outputs are byte-identical before and
  after;
- headless player movement is measured, not assumed (see below).

Not verified on the development machine (no interactive display):

- the actual window presentation and the visible textured frame;
- mouse orbit/pan/zoom in the window;
- camera drag orbit and the visual frame in player mode.

These remain manual playtest items for the milestone report.

## Player movement diagnostic

`--play --headless --input-script <SPEC>` runs a bounded, deterministic input
sequence and prints one parseable line. `SPEC` is a comma-free sequence of
`button:ticks` pairs applied from tick 0, where `button` is `forward`, `back`,
`left`, `right`, `jump` or `action`. Each step presses at its first tick and
releases at the boundary tick, so the run length is exact. A script always runs
to its own end; an explicit `--ticks` may only extend the idle tail.

```
movement ticks=60 start=0,0,0 end=0,240,0 dx=0 dy=240 dz=0 blocked_ticks=0 grounded=1 player_bound=1
```

- `start`/`end`/`dx dy dz` are integer world units (`Z` is up).
- `blocked_ticks` counts ticks where the sampled movement axis was non-zero but
  the committed horizontal delta was zero. It separates "input never arrived"
  (zero, player moves) from "collision refused the step" (equal to the moving
  ticks).
- `player_bound` is `FrameState::player()` presence; `grounded` is the vertical
  resolver's last result.
- `--player-pos X,Y,Z` overrides the authored record before the first tick. It
  is a test aid, not gameplay, and is the escape hatch for a bad spawn without
  editing level data.

The diagnostic deliberately measures the same fixed ticks as the acceptance
run: text can only come from stdout, and no wall clock, RNG or window is
involved. It runs only with `--headless`.

### The `under1` finding

The authored `raziel` record is the only one in the real archive and sits at
`0,0,0`. Before the fix, `forward:60` printed:

```
movement ticks=60 start=0,0,0 end=-13,33,0 dx=-13 dy=33 dz=0 blocked_ticks=0 grounded=1 player_bound=1
```

A 300-tick run ended at the same `(-13, 33)`: the input reached the simulation,
but the horizontal sweep slid back and forth inside a pocket of walls. The
cause was not the spawn point. The collision mesh projected every near-vertical
triangle to the horizontal plane without keeping its vertical extent, so walls
from the rooms stacked above and below `0,0,0` became phantom barriers at that
`X`/`Y`.

`WallTriangle` now keeps `min_up`/`max_up`, and `first_contact` skips a wall
whose source triangle does not overlap the body span
`[feet, feet + PLAYER_HEIGHT_UNITS]`. After the fix the same command prints:

```
movement ticks=60 start=0,0,0 end=0,240,0 dx=0 dy=240 dz=0 blocked_ticks=0 grounded=1 player_bound=1
```

The full 4-units-per-tick translation now commits from the authored spawn in
every direction, and the default headless digest is unchanged because no input
is applied without a script.

Known limitation: the collision body height is the documented synthetic
`PLAYER_HEIGHT_UNITS` (48), so a wall whose bottom edge starts more than 48
units above the feet is not a barrier even though the visible model is ~509
units tall. Real walls sit on the floor, so the overlapping case still blocks.

## Player camera contract

`CameraMode::Player` is presentation only: it reads
`FrameState::player_position()` and `FrameState::player_yaw_binary()` and writes
no simulation state, so it cannot move a golden.

- The boom follows the produced yaw. The eye sits on the horizontal circle
  behind the player (`-forward`, where forward is the direction the committed
  movement delta turned the body), so walking forward keeps the camera behind
  him. Before the player first moves, the fallback yaw is `+Y` forward, which
  places the boom on `-Y` like the previous fixed camera.
- `PLAYER_CAMERA_DISTANCE` (900), `PLAYER_CAMERA_HEIGHT` (520) and
  `PLAYER_LOOK_HEIGHT` (280) are world-unit constants. The model is ~509 units
  tall and the MVP uses a 45° vertical field of view, so 900 frames ~745 units
  of height at the look plane and 280 centres the body at mid-torso. The old
  600/400/60 boom aimed at the ankles and cropped the torso and head.
- `--cam-distance <N>` (100..=5000) and `--cam-height <N>` (0..=5000) override
  the two scalars; out-of-range and non-numeric values are typed parse errors.
- In player mode, mouse drag orbits the boom: horizontal drag adds a yaw offset,
  vertical drag adds a pitch offset clamped to ±1.2 rad, and neither touches
  `FrameState`. Pitch raises or lowers the eye by `distance * sin(pitch)`; the
  horizontal radius shrinks by `cos(pitch)` so the boom stays a rigid arc.

`the_player_camera_puts_the_eye_behind_the_player_relative_to_its_yaw` rotates
the yaw by 90° and asserts the eye rotates with it; the defaults are also
asserted to frame the 509-unit body from feet to head.


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
  after.

Not verified on the development machine (no interactive display):

- the actual window presentation and the visible textured frame;
- mouse orbit/pan/zoom and fly-camera movement in the window;
- player-mode movement in the window.

These remain manual playtest items for the milestone report.

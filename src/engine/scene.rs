//! CPU render scene: a pure description of what a backend should draw.
//!
//! This module deliberately does not import `wgpu` or `winit`. It turns the
//! live list plus the loaded level into [`RenderScene`], which a backend turns
//! into GPU work. The split keeps the simulation deterministic and testable
//! without a window.
//!
//! Scene construction is a pure function of the live state and the static
//! assets: it walks the live arena in live order (never a hash map), assigns
//! mesh handles in first-use order and remaps every local material page into
//! one scene-wide table in first-use order. The same state therefore produces
//! the same draw order, the same handles and the same material table.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::engine::class::RenderClass;
use crate::engine::live::LiveArena;
use crate::level::{self, LevelError};
use crate::object::{self, ModelResolver, ObjectError};
use crate::object_texture::{self, PageBank};
use crate::render;
use crate::textures::TexturesError;

/// Stable identity of one mesh inside a [`RenderScene`].
///
/// Handles are assigned deterministically: the level mesh is always `0` when a
/// room is present, then live-object meshes get `1..` in first-use live order.
/// Duplicate names share one handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshHandle(pub u32);

impl MeshHandle {
    /// The level/room mesh handle.
    pub const LEVEL: Self = Self(0);

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// One source mesh in model space (objects) or world space (the room).
///
/// `material_indices` are 1-based into this mesh's own `pages`; value `1` is
/// the flat fallback. The scene builder remaps them into the scene-wide table.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SourceMesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[u8; 2]>,
    pub indices: Vec<u32>,
    pub material_indices: Vec<u16>,
    pub pages: Vec<u16>,
}

impl SourceMesh {
    fn from_room(room: level::RoomMesh) -> Self {
        let count = room.vertices.len();
        let uvs = if room.uvs.len() == count {
            room.uvs
        } else {
            vec![[0, 0]; count]
        };
        let material_indices = if room.material_indices.len() == count {
            room.material_indices
                .iter()
                .map(|&material| u16::from(material))
                .collect()
        } else {
            vec![1; count]
        };
        Self {
            positions: room.vertices,
            uvs,
            indices: room.indices,
            material_indices,
            pages: room.pages,
        }
    }

    fn from_textured(mesh: object::TexturedMesh) -> Self {
        Self {
            positions: mesh.positions,
            uvs: mesh.uvs,
            indices: mesh.indices,
            material_indices: mesh
                .material_indices
                .iter()
                .map(|&material| u16::from(material))
                .collect(),
            pages: mesh.pages,
        }
    }
}

/// One mesh after the scene-wide material remap. `material_indices` are
/// 1-based into [`RenderScene::material_pages`].
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[u8; 2]>,
    pub indices: Vec<u32>,
    pub material_indices: Vec<u16>,
}

/// One draw submission: a mesh plus its integer world translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawItem {
    pub mesh: MeshHandle,
    pub world: [i32; 3],
    pub visible: bool,
    pub class: u8,
}

/// Uniform transform that normalises the merged scene into the same unit
/// extent the legacy viewer frames. Applying it to a world point yields render
/// space, where the reused orbit/fly cameras operate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneTransform {
    pub center: [f32; 3],
    pub scale: f32,
}

impl SceneTransform {
    pub const fn identity() -> Self {
        Self {
            center: [0.0; 3],
            scale: 1.0,
        }
    }

    pub fn apply(&self, point: [f32; 3]) -> [f32; 3] {
        [
            (point[0] - self.center[0]) * self.scale,
            (point[1] - self.center[1]) * self.scale,
            (point[2] - self.center[2]) * self.scale,
        ]
    }
}

impl Default for SceneTransform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Camera view consumed by a backend. The matrix is column-major and already
/// in render space; the scene never reads a clock or a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraView {
    pub mvp: [[f32; 4]; 4],
}

impl CameraView {
    pub const fn identity() -> Self {
        Self {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

impl Default for CameraView {
    fn default() -> Self {
        Self::identity()
    }
}

/// The complete CPU scene: draw order, shared meshes, material table, camera.
///
/// `material_pages[0]` is the flat fallback (`None`); every later entry is a
/// decoded page from `TEXTURES.BIG`. Material index `m` (1-based) is
/// `material_pages[m - 1]`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RenderScene {
    pub camera: CameraView,
    pub transform: SceneTransform,
    pub items: Vec<DrawItem>,
    pub meshes: Vec<MeshData>,
    pub material_pages: Vec<Option<u16>>,
}

impl RenderScene {
    /// Number of materials, including the flat fallback.
    pub fn material_count(&self) -> usize {
        self.material_pages.len()
    }

    /// Visible draw items in scene order.
    pub fn visible_items(&self) -> impl Iterator<Item = &DrawItem> + '_ {
        self.items.iter().filter(|item| item.visible)
    }

    /// Resolve `handle` to its mesh.
    pub fn mesh(&self, handle: MeshHandle) -> Option<&MeshData> {
        self.meshes.get(handle.index() as usize)
    }
}

/// Why a scene could not be loaded or built.
#[derive(Debug)]
pub enum SceneError {
    Level(LevelError),
    Object(ObjectError),
    Textures(TexturesError),
    Empty,
}

impl SceneError {
    /// A texture-page failure must exit 2 before a window opens.
    pub fn is_texture(&self) -> bool {
        matches!(self, Self::Textures(_))
    }
}

impl From<LevelError> for SceneError {
    fn from(error: LevelError) -> Self {
        Self::Level(error)
    }
}

impl From<ObjectError> for SceneError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}

impl From<TexturesError> for SceneError {
    fn from(error: TexturesError) -> Self {
        Self::Textures(error)
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Level(error) => write!(f, "{error}"),
            Self::Object(error) => write!(f, "{error}"),
            Self::Textures(error) => write!(f, "{error}"),
            Self::Empty => write!(f, "the selected unit produced no renderable room geometry"),
        }
    }
}

impl std::error::Error for SceneError {}

/// Static scene assets plus the memoized object-model cache.
///
/// The cache is memoization only: it never changes the scene produced by the
/// same live state, so scene construction stays deterministic.
pub struct SceneAssets {
    level: SourceMesh,
    transform: SceneTransform,
    spawn: Option<[f32; 3]>,
    resolver: Option<ModelResolver>,
    cache: BTreeMap<String, Option<SourceMesh>>,
}

impl SceneAssets {
    /// Load the room geometry and, when requested, validate its texture pages.
    ///
    /// Page validation happens here, before any window exists: a missing or
    /// out-of-range page is a [`SceneError::Textures`] (exit 2). The returned
    /// bank is moved to the wgpu backend once the window opens.
    pub fn load(
        path: &Path,
        entry: usize,
        textures: Option<&Path>,
    ) -> Result<(Self, Option<PageBank>), SceneError> {
        let room = match level::read_entry_room_mesh_textured(path, entry, false, None) {
            Ok((_, room)) => room,
            Err(LevelError::RoomTexturesMissing) => {
                let (_, room) = level::read_entry_room_mesh(path, entry, false, None)?;
                room
            }
            Err(error) => return Err(SceneError::Level(error)),
        };
        let spawn = room.spawn;
        let level = SourceMesh::from_room(room);
        if level.positions.is_empty() || level.indices.is_empty() {
            return Err(SceneError::Empty);
        }
        let (center, scale) =
            render::fit_bounds(&level.positions).map_err(|_| SceneError::Empty)?;
        let bank = match textures {
            Some(path) => {
                let bank = PageBank::open(path)?;
                // Decode every room page now, before the event loop exists.
                let _ = object_texture::scene(&bank, &level.pages)?;
                Some(bank)
            }
            None => None,
        };
        let resolver = ModelResolver::load(path)?;
        Ok((
            Self {
                level,
                transform: SceneTransform { center, scale },
                spawn,
                resolver: Some(resolver),
                cache: BTreeMap::new(),
            },
            bank,
        ))
    }

    pub fn transform(&self) -> SceneTransform {
        self.transform
    }

    /// Raw (pre-transform) first `raziel` position, when the unit places one.
    pub fn spawn(&self) -> Option<[f32; 3]> {
        self.spawn
    }

    /// Fitted spawn position in render space.
    pub fn render_spawn(&self) -> Option<[f32; 3]> {
        self.spawn.map(|point| self.transform.apply(point))
    }

    /// Build the deterministic scene for `live`.
    pub fn build(&mut self, live: &LiveArena, camera: CameraView) -> RenderScene {
        let mut material_pages: Vec<Option<u16>> = vec![None];
        let mut page_indices: BTreeMap<u16, u16> = BTreeMap::new();
        let mut meshes = vec![remap_mesh(
            &self.level,
            &mut material_pages,
            &mut page_indices,
        )];
        let mut items = vec![DrawItem {
            mesh: MeshHandle::LEVEL,
            world: [0, 0, 0],
            visible: true,
            class: RenderClass::Unclassified.code(),
        }];

        let mut handles: BTreeMap<String, MeshHandle> = BTreeMap::new();
        for (_, object) in live.iter() {
            let handle = match handles.get(&object.name) {
                Some(handle) => *handle,
                None => {
                    let data = match self.object_mesh(&object.name) {
                        Some(mesh) => {
                            remap_mesh(&mesh, &mut material_pages, &mut page_indices)
                        }
                        None => MeshData::default(),
                    };
                    let handle = MeshHandle(meshes.len() as u32);
                    meshes.push(data);
                    handles.insert(object.name.clone(), handle);
                    handle
                }
            };
            items.push(DrawItem {
                mesh: handle,
                world: object.position,
                visible: object.visible,
                class: object.class.code(),
            });
        }

        RenderScene {
            camera,
            transform: self.transform,
            items,
            meshes,
            material_pages,
        }
    }

    fn object_mesh(&mut self, name: &str) -> Option<SourceMesh> {
        if let Some(cached) = self.cache.get(name) {
            return cached.clone();
        }
        let mesh = self
            .resolver
            .as_mut()
            .and_then(|resolver| resolver.textured(name))
            .map(SourceMesh::from_textured);
        self.cache.insert(name.to_string(), mesh.clone());
        mesh
    }

    #[cfg(test)]
    fn for_test(level: SourceMesh, objects: &[(&str, SourceMesh)]) -> Self {
        let mut cache = BTreeMap::new();
        for (name, mesh) in objects {
            cache.insert((*name).to_string(), Some(mesh.clone()));
        }
        Self {
            level,
            transform: SceneTransform::identity(),
            spawn: None,
            resolver: None,
            cache,
        }
    }
}

/// Remap one source mesh's local material indices into the scene-wide table.
///
/// First use wins, so the flat fallback is always material 1 and pages follow
/// in mesh-order first-use order. `BTreeMap` lookup is fine; its iteration is
/// never used.
fn remap_mesh(
    source: &SourceMesh,
    material_pages: &mut Vec<Option<u16>>,
    page_indices: &mut BTreeMap<u16, u16>,
) -> MeshData {
    let material_indices = source
        .material_indices
        .iter()
        .map(|&local| {
            if local <= 1 {
                1
            } else {
                let page = source
                    .pages
                    .get(usize::from(local) - 2)
                    .copied()
                    .unwrap_or(0);
                match page_indices.get(&page) {
                    Some(&global) => global,
                    None => {
                        material_pages.push(Some(page));
                        let global = material_pages.len() as u16;
                        page_indices.insert(page, global);
                        global
                    }
                }
            }
        })
        .collect();
    MeshData {
        positions: source.positions.clone(),
        uvs: source.uvs.clone(),
        indices: source.indices.clone(),
        material_indices,
    }
}

#[cfg(test)]
mod tests {
    use super::{CameraView, MeshHandle, RenderScene, SceneAssets, SourceMesh};
    use crate::engine::class::{RenderClass, classify};
    use crate::engine::live::{LiveArena, LiveObject};

    fn mesh(page: u16) -> SourceMesh {
        SourceMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 0.0, 0.0],
            ],
            uvs: vec![[0, 0], [255, 0], [0, 255], [255, 255]],
            indices: vec![0, 1, 2, 1, 2, 3],
            material_indices: vec![1, 1, 1, 2],
            pages: vec![page],
        }
    }

    fn object(name: &str, position: [i32; 3]) -> LiveObject {
        LiveObject {
            name: name.to_string(),
            class: classify(name),
            position,
            visible: true,
            slot: None,
            anim_index: 0,
            actor: None,
        }
    }

    fn arena() -> LiveArena {
        let mut live = LiveArena::new();
        live.spawn(object("monster_wallcr", [10, 0, 0]));
        live.spawn(object("raziel__player", [0, 0, 0]));
        live.spawn(object("monster_wallcr", [20, 0, 0]));
        live
    }

    fn assets() -> SceneAssets {
        SceneAssets::for_test(
            mesh(7),
            &[
                ("monster_wallcr", mesh(11)),
                ("raziel__player", mesh(13)),
            ],
        )
    }

    #[test]
    fn the_level_is_handle_zero_and_live_handles_follow_live_order() {
        let live = arena();
        let mut assets = assets();
        let scene = assets.build(&live, CameraView::identity());

        assert_eq!(scene.items.len(), 4, "room plus three live objects");
        assert_eq!(scene.items[0].mesh, MeshHandle::LEVEL);
        assert_eq!(scene.items[1].mesh, MeshHandle(1), "first live name");
        assert_eq!(scene.items[2].mesh, MeshHandle(2), "second live name");
        assert_eq!(
            scene.items[3].mesh,
            MeshHandle(1),
            "a duplicate name reuses its mesh handle"
        );
        assert_eq!(scene.items[1].world, [10, 0, 0]);
        assert_eq!(scene.items[3].world, [20, 0, 0]);
        assert_eq!(
            scene.items[0].class,
            RenderClass::Unclassified.code(),
            "room geometry is not a live class"
        );
        assert_eq!(scene.items[1].class, RenderClass::Monster.code());
    }

    #[test]
    fn material_one_is_the_flat_fallback_and_pages_follow_first_use() {
        let live = arena();
        let mut assets = assets();
        let scene = assets.build(&live, CameraView::identity());

        assert_eq!(
            scene.material_pages[0], None,
            "material 1 is always the flat fallback"
        );
        assert_eq!(scene.material_pages[1], Some(7), "level page first");
        assert_eq!(scene.material_pages[2], Some(11), "first live page");
        assert_eq!(scene.material_pages[3], Some(13), "second live page");
        assert_eq!(scene.material_count(), 4);

        assert_eq!(
            scene.meshes[0].material_indices,
            vec![1, 1, 1, 2],
            "level vertices keep flat then page 7"
        );
        assert_eq!(
            scene.meshes[1].material_indices,
            vec![1, 1, 1, 3],
            "the second page seen becomes global material 3"
        );
        assert_eq!(
            scene.meshes[2].material_indices,
            vec![1, 1, 1, 4],
            "the third page seen becomes global material 4"
        );
    }

    #[test]
    fn invisible_objects_keep_their_place_and_handle() {
        let live = {
            let mut live = LiveArena::new();
            let mut hidden = object("raziel__player", [1, 2, 3]);
            hidden.visible = false;
            live.spawn(hidden);
            live
        };
        let mut assets = assets();
        let scene = assets.build(&live, CameraView::identity());

        assert_eq!(scene.items.len(), 2);
        assert!(!scene.items[1].visible);
        assert_eq!(scene.items[1].mesh, MeshHandle(1));
        assert_eq!(scene.visible_items().count(), 1);
    }

    #[test]
    fn an_unresolved_name_gets_a_stable_empty_handle() {
        let live = {
            let mut live = LiveArena::new();
            live.spawn(object("marker", [0, 0, 0]));
            live
        };
        let mut assets = SceneAssets::for_test(mesh(7), &[]);
        let scene = assets.build(&live, CameraView::identity());

        assert_eq!(scene.items.len(), 2);
        assert_eq!(scene.items[1].mesh, MeshHandle(1));
        assert!(scene.mesh(MeshHandle(1)).expect("handle exists").positions.is_empty());
    }

    fn scene_snapshot(scene: &RenderScene) -> (Vec<(u32, [i32; 3], bool)>, Vec<Vec<u16>>, Vec<Option<u16>>) {
        (
            scene
                .items
                .iter()
                .map(|item| (item.mesh.index(), item.world, item.visible))
                .collect(),
            scene
                .meshes
                .iter()
                .map(|mesh| mesh.material_indices.clone())
                .collect(),
            scene.material_pages.clone(),
        )
    }

    #[test]
    fn the_same_live_state_builds_the_same_scene_twice() {
        let live = arena();
        let mut assets = assets();
        let first = assets.build(&live, CameraView::identity());
        let second = assets.build(&live, CameraView::identity());

        assert_eq!(first, second, "the whole scene must replay byte for byte");
        assert_eq!(scene_snapshot(&first), scene_snapshot(&second));
        assert_eq!(
            first.items.iter().map(|item| item.mesh).collect::<Vec<_>>(),
            second.items.iter().map(|item| item.mesh).collect::<Vec<_>>()
        );
    }
}

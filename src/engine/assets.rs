//! Asset source seam: model containers, textured meshes and the optional actor
//! profile.
//!
//! [`AssetSource`] is the open backend seam. The milestone has one synchronous
//! implementation, [`ImmediateSource`], which resolves a placed name to its
//! archived object container through the existing bigfile reader and reuses the
//! existing model and actor parsers. [`LoadQueue`] is the **shape** of the later
//! asynchronous source: a deterministic FIFO of requests with no threads, no
//! async runtime and no dependency.
//!
//! A name with no matching container is [`AssetCompletion::Unresolved`], never
//! a panic. A matched container whose actor block is truncated is a loud
//! [`AssetError`], so corruption is not silently reported as "missing".

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::bigfile::{self, BigfileEntry};
use crate::engine::actor::ActorProfile;
use crate::object::{self, ObjectError};

/// One asset request, keyed by the placed-object name.
///
/// The name is the only key: the spawn policy chooses the tree (standard or E3)
/// before the request is built, and the archive reader resolves by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRequest {
    pub name: String,
}

impl AssetRequest {
    /// Build a request for a raw placed name.
    pub fn for_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

/// A loaded model plus the data-derived pieces the engine needs.
///
/// `actor: None` is the normal non-actor case: a door, prop, effect or pickup.
/// The profile is the only bridge between the model data and [`ActorRuntime`].
#[derive(Debug, PartialEq)]
pub struct ModelAsset {
    pub model: object::ObjectModel,
    pub textured: object::TexturedMesh,
    pub actor: Option<ActorProfile>,
}

/// The result of one load. Completions carry their request so a later
/// asynchronous source can match a result back to its caller without a
/// side channel.
#[derive(Debug, PartialEq)]
pub enum AssetCompletion {
    /// The container parsed; the actor profile is present only for actors.
    Loaded(AssetRequest, ModelAsset),
    /// No matching container, so the name is counted, never fatal.
    Unresolved(AssetRequest),
}

impl AssetCompletion {
    pub fn request(&self) -> &AssetRequest {
        match self {
            Self::Loaded(request, _) | Self::Unresolved(request) => request,
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_, _))
    }

    /// The loaded asset, or `None` when the name is unresolved.
    pub fn asset(&self) -> Option<&ModelAsset> {
        match self {
            Self::Loaded(_, asset) => Some(asset),
            Self::Unresolved(_) => None,
        }
    }
}

/// A loud asset-layer failure.
#[derive(Debug)]
pub enum AssetError {
    /// The archive index could not be read.
    Archive(String),
    /// A matched container carries a malformed actor block.
    Object(ObjectError),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive(detail) => write!(f, "cannot read the asset archive: {detail}"),
            Self::Object(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<ObjectError> for AssetError {
    fn from(error: ObjectError) -> Self {
        Self::Object(error)
    }
}

/// The asset backend seam.
///
/// The milestone source is synchronous; a later source may resolve requests
/// from [`LoadQueue`] without changing this signature.
pub trait AssetSource {
    fn load(&self, request: &AssetRequest) -> Result<AssetCompletion, AssetError>;
}

/// Synchronous source over one `BIGFILE.DAT`.
///
/// The index is read once. For each request the suffix map narrows the
/// candidates, the existing container parser confirms the exact name, and the
/// existing actor parser extracts the optional profile. Absent names are
/// unresolved; unreadable candidates are skipped so one bad auxiliary entry does
/// not hide a good match.
pub struct ImmediateSource {
    path: PathBuf,
    entries: Vec<BigfileEntry>,
    by_suffix: BTreeMap<[u8; 4], Vec<usize>>,
}

impl ImmediateSource {
    /// Read the archive index. A missing or malformed index is a loud error.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        let entries =
            bigfile::load_index(path).map_err(|error| AssetError::Archive(error.to_string()))?;
        let mut by_suffix: BTreeMap<[u8; 4], Vec<usize>> = BTreeMap::new();
        for (index, entry) in entries.iter().enumerate() {
            by_suffix.entry(entry.suffix).or_default().push(index);
        }
        Ok(Self {
            path: path.to_path_buf(),
            entries,
            by_suffix,
        })
    }
}

impl AssetSource for ImmediateSource {
    fn load(&self, request: &AssetRequest) -> Result<AssetCompletion, AssetError> {
        let Some(key) = suffix_key(&request.name) else {
            return Ok(AssetCompletion::Unresolved(request.clone()));
        };
        let Some(candidates) = self.by_suffix.get(&key) else {
            return Ok(AssetCompletion::Unresolved(request.clone()));
        };
        for &index in candidates {
            let Some(entry) = self.entries.get(index) else {
                continue;
            };
            let Ok(bytes) = bigfile::read_entry(&self.path, entry) else {
                continue;
            };
            let Ok(model) = object::parse_object(&bytes) else {
                continue;
            };
            if !model.name.eq_ignore_ascii_case(&request.name) {
                continue;
            }
            let actor = ActorProfile::parse(&bytes)?;
            let textured = object::textured_mesh(&model);
            return Ok(AssetCompletion::Loaded(
                request.clone(),
                ModelAsset {
                    model,
                    textured,
                    actor,
                },
            ));
        }
        Ok(AssetCompletion::Unresolved(request.clone()))
    }
}

/// The last four characters of a name, uppercased: the archive index suffix key.
fn suffix_key(name: &str) -> Option<[u8; 4]> {
    let bytes = name.as_bytes();
    let mut key = [0u8; 4];
    key.copy_from_slice(bytes.get(bytes.len().checked_sub(4)?..)?);
    key.make_ascii_uppercase();
    Some(key)
}

/// Name-keyed cache over an [`AssetSource`].
///
/// Loaded and unresolved results are both cached, so a repeated request is a
/// lookup instead of a second archive read. The map is ordered, so any debug
/// listing is deterministic.
pub struct AssetStore<S: AssetSource> {
    source: S,
    completions: BTreeMap<String, AssetCompletion>,
}

impl<S: AssetSource> AssetStore<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            completions: BTreeMap::new(),
        }
    }

    /// Load `request` once and return the cached completion.
    pub fn load(&mut self, request: AssetRequest) -> Result<&AssetCompletion, AssetError> {
        let name = request.name.clone();
        if !self.completions.contains_key(&name) {
            let completion = self.source.load(&request)?;
            self.completions.insert(name.clone(), completion);
        }
        Ok(self
            .completions
            .get(&name)
            .expect("the completion was just inserted"))
    }

    /// The loaded asset behind `name`, if the request succeeded.
    pub fn loaded(&self, name: &str) -> Option<&ModelAsset> {
        self.completions.get(name).and_then(AssetCompletion::asset)
    }

    /// Whether `name` was requested and produced no container.
    pub fn is_unresolved(&self, name: &str) -> bool {
        self.completions
            .get(name)
            .is_some_and(|completion| matches!(completion, AssetCompletion::Unresolved(_)))
    }

    pub fn loaded_count(&self) -> usize {
        self.completions
            .values()
            .filter(|completion| completion.is_loaded())
            .count()
    }

    pub fn unresolved_count(&self) -> usize {
        self.completions
            .values()
            .filter(|completion| !completion.is_loaded())
            .count()
    }
}

/// Deterministic FIFO of pending asset requests.
///
/// This is a **shape**, not a working scheduler: the milestone source is
/// synchronous. It exists so a later asynchronous source can consume requests
/// and feed [`AssetCompletion`]s back without changing [`AssetSource`], and it
/// deliberately uses `VecDeque` with FIFO order so the sequence is reproducible.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoadQueue {
    pending: VecDeque<AssetRequest>,
}

impl LoadQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a request to the tail.
    pub fn enqueue(&mut self, request: AssetRequest) {
        self.pending.push_back(request);
    }

    /// Take the oldest request, if any.
    pub fn dequeue(&mut self) -> Option<AssetRequest> {
        self.pending.pop_front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AssetCompletion, AssetRequest, AssetSource, AssetStore, ImmediateSource, LoadQueue,
    };
    use crate::actor::ActorKind;

    fn trios() -> [[u8; 3]; 8] {
        [
            [13, 31, 31],
            [25, 16, 12],
            [9, 5, 11],
            [7, 13, 27],
            [19, 2, 11],
            [20, 3, 2],
            [9, 0, 55],
            [19, 8, 1],
        ]
    }

    /// A non-actor container under an arbitrary name.
    fn named_model(name: &str) -> Vec<u8> {
        let mut bytes = crate::object::synthetic_model();
        let base = crate::level::data_base(1);
        bytes[base + 0x4C..base + 0x58].fill(0);
        bytes[base + 0x4C..base + 0x4C + name.len()].copy_from_slice(name.as_bytes());
        bytes
    }

    /// Minimal archive: `count`, `count` index entries, then the payloads.
    fn archive(entries: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
        let table_len = 4 + entries.len() * 16;
        let mut bytes = (entries.len() as u32).to_le_bytes().to_vec();
        let mut offset = table_len;
        for (index, (suffix, payload)) in entries.iter().enumerate() {
            bytes.extend_from_slice(&(index as u32 + 1).to_le_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&(offset as u32).to_le_bytes());
            bytes.extend_from_slice(suffix);
            offset += payload.len();
        }
        for (_, payload) in entries {
            bytes.extend_from_slice(payload);
        }
        bytes
    }

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("BIGFILE.DAT");
        let skinner =
            crate::actor::synthetic_actor("skinner", &[0x18, 0x25, 0x26], trios());
        let bytes = archive(&[
            (*b"NNER", skinner),
            (*b"HEST", named_model("chest")),
        ]);
        std::fs::write(&path, bytes).expect("synthetic archive");
        (dir, path)
    }

    #[test]
    fn actor_model_loads_a_profile_with_the_shipped_ids() {
        let (_dir, path) = fixture();
        let source = ImmediateSource::open(&path).expect("index");
        let completion = source
            .load(&AssetRequest::for_name("skinner"))
            .expect("load");

        let AssetCompletion::Loaded(request, asset) = completion else {
            panic!("skinner must load, got {completion:?}");
        };
        assert_eq!(request.name, "skinner");
        assert_eq!(asset.model.name, "skinner");
        assert_eq!(asset.textured.positions.len(), 6);
        let profile = asset.actor.as_ref().expect("skinner is an actor");
        assert_eq!(profile.kind, ActorKind::Monster);
        assert_eq!(profile.trios, trios());
        assert_eq!(
            profile.entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![0x18, 0x25, 0x26]
        );
    }

    #[test]
    fn non_actor_model_loads_without_a_profile() {
        let (_dir, path) = fixture();
        let source = ImmediateSource::open(&path).expect("index");
        let completion = source
            .load(&AssetRequest::for_name("chest"))
            .expect("load");

        let AssetCompletion::Loaded(_, asset) = completion else {
            panic!("chest must load, got {completion:?}");
        };
        assert_eq!(asset.model.name, "chest");
        assert_eq!(asset.actor, None);
    }

    #[test]
    fn missing_model_is_unresolved_not_a_panic() {
        let (_dir, path) = fixture();
        let source = ImmediateSource::open(&path).expect("index");
        let completion = source
            .load(&AssetRequest::for_name("campath"))
            .expect("a missing name is not an error");

        let AssetCompletion::Unresolved(request) = completion else {
            panic!("campath must be unresolved, got {completion:?}");
        };
        assert_eq!(request.name, "campath");
    }

    #[test]
    fn short_name_is_unresolved() {
        let (_dir, path) = fixture();
        let source = ImmediateSource::open(&path).expect("index");
        assert!(matches!(
            source.load(&AssetRequest::for_name("raz")).expect("load"),
            AssetCompletion::Unresolved(_)
        ));
    }

    #[test]
    fn store_caches_loaded_and_unresolved_once() {
        let (_dir, path) = fixture();
        let mut store = AssetStore::new(ImmediateSource::open(&path).expect("index"));

        assert!(store.load(AssetRequest::for_name("skinner")).expect("load").is_loaded());
        assert!(store.load(AssetRequest::for_name("skinner")).expect("cached").is_loaded());
        assert!(store.load(AssetRequest::for_name("chest")).expect("load").is_loaded());
        assert!(matches!(
            store.load(AssetRequest::for_name("campath")).expect("load"),
            AssetCompletion::Unresolved(_)
        ));

        assert_eq!(store.loaded_count(), 2);
        assert_eq!(store.unresolved_count(), 1);
        assert!(store.loaded("skinner").is_some());
        assert!(store.is_unresolved("campath"));
        assert!(store.loaded("campath").is_none());
        assert!(!store.is_unresolved("skinner"));
    }

    #[test]
    fn load_queue_is_a_deterministic_fifo_shape() {
        let mut queue = LoadQueue::new();
        assert!(queue.is_empty());
        queue.enqueue(AssetRequest::for_name("skinner"));
        queue.enqueue(AssetRequest::for_name("chest"));
        queue.enqueue(AssetRequest::for_name("campath"));
        assert_eq!(queue.len(), 3);

        assert_eq!(queue.dequeue().map(|request| request.name), Some("skinner".to_string()));
        assert_eq!(queue.dequeue().map(|request| request.name), Some("chest".to_string()));
        assert_eq!(queue.dequeue().map(|request| request.name), Some("campath".to_string()));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }
}

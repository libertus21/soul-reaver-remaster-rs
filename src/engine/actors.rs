//! Per-actor `.SRA` resolution and stored clip labels.
//!
//! This module is presentation-only. It reads the actor names the live list
//! already carries, resolves each name to `<UPPERNAME>.SRA` in a caller-supplied
//! directory, and binds that file to the actor's own model slot count. Clips are
//! selected from the persistent label store, one state at a time.
//!
//! A resolved actor with a missing label for a state keeps its **rest pose** for
//! that state. It never borrows the player's indices and never guesses a table:
//! a stored index that falls outside the file's decoded clips is dropped to rest
//! as well. An actor with no store entry therefore animates nothing, and an
//! actor with no `.SRA` file is reported once as unresolved.
//!
//! None of this enters `FrameState` or the canonical fold, so headless digests
//! and goldens cannot move.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::engine::class::RenderClass;
use crate::engine::frame::FrameState;
use crate::engine::live::{LiveArena, LiveId};
use crate::engine::scene::{ActorPose, SceneAssets};
use crate::pose::SlotPose;
use crate::sra::{
    ClipLabels, ClipSelector, MotionState, PlaybackLoop, SraAnimation, SraClipInfo,
    units_per_second_for_fps,
};

/// Case-insensitive index of `<NAME>.SRA` files in one directory.
///
/// Keys are the uppercase file stems, so `sluagh`, `SLUAGH` and `Sluagh` all
/// resolve the same file. Two files whose stems differ only by case are
/// resolved by sorted file name, which keeps the choice deterministic.
#[derive(Debug, Clone, Default)]
pub struct SraDirectory {
    by_name: BTreeMap<String, (String, PathBuf)>,
}

impl SraDirectory {
    /// Index every `.SRA` file directly inside `dir`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let reader = std::fs::read_dir(dir)
            .map_err(|error| format!("cannot read SRA directory {}: {error}", dir.display()))?;
        let mut files: Vec<(String, PathBuf)> = Vec::new();
        for entry in reader {
            let entry = entry
                .map_err(|error| format!("cannot read SRA directory {}: {error}", dir.display()))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let (Some(name), Some(extension)) = (
                path.file_name().and_then(|name| name.to_str()),
                path.extension().and_then(|extension| extension.to_str()),
            ) else {
                continue;
            };
            if extension.eq_ignore_ascii_case("SRA") {
                files.push((name.to_string(), path));
            }
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        let mut by_name = BTreeMap::new();
        for (name, path) in files {
            let stem = Path::new(&name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_ascii_uppercase();
            by_name.entry(stem).or_insert((name, path));
        }
        Ok(Self { by_name })
    }

    /// Resolve one actor name to its file name and path.
    ///
    /// Resolution is the exact case-insensitive stem match; a miss is `None`.
    pub fn resolve(&self, actor_name: &str) -> Option<(&str, &Path)> {
        let key = actor_name.to_ascii_uppercase();
        self.by_name
            .get(&key)
            .map(|(name, path)| (name.as_str(), path.as_path()))
    }

    /// Number of distinct stems indexed.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Whether the directory held no `.SRA` file.
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// One per-actor label entry, keyed by uppercase `.SRA` file name.
///
/// The store schema is the probe's: `{"entries": {"FILE.SRA": {...}}}` with
/// optional `idle`, `walk`, `jump_up`, `fall` and `land` fields. A value may be
/// a JSON string (`"55"` or `"unknown"`) or a JSON number; anything else is a
/// missing label. Extra fields (`agreement`, `runs`, `samples`, ...) are
/// ignored by this consumer. `field_method` and `field_assigned_at` carry the
/// per-field evidence class and, for a visual assignment, its timestamp.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelStore {
    entries: BTreeMap<String, StoredLabels>,
}

/// How one stored field was obtained. The ordering is the precedence:
/// `motion < visual < capture`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LabelMethod {
    Motion = 0,
    Visual = 1,
    Capture = 2,
}

impl LabelMethod {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "motion" => Some(Self::Motion),
            "visual" => Some(Self::Visual),
            "capture" => Some(Self::Capture),
            _ => None,
        }
    }

    /// Resolve a legacy entry-level evidence tag when no `field_method` exists.
    fn from_evidence(raw: Option<&str>) -> Option<Self> {
        match raw {
            Some("visual") => Some(Self::Visual),
            Some("runtime-capture") => Some(Self::Capture),
            Some(other) if !other.is_empty() => Some(Self::Motion),
            _ => None,
        }
    }
}

/// The five label fields, in store order.
const STATES: [MotionState; 5] = [
    MotionState::Idle,
    MotionState::Moving,
    MotionState::Ascending,
    MotionState::Falling,
    MotionState::Landing,
];

fn field_index(state: MotionState) -> usize {
    match state {
        MotionState::Idle => 0,
        MotionState::Moving => 1,
        MotionState::Ascending => 2,
        MotionState::Falling => 3,
        MotionState::Landing => 4,
    }
}

fn state_for_field(field: &str) -> Option<MotionState> {
    STATES
        .into_iter()
        .find(|state| state.label_field() == field)
}

/// The labels of one store entry plus its per-field evidence metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredLabels {
    labels: ClipLabels,
    updated: Option<String>,
    methods: [Option<LabelMethod>; 5],
    assigned_at: [Option<String>; 5],
}

impl Default for StoredLabels {
    fn default() -> Self {
        Self {
            labels: ClipLabels::NONE,
            updated: None,
            methods: [None; 5],
            assigned_at: [None, None, None, None, None],
        }
    }
}

impl LabelStore {
    /// Read and parse the store. A missing file is an empty store (every state
    /// at rest); a malformed file is a loud error.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text)
                .map_err(|error| format!("cannot parse labels store {}: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!(
                "cannot read labels store {}: {error}",
                path.display()
            )),
        }
    }

    /// Parse the probe's JSON store.
    pub fn parse(text: &str) -> Result<Self, String> {
        let json = JsonParser::parse(text)?;
        let Json::Object(root) = json else {
            return Err("the labels store must be a JSON object".to_string());
        };
        let entries = root
            .into_iter()
            .find(|(key, _)| key == "entries")
            .map(|(_, value)| value);
        let Some(Json::Object(entries)) = entries else {
            return Ok(Self::default());
        };
        let mut by_name = BTreeMap::new();
        for (file, value) in entries {
            let Json::Object(fields) = value else {
                continue;
            };
            let mut stored = StoredLabels::default();
            let mut evidence = None;
            let mut top_assigned = None;
            let mut global_method = None;
            let mut methods = Vec::new();
            let mut assigned = Vec::new();
            for (field, value) in fields {
                match field.as_str() {
                    "updated" => stored.updated = json_string(&value),
                    "assigned_at" => top_assigned = json_string(&value),
                    "evidence" => evidence = json_string(&value),
                    "field_method" => match value {
                        Json::Object(pairs) => methods = pairs,
                        other => {
                            global_method =
                                json_string(&other).and_then(|raw| LabelMethod::parse(&raw));
                        }
                    },
                    "field_assigned_at" => match value {
                        Json::Object(pairs) => assigned = pairs,
                        other => top_assigned = top_assigned.or_else(|| json_string(&other)),
                    },
                    other => {
                        if let Some(state) = state_for_field(other) {
                            stored.labels.set(state, label_index(&value));
                        }
                    }
                }
            }
            let evidence_method = LabelMethod::from_evidence(evidence.as_deref());
            for state in STATES {
                let index = field_index(state);
                let field = state.label_field();
                stored.methods[index] = methods
                    .iter()
                    .find(|(key, _)| key == field)
                    .and_then(|(_, value)| json_string(value))
                    .and_then(|raw| LabelMethod::parse(&raw))
                    .or(global_method)
                    .or(evidence_method);
                stored.assigned_at[index] = assigned
                    .iter()
                    .find(|(key, _)| key == field)
                    .and_then(|(_, value)| json_string(value))
                    .or_else(|| top_assigned.clone());
            }
            by_name.insert(file.to_ascii_uppercase(), stored);
        }
        Ok(Self { entries: by_name })
    }

    /// Merge two stores field by field.
    ///
    /// The field's method decides first: `motion < visual < capture`. A
    /// `motion` value never replaces a `visual` or `capture` value, and a
    /// `visual` value never replaces a `capture` value. Within one method the
    /// newer field stamp wins (`assigned_at`, else the entry's `updated`).
    /// A field only present in one source is taken from it, so a partial entry
    /// cannot erase an older measured state. Ties keep the base (the repository
    /// copy, passed first), which keeps the merge deterministic.
    pub fn merge(base: Self, overlay: Self) -> Self {
        let mut entries = base.entries;
        for (file, incoming) in overlay.entries {
            match entries.get_mut(&file) {
                None => {
                    entries.insert(file, incoming);
                }
                Some(current) => merge_entry(current, &incoming),
            }
        }
        Self { entries }
    }

    /// The stored labels for one `.SRA` file name, or no labels.
    pub fn for_sra(&self, file_name: &str) -> ClipLabels {
        self.entries
            .get(&file_name.to_ascii_uppercase())
            .map(|stored| stored.labels)
            .unwrap_or(ClipLabels::NONE)
    }

    /// Number of files with a stored entry.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store held no entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn merge_entry(current: &mut StoredLabels, incoming: &StoredLabels) {
    for state in STATES {
        let index = field_index(state);
        let Some(incoming_clip) = incoming.labels.clip_for(state) else {
            continue;
        };
        if current.labels.clip_for(state).is_none() {
            current.labels.set(state, Some(incoming_clip));
            current.methods[index] = resolved_method(incoming, index);
            current.assigned_at[index] = incoming.assigned_at[index].clone();
            continue;
        }
        let current_method = resolved_method(current, index).unwrap_or(LabelMethod::Capture);
        let incoming_method = resolved_method(incoming, index).unwrap_or(LabelMethod::Capture);
        let take = match incoming_method.cmp(&current_method) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => field_stamp(incoming, index) > field_stamp(current, index),
        };
        if take {
            current.labels.set(state, Some(incoming_clip));
            current.methods[index] = Some(incoming_method);
            current.assigned_at[index] = incoming.assigned_at[index].clone();
        }
    }
    if is_newer(incoming.updated.as_deref(), current.updated.as_deref()) {
        current.updated = incoming.updated.clone();
    }
}

/// A labelled field with no recorded method is legacy evidence, protected as a
/// capture; an unlabelled field has no method at all.
fn resolved_method(stored: &StoredLabels, index: usize) -> Option<LabelMethod> {
    stored.methods[index].or_else(|| {
        STATES
            .get(index)
            .and_then(|state| stored.labels.clip_for(*state))
            .map(|_| LabelMethod::Capture)
    })
}

fn field_stamp(stored: &StoredLabels, index: usize) -> String {
    let stamp = stored.assigned_at[index]
        .as_deref()
        .or(stored.updated.as_deref())
        .unwrap_or("");
    stamp_key(stamp)
}

/// Compare two ISO-8601 UTC stamps. A digits-only key ignores separators and
/// keeps the format's ordering; a missing or empty stamp is the oldest.
fn is_newer(incoming: Option<&str>, base: Option<&str>) -> bool {
    let incoming = incoming.map(stamp_key).unwrap_or_default();
    let base = base.map(stamp_key).unwrap_or_default();
    !incoming.is_empty() && incoming > base
}

fn stamp_key(stamp: &str) -> String {
    stamp.chars().filter(char::is_ascii_digit).collect()
}

/// A JSON string or number is a label; `"unknown"`, `null` and other types are
/// missing evidence, not a negative and not a zero.
fn label_index(value: &Json) -> Option<usize> {
    match value {
        Json::Number(text) | Json::String(text) => text.parse::<usize>().ok(),
        Json::Null | Json::Bool(_) | Json::Array(_) | Json::Object(_) => None,
    }
}

fn json_string(value: &Json) -> Option<String> {
    match value {
        Json::String(text) | Json::Number(text) => Some(text.clone()),
        Json::Null | Json::Bool(_) | Json::Array(_) | Json::Object(_) => None,
    }
}

impl LabelStore {
    /// Write visual assignments into the store at `path`, preserving every
    /// existing field.
    ///
    /// Each assigned field becomes a JSON string label with
    /// `field_method: "visual"` and an `assigned_at` stamp in
    /// `field_assigned_at`. `runs`/`samples` are never fabricated: a visual
    /// field simply carries no such metadata. An entry whose labelled fields
    /// are all visual gets `evidence: "visual"`; a mixed entry keeps its
    /// existing evidence tag. The file is written as UTF-8 with no BOM.
    pub fn save_visual(
        path: &Path,
        assignments: &[(String, MotionState, usize)],
        assigned_at: &str,
    ) -> Result<(), String> {
        let mut root = match std::fs::read_to_string(path) {
            Ok(text) => JsonParser::parse(&text)
                .map_err(|error| format!("cannot parse labels store {}: {error}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Json::Object(vec![
                ("conflicts".to_string(), Json::Array(Vec::new())),
                ("entries".to_string(), Json::Object(Vec::new())),
                ("version".to_string(), Json::Number("1".to_string())),
            ]),
            Err(error) => {
                return Err(format!(
                    "cannot read labels store {}: {error}",
                    path.display()
                ));
            }
        };
        let Json::Object(members) = &mut root else {
            return Err("the labels store must be a JSON object".to_string());
        };
        let entries = object_field(members, "entries");
        for (file, state, clip) in assignments {
            let fields = entry_fields(entries, &file.to_ascii_uppercase());
            set_field(fields, state.label_field(), Json::String(clip.to_string()));
            let methods = object_field(fields, "field_method");
            set_field(methods, state.label_field(), Json::String("visual".to_string()));
            let stamps = object_field(fields, "field_assigned_at");
            set_field(stamps, state.label_field(), Json::String(assigned_at.to_string()));
            set_field(fields, "updated", Json::String(assigned_at.to_string()));
            set_field(fields, "assigned_at", Json::String(assigned_at.to_string()));
        }
        for (file, _, _) in assignments {
            let fields = entry_fields(entries, &file.to_ascii_uppercase());
            if entry_is_visual(fields) {
                set_field(fields, "evidence", Json::String("visual".to_string()));
            }
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "cannot create labels store directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(path, root_text(&root).into_bytes())
            .map_err(|error| format!("cannot write labels store {}: {error}", path.display()))
    }
}

fn root_text(root: &Json) -> String {
    let mut out = String::new();
    root.write(&mut out, 0);
    out.push('\n');
    out
}

/// Find or create an object-valued field and return its members.
fn object_field<'a>(fields: &'a mut Vec<(String, Json)>, key: &str) -> &'a mut Vec<(String, Json)> {
    let position = match fields.iter().position(|(name, _)| name == key) {
        Some(position) => position,
        None => {
            fields.push((key.to_string(), Json::Object(Vec::new())));
            fields.len() - 1
        }
    };
    if !matches!(fields[position].1, Json::Object(_)) {
        fields[position].1 = Json::Object(Vec::new());
    }
    match &mut fields[position].1 {
        Json::Object(members) => members,
        _ => unreachable!("the field was just replaced with an object"),
    }
}

/// Find or create one entry object and return its members.
fn entry_fields<'a>(
    entries: &'a mut Vec<(String, Json)>,
    file: &str,
) -> &'a mut Vec<(String, Json)> {
    let position = match entries.iter().position(|(name, _)| name == file) {
        Some(position) => position,
        None => {
            entries.push((file.to_string(), Json::Object(Vec::new())));
            entries.len() - 1
        }
    };
    if !matches!(entries[position].1, Json::Object(_)) {
        entries[position].1 = Json::Object(Vec::new());
    }
    match &mut entries[position].1 {
        Json::Object(members) => members,
        _ => unreachable!("the entry was just replaced with an object"),
    }
}

fn set_field(fields: &mut Vec<(String, Json)>, key: &str, value: Json) {
    match fields.iter_mut().find(|(name, _)| name == key) {
        Some((_, slot)) => *slot = value,
        None => fields.push((key.to_string(), value)),
    }
}

/// Whether every labelled field in the entry carries a `visual` method.
fn entry_is_visual(fields: &[(String, Json)]) -> bool {
    let methods = fields
        .iter()
        .find(|(name, _)| name == "field_method")
        .and_then(|(_, value)| match value {
            Json::Object(members) => Some(members),
            _ => None,
        });
    let mut any = false;
    for state in STATES {
        let field = state.label_field();
        if !fields.iter().any(|(name, _)| name == field) {
            continue;
        }
        any = true;
        let method = methods
            .and_then(|members| members.iter().find(|(name, _)| name == field))
            .and_then(|(_, value)| json_string(value));
        if method.as_deref() != Some("visual") {
            return false;
        }
    }
    any
}

impl Json {
    fn write(&self, out: &mut String, indent: usize) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Self::Number(text) => out.push_str(text),
            Self::String(text) => write_json_string(text, out),
            Self::Array(values) => {
                out.push('[');
                if !values.is_empty() {
                    out.push('\n');
                    for (index, value) in values.iter().enumerate() {
                        indent_into(out, indent + 1);
                        value.write(out, indent + 1);
                        if index + 1 < values.len() {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    indent_into(out, indent);
                }
                out.push(']');
            }
            Self::Object(members) => {
                let mut ordered: Vec<&(String, Json)> = members.iter().collect();
                ordered.sort_by(|left, right| left.0.cmp(&right.0));
                out.push('{');
                if !ordered.is_empty() {
                    out.push('\n');
                    for (index, (key, value)) in ordered.iter().enumerate() {
                        indent_into(out, indent + 1);
                        write_json_string(key, out);
                        out.push_str(": ");
                        value.write(out, indent + 1);
                        if index + 1 < ordered.len() {
                            out.push(',');
                        }
                        out.push('\n');
                    }
                    indent_into(out, indent);
                }
                out.push('}');
            }
        }
    }
}

fn indent_into(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// One live actor whose `.SRA` could not be resolved or bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedActor {
    pub id: LiveId,
    pub name: String,
}

/// One resolved live actor with its own animation, selector and observed pose.
#[derive(Debug, Clone)]
pub struct ActorAnimation {
    id: LiveId,
    name: String,
    sra_file: String,
    animation: SraAnimation,
    labels: ClipLabels,
    selector: ClipSelector,
    last_position: [i32; 3],
    last_tick: Option<u64>,
}

impl ActorAnimation {
    /// Bind one resolved animation to one live actor.
    ///
    /// Stored labels outside the file's clip count are dropped to rest. The
    /// selector starts on the actor's idle label (or rest when it has none).
    pub fn new(
        id: LiveId,
        name: String,
        sra_file: String,
        animation: SraAnimation,
        labels: ClipLabels,
    ) -> Self {
        let labels = labels.validated(animation.clip_count());
        let landing_ticks = labels.landing_window(&animation);
        Self {
            id,
            name,
            sra_file,
            selector: ClipSelector::from_labels(labels, landing_ticks),
            animation,
            labels,
            last_position: [0; 3],
            last_tick: None,
        }
    }

    pub fn id(&self) -> LiveId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sra_file(&self) -> &str {
        &self.sra_file
    }

    pub fn labels(&self) -> ClipLabels {
        self.labels
    }

    pub fn clip_count(&self) -> usize {
        self.animation.clip_count()
    }

    /// The clip selected for the current state, or `None` for rest.
    pub fn current_clip(&self) -> Option<usize> {
        self.selector.clip_opt()
    }

    /// Descriptor of one clip of this actor's file.
    pub fn clip_info(&self, clip: usize) -> Option<SraClipInfo> {
        self.animation.clip_info(clip).ok()
    }

    /// The clip an inspector plays: the requested index clamped to this file.
    pub fn inspect_clip(&self, clip: usize) -> Option<usize> {
        let count = self.clip_count();
        (count > 0).then(|| clip.min(count - 1))
    }

    /// Observe the actor's position once per simulation tick.
    ///
    /// Consecutive renders of the same tick reuse the cached state, so a static
    /// actor cannot flicker between idle and moving. A movement sample uses the
    /// integer position delta the engine already keeps: horizontal delta is the
    /// walk signal, non-zero vertical delta is ascending/falling, and the return
    /// to a zero vertical delta is the touchdown the landing window consumes.
    pub fn observe(&mut self, tick: u64, position: [i32; 3]) -> Option<usize> {
        if self.last_tick == Some(tick) {
            return self.selector.clip_opt();
        }
        match self.last_tick {
            None => {
                self.selector.observe_signals(tick, true, false, 0);
            }
            Some(_) => {
                let dx = position[0] - self.last_position[0];
                let dy = position[1] - self.last_position[1];
                let dz = position[2] - self.last_position[2];
                let grounded = dz == 0;
                let moving = dx != 0 || dy != 0;
                self.selector
                    .observe_signals(tick, grounded, moving, i64::from(dz));
            }
        }
        self.last_position = position;
        self.last_tick = Some(tick);
        self.selector.clip_opt()
    }

    /// Decode the current clip at the selector's local tick.
    pub fn poses(
        &self,
        tick: u64,
        fps: u32,
        policy: PlaybackLoop,
    ) -> Option<Vec<SlotPose>> {
        let clip = self.selector.clip_opt()?;
        self.animation
            .poses_for_clip_tick(
                clip,
                self.selector.local_tick(tick),
                units_per_second_for_fps(fps),
                policy,
            )
            .ok()
    }

    /// The bounded per-actor diagnostic line.
    pub fn label_line(&self) -> String {
        let field = |clip: Option<usize>| {
            clip.map_or_else(|| "-".to_string(), |index| index.to_string())
        };
        format!(
            "actor name={} sra={} clips={} labels=idle={},walk={},jump_up={},fall={},land={}",
            self.name,
            self.sra_file,
            self.animation.clip_count(),
            field(self.labels.idle),
            field(self.labels.walk),
            field(self.labels.jump_up),
            field(self.labels.fall),
            field(self.labels.land),
        )
    }
}

/// Every live actor's presentation animation, plus the unresolved names.
#[derive(Debug, Clone, Default)]
pub struct ActorAnimations {
    resolved: Vec<ActorAnimation>,
    unresolved: Vec<UnresolvedActor>,
}

impl ActorAnimations {
    /// Load and bind every live actor from `sra_dir`.
    ///
    /// Without `sra_dir` this is an empty set and performs no I/O, so the
    /// single-file `--sra` path and the default headless run are unchanged.
    /// `labels_stores` is only read when a directory was supplied; the stores
    /// are merged per field with the newer `updated` stamp winning. A missing
    /// store file contributes nothing, so every state stays at rest when no
    /// store exists.
    pub fn load(
        frame: &FrameState,
        assets: &mut SceneAssets,
        sra_dir: Option<&Path>,
        labels_stores: &[PathBuf],
        exclude: Option<LiveId>,
    ) -> Result<Self, String> {
        let Some(sra_dir) = sra_dir else {
            return Ok(Self::default());
        };
        let directory = SraDirectory::load(sra_dir)?;
        let store = labels_stores
            .iter()
            .try_fold(LabelStore::default(), |store, path| {
                let loaded = LabelStore::load(path)?;
                Ok::<LabelStore, String>(LabelStore::merge(store, loaded))
            })?;
        let mut resolved = Vec::new();
        let mut unresolved = Vec::new();
        for (id, object) in frame.live().iter() {
            if exclude == Some(id) {
                continue;
            }
            // An actor is a live object that carries an actor block, plus the
            // two classes whose tick hook is the actor path. The carrier is the
            // data key; the class keeps the player (whose block is attached
            // after load) in the set even when its model profile is absent.
            let is_actor = object.actor.is_some()
                || matches!(object.class, RenderClass::Raziel | RenderClass::Monster);
            if !is_actor {
                continue;
            }
            let Some((file_name, path)) = directory.resolve(&object.name) else {
                unresolved.push(UnresolvedActor {
                    id,
                    name: object.name.clone(),
                });
                continue;
            };
            let Some(slots) = assets.bone_count(&object.name) else {
                // The file exists, but the actor's model did not resolve, so the
                // slot count the bit planes need is unknown. Count it as
                // unresolved rather than guessing a slot count.
                unresolved.push(UnresolvedActor {
                    id,
                    name: object.name.clone(),
                });
                continue;
            };
            // A file that cannot be read or decoded for this actor's slot count
            // cannot play a clip either. It is reported as unresolved (rest),
            // not as a hard failure: one unreadable body must not stop the
            // window, and a wrong slot count is a reason to stay at rest.
            let Ok(bytes) = std::fs::read(path) else {
                unresolved.push(UnresolvedActor {
                    id,
                    name: object.name.clone(),
                });
                continue;
            };
            let Ok(animation) = SraAnimation::load(bytes, slots) else {
                unresolved.push(UnresolvedActor {
                    id,
                    name: object.name.clone(),
                });
                continue;
            };
            let labels = store.for_sra(file_name);
            resolved.push(ActorAnimation::new(
                id,
                object.name.clone(),
                file_name.to_string(),
                animation,
                labels,
            ));
        }
        Ok(Self {
            resolved,
            unresolved,
        })
    }

    pub fn resolved(&self) -> &[ActorAnimation] {
        &self.resolved
    }

    pub fn unresolved(&self) -> &[UnresolvedActor] {
        &self.unresolved
    }

    pub fn is_empty(&self) -> bool {
        self.resolved.is_empty() && self.unresolved.is_empty()
    }

    /// One bounded summary line plus one line per distinct unresolved name and
    /// one per distinct resolved name. Duplicate instances of the same name do
    /// not repeat a line; the summary counts instances.
    pub fn diagnostic_lines(&self) -> Vec<String> {
        let labelled = self
            .resolved
            .iter()
            .filter(|actor| actor.labels.labelled() > 0)
            .count();
        let total = self.resolved.len() + self.unresolved.len();
        let mut lines = vec![format!(
            "actors resolved={} unresolved={} labelled={} rest={}",
            self.resolved.len(),
            self.unresolved.len(),
            labelled,
            total.saturating_sub(labelled),
        )];
        let mut seen = BTreeSet::new();
        for actor in &self.resolved {
            if seen.insert(actor.name.as_str()) {
                lines.push(actor.label_line());
            }
        }
        seen.clear();
        for actor in &self.unresolved {
            if seen.insert(actor.name.as_str()) {
                lines.push(format!(
                    "actor name={} sra=unresolved rest=1",
                    actor.name
                ));
            }
        }
        lines
    }

    /// Observe each resolved actor and return the poses the scene should apply.
    ///
    /// The player's own yaw is used for its pose so movement still turns the
    /// body; every other actor has no authored rotation and keeps yaw zero.
    pub fn poses(
        &mut self,
        live: &LiveArena,
        tick: u64,
        fps: u32,
        policy: PlaybackLoop,
        player: Option<LiveId>,
        player_yaw: Option<f32>,
    ) -> Vec<ActorPose> {
        let mut poses = Vec::new();
        for actor in &mut self.resolved {
            let Ok(object) = live.get(actor.id) else {
                continue;
            };
            actor.observe(tick, object.position);
            let animation = actor.poses(tick, fps, policy);
            let is_player = Some(actor.id) == player;
            let yaw = if is_player {
                player_yaw.unwrap_or(0.0)
            } else {
                0.0
            };
            if animation.is_some() || (is_player && player_yaw.is_some()) {
                poses.push(ActorPose {
                    actor: actor.id,
                    yaw_binary: yaw,
                    animation,
                });
            }
        }
        poses
    }

    /// Every resolved actor plays one inspected clip, looped from the global
    /// tick. Each actor clamps the request to its own file, so a shorter file
    /// never errors while a longer one still shows the requested clip.
    pub fn inspect_poses(
        &self,
        live: &LiveArena,
        tick: u64,
        fps: u32,
        clip: usize,
        player: Option<LiveId>,
        player_yaw: Option<f32>,
    ) -> Vec<ActorPose> {
        let mut poses = Vec::new();
        for actor in &self.resolved {
            if live.get(actor.id).is_err() {
                continue;
            }
            let Some(clip) = actor.inspect_clip(clip) else {
                continue;
            };
            let animation = actor
                .animation
                .poses_for_clip_tick(
                    clip,
                    tick,
                    units_per_second_for_fps(fps),
                    PlaybackLoop::Loop,
                )
                .ok();
            let is_player = Some(actor.id) == player;
            poses.push(ActorPose {
                actor: actor.id,
                yaw_binary: if is_player {
                    player_yaw.unwrap_or(0.0)
                } else {
                    0.0
                },
                animation,
            });
        }
        poses
    }

    /// Distinct `.SRA` files with their clip counts, in file-name order.
    pub fn files(&self) -> Vec<(String, usize)> {
        let mut files = BTreeMap::new();
        for actor in &self.resolved {
            files
                .entry(actor.sra_file.clone())
                .or_insert(actor.clip_count());
        }
        files.into_iter().collect()
    }

    /// The descriptor of `clip` in one distinct file.
    pub fn file_clip_info(&self, file: &str, clip: usize) -> Option<SraClipInfo> {
        self.resolved
            .iter()
            .find(|actor| actor.sra_file == file)
            .and_then(|actor| actor.clip_info(clip))
    }

    /// The one-time `--clip-inspect` startup lines.
    ///
    /// The first line names the inspected clip and, from the first resolved
    /// file, its frame count, divisor and duration. One line per distinct file
    /// then makes every file's clip count visible.
    pub fn inspect_lines(&self, clip: usize) -> Vec<String> {
        let files = self.files();
        let mut per_file = Vec::new();
        let mut summary = None;
        for (file, count) in &files {
            let index = clip.min(count.saturating_sub(1));
            let info = self
                .resolved
                .iter()
                .find(|actor| &actor.sra_file == file)
                .and_then(|actor| actor.clip_info(index));
            let (frames, divisor, duration) = info.map_or((0, 0, 0.0), |info| {
                (
                    info.frame_count,
                    info.divisor,
                    duration_seconds(info.frame_count, info.divisor),
                )
            });
            summary.get_or_insert((frames, divisor, duration));
            per_file.push(format!(
                "clip-inspect sra={file} clips={count} clip={index} frames={frames} divisor={divisor} dur={duration:.3}"
            ));
        }
        let (frames, divisor, duration) = summary.unwrap_or((0, 0, 0.0));
        let mut lines = vec![format!(
            "clip-inspect clip={clip} frames={frames} divisor={divisor} dur={duration:.3} actors={}",
            self.resolved.len()
        )];
        lines.extend(per_file);
        lines
    }
}

fn duration_seconds(frames: usize, divisor: i16) -> f64 {
    frames as f64 * f64::from(divisor) / crate::sra::ANIMATION_UNITS_PER_SECOND as f64
}

/// A deliberately small JSON value, enough for the probe's store.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Json {
    Null,
    Bool(bool),
    /// Raw numeric token: the store carries floating-point evidence fields
    /// (`agreement`), so the parser keeps the token and only a label field asks
    /// whether it is a non-negative integer.
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Recursive-descent parser for the label store. No dependency is added for a
/// file this small, and the parser is strict: malformed JSON is a loud error.
struct JsonParser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> JsonParser<'a> {
    fn parse(text: &'a str) -> Result<Json, String> {
        let mut parser = Self {
            bytes: text.as_bytes(),
            position: 0,
        };
        let value = parser.value()?;
        parser.skip_whitespace();
        if parser.position != parser.bytes.len() {
            return Err(format!(
                "trailing data at byte {}",
                parser.position
            ));
        }
        Ok(value)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.position += 1;
        Some(byte)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(byte) => Err(format!(
                "unexpected byte {byte:?} at {}",
                self.position
            )),
            None => Err("unexpected end of JSON".to_string()),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.position..].starts_with(word.as_bytes()) {
            self.position += word.len();
            Ok(value)
        } else {
            Err(format!("expected '{word}' at {}", self.position))
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.bump();
        let mut entries = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Json::Object(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.string()?;
            self.skip_whitespace();
            if self.bump() != Some(b':') {
                return Err(format!("expected ':' at {}", self.position));
            }
            entries.push((key, self.value()?));
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return Err(format!("expected ',' or '}}' at {}", self.position)),
            }
        }
        Ok(Json::Object(entries))
    }

    fn array(&mut self) -> Result<Json, String> {
        self.bump();
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Json::Array(values));
        }
        loop {
            values.push(self.value()?);
            self.skip_whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err(format!("expected ',' or ']' at {}", self.position)),
            }
        }
        Ok(Json::Array(values))
    }

    fn string(&mut self) -> Result<String, String> {
        if self.bump() != Some(b'"') {
            return Err(format!("expected a string at {}", self.position));
        }
        let mut out = Vec::new();
        loop {
            let start = self.position;
            while let Some(byte) = self.peek() {
                if byte == b'"' || byte == b'\\' {
                    break;
                }
                self.position += 1;
            }
            out.extend_from_slice(&self.bytes[start..self.position]);
            match self.bump() {
                Some(b'"') => break,
                Some(b'\\') => self.escape(&mut out)?,
                _ => return Err("unterminated string".to_string()),
            }
        }
        String::from_utf8(out).map_err(|_| "invalid UTF-8 in JSON string".to_string())
    }

    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let escaped = self
            .bump()
            .ok_or_else(|| "unterminated escape".to_string())?;
        let byte = match escaped {
            b'"' => b'"',
            b'\\' => b'\\',
            b'/' => b'/',
            b'b' => 0x08,
            b'f' => 0x0c,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'u' => {
                let code = self.hex4()?;
                let character = char::from_u32(code)
                    .ok_or_else(|| format!("invalid unicode escape {code:04x}"))?;
                let mut buffer = [0u8; 4];
                out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                return Ok(());
            }
            other => return Err(format!("invalid escape '\\{}'", other as char)),
        };
        out.push(byte);
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self
                .bump()
                .ok_or_else(|| "short unicode escape".to_string())?;
            let digit = (byte as char)
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit '{}'", byte as char))?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.position;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.position += 1;
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| "invalid number bytes".to_string())?;
        Ok(Json::Number(raw.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActorAnimation, ActorAnimations, LabelStore, SraDirectory, UnresolvedActor,
    };
    use crate::engine::live::LiveId;
    use crate::sra::{
        ClipLabels, MotionState, SraAnimation, synthetic_animation_container,
        synthetic_multi_clip_container,
    };

    fn animation(frames: u16, values: &[f32]) -> crate::sra::SraAnimation {
        let container = synthetic_animation_container(&[0x001], frames, 100, values);
        crate::sra::SraAnimation::load(container, 1).expect("one synthetic table")
    }

    #[test]
    fn sra_resolution_is_case_insensitive_and_reports_a_miss() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("SLUAGH.SRA"), b"x").expect("write");
        std::fs::write(dir.path().join("Raziel.sra"), b"x").expect("write");
        std::fs::write(dir.path().join("ignore.txt"), b"x").expect("write");

        let index = SraDirectory::load(dir.path()).expect("index must load");
        assert_eq!(index.len(), 2);
        assert_eq!(
            index.resolve("sluagh").map(|(name, _)| name),
            Some("SLUAGH.SRA")
        );
        assert_eq!(
            index.resolve("raziel").map(|(name, _)| name),
            Some("Raziel.sra")
        );
        assert_eq!(
            index.resolve("SLUAGH").map(|(name, _)| name),
            Some("SLUAGH.SRA")
        );
        assert!(index.resolve("missing").is_none(), "a miss is None");
    }

    #[test]
    fn a_partial_store_entry_leaves_missing_states_at_rest() {
        let text = r#"{
            "entries": {
                "SLUAGH.SRA": {
                    "idle": "8",
                    "walk": "1",
                    "jump_up": "unknown",
                    "fall": null
                }
            }
        }"#;
        let store = LabelStore::parse(text).expect("store must parse");
        let labels = store.for_sra("sluagh.sra");
        assert_eq!(labels.idle, Some(8));
        assert_eq!(labels.walk, Some(1));
        assert_eq!(labels.jump_up, None);
        assert_eq!(labels.fall, None);
        assert_eq!(labels.land, None, "a missing field is rest");
        assert_eq!(labels.labelled(), 2);
    }

    #[test]
    fn an_actor_with_no_store_entry_is_rest_and_is_reported() {
        let animation = animation(2, &[0.0]);
        let id = LiveId {
            index: 0,
            generation: 0,
        };
        let actor = ActorAnimation::new(
            id,
            "sluagh".to_string(),
            "SLUAGH.SRA".to_string(),
            animation,
            ClipLabels::NONE,
        );
        let set = ActorAnimations {
            resolved: vec![actor],
            unresolved: vec![UnresolvedActor {
                id: LiveId {
                    index: 1,
                    generation: 0,
                },
                name: "unbound".to_string(),
            }],
        };
        assert_eq!(
            set.resolved()[0].current_clip(),
            None,
            "no label means no clip even though the file resolved"
        );
        let lines = set.diagnostic_lines();
        assert_eq!(lines[0], "actors resolved=1 unresolved=1 labelled=0 rest=2");
        assert!(
            lines
                .iter()
                .any(|line| line.contains("actor name=sluagh") && line.contains("sra=SLUAGH.SRA"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "actor name=unbound sra=unresolved rest=1")
        );
    }

    #[test]
    fn two_actors_with_different_maps_do_not_share_state() {
        let idle_only = ClipLabels {
            idle: Some(0),
            ..ClipLabels::NONE
        };
        let walk_only = ClipLabels {
            walk: Some(0),
            ..ClipLabels::NONE
        };
        let mut first = ActorAnimation::new(
            LiveId {
                index: 0,
                generation: 0,
            },
            "first".to_string(),
            "FIRST.SRA".to_string(),
            animation(1, &[0.0]),
            idle_only,
        );
        let mut second = ActorAnimation::new(
            LiveId {
                index: 1,
                generation: 0,
            },
            "second".to_string(),
            "SECOND.SRA".to_string(),
            animation(1, &[0.0]),
            walk_only,
        );

        assert_eq!(first.observe(0, [0, 0, 0]), Some(0));
        assert_eq!(second.observe(0, [0, 0, 0]), None, "no idle label is rest");

        assert_eq!(first.observe(1, [4, 0, 0]), None, "no walk label is rest");
        assert_eq!(
            second.observe(1, [4, 0, 0]),
            Some(0),
            "the walk label starts its own clip"
        );
        assert_eq!(
            first.observe(2, [4, 0, 0]),
            Some(0),
            "one step later the actor has stopped, so its idle clip is back"
        );
        assert_eq!(
            second.observe(2, [8, 0, 0]),
            Some(0),
            "the other actor keeps its own walk clip"
        );
    }

    #[test]
    fn a_label_past_a_short_animation_is_rest_not_an_arbitrary_clip() {
        let labels = ClipLabels {
            idle: Some(5),
            walk: Some(0),
            ..ClipLabels::NONE
        };
        let actor = ActorAnimation::new(
            LiveId {
                index: 0,
                generation: 0,
            },
            "short".to_string(),
            "SHORT.SRA".to_string(),
            animation(1, &[0.0]),
            labels,
        );
        assert_eq!(actor.clip_count(), 1);
        assert_eq!(actor.labels().idle, None, "the stale index is dropped");
        assert_eq!(actor.labels().walk, Some(0));
        assert_eq!(actor.current_clip(), None);
    }

    #[test]
    fn malformed_json_is_a_loud_error() {
        assert!(LabelStore::parse("{").is_err());
        assert!(LabelStore::parse("[]").is_err());
        assert!(LabelStore::parse(r#"{"entries": []}"#).is_ok());
    }

    #[test]
    fn a_malformed_store_reports_the_failing_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("clip-labels.json");
        std::fs::write(&path, "{").expect("write a malformed store");

        let error = LabelStore::load(&path).expect_err("a malformed store must fail");
        assert!(error.contains("cannot parse labels store"));
        assert!(
            error.contains(&path.display().to_string()),
            "the error must name the resolved path: {error}"
        );
    }

    #[test]
    fn the_versioned_store_parses_to_the_measured_labels() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("clip-labels.json");
        let bytes = std::fs::read(&path).expect("the committed store must be readable");
        assert_ne!(
            bytes.first().copied(),
            Some(0xEF),
            "the committed store must be UTF-8 without a BOM"
        );

        let store = LabelStore::load(&path).expect("the committed store must parse");
        // The store grows every time a type is measured, so this asserts a lower
        // bound rather than an exact count: adding a measurement must never
        // break the suite, while losing a confirmed entry still must.
        assert!(
            store.len() >= 3,
            "the committed store must keep every confirmed entry, got {}",
            store.len()
        );

        let raziel = store.for_sra("RAZIEL.SRA");
        assert_eq!(
            (
                raziel.idle,
                raziel.walk,
                raziel.jump_up,
                raziel.fall,
                raziel.land
            ),
            (Some(55), Some(2), Some(35), Some(36), Some(37)),
            "the confirmed Raziel capture must survive version control"
        );

        let sluagh = store.for_sra("SLUAGH.SRA");
        assert_eq!((sluagh.idle, sluagh.walk), (Some(8), Some(1)));
        assert_eq!(sluagh.jump_up, None, "unmeasured states stay at rest");

        let hunter = store.for_sra("HUNTER.SRA");
        assert_eq!((hunter.idle, hunter.walk), (Some(2), Some(7)));
        assert_eq!(
            (hunter.jump_up, hunter.fall, hunter.land),
            (None, None, None),
            "the visual pass covered locomotion only"
        );
    }

    #[test]
    fn label_stores_merge_per_field_with_the_newer_updated_winning() {
        let repo = LabelStore::parse(
            r#"{
                "entries": {
                    "RAZIEL.SRA": {
                        "idle": "55", "walk": "2",
                        "updated": "2026-11-15T16:50:24Z"
                    },
                    "SLUAGH.SRA": {
                        "idle": "8", "walk": "1",
                        "updated": "2026-11-22T03:24:01Z"
                    }
                }
            }"#,
        )
        .expect("the repository store parses");
        let home = LabelStore::parse(
            r#"{
                "entries": {
                    "RAZIEL.SRA": {
                        "idle": "60", "updated": "2026-11-22T20:20:38Z"
                    },
                    "SLUAGH.SRA": {
                        "walk": "9", "updated": "2026-11-01T00:00:00Z"
                    },
                    "HUMAN.SRA": {
                        "idle": "17", "updated": "2026-11-22T05:21:23Z"
                    }
                }
            }"#,
        )
        .expect("the home store parses");

        let merged = LabelStore::merge(repo, home);
        let raziel = merged.for_sra("raziel.sra");
        assert_eq!(
            raziel.idle,
            Some(60),
            "the newer home field must reach the renderer"
        );
        assert_eq!(
            raziel.walk,
            Some(2),
            "a state missing from the newer entry keeps the older measured value"
        );

        let sluagh = merged.for_sra("SLUAGH.SRA");
        assert_eq!(sluagh.idle, Some(8), "the newer repository idle is kept");
        assert_eq!(
            sluagh.walk,
            Some(1),
            "an older home field must not override the repository copy"
        );

        assert_eq!(
            merged.for_sra("HUMAN.SRA").idle,
            Some(17),
            "a home-only file is admitted by the merge"
        );
    }

    #[test]
    fn a_visual_assignment_writes_method_and_assigned_at_without_run_metadata() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("clip-labels.json");
        std::fs::write(
            &path,
            r#"{"version":1,"conflicts":[],"entries":{"HUNTER.SRA":{"slots":24}}}"#,
        )
        .expect("write a base store");

        LabelStore::save_visual(
            &path,
            &[
                ("HUNTER.SRA".to_string(), MotionState::Idle, 2),
                ("HUNTER.SRA".to_string(), MotionState::Moving, 7),
            ],
            "2026-11-22T12:00:00Z",
        )
        .expect("the visual save must succeed");

        let text = std::fs::read_to_string(&path).expect("read the store");
        let store = LabelStore::parse(&text).expect("the written store must parse");
        let labels = store.for_sra("hunter.sra");
        assert_eq!((labels.idle, labels.walk), (Some(2), Some(7)));
        assert_eq!((labels.jump_up, labels.fall, labels.land), (None, None, None));
        assert!(text.contains("\"field_method\""), "the method is per field");
        assert!(text.contains("\"visual\""), "the method name is visual");
        assert!(text.contains("\"field_assigned_at\""));
        assert!(text.contains("2026-11-22T12:00:00Z"));
        assert_eq!(
            text.matches("\"runs\"").count(),
            0,
            "visual evidence must not fabricate run counts"
        );
        assert_eq!(
            text.matches("\"samples\"").count(),
            0,
            "visual evidence must not fabricate sample counts"
        );
        assert_eq!(
            text.matches("\"slots\"").count(),
            1,
            "unrelated entry metadata is preserved"
        );
    }

    fn store_with(file: &str, clip: usize, method: &str, stamp: &str) -> LabelStore {
        LabelStore::parse(&format!(
            r#"{{"entries":{{"{file}":{{"idle":"{clip}","field_method":{{"idle":"{method}"}},"updated":"{stamp}"}}}}}}"#
        ))
        .expect("the synthetic store parses")
    }

    #[test]
    fn precedence_is_motion_then_visual_then_capture() {
        let motion = store_with("HUNTER.SRA", 1, "motion", "2026-11-01T00:00:00Z");
        let visual = store_with("HUNTER.SRA", 2, "visual", "2026-11-02T00:00:00Z");
        let capture = store_with("HUNTER.SRA", 3, "capture", "2026-11-03T00:00:00Z");

        let visual_over_motion = LabelStore::merge(motion, visual.clone());
        assert_eq!(
            visual_over_motion.for_sra("HUNTER.SRA").idle,
            Some(2),
            "visual evidence may replace a motion label"
        );
        let capture_over_visual = LabelStore::merge(visual.clone(), capture.clone());
        assert_eq!(
            capture_over_visual.for_sra("HUNTER.SRA").idle,
            Some(3),
            "capture evidence outranks visual"
        );
        let visual_over_capture = LabelStore::merge(capture, visual);
        assert_eq!(
            visual_over_capture.for_sra("HUNTER.SRA").idle,
            Some(3),
            "visual evidence never replaces capture"
        );
        let motion_over_visual = LabelStore::merge(
            store_with("HUNTER.SRA", 2, "visual", "2026-11-02T00:00:00Z"),
            store_with("HUNTER.SRA", 1, "motion", "2026-11-09T00:00:00Z"),
        );
        assert_eq!(
            motion_over_visual.for_sra("HUNTER.SRA").idle,
            Some(2),
            "motion evidence never replaces visual, even when newer"
        );
    }

    #[test]
    fn a_visual_entry_without_run_metadata_is_tolerated() {
        let store = LabelStore::parse(
            r#"{
                "entries": {
                    "HUNTER.SRA": {
                        "idle": "2", "walk": "7",
                        "field_method": {"idle": "visual", "walk": "visual"},
                        "field_assigned_at": {
                            "idle": "2026-11-22T12:00:00Z",
                            "walk": "2026-11-22T12:00:00Z"
                        },
                        "assigned_at": "2026-11-22T12:00:00Z",
                        "evidence": "visual",
                        "updated": "2026-11-22T12:00:00Z"
                    }
                }
            }"#,
        )
        .expect("a visual-only entry parses without runs or samples");
        let labels = store.for_sra("HUNTER.SRA");
        assert_eq!((labels.idle, labels.walk), (Some(2), Some(7)));
        assert_eq!(labels.labelled(), 2);
    }

    fn multi_clip_animation(clips: &[(u16, i16)]) -> SraAnimation {
        let container = synthetic_multi_clip_container(&[0x001], clips);
        SraAnimation::load(container, 1).expect("one synthetic table per clip")
    }

    #[test]
    fn clip_inspect_selects_clip_n_for_every_actor_and_clamps_shorter_files() {
        let long = multi_clip_animation(&[(1, 100), (2, 100), (3, 100)]);
        let short = multi_clip_animation(&[(1, 100), (2, 100)]);
        let actors = ActorAnimations {
            resolved: vec![
                ActorAnimation::new(
                    LiveId { index: 0, generation: 0 },
                    "hunter".to_string(),
                    "HUNTER.SRA".to_string(),
                    long,
                    ClipLabels::NONE,
                ),
                ActorAnimation::new(
                    LiveId { index: 1, generation: 0 },
                    "sluagh".to_string(),
                    "SLUAGH.SRA".to_string(),
                    short,
                    ClipLabels::NONE,
                ),
            ],
            unresolved: Vec::new(),
        };
        assert_eq!(actors.resolved()[0].inspect_clip(2), Some(2));
        assert_eq!(
            actors.resolved()[1].inspect_clip(2),
            Some(1),
            "a shorter file clamps to its own last clip"
        );
        assert_eq!(actors.resolved()[0].inspect_clip(9), Some(2));
        let lines = actors.inspect_lines(2);
        assert!(
            lines[0].starts_with("clip-inspect clip=2 frames=3 divisor=100"),
            "the startup line names the requested clip: {lines:?}"
        );
        assert!(lines.iter().any(|line| {
            line.contains("sra=HUNTER.SRA") && line.contains("clips=3")
        }));
        assert!(lines.iter().any(|line| {
            line.contains("sra=SLUAGH.SRA") && line.contains("clips=2")
        }));
    }
}

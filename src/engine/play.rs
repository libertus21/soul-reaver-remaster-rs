//! `--play` option parsing and the engine entry point.
//!
//! Parsing is pure: it never touches the filesystem and never constructs an
//! engine. [`run`] loads the world and the static scene first, then either
//! drives a headless fixed-tick loop through [`NullBackend`] or opens a winit
//! window with the wgpu backend.
//!
//! Texture pages are validated before the event loop is created, so a missing
//! page is a fail-fast exit 2 instead of a half-open window. The authored
//! placed-object seed pass is shared by both modes and always runs before a
//! window exists, so the window shows the same live world as the headless run.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::cli_out::outln;
use crate::engine::actor::ActorProfile;
use crate::engine::actors::ActorAnimations;
use crate::engine::assets::{AssetRequest, AssetStore, ImmediateSource};
use crate::engine::backend_wgpu::WgpuBackend;
use crate::engine::class::RenderClass;
use crate::engine::clock::ObjectTick;
use crate::engine::determinism::world_state_hash;
use crate::engine::frame::{FrameError, FrameState, PLAYER_NAME};
use crate::engine::input::{InputButton, InputEvent};
use crate::engine::render_backend::RenderBackend;
use crate::engine::scene::{ActorPose, CameraView, PlayerFacing, SceneAssets};
use crate::engine::spawn::{Admission, EngineNameTable};
use crate::engine::world::{World, WorldError};
use crate::object_texture::PageBank;
use crate::pose;
use crate::render::{self, FlyCamera, OrbitCamera};
use crate::sra::{
    ANIMATION_FPS, ClipError, ClipMap, ClipSelection, ClipSelector, PlaybackLoop,
    units_per_second_for_fps,
};

/// Camera modes accepted by `--camera`.
///
/// The discriminants are explicit and `#[repr(u8)]` so the value hashed by
/// `determinism::write_options` cannot drift when a variant is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CameraMode {
    Orbit = 0,
    Fly = 1,
    Player = 2,
}

impl CameraMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "orbit" => Some(Self::Orbit),
            "fly" => Some(Self::Fly),
            "player" => Some(Self::Player),
            _ => None,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Orbit => Self::Fly,
            Self::Fly => Self::Player,
            Self::Player => Self::Orbit,
        }
    }
}

/// One step of an `--input-script` run: hold `button` for `ticks` ticks.
///
/// Steps are applied sequentially from tick 0, so the first step owns ticks
/// `0..ticks` and the next one starts where it ended. A step is a press at its
/// first tick and a release at the tick after its last, never a wall-clock
/// timer: the diagnostic therefore measures the same fixed ticks a headless
/// run always uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptStep {
    pub button: InputButton,
    pub ticks: u32,
}

/// A parsed `--input-script` sequence.
///
/// Parsing is pure and bounded: the total tick count is capped so an untrusted
/// spec cannot request an unbounded run. The type exposes the exact per-tick
/// key events, which is what the diagnostics tests assert against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputScript {
    steps: Vec<ScriptStep>,
}

/// Largest tick count one `--input-script` step may request.
pub const MAX_SCRIPT_STEP_TICKS: u32 = 1_000_000;

/// Largest total tick count an `--input-script` may request.
pub const MAX_SCRIPT_TOTAL_TICKS: u64 = 1_000_000;

impl InputScript {
    /// Parse `button:ticks[,button:ticks...]`.
    ///
    /// Buttons are `forward`, `back`, `left`, `right`, `jump` and `action`.
    /// Every step must carry a positive tick count and the total must stay
    /// within [`MAX_SCRIPT_TOTAL_TICKS`].
    pub fn parse(spec: &str) -> Option<Self> {
        let mut steps = Vec::new();
        let mut total = 0u64;
        for part in spec.split(',') {
            let (raw_button, raw_ticks) = part.split_once(':')?;
            let button = parse_script_button(raw_button.trim())?;
            let ticks = raw_ticks.trim().parse::<u32>().ok()?;
            if ticks == 0 || ticks > MAX_SCRIPT_STEP_TICKS {
                return None;
            }
            total = total.checked_add(u64::from(ticks))?;
            if total > MAX_SCRIPT_TOTAL_TICKS {
                return None;
            }
            steps.push(ScriptStep { button, ticks });
        }
        (!steps.is_empty()).then_some(Self { steps })
    }

    /// Total number of ticks the whole sequence occupies.
    pub fn total_ticks(&self) -> u64 {
        self.steps
            .iter()
            .map(|step| u64::from(step.ticks))
            .sum()
    }

    /// The key events applied at the start of `tick`, in step order.
    ///
    /// A step releases at the boundary tick and the following step presses at
    /// the same boundary, so two adjacent steps of the same button are two
    /// separate presses, not one held button.
    pub fn events_at(&self, tick: u64) -> Vec<InputEvent> {
        let mut events = Vec::new();
        let mut cursor = 0u64;
        for step in &self.steps {
            let end = cursor + u64::from(step.ticks);
            if tick == cursor {
                events.push(InputEvent::press(step.button));
            }
            if tick == end {
                events.push(InputEvent::release(step.button));
            }
            cursor = end;
        }
        events
    }
}

fn parse_script_button(raw: &str) -> Option<InputButton> {
    match raw.to_ascii_lowercase().as_str() {
        "forward" => Some(InputButton::Forward),
        "back" => Some(InputButton::Back),
        "left" => Some(InputButton::Left),
        "right" => Some(InputButton::Right),
        "jump" => Some(InputButton::Jump),
        "action" => Some(InputButton::Action),
        _ => None,
    }
}

/// Parse `X,Y,Z` into an integer world position.
fn parse_player_pos(raw: &str) -> Option<[i32; 3]> {
    let mut values = raw.split(',');
    let position = [
        values.next()?.trim().parse::<i32>().ok()?,
        values.next()?.trim().parse::<i32>().ok()?,
        values.next()?.trim().parse::<i32>().ok()?,
    ];
    values.next().is_none().then_some(position)
}

/// Parsed `--play` options.
///
/// `ticks` is resolved during parsing: headless defaults to 60, while `None`
/// in windowed mode means no frame cap. Windowed runs require `--textures`;
/// that is a runtime check so the parser stays pure and the T1 error surface is
/// unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayOptions {
    pub bigfile: PathBuf,
    pub textures: Option<PathBuf>,
    pub unit: String,
    /// Archive level-family prefix (`--group <PREFIX>`).
    ///
    /// Loads every archived unit whose self-declared name is the prefix plus
    /// ASCII digits, in numeric-suffix order, through the same discovery the
    /// `--level-* --group` modes use. Mutually exclusive with `--unit`.
    pub group: Option<String>,
    pub ticks: Option<u32>,
    pub headless: bool,
    pub camera: CameraMode,
    /// Presentation-only `.SRA` animation resource for the player mesh.
    ///
    /// This is the single-actor override; it keeps the explicit/default clip
    /// map and applies only to the player. Per-actor resolution lives in
    /// [`PlayOptions::sra_dir`].
    pub sra: Option<PathBuf>,
    /// Directory of per-actor `.SRA` files (`--sra-dir <DIR>`).
    ///
    /// Each live actor name resolves its own `<UPPERNAME>.SRA` case-insensitively.
    pub sra_dir: Option<PathBuf>,
    /// Persistent per-file label store (`--labels-store <FILE>`).
    ///
    /// Lookup order is documented on [`PlayOptions::labels_store_sources`]:
    /// explicit, then the versioned `data/clip-labels.json` merged with the
    /// probe's `~/.sr1_clip_labels.json`. Only read when a directory was
    /// supplied.
    pub labels_store: Option<PathBuf>,
    /// Presentation playback rate, in frames per second for a divisor-100 clip.
    pub anim_fps: u32,
    /// Whether the animation wraps (`false`) or holds the last frame (`true`).
    pub anim_once: bool,
    /// Frozen clip override (`--clip <N>`); state selection is ignored.
    pub clip: Option<usize>,
    /// Explicit grounded/idle clip (`--clip-idle <N>`).
    pub clip_idle: Option<usize>,
    /// Explicit grounded/moving clip (`--clip-walk <N>`).
    pub clip_walk: Option<usize>,
    /// Airborne shortcut (`--clip-jump <N>`): sets ascending, falling and
    /// landing together. Testing aid, not a per-state override.
    pub clip_jump: Option<usize>,
    /// Explicit ascending clip (`--clip-jump-up <N>`).
    pub clip_jump_up: Option<usize>,
    /// Explicit falling clip (`--clip-fall <N>`).
    pub clip_fall: Option<usize>,
    /// Explicit landing clip (`--clip-land <N>`).
    pub clip_land: Option<usize>,
    /// Deterministic diagnostic input sequence (`--input-script`, headless only).
    pub input_script: Option<InputScript>,
    /// Explicit player position that overrides the authored record
    /// (`--player-pos`, a test aid).
    pub player_pos: Option<[i32; 3]>,
    /// Print the read-only per-object visibility line (`--cull-report`).
    ///
    /// Headless only: it reports the cull view and how many live objects the
    /// last tick left visible, after the acceptance line. It changes no state.
    pub cull_report: bool,
    /// Third-person boom distance override in world units (`--cam-distance`).
    pub cam_distance: Option<f32>,
    /// Third-person eye height override in world units (`--cam-height`).
    pub cam_height: Option<f32>,
}

impl PlayOptions {
    pub const DEFAULT_UNIT: &'static str = "under1";

    /// Repository-relative copy of the measured label store.
    ///
    /// This copy is versioned so the measurements survive a lost home
    /// directory. It is data, not a game asset: only clip indices and `.SRA`
    /// file names.
    pub const REPO_LABELS_STORE: &'static str = "data/clip-labels.json";

    /// The primary label store path, in documented lookup order.
    ///
    /// 1. `--labels-store <FILE>` when given (explicit wins, even if missing).
    /// 2. [`Self::REPO_LABELS_STORE`] relative to the current directory when
    ///    that file exists.
    /// 3. The probe's user-home default (`~/.sr1_clip_labels.json`, resolved
    ///    from `USERPROFILE` or `HOME`).
    ///
    /// The actor loader does not read only this path: it uses
    /// [`Self::labels_store_sources`] so the repository copy and the home copy
    /// are merged per field. This method is the test-only single-path answer.
    #[cfg(test)]
    pub fn labels_store_path(&self) -> PathBuf {
        self.labels_store_path_with(Path::new(Self::REPO_LABELS_STORE))
    }

    /// Every label store to merge, in precedence order.
    ///
    /// An explicit `--labels-store` is the only source. Otherwise the
    /// repository copy (when it exists) and the probe's home copy are both
    /// returned, and `LabelStore::merge` keeps the newer `updated` per field.
    /// Reading only the first existing file was the consumption gap: the probe
    /// writes the home copy, which was ignored whenever the versioned copy
    /// existed.
    pub fn labels_store_sources(&self) -> Vec<PathBuf> {
        self.labels_store_sources_with(Path::new(Self::REPO_LABELS_STORE))
    }

    /// The merge sources with an injectable repository path, so the order is
    /// testable without changing the process directory.
    fn labels_store_sources_with(&self, repo: &Path) -> Vec<PathBuf> {
        if let Some(path) = &self.labels_store {
            return vec![path.clone()];
        }
        let home = default_labels_store();
        let mut sources = Vec::new();
        if repo.is_file() {
            sources.push(repo.to_path_buf());
        }
        if sources.first() != Some(&home) {
            sources.push(home);
        }
        sources
    }

    /// Lookup order with an injectable repository path, so the order is
    /// testable without changing the process directory.
    #[cfg(test)]
    fn labels_store_path_with(&self, repo: &Path) -> PathBuf {
        if let Some(path) = &self.labels_store {
            return path.clone();
        }
        if repo.is_file() {
            return repo.to_path_buf();
        }
        default_labels_store()
    }

    /// Resolve the state-to-clip mapping against the loaded clip count.
    ///
    /// Explicit indices are strict: an index outside `0..clip_count` is a typed
    /// [`ClipError`]. The default map comes from
    /// [`ClipMap::for_clip_count`], which is always in range. `--clip-jump <N>`
    /// is a shortcut that sets all three airborne states; the per-state flags
    /// (`--clip-jump-up`, `--clip-fall`, `--clip-land`) override it
    /// individually because they are applied afterwards.
    pub fn clip_selection(&self, clip_count: usize) -> Result<ClipSelection, ClipError> {
        let default = ClipMap::for_clip_count(clip_count);
        let map = ClipMap {
            idle: self.clip_idle.unwrap_or(default.idle),
            walk: self.clip_walk.unwrap_or(default.walk),
            jump_up: self.clip_jump_up.or(self.clip_jump).unwrap_or(default.jump_up),
            fall: self.clip_fall.or(self.clip_jump).unwrap_or(default.fall),
            land: self.clip_land.or(self.clip_jump).unwrap_or(default.land),
        };
        let out_of_range = |clip: usize| ClipError::OutOfRange { clip, clip_count };
        if let Some(fixed) = self.clip {
            if fixed >= clip_count {
                return Err(out_of_range(fixed));
            }
            return Ok(ClipSelection {
                fixed: Some(fixed),
                map,
            });
        }
        for clip in [map.idle, map.walk, map.jump_up, map.fall, map.land] {
            if clip >= clip_count {
                return Err(out_of_range(clip));
            }
        }
        Ok(ClipSelection { fixed: None, map })
    }

    /// Parse the argv that follows `--play`.
    ///
    /// Pure by construction: unknown flags, missing values, a missing
    /// `--bigfile` and malformed `--ticks`/`--camera` values are typed errors.
    pub fn parse(args: &[String]) -> Result<Self, PlayError> {
        let mut options = Self {
            bigfile: PathBuf::new(),
            textures: None,
            unit: Self::DEFAULT_UNIT.to_string(),
            group: None,
            ticks: None,
            headless: false,
            camera: CameraMode::Fly,
            sra: None,
            sra_dir: None,
            labels_store: None,
            anim_fps: ANIMATION_FPS,
            anim_once: false,
            clip: None,
            clip_idle: None,
            clip_walk: None,
            clip_jump: None,
            clip_jump_up: None,
            clip_fall: None,
            clip_land: None,
            input_script: None,
            player_pos: None,
            cull_report: false,
            cam_distance: None,
            cam_height: None,
        };
        let mut bigfile = None;
        let mut unit_given = false;
        let mut index = 0;
        while index < args.len() {
            let token = args[index].as_str();
            match token {
                "--bigfile" => bigfile = Some(PathBuf::from(value(args, index, "--bigfile")?)),
                "--textures" => {
                    options.textures = Some(PathBuf::from(value(args, index, "--textures")?));
                }
                "--unit" => {
                    options.unit = value(args, index, "--unit")?.to_string();
                    unit_given = true;
                }
                "--group" => {
                    options.group = Some(value(args, index, "--group")?.to_string());
                }
                "--ticks" => {
                    let raw = value(args, index, "--ticks")?;
                    options.ticks = Some(
                        raw.parse::<u32>()
                            .map_err(|_| PlayError::InvalidTicks(raw.to_string()))?,
                    );
                }
                "--camera" => {
                    let raw = value(args, index, "--camera")?;
                    options.camera = CameraMode::parse(raw)
                        .ok_or_else(|| PlayError::InvalidCamera(raw.to_string()))?;
                }
                "--sra" => {
                    options.sra = Some(PathBuf::from(value(args, index, "--sra")?));
                }
                "--sra-dir" => {
                    options.sra_dir = Some(PathBuf::from(value(args, index, "--sra-dir")?));
                }
                "--labels-store" => {
                    options.labels_store =
                        Some(PathBuf::from(value(args, index, "--labels-store")?));
                }
                "--anim-fps" => {
                    let raw = value(args, index, "--anim-fps")?;
                    let fps = raw
                        .parse::<u32>()
                        .map_err(|_| PlayError::InvalidAnimFps(raw.to_string()))?;
                    if !(1..=240).contains(&fps) {
                        return Err(PlayError::InvalidAnimFps(raw.to_string()));
                    }
                    options.anim_fps = fps;
                }
                "--anim-once" => {
                    options.anim_once = true;
                    index += 1;
                    continue;
                }
                "--clip" => options.clip = Some(clip_value(args, index, "--clip")?),
                "--clip-idle" => {
                    options.clip_idle = Some(clip_value(args, index, "--clip-idle")?)
                }
                "--clip-walk" => {
                    options.clip_walk = Some(clip_value(args, index, "--clip-walk")?)
                }
                "--clip-jump" => {
                    options.clip_jump = Some(clip_value(args, index, "--clip-jump")?)
                }
                "--clip-jump-up" => {
                    options.clip_jump_up = Some(clip_value(args, index, "--clip-jump-up")?)
                }
                "--clip-fall" => {
                    options.clip_fall = Some(clip_value(args, index, "--clip-fall")?)
                }
                "--clip-land" => {
                    options.clip_land = Some(clip_value(args, index, "--clip-land")?)
                }
                "--input-script" => {
                    let raw = value(args, index, "--input-script")?;
                    options.input_script = Some(
                        InputScript::parse(raw)
                            .ok_or_else(|| PlayError::InvalidInputScript(raw.to_string()))?,
                    );
                }
                "--player-pos" => {
                    let raw = value(args, index, "--player-pos")?;
                    options.player_pos = Some(
                        parse_player_pos(raw)
                            .ok_or_else(|| PlayError::InvalidPlayerPos(raw.to_string()))?,
                    );
                }
                "--cam-distance" => {
                    let raw = value(args, index, "--cam-distance")?;
                    let distance = raw
                        .parse::<f32>()
                        .ok()
                        .filter(|distance| distance.is_finite())
                        .filter(|distance| (100.0..=5000.0).contains(distance))
                        .ok_or_else(|| PlayError::InvalidCamDistance(raw.to_string()))?;
                    options.cam_distance = Some(distance);
                }
                "--cam-height" => {
                    let raw = value(args, index, "--cam-height")?;
                    let height = raw
                        .parse::<f32>()
                        .ok()
                        .filter(|height| height.is_finite())
                        .filter(|height| (0.0..=5000.0).contains(height))
                        .ok_or_else(|| PlayError::InvalidCamHeight(raw.to_string()))?;
                    options.cam_height = Some(height);
                }
                "--headless" => {
                    options.headless = true;
                    index += 1;
                    continue;
                }
                "--cull-report" => {
                    options.cull_report = true;
                    index += 1;
                    continue;
                }
                other => return Err(PlayError::UnknownOption(other.to_string())),
            }
            index += 2;
        }
        let Some(bigfile) = bigfile else {
            return Err(PlayError::MissingArchive);
        };
        options.bigfile = bigfile;
        if options.group.is_some() && unit_given {
            return Err(PlayError::UnitAndGroup);
        }
        if options.input_script.is_some() && !options.headless {
            return Err(PlayError::InputScriptRequiresHeadless);
        }
        if options.cull_report && !options.headless {
            return Err(PlayError::CullReportRequiresHeadless);
        }
        if options.headless {
            if options.ticks.is_none() {
                options.ticks = Some(60);
            }
            if options.ticks == Some(0) {
                return Err(PlayError::ZeroTicksHeadless);
            }
            if let Some(script) = &options.input_script {
                // The script always runs to its own end; `--ticks` may only
                // extend the idle tail, never truncate the sequence.
                let script_ticks = u32::try_from(script.total_ticks()).unwrap_or(u32::MAX);
                options.ticks = Some(options.ticks.unwrap_or(0).max(script_ticks));
            }
        }
        Ok(options)
    }
}

/// The probe's default label store, resolved at runtime from the environment.
fn default_labels_store() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map_or_else(
            || PathBuf::from(".sr1_clip_labels.json"),
            |home| Path::new(&home).join(".sr1_clip_labels.json"),
        )
}

fn value<'a>(
    args: &'a [String],
    index: usize,
    option: &'static str,
) -> Result<&'a str, PlayError> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or(PlayError::MissingValue(option))
}

fn clip_value(
    args: &[String],
    index: usize,
    option: &'static str,
) -> Result<usize, PlayError> {
    let raw = value(args, index, option)?;
    raw.parse::<usize>()
        .map_err(|_| PlayError::InvalidClip(raw.to_string()))
}

/// Typed `--play` failures. `main` prints each one and exits 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    UnknownOption(String),
    MissingValue(&'static str),
    MissingArchive,
    UnitAndGroup,
    InvalidTicks(String),
    ZeroTicksHeadless,
    InvalidCamera(String),
    InvalidAnimFps(String),
    InvalidClip(String),
    InvalidInputScript(String),
    InputScriptRequiresHeadless,
    CullReportRequiresHeadless,
    InvalidPlayerPos(String),
    InvalidCamDistance(String),
    InvalidCamHeight(String),
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOption(option) => write!(f, "unknown option '{option}' for --play"),
            Self::MissingValue(option) => write!(f, "option '{option}' needs a value"),
            Self::MissingArchive => write!(f, "--play requires --bigfile <BIGFILE.DAT>"),
            Self::UnitAndGroup => write!(
                f,
                "--unit and --group are mutually exclusive: pass one of them"
            ),
            Self::InvalidTicks(value) => write!(
                f,
                "invalid value '{value}' for --ticks: expected a non-negative integer"
            ),
            Self::ZeroTicksHeadless => write!(
                f,
                "invalid value '0' for --ticks: expected a positive integer in headless mode"
            ),
            Self::InvalidCamera(value) => write!(
                f,
                "invalid value '{value}' for --camera: expected orbit, fly or player"
            ),
            Self::InvalidAnimFps(value) => write!(
                f,
                "invalid value '{value}' for --anim-fps: expected an integer in 1..=240"
            ),
            Self::InvalidClip(value) => write!(
                f,
                "invalid value '{value}' for a clip option: expected a non-negative integer"
            ),
            Self::InvalidInputScript(value) => write!(
                f,
                "invalid value '{value}' for --input-script: expected button:ticks pairs such as forward:60"
            ),
            Self::InputScriptRequiresHeadless => write!(
                f,
                "--input-script requires --headless: the diagnostic is a fixed-tick run"
            ),
            Self::CullReportRequiresHeadless => write!(
                f,
                "--cull-report requires --headless: the diagnostic is a fixed-tick run"
            ),
            Self::InvalidPlayerPos(value) => write!(
                f,
                "invalid value '{value}' for --player-pos: expected X,Y,Z integers"
            ),
            Self::InvalidCamDistance(value) => write!(
                f,
                "invalid value '{value}' for --cam-distance: expected a number in 100..=5000"
            ),
            Self::InvalidCamHeight(value) => write!(
                f,
                "invalid value '{value}' for --cam-height: expected a number in 0..=5000"
            ),
        }
    }
}

impl std::error::Error for PlayError {}

/// Runtime failure of a real run. `exit_code` is the process code `main` uses.
#[derive(Debug)]
pub enum RunError {
    MissingTextures,
    /// A world load or parse failure: fail with exit code 2.
    World(WorldError),
    Scene(String),
    /// A clip index outside the decoded set: a typed option error.
    Clip(crate::sra::ClipError),
    /// A missing or undecodable texture page: fail fast with exit code 2.
    Texture(String),
    /// An unreadable asset archive during headless boot: fail with exit code 2.
    Asset(String),
    Window(String),
    /// Engine invariant violated (tracker full, corrupt arena, impossible
    /// state). Exit code 4.
    Invariant(String),
}

impl RunError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::World(_) | Self::Texture(_) | Self::Asset(_) => 2,
            Self::Invariant(_) => 4,
            _ => 1,
        }
    }

    fn scene(error: crate::engine::scene::SceneError) -> Self {
        if error.is_texture() {
            Self::Texture(error.to_string())
        } else {
            Self::Scene(error.to_string())
        }
    }

    /// Map a frame-driver failure to its process exit class.
    ///
    /// T8 left this pending: a full tracker is an engine invariant (exit 4),
    /// never a generic runtime failure.
    fn engine(error: FrameError) -> Self {
        match error {
            FrameError::Tracker(inner) => Self::Invariant(inner.to_string()),
        }
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTextures => write!(
                f,
                "--play without --headless requires --textures <TEXTURES.BIG>"
            ),
            Self::World(error) => write!(f, "{error}"),
            Self::Scene(detail) => write!(f, "cannot build the render scene: {detail}"),
            Self::Clip(error) => write!(f, "{error}"),
            Self::Texture(detail) => write!(f, "cannot decode texture page: {detail}"),
            Self::Asset(detail) => write!(f, "cannot read the asset archive: {detail}"),
            Self::Window(detail) => write!(f, "cannot open the play window: {detail}"),
            Self::Invariant(detail) => write!(f, "engine invariant failed: {detail}"),
        }
    }
}

impl std::error::Error for RunError {}

/// Engine entry point.
///
/// The world and the static scene are loaded before a window exists. Headless
/// runs use [`NullBackend`] and a fixed tick sequence; windowed runs validate
/// every texture page used by the initial scene and only then create the winit
/// event loop.
pub fn run(options: &PlayOptions) -> Result<(), RunError> {
    if !options.headless && options.textures.is_none() {
        return Err(RunError::MissingTextures);
    }

    let world = match &options.group {
        Some(prefix) => World::load_group(&options.bigfile, prefix),
        None => World::load(&options.bigfile, &options.unit),
    }
    .map_err(RunError::World)?;
    let textures = if options.headless {
        None
    } else {
        options.textures.as_deref()
    };
    let (mut assets, bank) =
        SceneAssets::load(&options.bigfile, world.level.entry, textures).map_err(RunError::scene)?;
    let mut frame = FrameState::new(world);
    if let Some(position) = options.player_pos {
        // Test aid: override the authored record before the first tick. The
        // result is still a pure function of the same argv.
        let _placed = frame.place_player(position);
    }
    let animation = if options.headless {
        None
    } else {
        match options.sra.as_deref() {
            Some(path) => {
                let bytes = std::fs::read(path).map_err(|error| {
                    RunError::Asset(format!("cannot read {}: {error}", path.display()))
                })?;
                let slots = assets.bone_count(PLAYER_NAME).ok_or_else(|| {
                    RunError::Scene(format!(
                        "cannot resolve the player model '{PLAYER_NAME}' for --sra"
                    ))
                })?;
                Some(crate::sra::SraAnimation::load(bytes, slots).map_err(RunError::Scene)?)
            }
            None => None,
        }
    };
    let clip_selector = match &animation {
        Some(animation) => {
            let selection = options
                .clip_selection(animation.clip_count())
                .map_err(RunError::Clip)?;
            let landing_ticks = selection.map.landing_window(animation);
            Some(ClipSelector::new(selection, landing_ticks))
        }
        None => None,
    };

    if options.headless {
        let ticks = options.ticks.unwrap_or(60);
        if let Some(script) = &options.input_script {
            return run_scripted(frame, script, ticks, options, &mut assets);
        }
        return run_headless(frame, ticks, options, &mut assets);
    }

    run_windowed(
        frame,
        assets,
        bank.expect("windowed mode requested textures"),
        options,
        animation,
        clip_selector,
    )
}

/// The stable one-line headless acceptance contract.
///
/// The field order is part of the contract: `spec.md` states the same line and
/// the integration tests parse it. `digest` is the canonical FNV-1a 64 fold of
/// tick count plus the live-list order, class, position and visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadlessReport {
    pub unit: String,
    pub rooms: usize,
    pub placed: usize,
    pub admitted: usize,
    pub tracked: usize,
    pub skipped_markers: usize,
    pub inert: usize,
    pub unresolved: usize,
    pub live: usize,
    pub ticks: u64,
    pub digest: u64,
}

impl HeadlessReport {
    /// Render the exact contract line, digest as 16 lowercase hex digits.
    pub fn line(&self) -> String {
        format!(
            "engine unit={} rooms={} placed={} admitted={} tracked={} skipped_markers={} inert={} unresolved={} live={} ticks={} digest={:016x}",
            self.unit,
            self.rooms,
            self.placed,
            self.admitted,
            self.tracked,
            self.skipped_markers,
            self.inert,
            self.unresolved,
            self.live,
            self.ticks,
            self.digest,
        )
    }
}

/// The read-only `--cull-report` diagnostic contract.
///
/// Printed as its own line after the acceptance line, so the acceptance
/// contract is unchanged. `visible` counts the live objects whose `visible`
/// flag the last stage-6 pass left set; that is exactly the set the scene
/// copies into draw items. `activated + culled` is the live count at stage-6
/// time, before any spawn queued this tick lands in stage 18.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CullReport {
    pub eye: [i32; 3],
    pub radius: i32,
    pub live: usize,
    pub visible: usize,
    pub activated: usize,
    pub culled: usize,
}

impl CullReport {
    /// Render the exact diagnostic line. Field order is part of the contract.
    pub fn line(&self) -> String {
        format!(
            "cull eye={},{},{} radius={} live={} visible={} activated={} culled={}",
            self.eye[0],
            self.eye[1],
            self.eye[2],
            self.radius,
            self.live,
            self.visible,
            self.activated,
            self.culled,
        )
    }
}

/// Build the diagnostic from the frame state the last tick left behind.
///
/// Pure and read-only: it reads the view, the live arena and the stage-2
/// scratch block and changes nothing.
fn cull_report(frame: &FrameState) -> CullReport {
    let view = frame.view();
    let scratch = frame.scratch();
    CullReport {
        eye: view.eye,
        radius: view.radius,
        live: frame.live().len(),
        visible: frame
            .live()
            .iter()
            .filter(|(_, object)| object.visible)
            .count(),
        activated: scratch.activated(),
        culled: scratch.culled(),
    }
}

/// One-time placed-database counts produced by the headless boot pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SeedCounts {
    placed: usize,
    admitted: usize,
    skipped_markers: usize,
    inert: usize,
    unresolved: usize,
}

/// Shared boot path: seed the live world from the authored placed records.
///
/// Both the headless and the windowed mode call this single function, so the
/// simulated live set and the rendered live set cannot drift. In windowed mode
/// it runs before the initial page check and before [`EventLoop::new`], which
/// means a seed failure is raised while no window or event loop exists.
///
/// The spawn policy decides admission; markers never enter the tracker, inert
/// names are only counted, and an unresolved model is counted rather than
/// spawned. The player is already bound by [`FrameState::new`], so its record is
/// not spawned a second time. Registry order is the placed-array order, which is
/// deterministic.
fn boot_world(frame: &mut FrameState, bigfile: &Path) -> Result<SeedCounts, RunError> {
    let table = EngineNameTable::documented();
    let source = ImmediateSource::open(bigfile)
        .map_err(|error| RunError::Asset(error.to_string()))?;
    let mut assets = AssetStore::new(source);

    let player_bound = frame.player().is_some();
    let player = frame.player();
    let mut player_profile: Option<ActorProfile> = None;
    let mut counts = SeedCounts::default();
    let mut resolved: BTreeMap<String, (bool, Option<ActorProfile>, RenderClass)> = BTreeMap::new();
    let mut spawns: Vec<(String, RenderClass, [i32; 3], Option<ActorProfile>)> = Vec::new();

    let records = frame.world().placed.records_for_boot();
    counts.placed = records.len();
    for record in records {
        match table.admit(record) {
            Admission::MarkerOnly => {
                counts.skipped_markers += 1;
                continue;
            }
            Admission::Inert => {
                counts.inert += 1;
                continue;
            }
            Admission::System | Admission::Actor | Admission::BossSpecial | Admission::PlacedModel => {
                counts.admitted += 1;
            }
        }

        let name = &record.name;
        if player_bound && name.eq_ignore_ascii_case(PLAYER_NAME) {
            // `FrameState::new` already bound the player live object; only its
            // data-keyed actor carrier is still missing. A player model without
            // a container stays a non-actor, exactly like an unresolved spawn.
            let completion = assets
                .load(AssetRequest::for_name(name))
                .map_err(|error| RunError::Asset(error.to_string()))?;
            if let Some(profile) = completion.asset().and_then(|asset| asset.actor.clone()) {
                player_profile = Some(profile);
            }
            continue;
        }

        let (loaded, actor, class) = match resolved.get(name) {
            Some((loaded, actor, class)) => {
                (*loaded, actor.clone(), *class)
            }
            None => {
                let completion = assets
                    .load(AssetRequest::for_name(name))
                    .map_err(|error| RunError::Asset(error.to_string()))?;
                let loaded = completion.is_loaded();
                let asset = completion.asset();
                let actor = asset.and_then(|asset| asset.actor.clone());
                // The class was resolved at load-complete from the captured
                // class-flags word and padded prototype, before the actor
                // profile was parsed. The boot pass must not re-derive it.
                let class = asset.map_or(RenderClass::Unclassified, |asset| asset.class);
                if !loaded {
                    counts.unresolved += 1;
                }
                resolved.insert(name.clone(), (loaded, actor.clone(), class));
                (loaded, actor, class)
            }
        };
        if loaded {
            spawns.push((
                name.clone(),
                class,
                [
                    i32::from(record.position[0]),
                    i32::from(record.position[1]),
                    i32::from(record.position[2]),
                ],
                actor,
            ));
        }
    }

    for (name, class, position, actor) in spawns {
        frame.queue_spawn_with_class(name, Some(class), position, actor);
    }
    if let (Some(player), Some(profile)) = (player, player_profile) {
        let _attached = frame.attach_actor(player, profile);
    }
    Ok(counts)
}

/// Run the fixed tick sequence and build the acceptance report.
///
/// Test-only plain variant: the production path is
/// [`headless_run_with_actors`], which runs the same ticks plus the one-time
/// actor load. The digest is computed with the same [`world_state_hash`] the
/// determinism gate pins. Any tracker overflow becomes [`RunError::Invariant`]
/// (exit 4).
#[cfg(test)]
fn headless_run(
    frame: &mut FrameState,
    bigfile: &Path,
    ticks: u32,
) -> Result<HeadlessReport, RunError> {
    let counts = boot_world(frame, bigfile)?;
    finish_headless(frame, counts, ticks, 0)
}

/// Production headless path: boot, resolve and report per-actor animation, then
/// run the same fixed ticks.
///
/// The boot pass defers its spawns to the post-frame stage, so the first tick is
/// the point where the live list holds every boot actor. Running that tick is
/// not extra work: the remaining ticks start at 1. Loading animation only reads
/// files and never touches `FrameState`, so the digest cannot move.
fn headless_run_with_actors(
    frame: &mut FrameState,
    bigfile: &Path,
    ticks: u32,
    assets: &mut SceneAssets,
    options: &PlayOptions,
) -> Result<HeadlessReport, RunError> {
    let counts = boot_world(frame, bigfile)?;
    let mut start = 0;
    if ticks > 0 {
        frame.cull_to_player();
        frame
            .run_tick(ObjectTick::step(0))
            .map_err(RunError::engine)?;
        let _actors = load_actor_animations(frame, assets, options)?;
        start = 1;
    }
    finish_headless(frame, counts, ticks, start)
}

/// The fixed-tick tail shared by the plain and per-actor headless paths.
fn finish_headless(
    frame: &mut FrameState,
    counts: SeedCounts,
    ticks: u32,
    start: u64,
) -> Result<HeadlessReport, RunError> {
    for index in start..u64::from(ticks) {
        frame.cull_to_player();
        frame
            .run_tick(ObjectTick::step(index))
            .map_err(RunError::engine)?;
    }
    Ok(HeadlessReport {
        unit: frame.world().level.name.clone(),
        rooms: frame.world().level.rooms,
        placed: counts.placed,
        admitted: counts.admitted,
        tracked: frame.tracker().ready_slots().count(),
        skipped_markers: counts.skipped_markers,
        inert: counts.inert,
        unresolved: counts.unresolved,
        live: frame.live().len(),
        ticks: frame.ticks(),
        digest: world_state_hash(&frame.world_state()),
    })
}

fn run_headless(
    mut frame: FrameState,
    ticks: u32,
    options: &PlayOptions,
    assets: &mut SceneAssets,
) -> Result<(), RunError> {
    let report =
        headless_run_with_actors(&mut frame, &options.bigfile, ticks, assets, options)?;
    outln!("{}", report.line());
    if options.cull_report {
        outln!("{}", cull_report(&frame).line());
    }
    Ok(())
}

/// Load per-actor animation when `--sra-dir` is active, printing the bounded
/// load-time diagnostic. Without a directory this is an empty set and prints
/// nothing, which keeps the default headless contract identical.
fn load_actor_animations(
    frame: &FrameState,
    assets: &mut SceneAssets,
    options: &PlayOptions,
) -> Result<ActorAnimations, RunError> {
    if options.sra_dir.is_none() {
        return Ok(ActorAnimations::default());
    }
    let stores = options.labels_store_sources();
    // `--sra` is the player's single-actor override: when it is present the
    // per-actor set must not also bind the player, or the override wins only by
    // chance of ordering. The player keeps the legacy selector.
    let exclude = options.sra.is_some().then(|| frame.player()).flatten();
    let actors = ActorAnimations::load(
        frame,
        assets,
        options.sra_dir.as_deref(),
        &stores,
        exclude,
    )
    .map_err(RunError::Scene)?;
    for line in actors.diagnostic_lines() {
        outln!("{line}");
    }
    Ok(actors)
}

/// The stable one-line `--input-script` diagnostic contract.
///
/// `blocked_ticks` counts ticks where the sampled movement axis was non-zero
/// but the committed horizontal delta was zero. That single counter separates
/// "the input never reached the simulation" (low, and the player moves) from
/// "the sweep refused every step" (equal to the moving ticks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovementReport {
    pub ticks: u64,
    pub start: [i32; 3],
    pub end: [i32; 3],
    pub blocked_ticks: u64,
    pub grounded: bool,
    pub player_bound: bool,
}

impl MovementReport {
    /// Render the exact diagnostic line. Field order is part of the contract.
    pub fn line(&self) -> String {
        format!(
            "movement ticks={} start={},{},{} end={},{},{} dx={} dy={} dz={} blocked_ticks={} grounded={} player_bound={}",
            self.ticks,
            self.start[0],
            self.start[1],
            self.start[2],
            self.end[0],
            self.end[1],
            self.end[2],
            self.end[0] - self.start[0],
            self.end[1] - self.start[1],
            self.end[2] - self.start[2],
            self.blocked_ticks,
            u8::from(self.grounded),
            u8::from(self.player_bound),
        )
    }
}

/// Queue one scripted tick's key events before the tick runs.
fn apply_script_tick(frame: &mut FrameState, script: &InputScript, tick: u64) {
    for event in script.events_at(tick) {
        frame.queue_input(event);
    }
}

/// Run `ticks` fixed steps with `script` applied from tick 0 and measure the
/// player's committed movement. Pure with respect to I/O: tests can call it on
/// a synthetic [`FrameState`] without an archive or a window.
pub fn run_script_ticks(
    frame: &mut FrameState,
    script: &InputScript,
    ticks: u64,
) -> Result<MovementReport, RunError> {
    let start = frame.player_position().unwrap_or([0; 3]);
    let mut blocked_ticks = 0u64;
    for index in 0..ticks {
        let before = frame.player_position();
        apply_script_tick(frame, script, index);
        frame.cull_to_player();
        frame
            .run_tick(ObjectTick::step(index))
            .map_err(RunError::engine)?;
        let after = frame.player_position();
        let [right, forward] = frame.current_input().move_axis();
        if (right != 0 || forward != 0)
            && let (Some(before), Some(after)) = (before, after)
            && after[0] == before[0]
            && after[1] == before[1]
        {
            blocked_ticks = blocked_ticks.saturating_add(1);
        }
    }
    Ok(MovementReport {
        ticks: frame.ticks(),
        start,
        end: frame.player_position().unwrap_or(start),
        blocked_ticks,
        grounded: frame.player_grounded(),
        player_bound: frame.player().is_some(),
    })
}

fn run_scripted(
    mut frame: FrameState,
    script: &InputScript,
    ticks: u32,
    options: &PlayOptions,
    assets: &mut SceneAssets,
) -> Result<(), RunError> {
    let _counts = boot_world(&mut frame, &options.bigfile)?;
    let report = run_script_ticks(&mut frame, script, u64::from(ticks))?;
    // The scripted run has already realized every deferred spawn. Resolving the
    // one-time actor diagnostic here is still presentation-only and leaves the
    // movement report and state untouched.
    let _actors = load_actor_animations(&frame, assets, options)?;
    outln!("{}", report.line());
    if options.cull_report {
        outln!("{}", cull_report(&frame).line());
    }
    Ok(())
}

fn run_windowed(
    frame: FrameState,
    assets: SceneAssets,
    bank: PageBank,
    options: &PlayOptions,
    animation: Option<crate::sra::SraAnimation>,
    clip_selector: Option<ClipSelector>,
) -> Result<(), RunError> {
    boot_before_window(frame, &options.bigfile, |frame| {
        open_window(frame, assets, bank, options, animation, clip_selector)
    })
}

/// Run the shared boot pass, then hand the booted frame to `windowed`.
///
/// The continuation is the only route to [`PlayApp`] and, through it, to
/// [`EventLoop::new`]. Because [`boot_world`] runs first and `?` returns on
/// failure, a seeding error is raised while no window or event loop exists.
/// The continuation is a closure so the ordering is testable without a
/// graphics stack.
fn boot_before_window<F>(mut frame: FrameState, bigfile: &Path, windowed: F) -> Result<(), RunError>
where
    F: FnOnce(FrameState) -> Result<(), RunError>,
{
    boot_world(&mut frame, bigfile)?;
    windowed(frame)
}

/// Windowed continuation: validate the initial pages, build the app and drive
/// the event loop. Reached only after [`boot_world`] succeeded.
fn open_window(
    frame: FrameState,
    mut assets: SceneAssets,
    bank: PageBank,
    options: &PlayOptions,
    animation: Option<crate::sra::SraAnimation>,
    clip_selector: Option<ClipSelector>,
) -> Result<(), RunError> {
    let mode = options.camera;
    let unit = options.unit.as_str();
    // The boot spawns land on the first tick, so per-actor animation is resolved
    // lazily then (one pass, never per frame) instead of seeing only the player.
    let actor_dir = options.sra_dir.clone();
    let actor_store = options.labels_store_sources();
    let actor_exclude = options.sra.is_some().then(|| frame.player()).flatten();
    // The initial scene is the complete set of pages the first frame draws.
    // Decoding them here keeps a bad page strictly before the window.
    {
        let initial = assets.build(frame.live(), CameraView::identity());
        for page in initial.material_pages.iter().flatten() {
            bank.decode_page(*page)
                .map_err(|error| RunError::Texture(error.to_string()))?;
        }
    }

    // The shipped level data is Z-up (see `crate::engine::axis`), so the engine
    // window's free cameras use a Z-up basis.
    let start = match assets.render_spawn() {
        Some(spawn) => FlyCamera::looking_at_with_up(spawn, [0.0; 3], true),
        None => FlyCamera::new([0.0; 3]).z_up(),
    };
    let event_loop =
        EventLoop::new().map_err(|error| RunError::Window(format!("event loop: {error}")))?;
    let mut app = PlayApp {
        frame,
        assets,
        bank: Some(bank),
        backend: None,
        window: None,
        mode,
        orbit: OrbitCamera::new().z_up(),
        fly: start,
        start,
        keys: CameraKeys::default(),
        mouse_last: None,
        drag: None,
        cam_distance: options.cam_distance.unwrap_or(PLAYER_CAMERA_DISTANCE),
        cam_height: options.cam_height.unwrap_or(PLAYER_CAMERA_HEIGHT),
        player_orbit_yaw: 0.0,
        player_orbit_pitch: 0.0,
        last_frame: Instant::now(),
        last_dt: 0.0,
        init_error: None,
        title: format!("soul reaver - {unit}"),
        animation,
        clip_selector,
        actors: ActorAnimations::default(),
        actors_loaded: actor_dir.is_none(),
        actor_dir,
        actor_store,
        actor_exclude,
        anim_fps: options.anim_fps,
        anim_policy: if options.anim_once {
            PlaybackLoop::Once
        } else {
            PlaybackLoop::Loop
        },
    };
    event_loop
        .run_app(&mut app)
        .map_err(|error| RunError::Window(format!("event loop: {error}")))?;
    match app.init_error {
        Some(detail) => Err(RunError::Window(detail)),
        None => Ok(()),
    }
}

/// Held camera keys, mirroring the legacy viewer's WASD/QE/Space/Shift map.
#[derive(Default)]
struct CameraKeys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    fast: bool,
}

impl CameraKeys {
    fn update(&mut self, event: &KeyEvent) {
        let held = event.state == ElementState::Pressed;
        match &event.logical_key {
            Key::Character(text) => match text.to_ascii_lowercase().as_str() {
                "w" => self.forward = held,
                "s" => self.back = held,
                "a" => self.left = held,
                "d" => self.right = held,
                "e" => self.up = held,
                "c" | "q" => self.down = held,
                _ => {}
            },
            Key::Named(NamedKey::Space) => self.up = held,
            Key::Named(NamedKey::Shift) => self.fast = held,
            _ => {}
        }
    }

    fn axes(&self) -> (f32, f32, f32) {
        let axis = |on: bool, off: bool| f32::from(on as u8) - f32::from(off as u8);
        (
            axis(self.forward, self.back),
            axis(self.right, self.left),
            axis(self.up, self.down),
        )
    }
}

/// Map a key to a player input button in the engine window.
///
/// Player mode is `WASD` for the direct translation axes and `Space` for the
/// jump edge. The jump rule itself is stage 9; this binding only queues the edge.
fn player_button(key: &Key) -> Option<InputButton> {
    match key {
        Key::Character(text) => match text.to_ascii_lowercase().as_str() {
            "w" => Some(InputButton::Forward),
            "s" => Some(InputButton::Back),
            "a" => Some(InputButton::Left),
            "d" => Some(InputButton::Right),
            _ => None,
        },
        Key::Named(NamedKey::Space) => Some(InputButton::Jump),
        _ => None,
    }
}

/// Third-person boom distance behind the player, in world units.
///
/// The player model measures ~509 world units from feet to head. The view
/// matrix uses a 45° vertical field of view (`1 / tan(PI/8)`), so a boom of 900
/// frames roughly 745 units of height at the look plane: the whole body fits
/// with headroom. The previous 600-unit boom framed under 500 units and, with
/// the old 60-unit look height, aimed at the ankles, which is why the window
/// showed only legs and hands.
pub const PLAYER_CAMERA_DISTANCE: f32 = 900.0;

/// Third-person eye height above the player's feet, in world units.
///
/// A little above the 509-unit head, so the view is a gentle downward angle
/// that sees the whole body instead of a top-down or ground-level shot.
pub const PLAYER_CAMERA_HEIGHT: f32 = 520.0;

/// Height of the look target above the player's feet, in world units.
///
/// Mid-torso (~55% of the 509-unit body), so the frame centre is the body and
/// the character is not cropped at the waist.
pub const PLAYER_LOOK_HEIGHT: f32 = 280.0;

/// Fallback binary yaw used before the player has moved.
///
/// Direct forward input moves the player along world `+Y`, so the initial boom
/// sits behind on world `-Y`, exactly where the previous fixed boom was.
pub const PLAYER_FALLBACK_YAW_BINARY: f32 = pose::BINARY_ANGLE_UNITS_PER_TURN / 4.0;

/// Mouse orbit sensitivity for the player boom, in radians per pixel.
pub const PLAYER_ORBIT_SENSITIVITY: f32 = 0.006;

/// Pitch clamp for the player boom, in radians.
pub const PLAYER_ORBIT_PITCH_LIMIT: f32 = 1.2;

/// Build the third-person view matrix for the Z-up world.
///
/// `eye` looks at `target` and screen up is world `+Z`. `right` is the
/// horizontal cross product of `forward` with world up and `up` is their cross
/// product, so the basis stays orthonormal even when the eye is directly above
/// the target. This is presentation only: it never reads or writes simulation
/// state, so determinism is unaffected.
fn third_person_mvp(eye: [f32; 3], target: [f32; 3], aspect: f32) -> [[f32; 4]; 4] {
    let dir = [
        target[0] - eye[0],
        target[1] - eye[1],
        target[2] - eye[2],
    ];
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let forward = if len > 1e-6 {
        [dir[0] / len, dir[1] / len, dir[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    };
    let mut right = [forward[1], -forward[0], 0.0];
    let rlen = (right[0] * right[0] + right[1] * right[1]).sqrt();
    if rlen > 1e-6 {
        right = [right[0] / rlen, right[1] / rlen, 0.0];
    } else {
        right = [1.0, 0.0, 0.0];
    }
    let up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    render::view_proj(eye, (right, up), forward, 0.02, 100.0, aspect)
}

/// World-space eye and look target for the third-person player camera.
///
/// The boom follows the player's produced yaw, so walking forward keeps the
/// camera behind him. `yaw_binary` is the binary angle (`4096` units per turn)
/// from [`FrameState::player_yaw_binary`]; `None` means the player has not
/// moved yet and the fallback `+Y` forward is used.
///
/// `distance`, `height`, `yaw_offset` and `pitch` are presentation inputs: the
/// two scalars come from `--cam-distance`/`--cam-height` (or the defaults) and
/// the two offsets from the mouse orbit. The eye sits on the horizontal circle
/// behind the player (`-forward`), `height` above the feet, and is raised or
/// lowered by `distance * sin(pitch)`. The target is `PLAYER_LOOK_HEIGHT` above
/// the feet. This never reads or writes simulation state.
fn player_camera_world(
    player: [f32; 3],
    yaw_binary: Option<f32>,
    distance: f32,
    height: f32,
    yaw_offset: f32,
    pitch: f32,
) -> ([f32; 3], [f32; 3]) {
    let base_yaw = yaw_binary
        .map(pose::binary_angle_to_radians)
        .unwrap_or_else(|| pose::binary_angle_to_radians(PLAYER_FALLBACK_YAW_BINARY));
    let yaw = base_yaw + yaw_offset;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let horizontal = distance * cos_pitch;
    let target = [player[0], player[1], player[2] + PLAYER_LOOK_HEIGHT];
    let eye = [
        player[0] - cos_yaw * horizontal,
        player[1] - sin_yaw * horizontal,
        player[2] + height + distance * sin_pitch,
    ];
    (eye, target)
}

struct PlayApp {
    frame: FrameState,
    assets: SceneAssets,
    bank: Option<PageBank>,
    backend: Option<WgpuBackend>,
    window: Option<Arc<Window>>,
    mode: CameraMode,
    orbit: OrbitCamera,
    fly: FlyCamera,
    start: FlyCamera,
    keys: CameraKeys,
    mouse_last: Option<PhysicalPosition<f64>>,
    drag: Option<MouseButton>,
    /// Player-boom distance in world units (`--cam-distance` or the default).
    cam_distance: f32,
    /// Player-boom eye height in world units (`--cam-height` or the default).
    cam_height: f32,
    /// Mouse yaw offset around the player boom, in radians. Presentation only.
    player_orbit_yaw: f32,
    /// Mouse pitch offset around the player boom, in radians. Presentation only.
    player_orbit_pitch: f32,
    last_frame: Instant,
    last_dt: f32,
    init_error: Option<String>,
    title: String,
    animation: Option<crate::sra::SraAnimation>,
    /// State-driven clip selector; `None` when `--sra` is inactive.
    clip_selector: Option<ClipSelector>,
    /// Per-actor animations from `--sra-dir`; empty when it is inactive.
    actors: ActorAnimations,
    /// Whether the one-time per-actor load has run (or cannot run).
    actors_loaded: bool,
    /// `--sra-dir`, kept until the first tick realizes the boot spawns.
    actor_dir: Option<PathBuf>,
    /// `--labels-store` or the merged repository-plus-home sources.
    actor_store: Vec<PathBuf>,
    /// Player id excluded from the per-actor set when `--sra` is its override.
    actor_exclude: Option<crate::engine::live::LiveId>,
    anim_fps: u32,
    anim_policy: PlaybackLoop,
}

impl PlayApp {
    fn aspect(&self) -> f32 {
        self.backend
            .as_ref()
            .map(|backend| backend.aspect())
            .unwrap_or(1.0)
    }

    /// The player's produced root facing for the scene builder, if it has moved.
    ///
    /// When `--sra` is active the selector observes the deterministic grounded,
    /// moving and signed-vertical-velocity signals once per rendered frame and
    /// maps them to one of the five motion states. The playback clock consumes
    /// the clip's local tick, so a state change restarts the new clip at frame
    /// 0. Everything here is presentation-only: it is not part of `FrameState`
    /// and never enters the canonical digest.
    fn player_facing(&mut self) -> Option<PlayerFacing> {
        let player = self.frame.player()?;
        let yaw = self.frame.player_yaw_binary();
        let tick = self.frame.ticks();
        let grounded = self.frame.player_grounded();
        let moving = self.frame.player_moving();
        let vertical_velocity = self.frame.player_vertical_velocity();
        if let Some(selector) = self.clip_selector.as_mut() {
            selector.observe_signals(tick, grounded, moving, vertical_velocity);
        }
        let animation = match (&self.animation, &self.clip_selector) {
            (Some(animation), Some(selector)) => animation
                .poses_for_clip_tick(
                    selector.clip(),
                    selector.local_tick(tick),
                    units_per_second_for_fps(self.anim_fps),
                    self.anim_policy,
                )
                .ok(),
            _ => None,
        };
        if yaw.is_none() && animation.is_none() {
            return None;
        }
        Some(PlayerFacing {
            player,
            yaw_binary: yaw.unwrap_or(0.0),
            animation,
        })
    }

    /// Every live actor's pose for this frame.
    ///
    /// Per-actor animations from `--sra-dir` are observed first; the legacy
    /// single-file `--sra` player pose is added only when the per-actor set did
    /// not already cover that id. The player still turns on its produced yaw
    /// even when it has no clip, exactly as the no-animation path did.
    fn actor_poses(&mut self) -> Vec<ActorPose> {
        let tick = self.frame.ticks();
        let player = self.frame.player();
        let player_yaw = self.frame.player_yaw_binary();
        let fps = self.anim_fps;
        let policy = self.anim_policy;
        let mut poses = self.actors.poses(
            self.frame.live(),
            tick,
            fps,
            policy,
            player,
            player_yaw,
        );
        if let Some(facing) = self.player_facing()
            && !poses.iter().any(|pose| pose.actor == facing.player)
        {
            poses.push(ActorPose {
                actor: facing.player,
                yaw_binary: facing.yaw_binary,
                animation: facing.animation,
            });
        }
        poses
    }

    /// Resolve per-actor animation once, after the first tick realized the
    /// boot spawns. Skipped when no directory was supplied.
    fn load_actors_if_ready(&mut self) -> Result<(), String> {
        if self.actors_loaded || self.frame.ticks() == 0 {
            return Ok(());
        }
        self.actors_loaded = true;
        let Some(directory) = self.actor_dir.as_deref() else {
            return Ok(());
        };
        let actors = ActorAnimations::load(
            &self.frame,
            &mut self.assets,
            Some(directory),
            &self.actor_store,
            self.actor_exclude,
        )?;
        for line in actors.diagnostic_lines() {
            outln!("{line}");
        }
        self.actors = actors;
        Ok(())
    }

    fn camera_view(&mut self, aspect: f32) -> CameraView {
        match self.mode {
            CameraMode::Orbit => {
                self.orbit.tick(self.last_dt);
                CameraView {
                    mvp: self.orbit.mvp(aspect),
                }
            }
            CameraMode::Fly => {
                let (forward, strafe, rise) = self.keys.axes();
                self.fly
                    .fly(forward, strafe, rise, self.last_dt, self.keys.fast);
                CameraView {
                    mvp: self.fly.mvp(aspect),
                }
            }
            CameraMode::Player => {
                // Third-person follow: the boom sits behind the player along
                // its produced yaw and above it on world +Z, and tracks both
                // every frame. The horizontal boom lies in the X/Y plane; the
                // mouse offsets orbit it without touching simulation state.
                let player = self.frame.player_position().map_or(
                    [0.0; 3],
                    |position| {
                        [
                            position[0] as f32,
                            position[1] as f32,
                            position[2] as f32,
                        ]
                    },
                );
                let (eye_world, target_world) = player_camera_world(
                    player,
                    self.frame.player_yaw_binary(),
                    self.cam_distance,
                    self.cam_height,
                    self.player_orbit_yaw,
                    self.player_orbit_pitch,
                );
                let transform = self.assets.transform();
                CameraView {
                    mvp: third_person_mvp(
                        transform.apply(eye_world),
                        transform.apply(target_world),
                        aspect,
                    ),
                }
            }
        }
    }

    fn set_mode(&mut self, window: &Window, mode: CameraMode) {
        self.mode = mode;
        for button in InputButton::ALL {
            self.frame.queue_input(InputEvent::release(button));
        }
        render::set_fly_cursor(window, mode == CameraMode::Fly);
    }

    /// Track held keys, cycle camera mode with `F`, move the player in player
    /// mode, and exit on `Esc`.
    fn handle_key(&mut self, window: &Window, event: &KeyEvent) -> bool {
        self.keys.update(event);
        if event.state == ElementState::Pressed && !event.repeat {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => return true,
                Key::Character(text) if text.eq_ignore_ascii_case("f") => {
                    self.set_mode(window, self.mode.next());
                }
                Key::Character(text) if text.eq_ignore_ascii_case("r") => {
                    self.orbit.reset();
                    self.fly = self.start;
                }
                _ => {}
            }
        }
        if self.mode == CameraMode::Player && !event.repeat {
            if let Some(button) = player_button(&event.logical_key) {
                let input = if event.state == ElementState::Pressed {
                    InputEvent::press(button)
                } else {
                    InputEvent::release(button)
                };
                self.frame.queue_input(input);
            }
        }
        false
    }

    /// Mouse orbit/pan/zoom in orbit mode, look in fly mode, and orbit the
    /// third-person boom in player mode.
    fn handle_mouse(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                if matches!(self.mode, CameraMode::Orbit | CameraMode::Player) {
                    if *state == ElementState::Pressed {
                        self.drag = Some(*button);
                    } else if self.drag == Some(*button) {
                        self.drag = None;
                    }
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(previous) = self.mouse_last {
                    let dx = (position.x - previous.x) as f32;
                    let dy = (position.y - previous.y) as f32;
                    match self.mode {
                        CameraMode::Fly => self.fly.look(dx, dy),
                        CameraMode::Orbit => match self.drag {
                            Some(MouseButton::Left) => self.orbit.apply_orbit(dx, dy),
                            Some(MouseButton::Right) => self.orbit.apply_pan(dx, dy),
                            _ => {}
                        },
                        CameraMode::Player => {
                            if self.drag.is_some() {
                                // Yaw offset and pitch are presentation-only:
                                // they change the boom, never the player.
                                self.player_orbit_yaw -= dx * PLAYER_ORBIT_SENSITIVITY;
                                self.player_orbit_pitch = (self.player_orbit_pitch
                                    + dy * PLAYER_ORBIT_SENSITIVITY)
                                    .clamp(
                                        -PLAYER_ORBIT_PITCH_LIMIT,
                                        PLAYER_ORBIT_PITCH_LIMIT,
                                    );
                            }
                        }
                    }
                }
                self.mouse_last = Some(*position);
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.mode == CameraMode::Orbit {
                    let notches = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(pixel) => (pixel.y / 50.0) as f32,
                    };
                    self.orbit.apply_zoom(notches);
                }
                true
            }
            _ => false,
        }
    }
}

impl ApplicationHandler for PlayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(&self.title)
                .with_inner_size(PhysicalSize::new(800, 600)),
        ) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.init_error = Some(format!("cannot create window: {error}"));
                event_loop.exit();
                return;
            }
        };
        let Some(bank) = self.bank.take() else {
            self.init_error = Some("texture bank missing".to_string());
            event_loop.exit();
            return;
        };
        match WgpuBackend::new(window.clone(), bank) {
            Ok(backend) => {
                render::set_fly_cursor(&window, self.mode == CameraMode::Fly);
                self.backend = Some(backend);
                self.window = Some(window);
            }
            Err(error) => {
                self.init_error = Some(error.to_string());
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if window_id != window.id() {
            return;
        }
        if self.handle_mouse(&event) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(backend) = &mut self.backend {
                    backend.resize(size.width, size.height);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let aspect = self.aspect();
                let camera = self.camera_view(aspect);
                let poses = self.actor_poses();
                let live = self.frame.live();
                let scene = self.assets.build_with_poses(live, camera, &poses);
                if let Some(backend) = &mut self.backend {
                    if let Err(error) = backend.draw(&scene) {
                        self.init_error = Some(error.to_string());
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Focused(false) => self.keys = CameraKeys::default(),
            WindowEvent::KeyboardInput { event, .. } => {
                if self.handle_key(&window, &event) {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.last_dt = dt;
        if let Err(error) = self.frame.frame(Duration::from_secs_f32(dt)) {
            self.init_error = Some(format!("simulation tick failed: {error}"));
            event_loop.exit();
            return;
        }
        if let Err(error) = self.load_actors_if_ready() {
            self.init_error = Some(error);
            event_loop.exit();
            return;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CameraMode, CullReport, InputScript, PLAYER_CAMERA_DISTANCE, PLAYER_CAMERA_HEIGHT,
        PLAYER_FALLBACK_YAW_BINARY, PLAYER_LOOK_HEIGHT, PlayError, PlayOptions, RunError,
        apply_script_tick, cull_report, player_camera_world, run_script_ticks, third_person_mvp,
    };
    use crate::engine::clock::ObjectTick;
    use crate::engine::collision::CollisionMesh;
    use crate::engine::frame::FrameState;
    use crate::engine::input::InputButton;
    use crate::engine::world::World;
    use crate::level::{ObjectRecord, ObjectReport};
    use crate::sra::{ClipError, ClipMap};
    use std::path::PathBuf;

    fn argv(input: &[&str]) -> Vec<String> {
        input.iter().map(|token| token.to_string()).collect()
    }

    fn parse(input: &[&str]) -> Result<PlayOptions, PlayError> {
        PlayOptions::parse(&argv(input))
    }

    fn report(objects: Vec<ObjectRecord>) -> ObjectReport {
        ObjectReport {
            room_offset: 0,
            count: objects.len() as u32,
            array_offset: 0,
            objects,
        }
    }

    fn record(name: &str, position: [i16; 3]) -> ObjectRecord {
        ObjectRecord {
            name: name.to_string(),
            position,
            object_type: 4,
            archetype: 0,
            instance_id: 1,
            flags: 0,
        }
    }

    /// A world that places the player but carries no collision mesh.
    fn free_world() -> World {
        World::from_report(
            "under1".to_string(),
            1,
            &report(vec![record("raziel", [0, 0, 0])]),
        )
    }

    /// A world with one constant-Y wall triangle in front of the player, so the
    /// horizontal sweep has no room to step along +Y.
    fn wall_world() -> World {
        let mut world = free_world();
        world.collision = CollisionMesh::from_room_triangles(
            &[[-100, 2, -100], [100, 2, -100], [0, 2, 100]],
            &[[0, 1, 2]],
        );
        world
    }

    fn playerless_world() -> World {
        World::from_report("under1".to_string(), 1, &report(Vec::new()))
    }

    #[test]
    fn defaults_fill_unit_camera_and_ticks() {
        let options = parse(&["--bigfile", "BIGFILE.DAT"]).expect("defaults must parse");
        assert_eq!(options.bigfile, PathBuf::from("BIGFILE.DAT"));
        assert_eq!(options.textures, None);
        assert_eq!(options.unit, "under1");
        assert_eq!(options.ticks, None);
        assert!(!options.headless);
        assert_eq!(options.camera, CameraMode::Fly);
        assert_eq!(options.sra, None);
        assert_eq!(options.sra_dir, None);
        assert_eq!(options.labels_store, None);
        assert_eq!(
            options.labels_store_path(),
            PathBuf::from(PlayOptions::REPO_LABELS_STORE),
            "the versioned repository copy wins over the home path"
        );
        assert!(
            options
                .labels_store_path_with(std::path::Path::new("missing-labels.json"))
                .ends_with(".sr1_clip_labels.json"),
            "a missing repository copy falls back to the probe's home path"
        );
        assert_eq!(options.anim_fps, super::ANIMATION_FPS);
        assert!(!options.anim_once);
        assert_eq!(
            (
                options.clip,
                options.clip_idle,
                options.clip_walk,
                options.clip_jump,
                options.clip_jump_up,
                options.clip_fall,
                options.clip_land
            ),
            (None, None, None, None, None, None, None)
        );
        assert_eq!(options.input_script, None);
        assert_eq!(options.player_pos, None);
        assert_eq!(options.cam_distance, None);
        assert_eq!(options.cam_height, None);
    }

    #[test]
    fn every_option_is_parsed() {
        let options = parse(&[
            "--bigfile",
            "BIGFILE.DAT",
            "--textures",
            "TEXTURES.BIG",
            "--unit",
            "under2",
            "--ticks",
            "120",
            "--headless",
            "--camera",
            "player",
            "--sra",
            "RAZIEL.SRA",
            "--sra-dir",
            "OBJECT",
            "--labels-store",
            "labels.json",
            "--anim-fps",
            "20",
            "--anim-once",
        ])
        .expect("full argv must parse");
        assert_eq!(options.bigfile, PathBuf::from("BIGFILE.DAT"));
        assert_eq!(options.textures, Some(PathBuf::from("TEXTURES.BIG")));
        assert_eq!(options.unit, "under2");
        assert_eq!(options.ticks, Some(120));
        assert!(options.headless);
        assert_eq!(options.camera, CameraMode::Player);
        assert_eq!(options.sra, Some(PathBuf::from("RAZIEL.SRA")));
        assert_eq!(options.sra_dir, Some(PathBuf::from("OBJECT")));
        assert_eq!(options.labels_store, Some(PathBuf::from("labels.json")));
        assert_eq!(options.anim_fps, 20);
        assert!(options.anim_once);
    }

    #[test]
    fn group_is_parsed_and_is_mutually_exclusive_with_unit() {
        let options = parse(&["--bigfile", "B", "--group", "cathy"])
            .expect("a group prefix must parse");
        assert_eq!(options.group.as_deref(), Some("cathy"));
        assert_eq!(
            options.unit, "under1",
            "the default unit stays unset; the group selects the rooms"
        );

        assert_eq!(
            parse(&[
                "--bigfile", "B", "--group", "cathy", "--unit", "under1"
            ]),
            Err(PlayError::UnitAndGroup),
            "the two selectors are mutually exclusive"
        );
        assert!(
            PlayError::UnitAndGroup
                .to_string()
                .contains("mutually exclusive")
        );

        let explicit = parse(&["--bigfile", "B", "--unit", "under2"]).expect("unit parses");
        assert_eq!(explicit.group, None);
    }

    #[test]
    fn label_store_lookup_is_explicit_then_repo_then_home() {
        let dir = tempfile::tempdir().expect("temp dir");
        let repo = dir.path().join("clip-labels.json");
        std::fs::write(&repo, "{}").expect("write the repository copy");

        let explicit = parse(&["--bigfile", "B", "--labels-store", "mine.json"])
            .expect("explicit store parses");
        assert_eq!(
            explicit.labels_store_path_with(&repo),
            PathBuf::from("mine.json"),
            "an explicit --labels-store wins even over an existing repo copy"
        );

        let default = parse(&["--bigfile", "B"]).expect("defaults parse");
        assert_eq!(
            default.labels_store_path_with(&repo),
            repo,
            "an existing repository copy is second in the lookup"
        );

        let missing = dir.path().join("absent.json");
        assert!(
            default
                .labels_store_path_with(&missing)
                .ends_with(".sr1_clip_labels.json"),
            "a missing repository copy falls through to the home path"
        );
    }

    #[test]
    fn label_store_sources_merge_repo_and_home_by_default() {
        let dir = tempfile::tempdir().expect("temp dir");
        let repo = dir.path().join("clip-labels.json");
        std::fs::write(&repo, "{}").expect("write the repository copy");

        let explicit = parse(&["--bigfile", "B", "--labels-store", "mine.json"])
            .expect("explicit store parses");
        assert_eq!(
            explicit.labels_store_sources_with(&repo),
            vec![PathBuf::from("mine.json")],
            "an explicit --labels-store is the only source"
        );

        let default = parse(&["--bigfile", "B"]).expect("defaults parse");
        let sources = default.labels_store_sources_with(&repo);
        assert_eq!(sources.len(), 2, "both copies are read and merged");
        assert_eq!(sources[0], repo, "the repository copy is the base");
        assert!(
            sources[1].ends_with(".sr1_clip_labels.json"),
            "the probe's home copy is the overlay"
        );

        let missing = dir.path().join("absent.json");
        let home_only = default.labels_store_sources_with(&missing);
        assert_eq!(home_only.len(), 1, "no repository copy means home only");
        assert!(home_only[0].ends_with(".sr1_clip_labels.json"));
    }

    #[test]
    fn clip_options_parse_and_resolve_against_the_clip_count() {
        let options = parse(&[
            "--bigfile",
            "B",
            "--sra",
            "R.SRA",
            "--clip-idle",
            "2",
            "--clip-walk",
            "4",
            "--clip-jump",
            "6",
        ])
        .expect("clip map must parse");
        let selection = options.clip_selection(10).expect("indices are in range");
        assert_eq!(
            selection.map,
            ClipMap {
                idle: 2,
                walk: 4,
                jump_up: 6,
                fall: 6,
                land: 6,
            },
            "--clip-jump is the all-three shortcut"
        );
        assert_eq!(selection.fixed, None);

        let fixed = parse(&["--bigfile", "B", "--clip", "7"]).expect("fixed clip parses");
        assert_eq!(fixed.clip_selection(10).expect("in range").fixed, Some(7));

        let defaults = parse(&["--bigfile", "B"]).expect("defaults parse");
        assert_eq!(
            defaults.clip_selection(10).expect("in range").map,
            ClipMap {
                idle: 9,
                walk: 2,
                jump_up: 9,
                fall: 9,
                land: 9,
            },
            "the measured defaults clamp to the last decoded clip"
        );
        assert_eq!(
            defaults.clip_selection(230).expect("in range").map,
            ClipMap {
                idle: 55,
                walk: 2,
                jump_up: 35,
                fall: 36,
                land: 37,
            },
            "the shipped player resolves the five measured indices"
        );

        let bad_map = parse(&["--bigfile", "B", "--clip-walk", "10"]).expect("parses");
        assert_eq!(
            bad_map.clip_selection(10),
            Err(ClipError::OutOfRange {
                clip: 10,
                clip_count: 10
            })
        );
        let bad_fixed = parse(&["--bigfile", "B", "--clip", "11"]).expect("parses");
        assert_eq!(
            bad_fixed.clip_selection(10),
            Err(ClipError::OutOfRange {
                clip: 11,
                clip_count: 10
            })
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--clip", "x"]),
            Err(PlayError::InvalidClip("x".to_string()))
        );
    }

    #[test]
    fn each_clip_override_replaces_only_its_own_default() {
        let defaults = ClipMap::for_clip_count(230);
        assert_eq!(
            defaults,
            ClipMap {
                idle: 55,
                walk: 2,
                jump_up: 35,
                fall: 36,
                land: 37,
            },
            "the documented default map"
        );

        let idle = parse(&["--bigfile", "B", "--clip-idle", "64"])
            .expect("parses")
            .clip_selection(230)
            .expect("in range")
            .map;
        assert_eq!(
            idle,
            ClipMap {
                idle: 64,
                walk: 2,
                jump_up: 35,
                fall: 36,
                land: 37,
            }
        );

        let walk = parse(&["--bigfile", "B", "--clip-walk", "8"])
            .expect("parses")
            .clip_selection(230)
            .expect("in range")
            .map;
        assert_eq!(
            walk,
            ClipMap {
                idle: 55,
                walk: 8,
                jump_up: 35,
                fall: 36,
                land: 37,
            }
        );

        let jump = parse(&["--bigfile", "B", "--clip-jump", "30"])
            .expect("parses")
            .clip_selection(230)
            .expect("in range")
            .map;
        assert_eq!(
            jump,
            ClipMap {
                idle: 55,
                walk: 2,
                jump_up: 30,
                fall: 30,
                land: 30,
            },
            "--clip-jump sets ascending, falling and landing together"
        );
    }

    #[test]
    fn per_state_airborne_flags_override_the_jump_shortcut_individually() {
        let resolve = |argv: &[&str]| {
            parse(argv)
                .expect("parses")
                .clip_selection(230)
                .expect("in range")
                .map
        };

        let fall_only = resolve(&["--bigfile", "B", "--clip-jump", "30", "--clip-fall", "12"]);
        assert_eq!(
            (fall_only.jump_up, fall_only.fall, fall_only.land),
            (30, 12, 30)
        );

        let mixed = resolve(&[
            "--bigfile",
            "B",
            "--clip-jump",
            "30",
            "--clip-jump-up",
            "21",
            "--clip-land",
            "20",
        ]);
        assert_eq!((mixed.jump_up, mixed.fall, mixed.land), (21, 30, 20));

        let land_only = resolve(&["--bigfile", "B", "--clip-land", "40"]);
        assert_eq!(
            (land_only.jump_up, land_only.fall, land_only.land),
            (35, 36, 40),
            "a per-state flag alone leaves the other defaults intact"
        );

        for flag in ["--clip-jump-up", "--clip-fall", "--clip-land"] {
            assert_eq!(
                parse(&["--bigfile", "B", flag, "10"])
                    .expect("parses")
                    .clip_selection(10),
                Err(ClipError::OutOfRange {
                    clip: 10,
                    clip_count: 10
                }),
                "{flag} is validated strictly"
            );
        }
    }

    #[test]
    fn default_map_falls_back_when_the_animation_has_fewer_clips() {
        let container = crate::sra::synthetic_animation_container(
            &[0x001, 0x001],
            2,
            100,
            &[1024.0, 0.0, 0.0, 0.0],
        );
        let animation = crate::sra::SraAnimation::load(container, 2).expect("one synthetic table");
        assert_eq!(animation.clips().len(), 1, "the fixture has one clip");

        let options = parse(&["--bigfile", "B", "--sra", "R.SRA"]).expect("defaults parse");
        let selection = options
            .clip_selection(animation.clips().len())
            .expect("a default past the clip count must still resolve");
        assert_eq!(
            selection.map,
            ClipMap {
                idle: 0,
                walk: 0,
                jump_up: 0,
                fall: 0,
                land: 0,
            },
            "every default falls back to the only decoded clip"
        );
        assert_eq!(selection.fixed, None);
    }

    #[test]
    fn headless_defaults_to_sixty_ticks() {
        let options = parse(&["--bigfile", "B", "--headless"]).expect("headless must parse");
        assert_eq!(options.ticks, Some(60));
    }

    #[test]
    fn zero_ticks_is_allowed_in_windowed_mode() {
        let options = parse(&["--bigfile", "B", "--ticks", "0"]).expect("0 is unlimited");
        assert_eq!(options.ticks, Some(0));
        assert!(!options.headless);
    }

    #[test]
    fn unknown_flag_is_a_typed_error() {
        assert_eq!(
            parse(&["--bigfile", "B", "--bogus"]),
            Err(PlayError::UnknownOption("--bogus".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--bogus"]).unwrap_err().to_string(),
            "unknown option '--bogus' for --play"
        );
    }

    #[test]
    fn missing_values_are_typed_errors() {
        for option in [
            "--bigfile",
            "--textures",
            "--unit",
            "--ticks",
            "--camera",
            "--anim-fps",
        ] {
            assert_eq!(
                parse(&[option]),
                Err(PlayError::MissingValue(option)),
                "option {option}"
            );
        }
    }

    #[test]
    fn missing_archive_is_reported() {
        assert_eq!(parse(&[]), Err(PlayError::MissingArchive));
        assert_eq!(
            parse(&["--unit", "under1"]),
            Err(PlayError::MissingArchive)
        );
        assert_eq!(
            PlayError::MissingArchive.to_string(),
            "--play requires --bigfile <BIGFILE.DAT>"
        );
    }

    #[test]
    fn ticks_and_camera_validate() {
        assert_eq!(
            parse(&["--bigfile", "B", "--ticks", "abc"]),
            Err(PlayError::InvalidTicks("abc".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--ticks", "-1"]),
            Err(PlayError::InvalidTicks("-1".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--headless", "--ticks", "0"]),
            Err(PlayError::ZeroTicksHeadless)
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--camera", "top"]),
            Err(PlayError::InvalidCamera("top".to_string()))
        );
    }

    #[test]
    fn anim_fps_out_of_range_is_a_typed_error() {
        for raw in ["0", "241", "abc", "-1"] {
            assert_eq!(
                parse(&["--bigfile", "B", "--anim-fps", raw]),
                Err(PlayError::InvalidAnimFps(raw.to_string())),
                "value {raw}"
            );
        }
        assert!(
            PlayError::InvalidAnimFps("241".to_string())
                .to_string()
                .contains("1..=240")
        );
        for raw in ["1", "240"] {
            let options = parse(&["--bigfile", "B", "--anim-fps", raw])
                .expect("bounds are inclusive");
            assert_eq!(options.anim_fps.to_string(), raw);
        }
    }

    #[test]
    fn windowed_runs_require_textures_before_any_file_read() {
        let options = parse(&["--bigfile", "definitely-not-a-file.dat"])
            .expect("the parser stays pure");
        let error = super::run(&options).expect_err("windowed play needs textures");
        assert!(matches!(error, RunError::MissingTextures));
        assert_eq!(error.exit_code(), 1);
        assert!(error.to_string().contains("--textures"));
    }

    #[test]
    fn archive_world_and_texture_failures_are_exit_two() {
        use crate::engine::world::WorldError;

        assert_eq!(RunError::Texture("missing page".to_string()).exit_code(), 2);
        assert_eq!(RunError::Asset("bad archive".to_string()).exit_code(), 2);
        assert_eq!(
            RunError::World(WorldError::EmptyReport).exit_code(),
            2,
            "world load/parse errors are archive errors"
        );
        assert_eq!(RunError::Scene("bad scene".to_string()).exit_code(), 1);
        assert_eq!(RunError::MissingTextures.exit_code(), 1);
        assert_eq!(RunError::Window("no window".to_string()).exit_code(), 1);
        assert_eq!(
            RunError::Invariant("full tracker".to_string()).exit_code(),
            4
        );
    }

    #[test]
    fn windowed_seed_failure_returns_before_the_window_continuation() {
        use super::{FrameState, boot_before_window};
        use crate::engine::world::World;
        use crate::level::ObjectReport;
        use std::path::Path;

        let world = World::from_report(
            "under1".to_string(),
            1,
            &ObjectReport {
                room_offset: 0,
                count: 0,
                array_offset: 0,
                objects: Vec::new(),
            },
        );
        let frame = FrameState::new(world);
        let mut continuation_ran = false;
        let error = boot_before_window(
            frame,
            Path::new("definitely-not-an-archive.dat"),
            |_frame| {
                continuation_ran = true;
                Ok(())
            },
        )
        .expect_err("a missing archive must fail the boot pass");

        assert!(matches!(error, RunError::Asset(_)), "got {error:?}");
        assert_eq!(error.exit_code(), 2);
        assert!(
            !continuation_ran,
            "the window continuation ran after a seed failure; the window would exist"
        );
    }

    #[test]
    fn input_script_applies_presses_and_releases_for_exactly_its_ticks() {
        let script = InputScript::parse("forward:3,right:2").expect("script must parse");
        assert_eq!(script.total_ticks(), 5);

        let mut frame = FrameState::new(free_world());
        let mut held = Vec::new();
        let mut edges = Vec::new();
        for index in 0..6u64 {
            apply_script_tick(&mut frame, &script, index);
            frame
                .run_tick(ObjectTick::step(index))
                .expect("synthetic tick");
            let input = frame.current_input();
            held.push((
                input.is_held(InputButton::Forward),
                input.is_held(InputButton::Right),
            ));
            edges.push((
                input.is_pressed(InputButton::Forward),
                input.is_released(InputButton::Forward),
                input.is_pressed(InputButton::Right),
                input.is_released(InputButton::Right),
            ));
        }

        assert_eq!(
            held,
            vec![
                (true, false),
                (true, false),
                (true, false),
                (false, true),
                (false, true),
                (false, false),
            ],
            "each step holds its button for exactly its tick count"
        );
        assert_eq!(
            edges,
            vec![
                (true, false, false, false),
                (false, false, false, false),
                (false, false, false, false),
                (false, true, true, false),
                (false, false, false, false),
                (false, false, false, true),
            ],
            "one press edge at the first tick and one release edge at the boundary"
        );
    }

    #[test]
    fn blocked_ticks_counts_a_sweep_that_cannot_commit_any_delta() {
        let script = InputScript::parse("forward:30").expect("script must parse");
        let mut frame = FrameState::new(wall_world());
        let report = run_script_ticks(&mut frame, &script, 30).expect("scripted run");

        assert_eq!(report.ticks, 30);
        assert_eq!(report.start, [0, 0, 0]);
        assert_eq!(report.end, [0, 0, 0], "the wall refuses every step");
        assert_eq!(report.blocked_ticks, 30, "every moving tick was blocked");
        assert!(report.player_bound);
    }

    #[test]
    fn blocked_ticks_is_zero_when_the_player_moves() {
        let script = InputScript::parse("forward:30").expect("script must parse");
        let mut frame = FrameState::new(free_world());
        let report = run_script_ticks(&mut frame, &script, 30).expect("scripted run");

        assert_eq!(report.blocked_ticks, 0);
        assert_eq!(report.start, [0, 0, 0]);
        assert_eq!(
            report.end,
            [0, 30 * crate::engine::frame::PLAYER_SPEED_UNITS_PER_TICK, 0],
            "an empty mesh commits the full direct translation on +Y"
        );
        assert!(report.end[1] > report.start[1]);
    }

    #[test]
    fn player_bound_reflects_the_binding() {
        let script = InputScript::parse("forward:1").expect("script must parse");

        let mut bound = FrameState::new(free_world());
        assert!(run_script_ticks(&mut bound, &script, 1)
            .expect("scripted run")
            .player_bound);

        let mut unbound = FrameState::new(playerless_world());
        let report = run_script_ticks(&mut unbound, &script, 1).expect("scripted run");
        assert!(!report.player_bound, "no raziel record means no bound player");
    }

    #[test]
    fn the_scripted_run_length_never_truncates_the_sequence() {
        let options = parse(&[
            "--bigfile",
            "B",
            "--headless",
            "--ticks",
            "10",
            "--input-script",
            "forward:30",
        ])
        .expect("script must parse");
        assert_eq!(options.ticks, Some(30), "the script owns its own ticks");

        let options = parse(&[
            "--bigfile",
            "B",
            "--headless",
            "--ticks",
            "120",
            "--input-script",
            "forward:30",
        ])
        .expect("script must parse");
        assert_eq!(options.ticks, Some(120), "--ticks may extend the idle tail");
    }

    #[test]
    fn input_script_and_player_pos_validate() {
        assert_eq!(
            parse(&["--bigfile", "B", "--headless", "--input-script", "forward:0"]),
            Err(PlayError::InvalidInputScript("forward:0".to_string()))
        );
        assert_eq!(
            parse(&["--bigfile", "B", "--headless", "--input-script", "walk:4"]),
            Err(PlayError::InvalidInputScript("walk:4".to_string()))
        );
        assert!(matches!(
            parse(&["--bigfile", "B", "--input-script", "forward:4"]),
            Err(PlayError::InputScriptRequiresHeadless)
        ));

        let options = parse(&["--bigfile", "B", "--player-pos", "-10,20,30"])
            .expect("player position must parse");
        assert_eq!(options.player_pos, Some([-10, 20, 30]));
        assert_eq!(
            parse(&["--bigfile", "B", "--player-pos", "1,2"]),
            Err(PlayError::InvalidPlayerPos("1,2".to_string()))
        );
    }

    #[test]
    fn the_player_camera_puts_the_eye_behind_the_player_relative_to_its_yaw() {
        let player = [123.0, -450.0, 700.0];
        let (fallback, target) = player_camera_world(
            player,
            None,
            PLAYER_CAMERA_DISTANCE,
            PLAYER_CAMERA_HEIGHT,
            0.0,
            0.0,
        );
        assert!(
            (fallback[0] - player[0]).abs() < 1e-3,
            "the fallback boom keeps world X"
        );
        assert!(
            (fallback[1] - (player[1] - PLAYER_CAMERA_DISTANCE)).abs() < 1e-3,
            "the fallback boom is behind on world -Y"
        );
        assert!(
            (fallback[2] - (player[2] + PLAYER_CAMERA_HEIGHT)).abs() < 1e-3,
            "the eye is above the player"
        );
        assert!((target[2] - (player[2] + PLAYER_LOOK_HEIGHT)).abs() < 1e-3);

        // Forward +X (yaw 0) rotates the boom onto world -X.
        let (minus_x, _) = player_camera_world(
            player,
            Some(0.0),
            PLAYER_CAMERA_DISTANCE,
            PLAYER_CAMERA_HEIGHT,
            0.0,
            0.0,
        );
        assert!((minus_x[0] - (player[0] - PLAYER_CAMERA_DISTANCE)).abs() < 1e-3);
        assert!((minus_x[1] - player[1]).abs() < 1e-3);

        // A 90° yaw turn rotates the boom by 90° too: -X becomes -Y.
        let quarter_turn = crate::pose::BINARY_ANGLE_UNITS_PER_TURN / 4.0;
        let (minus_y, _) = player_camera_world(
            player,
            Some(quarter_turn),
            PLAYER_CAMERA_DISTANCE,
            PLAYER_CAMERA_HEIGHT,
            0.0,
            0.0,
        );
        assert!((minus_y[0] - player[0]).abs() < 1e-3);
        assert!((minus_y[1] - (player[1] - PLAYER_CAMERA_DISTANCE)).abs() < 1e-3);
        assert_eq!(
            quarter_turn, PLAYER_FALLBACK_YAW_BINARY,
            "the fallback forward is +Y, matching the first direct movement axis"
        );
    }

    /// A zone whose spawn sits far from the world origin, like the real
    /// `cathy28` (`raziel` at `-2849,5455,11510`).
    fn offset_world() -> World {
        World::from_report(
            "cathy28".to_string(),
            1,
            &report(vec![
                record("raziel", [9000, -7000, 11510]),
                record("monster_wallcr", [9600, -7000, 11510]),
            ]),
        )
    }

    #[test]
    fn camera_distance_and_height_overrides_move_the_eye() {
        let player = [0.0, 0.0, 0.0];
        let (default_eye, _) = player_camera_world(
            player,
            None,
            PLAYER_CAMERA_DISTANCE,
            PLAYER_CAMERA_HEIGHT,
            0.0,
            0.0,
        );
        let (far_eye, _) = player_camera_world(player, None, 1200.0, 800.0, 0.0, 0.0);
        assert!((far_eye[1] + 1200.0).abs() < 1e-3, "distance controls the boom");
        assert!((far_eye[2] - 800.0).abs() < 1e-3, "height controls the eye");
        assert!((far_eye[1] - default_eye[1]).abs() > 1.0);

        let options = parse(&["--bigfile", "B", "--cam-distance", "1200", "--cam-height", "700"])
            .expect("overrides must parse");
        assert_eq!(options.cam_distance, Some(1200.0));
        assert_eq!(options.cam_height, Some(700.0));

        for raw in ["99", "5001", "abc", "-1"] {
            assert_eq!(
                parse(&["--bigfile", "B", "--cam-distance", raw]),
                Err(PlayError::InvalidCamDistance(raw.to_string())),
                "distance {raw}"
            );
        }
        for raw in ["5001", "abc", "-1"] {
            assert_eq!(
                parse(&["--bigfile", "B", "--cam-height", raw]),
                Err(PlayError::InvalidCamHeight(raw.to_string())),
                "height {raw}"
            );
        }
    }

    #[test]
    fn cull_report_line_is_the_documented_contract() {
        let report = CullReport {
            eye: [-2849, 5455, 11505],
            radius: 4096,
            live: 8,
            visible: 8,
            activated: 8,
            culled: 0,
        };
        assert_eq!(
            report.line(),
            "cull eye=-2849,5455,11505 radius=4096 live=8 visible=8 activated=8 culled=0"
        );
    }

    #[test]
    fn cull_report_is_a_headless_only_diagnostic() {
        assert_eq!(
            parse(&["--bigfile", "B", "--cull-report"]),
            Err(PlayError::CullReportRequiresHeadless)
        );
        let options = parse(&["--bigfile", "B", "--headless", "--cull-report"])
            .expect("headless accepts the diagnostic");
        assert!(options.cull_report);
        assert!(
            !parse(&["--bigfile", "B", "--headless"])
                .expect("plain headless")
                .cull_report
        );
    }

    #[test]
    fn scripted_ticks_cull_from_the_player_so_an_offset_zone_is_visible() {
        let mut frame = FrameState::new(offset_world());
        frame.queue_spawn("monster_wallcr", [9600, -7000, 11510]);
        let report =
            run_script_ticks(&mut frame, &InputScript::default(), 2).expect("script ticks");
        assert!(report.player_bound);
        assert_eq!(
            frame.view().eye,
            frame.player_position().expect("the player is bound"),
            "the cull eye follows the player, not the world origin"
        );
        assert_eq!(frame.view().radius, crate::engine::frame::DEFAULT_CULL_RADIUS);
        let rendered = frame
            .live()
            .iter()
            .filter(|(_, object)| object.visible)
            .count();
        assert_eq!(rendered, 2, "the player body and the nearby enemy are drawn");
        let diagnostic = cull_report(&frame);
        assert_eq!(diagnostic.visible, 2);
        assert_eq!(diagnostic.culled, 0);
    }

    #[test]
    fn the_player_camera_keeps_the_target_in_front_and_uses_world_z_up() {
        let player = [0.0, 0.0, 0.0];
        let (eye, target) = player_camera_world(
            player,
            None,
            PLAYER_CAMERA_DISTANCE,
            PLAYER_CAMERA_HEIGHT,
            0.0,
            0.0,
        );
        let m = third_person_mvp(eye, target, 1.0);
        let point = [target[0], target[1], target[2], 1.0];
        let clip: Vec<f32> =
            (0..4).map(|row| (0..4).map(|col| m[col][row] * point[col]).sum()).collect();
        assert!(clip[3] > 0.0, "the player target is in front of the camera");
        assert!(clip.iter().all(|value| value.is_finite()));

        // World +Z must map to screen up: a point above the target has a larger
        // clip-space y than the target itself.
        let above = [target[0], target[1], target[2] + 100.0, 1.0];
        let clip_above: Vec<f32> =
            (0..4).map(|row| (0..4).map(|col| m[col][row] * above[col]).sum()).collect();
        assert!(
            clip_above[1] / clip_above[3] > clip[1] / clip[3],
            "world +Z points toward screen up"
        );
    }

    #[test]
    fn the_default_boom_frames_the_whole_body() {
        // The player is ~509 world units tall. At the 45° vertical FOV the
        // visible half-height at the look plane must cover head and feet.
        let (eye, target) = player_camera_world(
            [0.0, 0.0, 0.0],
            None,
            PLAYER_CAMERA_DISTANCE,
            PLAYER_CAMERA_HEIGHT,
            0.0,
            0.0,
        );
        let distance = ((target[0] - eye[0]).powi(2)
            + (target[1] - eye[1]).powi(2)
            + (target[2] - eye[2]).powi(2))
        .sqrt();
        let half_height = distance * std::f32::consts::FRAC_PI_8.tan();
        assert!(
            target[2] > 200.0,
            "the target is at mid-torso, not the ankles"
        );
        assert!(
            target[2] + half_height >= 509.0,
            "the head fits: half-height {half_height} above target {} < 509",
            target[2]
        );
        assert!(
            target[2] - half_height <= 0.0,
            "the feet fit"
        );
    }
}

#[cfg(test)]
#[path = "play_acceptance.rs"]
mod acceptance;


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
use crate::engine::assets::{AssetRequest, AssetStore, ImmediateSource};
use crate::engine::backend_wgpu::WgpuBackend;
use crate::engine::class::RenderClass;
use crate::engine::clock::ObjectTick;
use crate::engine::determinism::world_state_hash;
use crate::engine::frame::{FrameError, FrameState, PLAYER_NAME};
use crate::engine::input::{InputButton, InputEvent};
use crate::engine::render_backend::RenderBackend;
use crate::engine::scene::{CameraView, PlayerFacing, SceneAssets};
use crate::engine::spawn::{Admission, EngineNameTable};
use crate::engine::world::{World, WorldError};
use crate::object_texture::PageBank;
use crate::render::{self, FlyCamera, OrbitCamera};

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

/// Parsed `--play` options.
///
/// `ticks` is resolved during parsing: headless defaults to 60, while `None`
/// in windowed mode means no frame cap. Windowed runs require `--textures`;
/// that is a runtime check so the parser stays pure and the T1 error surface is
/// unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayOptions {
    pub bigfile: PathBuf,
    pub textures: Option<PathBuf>,
    pub unit: String,
    pub ticks: Option<u32>,
    pub headless: bool,
    pub camera: CameraMode,
}

impl PlayOptions {
    pub const DEFAULT_UNIT: &'static str = "under1";

    /// Parse the argv that follows `--play`.
    ///
    /// Pure by construction: unknown flags, missing values, a missing
    /// `--bigfile` and malformed `--ticks`/`--camera` values are typed errors.
    pub fn parse(args: &[String]) -> Result<Self, PlayError> {
        let mut options = Self {
            bigfile: PathBuf::new(),
            textures: None,
            unit: Self::DEFAULT_UNIT.to_string(),
            ticks: None,
            headless: false,
            camera: CameraMode::Fly,
        };
        let mut bigfile = None;
        let mut index = 0;
        while index < args.len() {
            let token = args[index].as_str();
            match token {
                "--bigfile" => bigfile = Some(PathBuf::from(value(args, index, "--bigfile")?)),
                "--textures" => {
                    options.textures = Some(PathBuf::from(value(args, index, "--textures")?));
                }
                "--unit" => options.unit = value(args, index, "--unit")?.to_string(),
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
                "--headless" => {
                    options.headless = true;
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
        if options.headless {
            if options.ticks.is_none() {
                options.ticks = Some(60);
            }
            if options.ticks == Some(0) {
                return Err(PlayError::ZeroTicksHeadless);
            }
        }
        Ok(options)
    }
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

/// Typed `--play` failures. `main` prints each one and exits 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    UnknownOption(String),
    MissingValue(&'static str),
    MissingArchive,
    InvalidTicks(String),
    ZeroTicksHeadless,
    InvalidCamera(String),
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOption(option) => write!(f, "unknown option '{option}' for --play"),
            Self::MissingValue(option) => write!(f, "option '{option}' needs a value"),
            Self::MissingArchive => write!(f, "--play requires --bigfile <BIGFILE.DAT>"),
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

    let world = World::load(&options.bigfile, &options.unit).map_err(RunError::World)?;
    let textures = if options.headless {
        None
    } else {
        options.textures.as_deref()
    };
    let (assets, bank) =
        SceneAssets::load(&options.bigfile, world.level.entry, textures).map_err(RunError::scene)?;
    let frame = FrameState::new(world);

    if options.headless {
        return run_headless(frame, options.ticks.unwrap_or(60), &options.bigfile);
    }

    run_windowed(
        frame,
        assets,
        bank.expect("windowed mode requested textures"),
        options.camera,
        &options.unit,
        &options.bigfile,
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
/// This is the production headless path: the digest is computed with the same
/// [`world_state_hash`] the determinism gate pins. Any tracker overflow becomes
/// [`RunError::Invariant`] (exit 4).
fn headless_run(
    frame: &mut FrameState,
    bigfile: &Path,
    ticks: u32,
) -> Result<HeadlessReport, RunError> {
    let counts = boot_world(frame, bigfile)?;
    for index in 0..u64::from(ticks) {
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

fn run_headless(mut frame: FrameState, ticks: u32, bigfile: &Path) -> Result<(), RunError> {
    let report = headless_run(&mut frame, bigfile, ticks)?;
    outln!("{}", report.line());
    Ok(())
}

fn run_windowed(
    frame: FrameState,
    assets: SceneAssets,
    bank: PageBank,
    mode: CameraMode,
    unit: &str,
    bigfile: &Path,
) -> Result<(), RunError> {
    boot_before_window(frame, bigfile, |frame| {
        open_window(frame, assets, bank, mode, unit)
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
    mode: CameraMode,
    unit: &str,
) -> Result<(), RunError> {
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
        last_frame: Instant::now(),
        last_dt: 0.0,
        init_error: None,
        title: format!("soul reaver - {unit}"),
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

/// Third-person boom distance behind the player, in integer world units.
const PLAYER_CAMERA_BACK: f32 = 600.0;

/// Third-person eye height above the player's feet, in integer world units.
const PLAYER_CAMERA_HEIGHT: f32 = 400.0;

/// Height of the look target above the player's feet, in integer world units.
const PLAYER_LOOK_HEIGHT: f32 = 60.0;

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
/// The eye is behind the player on world `-Y` (forward is `+Y` in the Z-up
/// world) and above it on world `+Z`. The target is a little above the feet.
fn player_camera_world(player: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let target = [player[0], player[1], player[2] + PLAYER_LOOK_HEIGHT];
    let eye = [
        player[0],
        player[1] - PLAYER_CAMERA_BACK,
        player[2] + PLAYER_CAMERA_HEIGHT,
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
    last_frame: Instant,
    last_dt: f32,
    init_error: Option<String>,
    title: String,
}

impl PlayApp {
    fn aspect(&self) -> f32 {
        self.backend
            .as_ref()
            .map(|backend| backend.aspect())
            .unwrap_or(1.0)
    }

    /// The player's produced root facing for the scene builder, if it has moved.
    fn player_facing(&self) -> Option<PlayerFacing> {
        Some(PlayerFacing {
            player: self.frame.player()?,
            yaw_binary: self.frame.player_yaw_binary()?,
        })
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
                // Third-person follow: the camera sits behind (world -Y) and
                // above (world +Z) the player and tracks it every frame. The
                // world is Z-up, so "above" is +Z and the horizontal boom lies
                // in the X/Y plane. UI-only: it never touches simulation state.
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
                let (eye_world, target_world) = player_camera_world(player);
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

    /// Mouse orbit/pan/zoom in orbit mode and look in fly mode.
    fn handle_mouse(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                if self.mode == CameraMode::Orbit {
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
                        CameraMode::Player => {}
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
                let facing = self.player_facing();
                let live = self.frame.live();
                let scene = self.assets.build_with_facing(live, camera, facing);
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
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CameraMode, PLAYER_CAMERA_BACK, PLAYER_CAMERA_HEIGHT, PLAYER_LOOK_HEIGHT, PlayError,
        PlayOptions, RunError, player_camera_world, third_person_mvp,
    };
    use std::path::PathBuf;

    fn argv(input: &[&str]) -> Vec<String> {
        input.iter().map(|token| token.to_string()).collect()
    }

    fn parse(input: &[&str]) -> Result<PlayOptions, PlayError> {
        PlayOptions::parse(&argv(input))
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
        ])
        .expect("full argv must parse");
        assert_eq!(options.bigfile, PathBuf::from("BIGFILE.DAT"));
        assert_eq!(options.textures, Some(PathBuf::from("TEXTURES.BIG")));
        assert_eq!(options.unit, "under2");
        assert_eq!(options.ticks, Some(120));
        assert!(options.headless);
        assert_eq!(options.camera, CameraMode::Player);
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
        for option in ["--bigfile", "--textures", "--unit", "--ticks", "--camera"] {
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
    fn the_player_camera_sits_behind_and_above_on_the_correct_axes() {
        let player = [123.0, -450.0, 700.0];
        let (eye, target) = player_camera_world(player);
        assert_eq!(eye[0], player[0], "the boom keeps the same world X");
        assert_eq!(eye[1], player[1] - PLAYER_CAMERA_BACK, "behind is world -Y");
        assert_eq!(eye[2], player[2] + PLAYER_CAMERA_HEIGHT, "above is world +Z");
        assert!(eye[2] > player[2], "the eye is above the player");
        assert!(eye[1] < player[1], "the eye is behind the player");
        assert_eq!(target[2], player[2] + PLAYER_LOOK_HEIGHT);
    }

    #[test]
    fn the_player_camera_keeps_the_target_in_front_and_uses_world_z_up() {
        let player = [0.0, 0.0, 0.0];
        let (eye, target) = player_camera_world(player);
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
}

#[cfg(test)]
#[path = "play_acceptance.rs"]
mod acceptance;


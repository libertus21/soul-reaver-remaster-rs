//! Ordered 18-stage frame pipeline.
//!
//! The original in-game frame coordinator runs a fixed list of subsystems in a
//! documented order (`docs/notes/actor-ai.md`, section 1). That order is
//! behaviour: reordering two stages can change the observable state of the same
//! inputs. [`STAGE_ORDER`] is therefore the single ordered definition, and
//! [`FrameState::run_tick`] is the only driver.
//!
//! Stages 1, 2, 6, 9, 12, 14, 15 and 18 are implemented for this milestone.
//! Stage 14 carries our own **synthetic** non-player locomotion
//! ([`crate::engine::synthetic_locomotion`]); the original moving-object pass is
//! UNKNOWN and is not claimed. The remaining stages are **named no-ops** with
//! the documented name, never `todo!()`: adding AI, animation, contact or
//! streaming later is filling a body, not editing the loop.
//!
//! Stage 8 (the four-step player move/collide/commit solver) stays a named
//! no-op: the original's exact solver is UNKNOWN and is not claimed. The
//! deterministic horizontal sweep and the gravity/ground resolver live beside
//! the movement they resolve, in stage 9, so the two cannot drift apart. That
//! resolver is a correct deterministic implementation, not a reproduction of the
//! original.
//!
//! Gravity acts only when the loaded collision mesh carries walkable floor
//! triangles. A synthetic or geometry-free world has no ground plane, so stage 9
//! leaves `Y` untouched and the synthetic determinism goldens stay stable.
//!
//! A jump is a deterministic input edge, not a timer: the `Jump` press edge
//! replaces the vertical velocity with a documented synthetic upward impulse
//! **only while the player ended the previous tick grounded**. Rising motion is
//! resolved by the same collision mesh, which clamps the head at the lowest
//! surface above the feet. A jump therefore cannot start in mid-air and there is
//! no double jump; landing re-enables the next press edge. The velocity and the
//! grounded flag are diagnostics and are not part of the canonical fold.
//!
//! The player is updated twice per tick, exactly as documented: first by the
//! direct player path (stage 9) and again as an ordinary live object in the
//! generic per-object pass (stage 12). The double update is original behaviour
//! and must not be "fixed" here.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use crate::engine::actor::{ActorProfile, ActorRuntime};
use crate::engine::axis::{HORIZONTAL_A, HORIZONTAL_B, UP};
use crate::engine::class::{CLASS_TABLE, Hooks, Query, RenderClass, classify};
use crate::engine::clock::{ObjectTick, SimClock};
use crate::engine::collision::{CollisionMesh, JUMP_IMPULSE_SUBUNITS_PER_TICK};
use crate::engine::determinism::{WorldEntry, WorldState, world_state_hash_with_interactions};
use crate::engine::events::{EngineEvent, EventQueue};
use crate::engine::input::{InputButton, InputEvent, InputFrame, InputQueue};
use crate::engine::interaction::{
    ActivateTarget, InteractOutcome, InteractTrigger, InteractionLayer,
};
use crate::engine::live::{LiveArena, LiveId, LiveObject};
use crate::engine::synthetic_locomotion::{self, SyntheticPatrol};
use crate::engine::tracker::{ObjectTracker, TrackerError};
use crate::engine::world::World;
use crate::pose;

/// The player's authored placed name. The spawn policy binds this record.
pub const PLAYER_NAME: &str = "raziel";

/// Direct translation speed in integer world units per tick.
///
/// Stage 9 applies this delta through the deterministic horizontal sweep. The
/// sweep only reduces it when a wall is in the way.
pub const PLAYER_SPEED_UNITS_PER_TICK: i32 = 4;

/// Player collision radius in integer world units.
///
/// The original radius is UNKNOWN; this is a documented synthetic value that
/// keeps the sweep small relative to the room scale (rooms span thousands of
/// units) while giving a visible stand-off from walls.
pub const PLAYER_RADIUS_UNITS: i32 = 16;

/// Default cull radius, in integer world units.
pub const DEFAULT_CULL_RADIUS: i32 = 4096;

/// One stage of the documented frame coordinator.
///
/// The discriminants are the documented stage numbers and are part of the
/// determinism gate; never renumber or reorder them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Stage {
    /// 1. Frame delta / fixed-point timestep and tick-boundary input sampling.
    Clock = 1,
    /// 2. Per-frame scratch reset (contact lists, accumulators).
    ScratchReset = 2,
    /// 3. Script/event unit execution (16 units).
    ScriptUnits = 3,
    /// 4. Per-unit audio emitters.
    AudioEmitters = 4,
    /// 5. Clock / time-of-day.
    TimeOfDay = 5,
    /// 6. Object activation and distance culling (query hook).
    ActivationCull = 6,
    /// 7. Per-unit animation/timer advance.
    AnimationTimers = 7,
    /// 8. Player movement solver. Documented no-op: direct translation only.
    PlayerSolver = 8,
    /// 9. Player input to movement (direct translation).
    PlayerMovement = 9,
    /// 10. Pairwise contact broad phase and resolver.
    PairwiseContact = 10,
    /// 11. Hit-record pass per object.
    HitList = 11,
    /// 12. Per-object tick (`+0x1A8`).
    PerObjectTick = 12,
    /// 13. Area/trigger query pass.
    TriggerQuery = 13,
    /// 14. Moving-object world/contact pass.
    MovingContact = 14,
    /// 15. Render submission (CPU scene snapshot).
    RenderSubmission = 15,
    /// 16. Streaming / prefetch.
    Streaming = 16,
    /// 17. Particles.
    Particles = 17,
    /// 18. Post-frame spawn/despawn and object-tracker recompute.
    PostFrameChanges = 18,
}

impl Stage {
    /// Documented stage number. Never renumber.
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Stable stage name used by the determinism golden table.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::ScratchReset => "scratch-reset",
            Self::ScriptUnits => "script-units",
            Self::AudioEmitters => "audio-emitters",
            Self::TimeOfDay => "time-of-day",
            Self::ActivationCull => "activation-cull",
            Self::AnimationTimers => "animation-timers",
            Self::PlayerSolver => "player-solver",
            Self::PlayerMovement => "player-movement",
            Self::PairwiseContact => "pairwise-contact",
            Self::HitList => "hit-list",
            Self::PerObjectTick => "per-object-tick",
            Self::TriggerQuery => "trigger-query",
            Self::MovingContact => "moving-contact",
            Self::RenderSubmission => "render-submission",
            Self::Streaming => "streaming",
            Self::Particles => "particles",
            Self::PostFrameChanges => "post-frame-changes",
        }
    }
}

/// The 18 documented stages, in documented order.
///
/// This array is the single ordered definition. The determinism gate pins it
/// with a golden table, so a reorder fails a test instead of silently changing
/// behaviour.
pub const STAGE_ORDER: [Stage; 18] = [
    Stage::Clock,
    Stage::ScratchReset,
    Stage::ScriptUnits,
    Stage::AudioEmitters,
    Stage::TimeOfDay,
    Stage::ActivationCull,
    Stage::AnimationTimers,
    Stage::PlayerSolver,
    Stage::PlayerMovement,
    Stage::PairwiseContact,
    Stage::HitList,
    Stage::PerObjectTick,
    Stage::TriggerQuery,
    Stage::MovingContact,
    Stage::RenderSubmission,
    Stage::Streaming,
    Stage::Particles,
    Stage::PostFrameChanges,
];

/// Integer camera view used by stage 6 distance culling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CullView {
    pub eye: [i32; 3],
    pub radius: i32,
}

impl Default for CullView {
    fn default() -> Self {
        Self {
            eye: [0, 0, 0],
            radius: DEFAULT_CULL_RADIUS,
        }
    }
}

/// One item of the CPU render snapshot produced by stage 15.
///
/// This is a data snapshot, not a renderer and not a mesh: the renderer slice
/// maps it to draw items later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderItem {
    pub id: LiveId,
    pub class: u8,
    pub position: [i32; 3],
    pub visible: bool,
}

/// Per-tick scratch state, reset by stage 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameScratch {
    activated: usize,
    culled: usize,
}

impl FrameScratch {
    pub const fn activated(self) -> usize {
        self.activated
    }

    pub const fn culled(self) -> usize {
        self.culled
    }
}

/// Deterministic counters for the frame driver.
///
/// These are diagnostics, not simulation state: they do not enter the canonical
/// digest. They make the documented double player update observable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameStats {
    pub ticks: u64,
    pub direct_player_ticks: u64,
    pub generic_ticks: u64,
    pub generic_player_ticks: u64,
    pub hooked_ticks: u64,
    pub hooked_player_ticks: u64,
    /// Tick-hook calls that reached an actor carrier. A class tick hook fires on
    /// `raziel__` and `monster_`; this counter additionally requires the live
    /// object to carry an [`ActorRuntime`], so it proves that the actor path ran
    /// and not merely that the class gate opened.
    pub actor_ticks: u64,
    /// Events pushed through [`FrameState::queue_event`]. Events synthesized by
    /// a producing stage would not pass through this counter.
    pub events_queued: u64,
    /// Events drained at the delivery stage and handed to a target lookup.
    pub events_delivered: u64,
    /// Delivered events whose target class installs an event hook. The hook body
    /// is an empty witness; this counter proves the seam reached the object.
    pub event_hook_calls: u64,
    /// Events handed to an actor carrier through the per-object tick. It counts
    /// only target-matched events on a live object that has an
    /// [`ActorRuntime`] and a tick hook, so it proves `advance` received a
    /// non-empty view without changing any state.
    pub actor_events_passed: u64,
    pub spawned: u64,
    pub despawned: u64,
}

/// A spawn deferred to the post-frame stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredSpawn {
    pub name: String,
    /// The loaded model's class, resolved at load-complete before actor
    /// initialisation. The class table is applied to this, not to the placed
    /// instance name (`object-prototype`). `None` is the synthetic/no-model
    /// fallback: the instance-name prefix lookup is the only key available.
    pub class: Option<RenderClass>,
    pub position: [i32; 3],
    /// The actor profile the loaded model carried, if any. It is copied from
    /// `ModelAsset::actor` and initialised here, **after** the class was
    /// captured; the class never comes from this profile.
    pub actor: Option<ActorProfile>,
}

/// What the frame driver can fail on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    Tracker(TrackerError),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tracker(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Milestone simulation state: authored world, live objects, tracker, clock and
/// the input edge queue.
///
/// The clock is fed by the caller; [`FrameState::frame`] converts wall time into
/// fixed ticks and runs [`FrameState::run_tick`] for each one. Input never
/// advances the clock.
pub struct FrameState {
    world: World,
    live: LiveArena,
    tracker: ObjectTracker,
    clock: SimClock,
    input_queue: InputQueue,
    events: EventQueue,
    current_input: InputFrame,
    player: Option<LiveId>,
    /// Signed vertical velocity in fixed-point subunits per tick. Positive is
    /// rising (a jump); negative is falling; zero is grounded or at the apex.
    /// Not part of the canonical fold.
    player_vertical_velocity: i64,
    /// Whether the player ended the last tick resting on a walkable floor.
    player_grounded: bool,
    /// Whether stage 9 committed a non-zero horizontal delta this tick.
    /// Presentation/diagnostic state only: it never enters the canonical fold.
    player_moving: bool,
    /// The facing yaw last produced from a non-zero horizontal movement delta.
    /// `None` until the player first moves; a zero delta keeps the previous
    /// value. Presentation state only: it never enters the canonical fold.
    player_yaw: Option<pose::Mat3>,
    /// The binary yaw (`4096` units per turn) behind [`Self::player_yaw`], so the
    /// scene can rebuild the same root pose through [`pose::pose_from_yaw`].
    player_yaw_binary: Option<f32>,
    view: CullView,
    stats: FrameStats,
    scratch: FrameScratch,
    /// Synthetic non-player locomotion switch (`--no-npc-motion` turns it off).
    /// On by default; see [`crate::engine::synthetic_locomotion`].
    npc_motion: bool,
    /// Per-actor synthetic patrol state, keyed by live id. This is a
    /// deterministic fold of the tick history, not canonical state: only the
    /// resulting positions enter [`Self::world_state`].
    patrols: BTreeMap<LiveId, SyntheticPatrol>,
    /// Player-driven open/closed state, closed-door collision and the activation
    /// log. See [`crate::engine::interaction`].
    interaction: InteractionLayer,
    spawn_queue: Vec<DeferredSpawn>,
    despawn_queue: Vec<LiveId>,
    scene: Vec<RenderItem>,
    ticks: u64,
    tick_index: u64,
    stage_trace: Vec<Stage>,
}

impl FrameState {
    /// Build the frame state from a loaded world.
    ///
    /// The player is bound immediately from the placed `raziel` record, exactly
    /// like the original player spawn path. When the unit has no `raziel`
    /// record the state stays playerless instead of panicking.
    pub fn new(world: World) -> Self {
        let mut state = Self {
            world,
            live: LiveArena::new(),
            tracker: ObjectTracker::new(),
            clock: SimClock::new(),
            input_queue: InputQueue::new(),
            events: EventQueue::new(),
            current_input: InputFrame::NONE,
            player: None,
            player_vertical_velocity: 0,
            player_grounded: false,
            player_moving: false,
            player_yaw: None,
            player_yaw_binary: None,
            view: CullView::default(),
            stats: FrameStats::default(),
            scratch: FrameScratch::default(),
            npc_motion: true,
            patrols: BTreeMap::new(),
            interaction: InteractionLayer::new(true),
            spawn_queue: Vec::new(),
            despawn_queue: Vec::new(),
            scene: Vec::new(),
            ticks: 0,
            tick_index: 0,
            stage_trace: Vec::with_capacity(STAGE_ORDER.len()),
        };
        state.bind_player();
        state
    }

    fn bind_player(&mut self) {
        let Some(slot) = self.world.placed.by_name(PLAYER_NAME).first().copied() else {
            return;
        };
        let Some(record) = self.world.placed.record(slot) else {
            return;
        };
        let position = [
            i32::from(record.position[0]),
            i32::from(record.position[1]),
            i32::from(record.position[2]),
        ];
        let object = LiveObject {
            name: PLAYER_NAME.to_string(),
            class: RenderClass::Raziel,
            position,
            visible: true,
            slot: None,
            anim_index: 0,
            actor: None,
        };
        self.player = Some(self.live.spawn(object));
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn live(&self) -> &LiveArena {
        &self.live
    }

    pub fn tracker(&self) -> &ObjectTracker {
        &self.tracker
    }

    pub fn clock(&self) -> &SimClock {
        &self.clock
    }

    pub fn player(&self) -> Option<LiveId> {
        self.player
    }

    pub fn player_position(&self) -> Option<[i32; 3]> {
        self.player
            .and_then(|id| self.live.get(id).ok())
            .map(|object| object.position)
    }

    /// Move the bound player to an explicit position.
    ///
    /// This is the `--player-pos` test aid: it overrides the authored record
    /// position before the first tick and changes nothing else. It is a pure
    /// function of its argument, so the same position replays identically.
    /// Returns `false` when no player is bound.
    pub fn place_player(&mut self, position: [i32; 3]) -> bool {
        let Some(player) = self.player else {
            return false;
        };
        match self.live.get_mut(player) {
            Ok(object) => {
                object.position = position;
                true
            }
            Err(_) => false,
        }
    }

    /// Move this frame into a different unit, preserving the player's identity.
    ///
    /// This is the one-unit half of a world swap: [`crate::engine::play::swap_unit`]
    /// loads the target [`World`] and calls this to replace the authored world.
    /// The bound player keeps its [`LiveId`] and only changes position; every
    /// other live object, the tracker slots, the synthetic patrol state, the
    /// event and input queues and the CPU render snapshot belong to the unit
    /// being left and are released here, so nothing leaks into the new unit.
    ///
    /// The global tick count and the clock are **kept**, so a warp is a pure
    /// function of a global tick schedule and a segment keeps counting ticks
    /// across the transition. The new unit re-seeds its live actors through the
    /// ordinary boot pass; they materialize at the next stage 18 exactly like a
    /// fresh run, so this method never invents a spawn.
    ///
    /// A frame with no bound player (the previous unit placed no `raziel` and
    /// used the fallback elsewhere) spawns the player at `player_position`
    /// instead of staying playerless.
    pub fn enter_unit(&mut self, world: World, player_position: [i32; 3]) {
        self.world = world;
        match self.player {
            Some(player) => {
                let leaving: Vec<LiveId> = self
                    .live
                    .order()
                    .iter()
                    .copied()
                    .filter(|id| *id != player)
                    .collect();
                for id in leaving {
                    let _ = self.live.despawn(id);
                }
                if let Ok(object) = self.live.get_mut(player) {
                    object.position = player_position;
                    object.visible = true;
                    object.slot = None;
                    object.anim_index = 0;
                    object.actor = None;
                }
            }
            None => {
                self.live = LiveArena::new();
                let object = LiveObject {
                    name: PLAYER_NAME.to_string(),
                    class: RenderClass::Raziel,
                    position: player_position,
                    visible: true,
                    slot: None,
                    anim_index: 0,
                    actor: None,
                };
                self.player = Some(self.live.spawn(object));
            }
        }
        self.tracker = ObjectTracker::new();
        self.patrols.clear();
        self.interaction.enter_unit();
        self.spawn_queue.clear();
        self.despawn_queue.clear();
        self.events = EventQueue::new();
        self.input_queue = InputQueue::new();
        self.current_input = InputFrame::NONE;
        self.scene.clear();
        self.scratch = FrameScratch::default();
        self.view = CullView::default();
        self.player_vertical_velocity = 0;
        self.player_grounded = false;
        self.player_moving = false;
        self.player_yaw = None;
        self.player_yaw_binary = None;
    }

    /// The player's facing yaw, produced by
    /// [`pose::yaw_rotation_from_direction`] from the committed movement delta.
    ///
    /// `None` until the player first moves. It is presentation state, not part
    /// of the canonical digest; standing still preserves the last value.
    pub fn player_yaw(&self) -> Option<pose::Mat3> {
        self.player_yaw
    }

    /// The binary angle (`4096` units per turn) behind [`Self::player_yaw`].
    ///
    /// This is the exact input [`pose::pose_from_yaw`] consumes, so the scene can
    /// rebuild the same root rotation without re-deriving it from a matrix.
    pub fn player_yaw_binary(&self) -> Option<f32> {
        self.player_yaw_binary
    }

    /// Whether the player ended the last tick resting on a walkable floor.
    ///
    /// This is a diagnostic, not part of the canonical digest. A world with no
    /// floor geometry always reports `false` because `Y` is not resolved there.
    pub fn player_grounded(&self) -> bool {
        self.player_grounded
    }

    /// Whether stage 9 committed a non-zero horizontal delta on the last tick.
    ///
    /// Diagnostic presentation state, not part of the canonical digest. A tick
    /// with no input, or one where the sweep was fully blocked, reports `false`.
    pub fn player_moving(&self) -> bool {
        self.player_moving
    }

    /// The player's signed vertical velocity in fixed-point subunits per tick.
    /// Positive is rising; negative is falling; zero is grounded or at the apex.
    /// Diagnostic only.
    pub fn player_vertical_velocity(&self) -> i64 {
        self.player_vertical_velocity
    }

    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    pub fn scratch(&self) -> FrameScratch {
        self.scratch
    }

    pub fn scene(&self) -> &[RenderItem] {
        &self.scene
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    pub fn tick_index(&self) -> u64 {
        self.tick_index
    }

    pub fn current_input(&self) -> InputFrame {
        self.current_input
    }

    /// Stage trace of the last [`FrameState::run_tick`] call.
    pub fn stage_trace(&self) -> &[Stage] {
        &self.stage_trace
    }

    pub fn set_view(&mut self, view: CullView) {
        self.view = view;
    }

    /// Enable or disable the synthetic non-player locomotion stage.
    ///
    /// The stage is on by default. `--no-npc-motion` turns it off, which leaves
    /// every actor at its authored position and makes the pre-locomotion
    /// digests reproducible. The switch never affects the player.
    pub fn set_npc_motion(&mut self, enabled: bool) {
        self.npc_motion = enabled;
    }

    /// Whether the synthetic non-player locomotion stage runs.
    pub fn npc_motion(&self) -> bool {
        self.npc_motion
    }

    /// The view stage 6 will use on the next tick.
    pub fn view(&self) -> CullView {
        self.view
    }

    /// Point the stage-6 cull eye at the bound player, keeping
    /// [`DEFAULT_CULL_RADIUS`].
    ///
    /// The third-person camera follows the player, so distance culling must
    /// agree with it. The original culls around the camera; a fixed origin eye
    /// hides every actor in a zone whose spawn sits far from the world origin
    /// (see `docs/notes/play-culling.md`).
    ///
    /// The eye is the player's position as of the **last completed tick**,
    /// captured before the next tick's stages run. That makes the view a pure
    /// function of the state the tick starts from, so a replay is identical; a
    /// playerless world keeps whatever view it already has.
    pub fn cull_to_player(&mut self) {
        if let Some(eye) = self.player_position() {
            self.view = CullView {
                eye,
                radius: DEFAULT_CULL_RADIUS,
            };
        }
    }

    pub fn queue_input(&mut self, event: InputEvent) {
        self.input_queue.push(event);
    }

    pub fn queued_input_len(&self) -> usize {
        self.input_queue.pending_len()
    }

    /// Queue one event for the tick that is about to run.
    ///
    /// This is the carrier seam, not gameplay. In this slice no producing stage
    /// feeds it from real world state: stage 11 has no animation timeline to
    /// reach a strike transition, and stage 13 has no pinned cylinder extent or
    /// trigger-record source. The seam exists so the queue, the delivery stage
    /// and the actor view are real and testable; a later evidence-backed slice
    /// replaces the empty producer bodies, not the plumbing.
    ///
    /// The event is delivered once, at the delivery stage of the next tick, and
    /// the queue is cleared there. It is never delivered by a later tick and it
    /// never enters the canonical digest.
    pub fn queue_event(&mut self, event: EngineEvent) {
        self.events.push(event);
        self.stats.events_queued = self.stats.events_queued.saturating_add(1);
    }

    /// Events waiting for the next tick's delivery stage.
    pub fn pending_events(&self) -> usize {
        self.events.len()
    }

    /// Defer a spawn to stage 18 of the next tick.
    ///
    /// A spawn queued during a frame is therefore not visible to stages 6-15 of
    /// that frame: it lands in the post-frame bookkeeping pass, exactly like an
    /// asynchronous load completion.
    pub fn queue_spawn(&mut self, name: impl Into<String>, position: [i32; 3]) {
        self.queue_spawn_loaded(name, position, None);
    }

    /// Defer a spawn that carries the actor profile the loaded model supplied.
    ///
    /// This is the spawner's only route to an [`ActorRuntime`]: a model with no
    /// actor block produces `actor = None`, so a non-actor can never gain an
    /// actor carrier by name or class.
    pub fn queue_spawn_loaded(
        &mut self,
        name: impl Into<String>,
        position: [i32; 3],
        actor: Option<ActorProfile>,
    ) {
        self.queue_spawn_with_class(name, None, position, actor);
    }

    /// Defer a spawn whose class was resolved from the loaded model.
    ///
    /// The class is captured at load-complete, before actor initialisation; the
    /// post-frame stage only copies it. `None` keeps the synthetic instance-name
    /// fallback.
    pub fn queue_spawn_with_class(
        &mut self,
        name: impl Into<String>,
        class: Option<RenderClass>,
        position: [i32; 3],
        actor: Option<ActorProfile>,
    ) {
        self.spawn_queue.push(DeferredSpawn {
            name: name.into(),
            class,
            position,
            actor,
        });
    }

    /// Attach an actor profile to an already-live object (the bound player).
    ///
    /// The player is bound by [`FrameState::new`] before assets are read, so the
    /// boot pass supplies its profile here once. Returns `true` when the id
    /// resolves and the carrier was attached.
    pub fn attach_actor(&mut self, id: LiveId, profile: ActorProfile) -> bool {
        let Ok(object) = self.live.get_mut(id) else {
            return false;
        };
        object.actor = Some(ActorRuntime::initial(profile));
        true
    }

    /// Defer a despawn to stage 18 of the next tick.
    pub fn queue_despawn(&mut self, id: LiveId) {
        self.despawn_queue.push(id);
    }

    pub fn pending_changes(&self) -> (usize, usize) {
        (self.spawn_queue.len(), self.despawn_queue.len())
    }

    /// Enable or disable the whole interaction layer (`--no-interactions`).
    ///
    /// Disabling clears every open flag and the closed-door collision overlay, so
    /// the run reproduces the pre-interaction digests exactly.
    pub fn set_interactions_enabled(&mut self, enabled: bool) {
        self.interaction.set_enabled(enabled);
    }

    pub fn interactions_enabled(&self) -> bool {
        self.interaction.enabled()
    }

    /// Install the per-placement closed-door collision meshes for the loaded
    /// unit. The simulation never parses game data; `play.rs` builds them from
    /// the shared [`SceneAssets`](crate::engine::scene::SceneAssets) resolver.
    pub fn install_interaction_base(
        &mut self,
        base: BTreeMap<(String, [i32; 3]), CollisionMesh>,
    ) {
        self.interaction.set_base(base);
    }

    /// Install the `--interact-at` fixed-tick schedule.
    pub fn set_interaction_schedule(
        &mut self,
        schedule: Vec<crate::engine::interaction::InteractAt>,
    ) {
        self.interaction.set_schedule(schedule);
    }

    /// Activate the nearest interactable within
    /// [`INTERACT_RADIUS_UNITS`](crate::engine::interaction::INTERACT_RADIUS_UNITS)
    /// of the player (the windowed `E` key).
    pub fn activate_nearest(&mut self, trigger: InteractTrigger) -> Option<InteractOutcome> {
        let player = self.player_position();
        self.interaction
            .activate(&self.live, ActivateTarget::Nearest, player, trigger)
    }

    /// Activate one live slot directly (tests and explicit scripted targets).
    pub fn activate_live(
        &mut self,
        slot: u32,
        generation: Option<u32>,
        trigger: InteractTrigger,
    ) -> Option<InteractOutcome> {
        let player = self.player_position();
        self.interaction.activate(
            &self.live,
            ActivateTarget::Id { slot, generation },
            player,
            trigger,
        )
    }

    /// Drain the activation log so the caller can print one console line each.
    pub fn take_interaction_log(&mut self) -> Vec<InteractOutcome> {
        self.interaction.take_log()
    }

    /// The production digest, including the open interaction state.
    ///
    /// With no door open it is byte-identical to
    /// [`world_state_hash`](crate::engine::determinism::world_state_hash) of
    /// [`Self::world_state`], so `--no-interactions` and any idle run keep the
    /// previous digests.
    pub fn production_digest(&self) -> u64 {
        let state = self.world_state();
        let open = self.interaction.open_ordinals(&self.live);
        world_state_hash_with_interactions(&state, &open)
    }

    /// Canonical world state in the existing determinism fold shape.
    ///
    /// T8 does not change the fold: it reuses [`WorldState`] and
    /// `world_state_hash` as a pure function. T10 owns the CLI digest.
    pub fn world_state(&self) -> WorldState {
        WorldState {
            tick: self.ticks,
            live: self
                .live
                .iter()
                .map(|(_, object)| {
                    let (actor, relation, current, queued, alternate, entry_action) =
                        match &object.actor {
                            Some(runtime) => (
                                1,
                                runtime.relation(),
                                runtime.current(),
                                runtime.queued(),
                                runtime.alternate(),
                                runtime.entry_action(),
                            ),
                            None => (0, 0, 0, 0, None, 0),
                        };
                    WorldEntry {
                        class: object.class.code(),
                        position: object.position,
                        visible: object.visible,
                        actor,
                        relation,
                        current,
                        queued,
                        alternate,
                        entry_action,
                    }
                })
                .collect(),
        }
    }

    /// Advance the fixed clock by `delta` and run every due tick.
    ///
    /// The return value is the number of ticks executed. The count comes from
    /// [`SimClock`] alone; queued input cannot change it.
    pub fn frame(&mut self, delta: Duration) -> Result<usize, FrameError> {
        let batch = self.clock.advance(delta);
        let count = batch.len();
        for tick in batch.ticks() {
            self.cull_to_player();
            self.run_tick(*tick)?;
        }
        Ok(count)
    }

    /// Run the 18 stages once, in documented order, for one fixed tick.
    ///
    /// Input is sampled exactly once here, at the tick boundary (stage 1).
    pub fn run_tick(&mut self, tick: ObjectTick) -> Result<(), FrameError> {
        self.stage_trace.clear();
        for stage in STAGE_ORDER {
            self.stage_trace.push(stage);
            match stage {
                Stage::Clock => self.stage_clock(tick),
                Stage::ScratchReset => self.stage_scratch_reset(),
                Stage::ScriptUnits => self.stage_script_units(),
                Stage::AudioEmitters => self.stage_audio_emitters(),
                Stage::TimeOfDay => self.stage_time_of_day(),
                Stage::ActivationCull => self.stage_activation_cull(),
                Stage::AnimationTimers => self.stage_animation_timers(),
                Stage::PlayerSolver => self.stage_player_solver(),
                Stage::PlayerMovement => self.stage_player_movement(),
                Stage::PairwiseContact => self.stage_pairwise_contact(),
                Stage::HitList => self.stage_hit_list(),
                Stage::PerObjectTick => self.stage_per_object_tick(),
                Stage::TriggerQuery => self.stage_trigger_query(),
                Stage::MovingContact => self.stage_moving_contact(),
                Stage::RenderSubmission => self.stage_render_submission(),
                Stage::Streaming => self.stage_streaming(),
                Stage::Particles => self.stage_particles(),
                Stage::PostFrameChanges => self.stage_post_frame_changes()?,
            }
        }
        self.ticks = self.ticks.saturating_add(1);
        self.stats.ticks = self.ticks;
        Ok(())
    }

    /// Stage 1 - fixed step: record the tick and sample input at the boundary.
    ///
    /// The interaction layer is registered and its `--interact-at` schedule is
    /// applied here, before any stage reads the live set, so a scripted
    /// activation is a pure function of the global tick index exactly like
    /// `--warp-at`.
    fn stage_clock(&mut self, tick: ObjectTick) {
        self.tick_index = tick.index;
        if self.interaction.enabled() {
            self.interaction.register_live(&self.live);
            let player = self.player_position();
            self.interaction
                .apply_schedule(&self.live, player, tick.index);
        }
        self.current_input = self.input_queue.sample();
    }

    /// Stage 2 - clear the per-frame scratch block.
    fn stage_scratch_reset(&mut self) {
        self.scratch = FrameScratch::default();
    }

    /// Stage 3 - script/event unit execution. Documented no-op until the script
    /// slice.
    fn stage_script_units(&mut self) {}

    /// Stage 4 - per-unit audio emitters. Documented no-op until the audio slice.
    fn stage_audio_emitters(&mut self) {}

    /// Stage 5 - clock / time-of-day. Documented no-op until the time-of-day
    /// table is wired.
    fn stage_time_of_day(&mut self) {}

    /// Stage 6 - activation and distance culling.
    ///
    /// Walks the live list in order, compares integer squared distance against
    /// the view radius, writes the `visible` flag and calls the class query hook
    /// when one is installed. A null query hook is the documented opt-out.
    fn stage_activation_cull(&mut self) {
        let view = self.view;
        for id in self.live.order().to_vec() {
            let Ok(position) = self.live.get(id).map(|object| object.position) else {
                continue;
            };
            let hidden = self.interaction.enabled() && self.interaction.is_open(id);
            let visible = within_radius(view.eye, position, view.radius) && !hidden;
            if let Ok(object) = self.live.get_mut(id) {
                object.visible = visible;
            }
            if visible {
                self.scratch.activated += 1;
            } else {
                self.scratch.culled += 1;
            }

            let hook = self
                .live
                .get(id)
                .ok()
                .and_then(|object| hooks_for_class(object.class).query);
            if let Some(hook) = hook {
                let _answer = hook(&self.world, id, Query);
            }
        }
    }

    /// Stage 7 - per-unit animation/timer advance. Documented no-op until the
    /// animation slice.
    fn stage_animation_timers(&mut self) {}

    /// Stage 8 - player movement solver. Documented no-op.
    ///
    /// The original runs a four-step sequential move/collide/commit solver from
    /// the frame tick. Its exact probe/resolve/commit order is UNKNOWN, so this
    /// stage stays a named no-op instead of inventing a partial copy. The
    /// deterministic horizontal sweep that makes walls solid lives in stage 9,
    /// next to the translation it resolves.
    fn stage_player_solver(&mut self) {}

    /// Stage 9 - player input to movement through the deterministic sweep, the
    /// jump rule and the gravity/ground/ceiling resolver.
    ///
    /// The sampled input becomes an integer horizontal `X`/`Y` delta (the world
    /// is Z-up, see [`crate::engine::axis`]) and is resolved against the world's
    /// [`CollisionMesh`](crate::engine::collision::CollisionMesh). A zero axis
    /// performs no horizontal movement and no collision query, so a no-input
    /// tick cannot change the position on its own. The sweep stops before walls
    /// and slides along them.
    ///
    /// A `Jump` press edge starts a jump only when the player ended the previous
    /// tick grounded; it replaces the signed vertical velocity with
    /// [`JUMP_IMPULSE_SUBUNITS_PER_TICK`]. Because the edge is consumed once and
    /// the flag is `false` while airborne, a held button and a mid-air press do
    /// nothing, so there is no double jump. Landing sets `grounded` again and the
    /// next press edge is allowed.
    ///
    /// Vertical motion (`Z`) is resolved after the horizontal sweep, at the
    /// resulting horizontal point: gravity accumulates a signed velocity, a
    /// rising player is clamped below the lowest ceiling above the feet, and a
    /// falling player lands exactly on the highest floor at or below the feet. A
    /// world with no floor triangles leaves `Z` untouched. This is a correct
    /// deterministic resolver, not the original four-step solver.
    ///
    /// The committed horizontal delta also produces the player's facing yaw:
    /// the requested input is not used directly, so a blocked player keeps the
    /// previous facing and a slide faces along the wall. The delta is integer,
    /// so the produced rotation is deterministic.
    fn stage_player_movement(&mut self) {
        let Some(player) = self.player else {
            return;
        };
        self.stats.direct_player_ticks = self.stats.direct_player_ticks.saturating_add(1);

        let [right, forward] = self.current_input.move_axis();
        let mut horizontal_delta = None;
        if right != 0 || forward != 0 {
            let mut delta = [0i32; 3];
            delta[HORIZONTAL_A] = right.saturating_mul(PLAYER_SPEED_UNITS_PER_TICK);
            delta[HORIZONTAL_B] = forward.saturating_mul(PLAYER_SPEED_UNITS_PER_TICK);
            // Closed doors add their walls to the room walls for the horizontal
            // sweep only; open doors and the room-alone case keep the old path.
            self.interaction.rebuild(&self.world.collision);
            let collision = self
                .interaction
                .combined()
                .unwrap_or(&self.world.collision);
            if let Ok(object) = self.live.get_mut(player) {
                let before = object.position;
                object.position = collision.sweep(object.position, delta, PLAYER_RADIUS_UNITS);
                horizontal_delta = Some([
                    object.position[HORIZONTAL_A] - before[HORIZONTAL_A],
                    object.position[HORIZONTAL_B] - before[HORIZONTAL_B],
                ]);
            }
        }
        self.player_moving = matches!(horizontal_delta, Some([dx, dy]) if dx != 0 || dy != 0);
        if let Some([dx, dy]) = horizontal_delta
            && let Some(rotation) = pose::yaw_rotation_from_direction(dx as f32, dy as f32)
        {
            self.player_yaw = Some(rotation);
            self.player_yaw_binary = pose::binary_yaw_from_direction(dx as f32, dy as f32);
        }

        if self.player_grounded && self.current_input.is_pressed(InputButton::Jump) {
            self.player_vertical_velocity = JUMP_IMPULSE_SUBUNITS_PER_TICK;
        }

        let Ok(position) = self.live.get(player).map(|object| object.position) else {
            return;
        };
        let step = self
            .world
            .collision
            .vertical_step(position, self.player_vertical_velocity);
        self.player_vertical_velocity = step.velocity_subunits;
        self.player_grounded = step.grounded;
        if let Ok(object) = self.live.get_mut(player) {
            object.position[UP] = step.vertical;
        }
    }

    /// Stage 10 - pairwise contact broad phase and resolver. Documented no-op
    /// until the contact slice.
    fn stage_pairwise_contact(&mut self) {}

    /// Stage 11 - hit-record pass per object.
    ///
    /// Evidence: the pass walks each object's attack/hit list and resolves it
    /// (`docs/notes/actor-ai.md` section 1). The **producer** of a hit record is
    /// the actor base update's strike transition, which needs the animation
    /// timeline's action kind (`docs/notes/actor-ai.md` section 3). This engine
    /// has no animation timeline, so it has no evidenced source of a hit record
    /// and this pass queues none. The body is the producer seam; it does not
    /// manufacture a hit to keep the pass busy.
    fn stage_hit_list(&mut self) {}

    /// Stage 12 - generic per-object tick.
    ///
    /// Walks the live list in live order and calls the class tick hook when one
    /// is installed. The player is in this list too, so it is updated a second
    /// time after stage 9; that double update is documented original behaviour.
    fn stage_per_object_tick(&mut self) {
        let tick = ObjectTick::step(self.tick_index);
        // Snapshot this tick's events once, in FIFO order. Stage 11 has already
        // run as a producer seam; stage 13 has not drained the queue yet, so the
        // carrier sees exactly the events that belong to this tick.
        let batch: Vec<EngineEvent> = self.events.pending().to_vec();
        for id in self.live.order().to_vec() {
            self.stats.generic_ticks = self.stats.generic_ticks.saturating_add(1);
            if self.player == Some(id) {
                self.stats.generic_player_ticks = self.stats.generic_player_ticks.saturating_add(1);
            }
            let (hook, has_actor) = match self.live.get(id) {
                Ok(object) => (hooks_for_class(object.class).tick, object.actor.is_some()),
                Err(_) => (None, false),
            };
            if let Some(hook) = hook {
                self.stats.hooked_ticks = self.stats.hooked_ticks.saturating_add(1);
                // The carrier receives only the events addressed to it, in the
                // same FIFO order. The slice is read-only: `advance` still
                // selects nothing.
                let targeted: Vec<EngineEvent> = batch
                    .iter()
                    .copied()
                    .filter(|event| event.target() == id)
                    .collect();
                if has_actor {
                    self.stats.actor_ticks = self.stats.actor_ticks.saturating_add(1);
                    self.stats.actor_events_passed = self
                        .stats
                        .actor_events_passed
                        .saturating_add(targeted.len() as u64);
                }
                if self.player == Some(id) {
                    self.stats.hooked_player_ticks =
                        self.stats.hooked_player_ticks.saturating_add(1);
                }
                hook(&mut self.world, &mut self.live, id, tick, &targeted);
            }
        }
    }

    /// Stage 13 - area/trigger query pass and the defined event delivery stage.
    ///
    /// Evidence: a spatial query from a cylinder around each object
    /// (`docs/notes/actor-ai.md` section 1). The cylinder dimensions and the
    /// trigger-record source are UNKNOWN, so this pass produces no contact
    /// event. It is also the **delivery stage**: every event queued for this
    /// tick is drained here, in FIFO order, delivered to the target's event hook
    /// and then forgotten. Because the drain clears the queue, an event produced
    /// in tick N can never be delivered by tick N+1.
    fn stage_trigger_query(&mut self) {
        self.deliver_events();
    }

    /// Deliver every queued event to its target object's event hook.
    ///
    /// Delivery follows the queue's FIFO order and never reorders by class,
    /// producer or target. A target that is no longer live still counts as
    /// delivered; a null hook is the documented class opt-out and makes zero
    /// calls. The hook body is an empty witness, so this function changes no
    /// simulation state; the drain is what makes the event transient.
    fn deliver_events(&mut self) {
        let events = self.events.drain();
        for event in events {
            self.stats.events_delivered = self.stats.events_delivered.saturating_add(1);
            let target = event.target();
            let hook = self
                .live
                .get(target)
                .ok()
                .and_then(|object| hooks_for_class(object.class).event);
            if let Some(hook) = hook {
                self.stats.event_hook_calls = self.stats.event_hook_calls.saturating_add(1);
                hook(&mut self.world, target, &event);
            }
        }
    }

    /// Stage 14 - moving-object world/contact pass.
    ///
    /// The original body of this pass is UNKNOWN, so its documented no-op is
    /// replaced by **our own** synthetic locomotion
    /// ([`crate::engine::synthetic_locomotion`]): only here do non-player
    /// `monster_` actors change position. The player is skipped explicitly, so
    /// stage 9 and the input path are untouched.
    ///
    /// Each actor's patrol is a deterministic integer fold of its live id, its
    /// authored start position and the tick. A step is committed only when the
    /// existing [`CollisionMesh`](crate::engine::collision::CollisionMesh)
    /// narrow phase reports no contact and the room fence keeps the body inside
    /// the geometry bounds; otherwise the heading turns 90 degrees and nothing
    /// moves this tick. The decoders gave no NPC pathing: this layer exists so
    /// the authored `walk` clip reacts to a real position delta, and it is not a
    /// reproduction of retail actor movement.
    fn stage_moving_contact(&mut self) {
        if !self.npc_motion {
            return;
        }
        let tick = self.tick_index;
        for id in self.live.order().to_vec() {
            let from = match self.live.get(id) {
                Ok(object) => {
                    if !synthetic_locomotion::is_moving_actor(object, id, self.player) {
                        continue;
                    }
                    object.position
                }
                Err(_) => continue,
            };
            let patrol = match self.patrols.get_mut(&id) {
                Some(patrol) => patrol,
                None => {
                    self.patrols
                        .insert(id, SyntheticPatrol::initial(id, from));
                    self.patrols
                        .get_mut(&id)
                        .expect("the patrol was just inserted")
                }
            };
            let next = patrol.step(tick, &self.world.collision, from);
            if let Some(next) = next
                && let Ok(object) = self.live.get_mut(id)
            {
                object.position = next;
            }
        }
        // Drop patrol state for objects that left the live list. The snapshot
        // keeps the retain closure from borrowing the arena.
        let live_ids = self.live.order().to_vec();
        self.patrols.retain(|id, _| live_ids.contains(id));
    }

    /// Stage 15 - render submission as a CPU snapshot.
    ///
    /// Items are emitted in live order, so the snapshot is deterministic. No
    /// mesh is built and no backend is called.
    fn stage_render_submission(&mut self) {
        self.scene.clear();
        for (id, object) in self.live.iter() {
            self.scene.push(RenderItem {
                id,
                class: object.class.code(),
                position: object.position,
                visible: object.visible,
            });
        }
    }

    /// Stage 16 - streaming / prefetch. Documented no-op until the streaming
    /// slice.
    fn stage_streaming(&mut self) {}

    /// Stage 17 - particles. Documented no-op until the particle slice.
    fn stage_particles(&mut self) {}

    /// Stage 18 - post-frame spawn/despawn bookkeeping and tracker recompute.
    ///
    /// Despawns are applied first, then spawns, then the tracker is reconciled
    /// against the live names: every live name owns a ready slot (duplicates
    /// share one) and a slot whose name has no live object is released.
    fn stage_post_frame_changes(&mut self) -> Result<(), FrameError> {
        for id in std::mem::take(&mut self.despawn_queue) {
            if self.live.despawn(id).is_ok() {
                if self.player == Some(id) {
                    self.player = None;
                }
                self.stats.despawned = self.stats.despawned.saturating_add(1);
            }
        }

        for spawn in std::mem::take(&mut self.spawn_queue) {
            let object = LiveObject {
                class: spawn
                    .class
                    .unwrap_or_else(|| classify(&spawn.name)),
                name: spawn.name,
                position: spawn.position,
                visible: true,
                slot: None,
                anim_index: 0,
                // Actor initialisation runs here, in stage 18, strictly after
                // the class was captured at load-complete. It never feeds back
                // into the class.
                actor: spawn.actor.map(ActorRuntime::initial),
            };
            self.live.spawn(object);
            self.stats.spawned = self.stats.spawned.saturating_add(1);
        }

        self.recompute_tracker()
    }

    fn recompute_tracker(&mut self) -> Result<(), FrameError> {
        for id in self.live.order().to_vec() {
            let name = match self.live.get(id) {
                Ok(object) => object.name.clone(),
                Err(_) => continue,
            };
            let slot = self
                .tracker
                .acquire(&name)
                .map_err(FrameError::Tracker)?;
            self.tracker
                .complete(slot)
                .map_err(FrameError::Tracker)?;
            if let Ok(object) = self.live.get_mut(id) {
                object.slot = Some(slot);
            }
        }

        let mut live_names: Vec<String> = Vec::with_capacity(self.live.len());
        for id in self.live.order() {
            if let Ok(object) = self.live.get(*id) {
                live_names.push(object.name.clone());
            }
        }
        let stale: Vec<usize> = self
            .tracker
            .slots()
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot.name() {
                Some(name) if !live_names.iter().any(|live| live.as_str() == name) => Some(index),
                _ => None,
            })
            .collect();
        for slot in stale {
            self.tracker.release(slot).map_err(FrameError::Tracker)?;
        }
        Ok(())
    }
}

fn within_radius(eye: [i32; 3], point: [i32; 3], radius: i32) -> bool {
    let dx = i64::from(point[0]) - i64::from(eye[0]);
    let dy = i64::from(point[1]) - i64::from(eye[1]);
    let dz = i64::from(point[2]) - i64::from(eye[2]);
    let radius = i64::from(radius);
    dx * dx + dy * dy + dz * dz <= radius * radius
}

fn hooks_for_class(class: RenderClass) -> Hooks {
    CLASS_TABLE
        .iter()
        .find(|entry| entry.class == class)
        .map_or(Hooks::NONE, |entry| entry.hooks)
}

#[cfg(test)]
mod tests {
    use super::{
        CullView, DEFAULT_CULL_RADIUS, FrameState, FrameStats, PLAYER_SPEED_UNITS_PER_TICK,
        STAGE_ORDER, Stage,
    };
    use crate::engine::actor::ActorProfile;
    use crate::engine::collision::CollisionMesh;
    use crate::engine::determinism::world_state_hash;
    use crate::engine::events::{ContactRecord, EngineEvent, HitRecord, Message};
    use crate::engine::input::{InputButton, InputEvent};
    use crate::engine::synthetic_locomotion::{
        SYNTHETIC_ACTOR_RADIUS_UNITS, SYNTHETIC_CYCLE_TICKS,
    };
    use crate::engine::clock::ObjectTick;
    use crate::engine::live::LiveId;
    use crate::engine::world::World;
    use crate::level::{ObjectRecord, ObjectReport};
    use std::time::Duration;

    fn record(name: &str, position: [i16; 3]) -> ObjectRecord {
        ObjectRecord {
            name: name.to_string(),
            position,
            object_type: 4,
            archetype: 7,
            instance_id: 1,
            flags: 0,
        }
    }

    fn world_with_raziel() -> World {
        let report = ObjectReport {
            room_offset: 0,
            count: 1,
            array_offset: 0,
            objects: vec![record("raziel", [0, 0, 0])],
        };
        World::from_report("test".to_string(), 1, &report)
    }

    fn empty_world() -> World {
        World::from_report(
            "test".to_string(),
            1,
            &ObjectReport {
                room_offset: 0,
                count: 0,
                array_offset: 0,
                objects: Vec::new(),
            },
        )
    }

    /// One representative name per documented class prefix.
    const CLASS_PROBES: [&str; 6] = [
        "raziel__player",
        "sreavr__probe",
        "glphicon_probe",
        "physical_probe",
        "monster_wallcr",
        "particle_dust",
    ];

    fn probe_trios() -> [[u8; 3]; 8] {
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

    fn actor_profile(ids: &[u8]) -> ActorProfile {
        ActorProfile::parse(&crate::actor::synthetic_actor("probe", ids, probe_trios()))
            .expect("a synthetic actor must parse")
            .expect("a synthetic actor must carry a block")
    }

    fn carrier_profiles(frame: &FrameState) -> Vec<(crate::actor::ActorKind, [[u8; 3]; 8], Vec<u8>)> {
        frame
            .live()
            .iter()
            .filter_map(|(_, object)| object.actor.as_ref())
            .map(|actor| {
                let profile = actor.profile();
                (
                    profile.kind,
                    profile.trios,
                    profile.entries.iter().map(|entry| entry.id).collect(),
                )
            })
            .collect()
    }

    fn frame() -> FrameState {
        FrameState::new(world_with_raziel())
    }

    /// The raziel world plus one vertical wall at `x = 100` spanning the
    /// horizontal `y = -500..500` and the vertical `z = -100..400`. The player
    /// starts at the origin, 100 units away.
    fn world_with_wall() -> World {
        let mut world = world_with_raziel();
        let vertices = vec![
            [100, -500, -100],
            [100, 500, -100],
            [100, 500, 400],
            [100, -500, 400],
        ];
        let faces = vec![[0u32, 1, 2], [0, 2, 3]];
        world.collision = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert!(!world.collision.is_empty(), "the test wall must be a barrier");
        world
    }

    /// A collision mesh made of upward-facing (+Z) floor quads. Each tuple is
    /// `(up, x0, x1, h0, h1)`, where `up` is the vertical coordinate and
    /// `h0..h1` is the horizontal world Y range.
    fn floor_mesh(quads: &[(i32, i32, i32, i32, i32)]) -> CollisionMesh {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for &(up, x0, x1, h0, h1) in quads {
            let base = vertices.len() as u32;
            vertices.extend([
                [x0, h0, up],
                [x1, h0, up],
                [x1, h1, up],
                [x0, h1, up],
            ]);
            faces.push([base, base + 1, base + 2]);
            faces.push([base, base + 2, base + 3]);
        }
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    /// The raziel world plus one walkable floor at vertical `floor_up`, with the
    /// player spawned `player_up` units above it.
    fn world_with_floor(floor_up: i32, player_up: i16) -> World {
        let report = ObjectReport {
            room_offset: 0,
            count: 1,
            array_offset: 0,
            objects: vec![record("raziel", [0, 0, player_up])],
        };
        let mut world = World::from_report("test".to_string(), 1, &report);
        world.collision = floor_mesh(&[(floor_up, -500, 500, -500, 500)]);
        assert!(world.collision.has_ground(), "the fixture must carry ground");
        world
    }

    /// One walkable floor at vertical `floor_up` and one downward-facing plane
    /// at `ceiling_up`, spanning the same 200x200 footprint.
    fn floor_and_ceiling_mesh(floor_up: i32, ceiling_up: i32) -> CollisionMesh {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        vertices.extend([
            [-100, -100, floor_up],
            [100, -100, floor_up],
            [100, 100, floor_up],
            [-100, 100, floor_up],
        ]);
        faces.push([0, 1, 2]);
        faces.push([0, 2, 3]);
        let base = vertices.len() as u32;
        vertices.extend([
            [-100, -100, ceiling_up],
            [-100, 100, ceiling_up],
            [100, 100, ceiling_up],
            [100, -100, ceiling_up],
        ]);
        faces.push([base, base + 1, base + 2]);
        faces.push([base, base + 2, base + 3]);
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    /// The raziel world at the origin with a walkable floor at 0 and a solid
    /// wall at `x = 100` tall enough that a jump cannot clear it.
    fn world_with_wall_and_floor() -> World {
        let mut world = world_with_raziel();
        let mut vertices = vec![
            [100, -500, -100],
            [100, 500, -100],
            [100, 500, 400],
            [100, -500, 400],
        ];
        let mut faces = vec![[0u32, 1, 2], [0, 2, 3]];
        let base = vertices.len() as u32;
        vertices.extend([
            [-500, -500, 0],
            [100, -500, 0],
            [100, 500, 0],
            [-500, 500, 0],
        ]);
        faces.push([base, base + 1, base + 2]);
        faces.push([base, base + 2, base + 3]);
        world.collision = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert!(!world.collision.is_empty(), "the fixture must carry a wall");
        assert!(world.collision.has_ground(), "the fixture must carry ground");
        world
    }

    fn tick(frame: &mut FrameState, index: u64) {
        frame
            .run_tick(ObjectTick::step(index))
            .expect("the synthetic frame must tick");
    }

    #[test]
    fn the_driver_invokes_the_eighteen_stages_in_order() {
        let mut frame = frame();
        tick(&mut frame, 0);
        assert_eq!(frame.stage_trace(), STAGE_ORDER);
        assert_eq!(frame.stage_trace().len(), 18);
        assert_eq!(frame.ticks(), 1);
    }

    #[test]
    fn enter_unit_keeps_the_player_and_releases_the_old_live_set() {
        let mut frame = frame();
        let player = frame.player().expect("the fixture binds a player");
        frame.queue_spawn("capsule0", [10, 0, 0]);
        tick(&mut frame, 0);
        assert_eq!(frame.live().len(), 2, "player plus the spawned actor");

        let world = World::from_report(
            "under2".to_string(),
            2,
            &ObjectReport {
                room_offset: 0,
                count: 1,
                array_offset: 0,
                objects: vec![record("monster_wallcr", [5, 0, 0])],
            },
        );
        frame.enter_unit(world, [7, 8, 9]);

        assert_eq!(frame.player(), Some(player), "the player keeps its identity");
        assert_eq!(frame.player_position(), Some([7, 8, 9]));
        assert_eq!(frame.live().len(), 1, "the old unit's actors are released");
        assert_eq!(frame.world().level.name, "under2");
        assert_eq!(frame.ticks(), 1, "the global tick count survives the swap");
        assert_eq!(frame.tracker().ready_slots().count(), 0);
    }

    #[test]
    fn enter_unit_spawns_a_player_when_the_previous_unit_had_none() {
        let mut frame = FrameState::new(empty_world());
        assert!(frame.player().is_none());
        frame.enter_unit(world_with_raziel(), [3, 4, 5]);
        assert_eq!(frame.player_position(), Some([3, 4, 5]));
        assert_eq!(frame.live().len(), 1);
    }

    #[test]
    fn the_player_is_ticked_directly_and_again_in_the_generic_pass() {
        let mut frame = frame();
        tick(&mut frame, 0);

        let stats = frame.stats();
        assert_eq!(stats.direct_player_ticks, 1, "stage 9 updates the player");
        assert_eq!(
            stats.generic_player_ticks, 1,
            "the player is also an ordinary live object in stage 12"
        );
        assert_eq!(
            stats.hooked_player_ticks, 1,
            "the Raziel class installs the non-null tick hook"
        );
        assert_eq!(stats.generic_ticks, frame.live().len() as u64);
        assert_eq!(
            stats.hooked_ticks, stats.generic_ticks,
            "every synthetic live object carries a tick hook in this fixture"
        );

        tick(&mut frame, 1);
        let stats = frame.stats();
        assert_eq!((stats.direct_player_ticks, stats.generic_player_ticks), (2, 2));
    }

    #[test]
    fn only_the_two_class_table_tick_rows_receive_the_per_object_pass() {
        let mut frame = FrameState::new(empty_world());
        for name in CLASS_PROBES {
            frame.queue_spawn(name, [0, 0, 0]);
        }
        // Stage 18 of the first tick applies the deferred spawns; stage 12 of
        // the second tick is the first pass that sees all six objects.
        tick(&mut frame, 0);
        assert_eq!(frame.live().len(), CLASS_PROBES.len());

        let expected_hooks = frame
            .live()
            .iter()
            .filter(|(_, object)| super::hooks_for_class(object.class).tick.is_some())
            .count() as u64;
        assert_eq!(
            expected_hooks, 2,
            "the class table installs a tick only for raziel__ and monster_"
        );

        tick(&mut frame, 1);
        let stats = frame.stats();
        assert_eq!(
            stats.generic_ticks,
            CLASS_PROBES.len() as u64,
            "stage 12 visits every live object"
        );
        assert_eq!(
            stats.hooked_ticks, expected_hooks,
            "only the two tick rows run a body; the other four make zero calls"
        );
        assert_eq!(
            stats.actor_ticks, 0,
            "no live object carries an actor profile in this fixture"
        );

        for index in 2..7 {
            tick(&mut frame, index);
        }
        let stats = frame.stats();
        assert_eq!(stats.generic_ticks, 6 * CLASS_PROBES.len() as u64);
        assert_eq!(
            stats.hooked_ticks,
            6 * expected_hooks,
            "the dispatch is stable across ticks and never drifts to another class"
        );
    }

    #[test]
    fn the_four_non_tick_classes_never_reach_the_actor_pass() {
        let mut frame = FrameState::new(empty_world());
        for name in ["sreavr__probe", "glphicon_probe", "physical_probe", "particle_dust"] {
            frame.queue_spawn(name, [0, 0, 0]);
        }
        tick(&mut frame, 0);
        for index in 1..11 {
            tick(&mut frame, index);
        }

        let stats = frame.stats();
        assert_eq!(stats.generic_ticks, 10 * 4);
        assert_eq!(stats.hooked_ticks, 0, "no non-tick class may run a tick body");
        assert_eq!(stats.actor_ticks, 0);
    }

    #[test]
    fn a_monster_instance_is_classified_from_its_loaded_class_not_its_name() {
        let mut frame = FrameState::new(empty_world());
        let profile = actor_profile(&[0x18, 0x25]);

        // Real-data shape: the placed instance is bare `skinner`; the loaded
        // model supplied the monster row. Contrast: the same bare name with no
        // model class stays unclassified.
        frame.queue_spawn_with_class(
            "skinner",
            Some(crate::engine::class::RenderClass::Monster),
            [0, 0, 0],
            Some(profile),
        );
        frame.queue_spawn("skinner", [1, 0, 0]);

        // Stage 18 of tick 0 applies the spawns; stage 12 of tick 1 sees them.
        tick(&mut frame, 0);
        tick(&mut frame, 1);

        let live: Vec<_> = frame.live().iter().map(|(_, object)| object).collect();
        assert_eq!(live.len(), 2);
        // The class-keyed monster now carries the synthetic patrol, so its
        // position is no longer the spawn point after tick 1. The carrier is the
        // stable key that separates the two same-named objects.
        let keyed = live
            .iter()
            .find(|object| object.actor.is_some())
            .expect("class-keyed spawn");
        let bare = live
            .iter()
            .find(|object| object.actor.is_none())
            .expect("bare spawn");
        assert_eq!(
            keyed.class,
            crate::engine::class::RenderClass::Monster,
            "the loaded class must select the monster row"
        );
        assert!(keyed.actor.is_some(), "the monster carrier is attached");
        assert_eq!(
            bare.class,
            crate::engine::class::RenderClass::Unclassified,
            "the bare instance name must not be the class key"
        );
        assert!(bare.actor.is_none());

        let stats = frame.stats();
        assert_eq!(stats.hooked_ticks, 1, "only the class-keyed monster ticks");
        assert_eq!(stats.actor_ticks, 1, "and its carrier is advanced");
    }

    #[test]
    fn the_class_is_fixed_before_actor_initialisation_runs() {
        let mut frame = FrameState::new(empty_world());
        let profile = actor_profile(&[0x18, 0x25]);

        frame.queue_spawn_with_class(
            "skinner_",
            Some(crate::engine::class::RenderClass::Physical),
            [0, 0, 0],
            Some(profile),
        );

        assert_eq!(frame.pending_changes().0, 1, "the spawn is still deferred");
        assert!(frame.live().is_empty(), "no actor exists before stage 18");

        // Stage 18 initialises the actor carrier. The class was already fixed in
        // the deferred spawn, so it cannot be re-derived from the profile.
        tick(&mut frame, 0);
        let object = frame
            .live()
            .iter()
            .map(|(_, object)| object)
            .next()
            .expect("the deferred spawn is applied");
        assert_eq!(
            object.class,
            crate::engine::class::RenderClass::Physical,
            "the load-time class survives actor initialisation"
        );
        let actor = object.actor.as_ref().expect("the profile is initialised");
        assert_eq!(
            actor.profile().kind,
            crate::actor::ActorKind::Monster,
            "the profile kind is not the class source"
        );
    }

    #[test]
    fn non_tick_classes_install_no_hook() {
        let mut frame = FrameState::new(empty_world());
        for class in [
            crate::engine::class::RenderClass::Sreavr,
            crate::engine::class::RenderClass::GlyphIcon,
            crate::engine::class::RenderClass::Physical,
            crate::engine::class::RenderClass::Particle,
        ] {
            frame.queue_spawn_with_class("probe", Some(class), [0, 0, 0], None);
        }
        tick(&mut frame, 0);
        for index in 1..6 {
            tick(&mut frame, index);
        }

        let stats = frame.stats();
        assert_eq!(stats.generic_ticks, 5 * 4);
        assert_eq!(
            stats.hooked_ticks, 0,
            "no non-tick class may install a hook"
        );
        assert_eq!(stats.actor_ticks, 0);
    }

    #[test]
    fn actor_state_sequence_is_stable_and_repeatable() {
        fn run() -> (Vec<Vec<(u8, u8)>>, u64) {
            let mut frame = FrameState::new(empty_world());
            let profile = actor_profile(&[0x18, 0x25]);
            frame.queue_spawn_loaded("monster_wallcr", [1, 0, 0], Some(profile.clone()));
            frame.queue_spawn_loaded("raziel__probe", [2, 0, 0], Some(profile));
            tick(&mut frame, 0);

            let mut sequence = Vec::new();
            for index in 1..=20 {
                tick(&mut frame, index);
                sequence.push(
                    frame
                        .live()
                        .iter()
                        .map(|(_, object)| {
                            let actor = object.actor.as_ref().expect("both probes are actors");
                            (actor.relation(), actor.current())
                        })
                        .collect::<Vec<_>>(),
                );
            }
            (sequence, frame.stats().actor_ticks)
        }

        let (first, first_ticks) = run();
        let (second, second_ticks) = run();
        assert_eq!(
            first, second,
            "the same actor world must replay the same per-tick runtime state"
        );
        assert_eq!(first_ticks, second_ticks);
        assert_eq!(
            first_ticks,
            2 * 20,
            "both tick classes reach an actor carrier on every tick"
        );
        assert!(
            first.iter().all(|row| row == &vec![(0u8, 0u8); 2]),
            "no relation or action id is selected while the decision body is UNKNOWN"
        );
    }

    #[test]
    fn profile_data_is_observable_and_unchanged_by_ticking() {
        let mut frame = FrameState::new(empty_world());
        let profile = actor_profile(&[0x18, 0x25, 0x26]);
        frame.queue_spawn_loaded("monster_wallcr", [0, 0, 0], Some(profile.clone()));
        tick(&mut frame, 0);

        let before = carrier_profiles(&frame);
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].1, probe_trios());
        assert_eq!(before[0].2, vec![0x18, 0x25, 0x26]);

        for index in 1..=30 {
            tick(&mut frame, index);
        }

        assert_eq!(
            carrier_profiles(&frame),
            before,
            "ticking must not consume, reorder or reinterpret the behaviour data"
        );
        assert_eq!(
            frame
                .live()
                .iter()
                .find(|(_, object)| object.name == "monster_wallcr")
                .and_then(|(_, object)| object.actor.as_ref())
                .expect("the carrier survives")
                .current(),
            0,
            "the profile ids stay data; no id is copied into the current state"
        );
    }

    #[test]
    fn a_non_actor_class_never_advances_an_actor_carrier() {
        let mut frame = FrameState::new(empty_world());
        frame.queue_spawn_loaded("particle_dust", [0, 0, 0], Some(actor_profile(&[0x18])));
        // A tick class with no actor block is hooked but has nothing to advance.
        frame.queue_spawn_loaded("monster_wallcr", [1, 0, 0], None);
        tick(&mut frame, 0);

        for index in 1..=10 {
            tick(&mut frame, index);
        }

        let stats = frame.stats();
        assert_eq!(stats.generic_ticks, 10 * 2);
        assert_eq!(
            stats.hooked_ticks, 10,
            "only the monster row installs a tick; the particle class never does"
        );
        assert_eq!(
            stats.actor_ticks, 0,
            "the particle carrier is never advanced and the monster has no carrier"
        );

        let particle = frame
            .live()
            .iter()
            .find(|(_, object)| object.name == "particle_dust")
            .and_then(|(_, object)| object.actor.as_ref())
            .expect("the non-actor class still carries the data it was given");
        assert_eq!((particle.relation(), particle.current()), (0, 0));
    }

    #[test]
    fn zero_input_leaves_the_player_position_unchanged() {
        let mut frame = frame();
        let before = frame.player_position().expect("the player is bound");
        for index in 0..60 {
            tick(&mut frame, index);
        }
        assert_eq!(frame.queued_input_len(), 0);
        assert_eq!(frame.player_position(), Some(before));
        assert_eq!(frame.ticks(), 60);
    }

    fn monster_position(frame: &FrameState, name: &str) -> [i32; 3] {
        let id = id_of(frame, name);
        frame.live().get(id).expect("the monster is live").position
    }

    #[test]
    fn synthetic_npc_motion_moves_a_monster_after_n_ticks() {
        let mut frame = FrameState::new(world_with_raziel());
        frame.queue_spawn("monster_wallcr", [500, 500, 0]);
        tick(&mut frame, 0);
        let start = monster_position(&frame, "monster_wallcr");
        for index in 1..SYNTHETIC_CYCLE_TICKS * 2 {
            tick(&mut frame, index);
        }
        assert_ne!(
            monster_position(&frame, "monster_wallcr"),
            start,
            "a monster must change position once synthetic locomotion runs"
        );
    }

    #[test]
    fn synthetic_npc_motion_keeps_the_actor_inside_the_room_bounds() {
        let mut frame = FrameState::new(world_with_wall_and_floor());
        frame.queue_spawn("monster_wallcr", [-100, 0, 0]);
        tick(&mut frame, 0);
        let bounds = frame
            .world()
            .collision
            .bounds()
            .expect("the fixture carries geometry");
        for index in 1..600 {
            tick(&mut frame, index);
            let position = monster_position(&frame, "monster_wallcr");
            assert!(
                bounds.contains_with_radius(position, SYNTHETIC_ACTOR_RADIUS_UNITS),
                "tick {index} left the room: {position:?}"
            );
        }
    }

    #[test]
    fn synthetic_npc_motion_replays_the_same_path() {
        fn run() -> Vec<[i32; 3]> {
            let mut frame = FrameState::new(world_with_wall_and_floor());
            frame.queue_spawn("monster_wallcr", [-100, 0, 0]);
            tick(&mut frame, 0);
            let mut path = Vec::new();
            for index in 1..400 {
                tick(&mut frame, index);
                path.push(monster_position(&frame, "monster_wallcr"));
            }
            path
        }
        assert_eq!(run(), run(), "the same start must reproduce the same path");
    }

    #[test]
    fn disabling_synthetic_npc_motion_keeps_the_authored_positions() {
        let mut frame = FrameState::new(world_with_wall_and_floor());
        frame.set_npc_motion(false);
        frame.queue_spawn("monster_wallcr", [-100, 0, 0]);
        tick(&mut frame, 0);
        let start = monster_position(&frame, "monster_wallcr");
        for index in 1..400 {
            tick(&mut frame, index);
        }
        assert_eq!(
            monster_position(&frame, "monster_wallcr"),
            start,
            "--no-npc-motion must leave the actor where the level placed it"
        );
    }

    #[test]
    fn synthetic_npc_motion_never_moves_the_player() {
        let mut frame = FrameState::new(world_with_raziel());
        frame.queue_spawn("monster_wallcr", [100, 100, 0]);
        let spawn = frame.player_position().expect("the player is bound");
        for index in 0..300 {
            tick(&mut frame, index);
        }
        assert_eq!(frame.player_position(), Some(spawn));
        assert!(!frame.player_moving(), "no input, no player movement");
    }

    /// A zone whose spawn sits far from the world origin, like the real
    /// `cathy28` (`raziel` at `-2849,5455,11510`). The second object is 600
    /// units away, well inside the default radius but far outside it when the
    /// eye is pinned to the origin.
    fn world_spawned_far_from_origin() -> World {
        World::from_report(
            "cathy28".to_string(),
            1,
            &ObjectReport {
                room_offset: 0,
                count: 2,
                array_offset: 0,
                objects: vec![
                    record("raziel", [9000, -7000, 11510]),
                    record("monster_wallcr", [9600, -7000, 11510]),
                ],
            },
        )
    }

    #[test]
    fn an_origin_fixed_eye_culls_every_actor_in_an_offset_zone() {
        // The regression fixture: without the driver sync, stage 6 uses
        // `CullView::default()` and both actors are outside the 4096 radius.
        let mut frame = FrameState::new(world_spawned_far_from_origin());
        frame.queue_spawn("monster_wallcr", [9600, -7000, 11510]);
        tick(&mut frame, 0); // stage 18 realizes the enemy after tick 0
        assert_eq!(frame.live().len(), 2);
        assert_eq!(frame.view(), CullView::default());

        tick(&mut frame, 1);
        assert_eq!(
            frame.scratch().activated(),
            0,
            "the origin eye must not reach the offset spawn"
        );
        assert_eq!(frame.scratch().culled(), 2);
        let player = frame.player().expect("the player is bound");
        assert!(!frame.live().get(player).expect("live player").visible);
    }

    #[test]
    fn cull_follows_the_player_so_an_offset_zone_draws_its_actors() {
        let mut frame = FrameState::new(world_spawned_far_from_origin());
        frame.queue_spawn("monster_wallcr", [9600, -7000, 11510]);
        let spawn = frame.player_position().expect("the player is bound");
        frame.cull_to_player();
        assert_eq!(
            frame.view(),
            CullView {
                eye: spawn,
                radius: DEFAULT_CULL_RADIUS,
            }
        );
        tick(&mut frame, 0);
        frame.cull_to_player();
        tick(&mut frame, 1);

        assert_eq!(frame.scratch().activated(), 2, "player and nearby enemy");
        assert_eq!(frame.scratch().culled(), 0);
        let player = frame.player().expect("the player is bound");
        assert!(
            frame.live().get(player).expect("live player").visible,
            "the player body must be drawn once the eye follows it"
        );
        assert!(
            frame
                .live()
                .iter()
                .find(|(_, object)| object.name == "monster_wallcr")
                .map(|(_, object)| object.visible)
                .unwrap_or(false),
            "the enemy inside the radius must survive the cull"
        );
    }

    #[test]
    fn a_playerless_world_keeps_its_view() {
        let mut frame = FrameState::new(empty_world());
        let custom = CullView {
            eye: [1, 2, 3],
            radius: 10,
        };
        frame.set_view(custom);
        frame.cull_to_player();
        assert_eq!(frame.view(), custom, "no bound player, no view change");
    }

    #[test]
    fn the_origin_player_keeps_the_default_view_and_the_under1_digest() {
        // Determinism guard. With the player at the origin the eye is [0,0,0],
        // which equals `CullView::default()`. `under1` therefore folds the same
        // visibility bytes as before this change and its 60-tick digest stays
        // `ede39c540a10ee39`. Any other unit may legitimately change digest.
        let mut frame = FrameState::new(world_with_raziel());
        assert_eq!(frame.player_position(), Some([0, 0, 0]));
        frame.cull_to_player();
        assert_eq!(frame.view(), CullView::default());
    }

    #[test]
    fn the_frame_driver_points_the_cull_at_the_player_each_tick() {
        let mut frame = FrameState::new(world_spawned_far_from_origin());
        let spawn = frame.player_position().expect("the player is bound");
        assert_eq!(frame.view(), CullView::default());
        frame
            .frame(Duration::from_secs(1))
            .expect("the synthetic frame must tick");
        assert!(frame.ticks() > 0, "one second runs real fixed ticks");
        assert_eq!(
            frame.view(),
            CullView {
                eye: spawn,
                radius: DEFAULT_CULL_RADIUS,
            },
            "the windowed driver must feed the player eye back into stage 6"
        );
    }

    #[test]
    fn the_live_player_body_tracks_the_player_position_as_it_moves() {
        // The scene copies `LiveObject.position` into `DrawItem.world`, so this
        // is the body's rendered position. Requirement: it must equal
        // `player_position()` and follow a committed move, not stay at spawn.
        let mut frame = frame();
        let player = frame.player().expect("the player is bound");
        let spawn = frame.live().get(player).expect("live player").position;

        frame.queue_input(InputEvent::press(InputButton::Forward));
        tick(&mut frame, 0);

        let moved = frame.player_position().expect("the player is bound");
        assert_eq!(moved, [0, PLAYER_SPEED_UNITS_PER_TICK, 0], "forward is +Y");
        assert_ne!(moved, spawn);
        assert_eq!(
            frame.live().get(player).expect("live player").position,
            moved,
            "the live entry the scene reads must carry the moved position"
        );
        let draw = frame
            .scene()
            .iter()
            .find(|item| item.id == player)
            .expect("stage 15 snapshots the player");
        assert_eq!(draw.position, moved, "the drawn body follows the player");
    }

    #[test]
    fn zero_input_with_a_wall_matches_the_collision_free_digest() {
        let mut walled = FrameState::new(world_with_wall());
        let mut plain = FrameState::new(world_with_raziel());
        for index in 0..40 {
            tick(&mut walled, index);
            tick(&mut plain, index);
        }
        assert_eq!(walled.player_position(), Some([0, 0, 0]), "no input, no collision move");
        assert_eq!(plain.player_position(), Some([0, 0, 0]));
        assert_eq!(
            world_state_hash(&walled.world_state()),
            world_state_hash(&plain.world_state()),
            "a wall must not change a no-input run"
        );
    }

    #[test]
    fn a_player_spawned_above_the_floor_falls_and_lands_on_it() {
        let mut frame = FrameState::new(world_with_floor(0, 50));
        let mut landed = false;
        for index in 0..200 {
            tick(&mut frame, index);
            let position = frame.player_position().expect("the player is bound");
            assert!(
                position[2] >= 0,
                "the player must never pass through the floor: {position:?}"
            );
            if frame.player_grounded() {
                landed = true;
                break;
            }
        }
        assert!(landed, "gravity must land the player on the floor");
        assert_eq!(frame.player_position().expect("the player is bound")[2], 0);
        assert_eq!(frame.player_vertical_velocity(), 0);
    }

    #[test]
    fn a_grounded_player_rests_without_jitter_or_sinking_over_300_ticks() {
        let mut frame = FrameState::new(world_with_floor(0, 0));
        for index in 0..300 {
            tick(&mut frame, index);
            assert_eq!(
                frame.player_position().expect("the player is bound")[2],
                0,
                "the resting height moved on tick {index}"
            );
            assert!(frame.player_grounded(), "the player lost the ground on tick {index}");
            assert_eq!(frame.player_vertical_velocity(), 0);
        }
    }

    #[test]
    fn walking_off_a_ledge_starts_a_fall_and_landing_stops_it() {
        let report = ObjectReport {
            room_offset: 0,
            count: 1,
            array_offset: 0,
            objects: vec![record("raziel", [0, 0, 0])],
        };
        let mut world = World::from_report("test".to_string(), 1, &report);
        // Upper floor y = 0 for x in -500..=100; lower floor y = -100 for
        // x in 100..=500.
        world.collision = floor_mesh(&[
            (0, -500, 100, -500, 500),
            (-100, 100, 500, -500, 500),
        ]);
        let mut frame = FrameState::new(world);
        frame.queue_input(InputEvent::press(InputButton::Right));

        let mut fell = false;
        let mut landed_low = false;
        for index in 0..300 {
            tick(&mut frame, index);
            let position = frame.player_position().expect("the player is bound");
            assert!(position[2] >= -100, "fell through the lower floor: {position:?}");
            if !frame.player_grounded() {
                fell = true;
            }
            if fell && frame.player_grounded() {
                landed_low = true;
                assert_eq!(position[2], -100, "must land on the lower floor");
                break;
            }
        }
        assert!(fell, "walking off the ledge must start a fall");
        assert!(landed_low, "the fall must stop on the lower floor");
        assert!(frame.player_position().expect("the player is bound")[0] > 100);
        assert_eq!(frame.player_vertical_velocity(), 0);
    }

    #[test]
    fn zero_input_after_landing_keeps_a_stable_position() {
        let mut frame = FrameState::new(world_with_floor(0, 50));
        for index in 0..200 {
            tick(&mut frame, index);
            if frame.player_grounded() {
                break;
            }
        }
        assert!(frame.player_grounded());
        let settled = frame.player_position();
        for index in 200..260 {
            tick(&mut frame, index);
            assert_eq!(frame.player_position(), settled, "tick {index} oscillated");
        }
    }

    #[test]
    fn the_same_gravity_run_twice_yields_the_same_positions_and_digest() {
        fn run() -> (Vec<[i32; 3]>, u64) {
            let mut frame = FrameState::new(world_with_floor(0, 50));
            frame.queue_input(InputEvent::press(InputButton::Right));
            let mut positions = Vec::new();
            for index in 0..120 {
                tick(&mut frame, index);
                positions.push(frame.player_position().expect("the player is bound"));
            }
            (positions, world_state_hash(&frame.world_state()))
        }

        let first = run();
        let second = run();
        assert_eq!(first.0, second.0, "the same gravity run must replay positions");
        assert_eq!(first.1, second.1, "the same gravity run must fold the same digest");
    }

    #[test]
    fn a_jump_rises_peaks_and_lands_back_on_the_ground() {
        let mut frame = FrameState::new(world_with_floor(0, 0));
        tick(&mut frame, 0);
        assert!(frame.player_grounded(), "the first tick must establish the ground");

        frame.queue_input(InputEvent::press(InputButton::Jump));
        let mut peak = 0;
        let mut landed = false;
        for index in 1..200 {
            tick(&mut frame, index);
            let y = frame.player_position().expect("the player is bound")[2];
            assert!(y >= 0, "the jump must not fall through the floor: tick {index}");
            peak = peak.max(y);
            if frame.player_grounded() {
                landed = true;
                assert_eq!(y, 0, "the jump must land exactly on the ground");
                assert_eq!(frame.player_vertical_velocity(), 0);
                break;
            }
        }
        assert!(peak > 0, "the jump must rise");
        assert_eq!(peak, 45, "the documented synthetic arc peaks at 45 units");
        assert!(landed, "the jump must come back down");
    }

    #[test]
    fn a_jump_under_a_low_ceiling_stops_there_and_falls_back() {
        let report = ObjectReport {
            room_offset: 0,
            count: 1,
            array_offset: 0,
            objects: vec![record("raziel", [0, 0, 0])],
        };
        let mut world = World::from_report("test".to_string(), 1, &report);
        world.collision = floor_and_ceiling_mesh(0, 60);
        let mut frame = FrameState::new(world);
        tick(&mut frame, 0);
        assert!(frame.player_grounded());

        let clearance = 60 - crate::engine::collision::PLAYER_HEIGHT_UNITS;
        frame.queue_input(InputEvent::press(InputButton::Jump));
        let mut peak = 0;
        let mut landed = false;
        for index in 1..200 {
            tick(&mut frame, index);
            let y = frame.player_position().expect("the player is bound")[2];
            assert!(y <= clearance, "the head must stop at the ceiling: tick {index}");
            peak = peak.max(y);
            if frame.player_grounded() {
                landed = true;
                assert_eq!(y, 0);
                break;
            }
        }
        assert_eq!(peak, clearance, "the jump stops at the ceiling clearance");
        assert!(landed, "the player must fall back to the floor");
    }

    #[test]
    fn a_jump_while_airborne_does_nothing() {
        let mut frame = FrameState::new(world_with_floor(0, 50));
        tick(&mut frame, 0);
        assert!(!frame.player_grounded(), "the spawn is airborne");

        let before = frame.player_position().expect("the player is bound")[2];
        frame.queue_input(InputEvent::press(InputButton::Jump));
        tick(&mut frame, 1);
        assert!(
            frame.player_vertical_velocity() <= 0,
            "a mid-air press must not start a jump"
        );
        assert!(
            frame.player_position().expect("the player is bound")[2] <= before,
            "a mid-air press must not lift the player"
        );

        // Landing restores the ability: a fresh press after the floor is
        // reached does start a jump.
        for index in 2..200 {
            tick(&mut frame, index);
            if frame.player_grounded() {
                break;
            }
        }
        assert!(frame.player_grounded(), "the fall must land");
        frame.queue_input(InputEvent::release(InputButton::Jump));
        tick(&mut frame, 200);
        frame.queue_input(InputEvent::press(InputButton::Jump));
        tick(&mut frame, 201);
        assert!(
            frame.player_vertical_velocity() > 0,
            "landing must re-enable the jump"
        );
    }

    #[test]
    fn a_held_jump_button_does_not_jump_again() {
        let mut frame = FrameState::new(world_with_floor(0, 0));
        tick(&mut frame, 0);
        frame.queue_input(InputEvent::press(InputButton::Jump));

        let mut rises = 0;
        let mut was_rising = false;
        let mut landed_after_jump = false;
        for index in 1..300 {
            tick(&mut frame, index);
            let rising = frame.player_vertical_velocity() > 0;
            if rising && !was_rising {
                rises += 1;
            }
            was_rising = rising;
            if !rising && frame.player_grounded() {
                landed_after_jump = true;
            }
        }
        assert_eq!(rises, 1, "a held button is one edge and one jump");
        assert!(landed_after_jump, "the single jump must land");
    }

    #[test]
    fn jumping_into_a_wall_corner_does_not_tunnel() {
        let mut frame = FrameState::new(world_with_wall_and_floor());
        tick(&mut frame, 0);
        assert!(frame.player_grounded());
        frame.queue_input(InputEvent::press(InputButton::Right));
        frame.queue_input(InputEvent::press(InputButton::Forward));
        frame.queue_input(InputEvent::press(InputButton::Jump));

        let mut peak = 0;
        for index in 1..180 {
            tick(&mut frame, index);
            let position = frame.player_position().expect("the player is bound");
            assert!(position[0] < 100, "the player tunnelled the wall on tick {index}");
            assert!(position[2] >= 0, "the player fell through the floor on tick {index}");
            peak = peak.max(position[2]);
        }
        assert!(peak > 0, "the combined script must include a jump");
    }

    #[test]
    fn zero_input_never_moves_the_player_upward() {
        let mut frame = FrameState::new(world_with_floor(0, 50));
        let mut previous = frame.player_position().expect("the player is bound")[2];
        for index in 0..200 {
            tick(&mut frame, index);
            let y = frame.player_position().expect("the player is bound")[2];
            assert!(y <= previous, "tick {index} moved upward without a jump");
            assert!(frame.player_vertical_velocity() <= 0);
            previous = y;
        }
        assert!(frame.player_grounded(), "the no-input fall still lands");
    }

    #[test]
    fn the_same_jump_input_script_replays_the_same_digest() {
        fn run() -> (Vec<[i32; 3]>, u64) {
            let mut frame = FrameState::new(world_with_floor(0, 0));
            tick(&mut frame, 0);
            let mut positions = Vec::new();
            for index in 1..=140 {
                match index {
                    5 => frame.queue_input(InputEvent::press(InputButton::Jump)),
                    40 => frame.queue_input(InputEvent::release(InputButton::Jump)),
                    70 => frame.queue_input(InputEvent::press(InputButton::Jump)),
                    _ => {}
                }
                tick(&mut frame, index);
                positions.push(frame.player_position().expect("the player is bound"));
            }
            (positions, world_state_hash(&frame.world_state()))
        }

        let first = run();
        assert_eq!(first, run(), "the same jump script must replay byte for byte");
    }

    #[test]
    fn a_wall_stops_the_player_on_the_near_side() {
        let mut frame = FrameState::new(world_with_wall());
        frame.queue_input(InputEvent::press(InputButton::Right));
        frame.queue_input(InputEvent::press(InputButton::Forward));
        for index in 0..60 {
            tick(&mut frame, index);
        }
        let position = frame.player_position().expect("the player is bound");
        assert!(position[0] < 100, "the player crossed the wall: {position:?}");
        assert!(
            (80..=84).contains(&position[0]),
            "the player must stop at the radius: {position:?}"
        );
        assert!(
            position[1] > 200,
            "a diagonal push must slide along the wall: {position:?}"
        );
    }

    #[test]
    fn the_same_walled_input_script_replays_identically() {
        fn run() -> Vec<([i32; 3], u64)> {
            let mut frame = FrameState::new(world_with_wall());
            frame.queue_input(InputEvent::press(InputButton::Right));
            frame.queue_input(InputEvent::press(InputButton::Forward));
            let mut observed = Vec::new();
            for index in 0..60 {
                tick(&mut frame, index);
                observed.push((
                    frame.player_position().expect("the player is bound"),
                    world_state_hash(&frame.world_state()),
                ));
            }
            observed
        }

        let first = run();
        assert_eq!(first, run(), "the same input script must replay byte for byte");
        assert!(first.last().expect("60 ticks").0[0] < 100);
    }

    #[test]
    fn forward_input_moves_the_player_by_the_expected_integer_delta() {
        let mut frame = frame();
        let before = frame.player_position().expect("the player is bound");
        let before_hash = world_state_hash(&frame.world_state());

        frame.queue_input(InputEvent::press(InputButton::Forward));
        for index in 0..60 {
            tick(&mut frame, index);
        }

        let after = frame.player_position().expect("the player is bound");
        assert_eq!(after[0], before[0]);
        assert_eq!(after[1], before[1] + 60 * PLAYER_SPEED_UNITS_PER_TICK);
        assert_eq!(
            after[2], before[2],
            "walking horizontally must not change the vertical (Z) coordinate"
        );
        assert_ne!(
            world_state_hash(&frame.world_state()),
            before_hash,
            "moving the player must change the canonical state"
        );
    }

    fn assert_facing_x(rotation: crate::pose::Mat3, expected: [f32; 3]) {
        let local_x = [rotation[0][0], rotation[1][0], rotation[2][0]];
        for axis in 0..3 {
            assert!(
                (local_x[axis] - expected[axis]).abs() < 1e-5,
                "local +X maps to {local_x:?}, expected {expected:?}"
            );
        }
    }

    #[test]
    fn facing_follows_the_committed_movement_and_survives_standing_still() {
        let mut frame = frame();
        assert_eq!(frame.player_yaw(), None, "no facing before the first move");

        frame.queue_input(InputEvent::press(InputButton::Forward));
        tick(&mut frame, 0);
        let forward = frame.player_yaw().expect("forward movement sets the facing");
        assert_facing_x(forward, [0.0, 1.0, 0.0]);
        assert!(frame.player_yaw_binary().is_some());

        frame.queue_input(InputEvent::release(InputButton::Forward));
        tick(&mut frame, 1);
        assert_eq!(
            frame.player_yaw(),
            Some(forward),
            "standing still preserves the previous facing"
        );
        assert_eq!(frame.player_position(), Some([0, 4, 0]));

        frame.queue_input(InputEvent::press(InputButton::Right));
        tick(&mut frame, 2);
        let right = frame.player_yaw().expect("right movement sets the facing");
        assert_facing_x(right, [1.0, 0.0, 0.0]);
        assert_ne!(right, forward, "a different direction produces a different rotation");
    }

    #[test]
    fn player_moving_tracks_the_committed_horizontal_delta() {
        let mut frame = frame();
        tick(&mut frame, 0);
        assert!(!frame.player_moving(), "no input means no movement");

        frame.queue_input(InputEvent::press(InputButton::Forward));
        tick(&mut frame, 1);
        assert!(frame.player_moving(), "a committed delta reports movement");

        frame.queue_input(InputEvent::release(InputButton::Forward));
        tick(&mut frame, 2);
        assert!(
            !frame.player_moving(),
            "releasing the button clears the movement signal"
        );

        let mut blocked = FrameState::new(world_with_wall());
        assert!(
            blocked.place_player([100 - super::PLAYER_RADIUS_UNITS, 0, 0]),
            "the player is bound"
        );
        blocked.queue_input(InputEvent::press(InputButton::Right));
        tick(&mut blocked, 3);
        assert!(
            !blocked.player_moving(),
            "a fully blocked sweep commits no delta"
        );
    }

    #[test]
    fn tick_count_is_independent_of_the_event_burst_size() {
        fn run(burst: usize) -> u64 {
            let mut frame = frame();
            for _ in 0..burst {
                frame.queue_input(InputEvent::press(InputButton::Forward));
            }
            // Same wall-time window: one second, in ten 100 ms deliverables,
            // then drain the capped remainder.
            for _ in 0..10 {
                frame.frame(Duration::from_millis(100)).expect("frame");
            }
            loop {
                let executed = frame.frame(Duration::ZERO).expect("drain");
                if executed == 0 {
                    break;
                }
            }
            frame.stats().ticks
        }

        let single = run(1);
        let burst = run(100);
        assert_eq!(single, burst, "input must never drive simulation stepping");
        assert_eq!(single, 60, "one second at 60 Hz is sixty ticks");
    }

    #[test]
    fn stage_two_resets_the_scratch_block_every_tick() {
        let mut frame = frame();
        frame.set_view(CullView {
            eye: [10_000, 0, 0],
            radius: 10,
        });
        tick(&mut frame, 0);
        assert_eq!(frame.scratch().culled(), 1);
        assert_eq!(frame.scratch().activated(), 0);
        let player = frame.player().expect("the player is bound");
        assert!(!frame.live().get(player).expect("live player").visible);

        tick(&mut frame, 1);
        assert_eq!(
            frame.scratch().culled(),
            1,
            "stage 2 clears the accumulation; it must not grow to 2"
        );
    }

    #[test]
    fn stage_fifteen_snapshots_live_order_before_post_frame_spawns() {
        let mut frame = frame();
        frame.queue_spawn("monster_wallcr", [10, 0, 0]);
        tick(&mut frame, 0);

        assert_eq!(frame.live().len(), 2, "stage 18 applied the deferred spawn");
        assert_eq!(
            frame.scene().len(),
            1,
            "stage 15 ran before stage 18, so the snapshot has only the player"
        );
        assert_eq!(frame.scene()[0].id, frame.player().expect("player"));

        tick(&mut frame, 1);
        assert_eq!(frame.scene().len(), 2);
        let scene_ids: Vec<_> = frame.scene().iter().map(|item| item.id).collect();
        let live_ids: Vec<_> = frame.live().order().to_vec();
        assert_eq!(scene_ids, live_ids, "the render snapshot follows live order");
        assert_eq!(
            frame.scene()[1].class,
            crate::engine::class::RenderClass::Monster.code()
        );
    }

    #[test]
    fn stage_eighteen_recomputes_the_tracker_for_spawns_and_despawns() {
        let mut frame = frame();
        frame.queue_spawn("monster_wallcr", [10, 0, 0]);
        tick(&mut frame, 0);

        assert_eq!(frame.tracker().find("raziel"), Some(0));
        assert_eq!(frame.tracker().find("monster_wallcr"), Some(1));
        for slot in [0usize, 1] {
            assert_eq!(
                frame.tracker().slots()[slot].state(),
                crate::engine::tracker::SlotState::Ready
            );
        }
        let player = frame.player().expect("the player is bound");
        assert_eq!(frame.live().get(player).expect("player").slot, Some(0));

        frame.queue_despawn(player);
        tick(&mut frame, 1);
        assert_eq!(frame.player(), None, "the despawned player is unbound");
        assert_eq!(frame.live().len(), 1);
        assert_eq!(frame.tracker().find("raziel"), None, "the slot is released");
        assert_eq!(frame.tracker().find("monster_wallcr"), Some(1));
        assert_eq!(frame.stats().despawned, 1);
    }

    #[test]
    fn despawn_of_an_unknown_id_is_a_no_op_not_a_panic() {
        let mut frame = frame();
        let stale = crate::engine::live::LiveId {
            index: 99,
            generation: 0,
        };
        frame.queue_despawn(stale);
        tick(&mut frame, 0);
        assert_eq!(frame.stats().despawned, 0);
        assert_eq!(frame.live().len(), 1);
    }

    #[test]
    fn stage_names_come_from_the_ordered_definition() {
        let codes: Vec<u8> = STAGE_ORDER.iter().map(|stage| stage.code()).collect();
        assert_eq!(codes, (1..=18u8).collect::<Vec<_>>());
        assert_eq!(STAGE_ORDER[0], Stage::Clock);
        assert_eq!(STAGE_ORDER[17], Stage::PostFrameChanges);
        assert_eq!(STAGE_ORDER[17].name(), "post-frame-changes");
    }

    fn id_of(frame: &FrameState, name: &str) -> LiveId {
        frame
            .live()
            .iter()
            .find(|(_, object)| object.name == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no live object named '{name}'"))
    }

    #[test]
    fn queued_events_are_drained_once_at_the_delivery_stage() {
        let mut frame = frame();
        let player = frame.player().expect("the player is bound");
        frame.queue_event(EngineEvent::Message(Message {
            target: player,
            code: 1,
        }));
        frame.queue_event(EngineEvent::Contact(ContactRecord {
            source: player,
            target: player,
        }));
        assert_eq!(frame.pending_events(), 2);

        tick(&mut frame, 0);
        assert_eq!(frame.pending_events(), 0, "the delivery stage drained the queue");
        assert_eq!(frame.stats().events_delivered, 2);
        assert_eq!(
            frame.stats().event_hook_calls,
            2,
            "the Raziel row installs the event witness, so delivery reaches a hook"
        );

        tick(&mut frame, 1);
        assert_eq!(
            frame.stats().events_delivered,
            2,
            "a drained event is never delivered again"
        );
        assert_eq!(frame.stats().event_hook_calls, 2);
    }

    #[test]
    fn an_event_produced_in_one_tick_is_never_delivered_in_the_next() {
        let mut frame = frame();
        let player = frame.player().expect("the player is bound");
        frame.queue_event(EngineEvent::Hit(HitRecord {
            attacker: player,
            target: player,
            bone: 2,
            radius_mask: 0x0004,
        }));
        tick(&mut frame, 0);
        let delivered = frame.stats().events_delivered;
        assert_eq!(delivered, 1);

        for index in 1..=5 {
            tick(&mut frame, index);
            assert_eq!(
                frame.stats().events_delivered,
                delivered,
                "tick {index} must not see the previous tick's event"
            );
            assert_eq!(frame.pending_events(), 0);
        }
        assert_eq!(frame.stats().events_queued, 1);
    }

    #[test]
    fn events_reach_the_actor_carrier_without_changing_its_state() {
        fn run(with_events: bool) -> (crate::engine::actor::ActorRuntime, u64, u64) {
            let mut frame = FrameState::new(empty_world());
            frame.queue_spawn_loaded(
                "monster_wallcr",
                [0, 0, 0],
                Some(actor_profile(&[0x18, 0x25, 0x26])),
            );
            tick(&mut frame, 0);
            let actor = id_of(&frame, "monster_wallcr");

            if with_events {
                frame.queue_event(EngineEvent::Hit(HitRecord {
                    attacker: actor,
                    target: actor,
                    bone: 1,
                    radius_mask: 0x0001,
                }));
                frame.queue_event(EngineEvent::Message(Message {
                    target: actor,
                    code: 9,
                }));
            }

            for index in 1..=20 {
                tick(&mut frame, index);
            }
            let runtime = frame
                .live()
                .get(actor)
                .expect("the actor is live")
                .actor
                .clone()
                .expect("the actor carrier survives");
            (
                runtime,
                frame.stats().actor_events_passed,
                world_state_hash(&frame.world_state()),
            )
        }

        let (without, passed_without, digest_without) = run(false);
        let (with, passed_with, digest_with) = run(true);

        assert_eq!(
            without, with,
            "advance receives events but must not change relation, state, target or flags"
        );
        assert_eq!((without.relation(), without.current()), (0, 0));
        assert_eq!(passed_without, 0, "the no-event run passes nothing");
        assert_eq!(
            passed_with, 2,
            "both target-matched events reached advance in their tick"
        );
        assert_eq!(
            digest_with, digest_without,
            "events are transient and must not enter the canonical fold"
        );
    }

    #[test]
    fn an_event_run_replays_the_same_digest_and_stats() {
        fn run() -> (u64, FrameStats) {
            let mut frame = FrameState::new(empty_world());
            frame.queue_spawn_loaded(
                "monster_wallcr",
                [1, 0, 0],
                Some(actor_profile(&[0x18, 0x25])),
            );
            tick(&mut frame, 0);
            let actor = id_of(&frame, "monster_wallcr");
            frame.queue_event(EngineEvent::Contact(ContactRecord {
                source: actor,
                target: actor,
            }));
            for index in 1..=15 {
                tick(&mut frame, index);
            }
            (world_state_hash(&frame.world_state()), frame.stats())
        }

        let first = run();
        let second = run();
        assert_eq!(first, second, "the same event run must replay byte for byte");
        assert!(first.1.actor_events_passed > 0);
    }

    #[test]
    fn the_hit_and_trigger_passes_produce_no_event_without_an_evidenced_source() {
        let mut frame = frame();
        for index in 0..10 {
            tick(&mut frame, index);
            assert_eq!(
                frame.pending_events(),
                0,
                "no producing pass may invent a hit or a trigger"
            );
        }
        assert_eq!(frame.stats().events_queued, 0);
        assert_eq!(frame.stats().events_delivered, 0);
    }

    #[test]
    fn a_scripted_activation_fires_at_its_tick_and_logs_one_line() {
        use crate::engine::interaction::{ActivateTarget, InteractAt, InteractTrigger};
        let mut frame = FrameState::new(world_with_raziel());
        frame.queue_spawn("sudoor", [0, 0, 0]);
        frame.set_interaction_schedule(vec![InteractAt {
            tick: 3,
            target: ActivateTarget::Nearest,
        }]);

        let mut seen = Vec::new();
        for index in 0..6 {
            tick(&mut frame, index);
            seen.extend(frame.take_interaction_log());
        }
        assert_eq!(seen.len(), 1, "exactly one activation across the run");
        assert_eq!(seen[0].name, "sudoor");
        assert!(seen[0].open);
        assert_eq!(seen[0].trigger, InteractTrigger::Proximity);
        assert_eq!(
            seen[0].line(),
            "interact object=sudoor pos=0,0,0 state=open trigger=proximity"
        );
    }

    #[test]
    fn the_digest_changes_only_when_a_door_opens() {
        use crate::engine::interaction::{ActivateTarget, InteractAt};
        fn run(activate: bool, interactions: bool) -> (u64, u64) {
            let mut frame = FrameState::new(world_with_raziel());
            frame.set_interactions_enabled(interactions);
            frame.queue_spawn("sudoor", [0, 0, 0]);
            if activate {
                frame.set_interaction_schedule(vec![InteractAt {
                    tick: 2,
                    target: ActivateTarget::Nearest,
                }]);
            }
            for index in 0..5 {
                tick(&mut frame, index);
            }
            (frame.production_digest(), world_state_hash(&frame.world_state()))
        }

        let (plain, plain_world) = run(false, true);
        assert_eq!(
            plain, plain_world,
            "no activation must be byte-identical to the pre-interaction fold"
        );
        let (opened, _) = run(true, true);
        assert_ne!(opened, plain, "opening a door must move the digest");

        let (disabled, disabled_world) = run(true, false);
        assert_eq!(
            disabled, disabled_world,
            "--no-interactions must keep the plain world fold"
        );
        assert_eq!(
            disabled, plain,
            "--no-interactions reproduces the no-activation digest exactly"
        );
    }

    #[test]
    fn a_closed_door_blocks_the_player_and_an_open_one_lets_it_through() {
        use crate::engine::interaction::InteractTrigger;
        // A vertical wall at world x=100, y -500..500, z 0..300.
        let vertices = vec![
            [100, -500, 0],
            [100, 500, 0],
            [100, 500, 300],
            [100, -500, 300],
        ];
        let faces = vec![[0, 1, 2], [0, 2, 3]];
        let door = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert!(!door.is_empty(), "the fixture door must carry walls");

        let mut frame = FrameState::new(world_with_raziel());
        let mut base = std::collections::BTreeMap::new();
        base.insert(("sudoor".to_string(), [0, 0, 0]), door);
        frame.install_interaction_base(base);
        frame.queue_spawn("sudoor", [0, 0, 0]);
        tick(&mut frame, 0);

        frame.queue_input(InputEvent::press(InputButton::Right));
        for index in 1..40 {
            tick(&mut frame, index);
        }
        let blocked = frame.player_position().expect("the player is bound");
        assert!(
            blocked[0] < 100,
            "the closed door must stop the player: {blocked:?}"
        );

        let door = id_of(&frame, "sudoor");
        let outcome = frame
            .activate_live(door.index, Some(door.generation), InteractTrigger::Script)
            .expect("the door toggles open");
        assert!(outcome.open);

        for index in 40..160 {
            tick(&mut frame, index);
        }
        let passed = frame.player_position().expect("the player is bound");
        assert!(
            passed[0] > 100,
            "the open door must let the player through: {passed:?}"
        );
    }
}

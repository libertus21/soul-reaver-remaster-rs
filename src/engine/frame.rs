//! Ordered 18-stage frame pipeline.
//!
//! The original in-game frame coordinator runs a fixed list of subsystems in a
//! documented order (`docs/notes/actor-ai.md`, section 1). That order is
//! behaviour: reordering two stages can change the observable state of the same
//! inputs. [`STAGE_ORDER`] is therefore the single ordered definition, and
//! [`FrameState::run_tick`] is the only driver.
//!
//! Stages 1, 2, 6, 9, 12, 15 and 18 are implemented for this milestone. The
//! remaining stages are **named no-ops** with the documented name, never
//! `todo!()`: adding AI, animation, contact or streaming later is filling a body,
//! not editing the loop.
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
//! The player is updated twice per tick, exactly as documented: first by the
//! direct player path (stage 9) and again as an ordinary live object in the
//! generic per-object pass (stage 12). The double update is original behaviour
//! and must not be "fixed" here.

use std::fmt;
use std::time::Duration;

use crate::engine::actor::{ActorProfile, ActorRuntime};
use crate::engine::class::{CLASS_TABLE, Hooks, Query, RenderClass, classify};
use crate::engine::clock::{ObjectTick, SimClock};
use crate::engine::determinism::{WorldEntry, WorldState};
use crate::engine::events::{EngineEvent, EventQueue};
use crate::engine::input::{InputEvent, InputFrame, InputQueue};
use crate::engine::live::{LiveArena, LiveId, LiveObject};
use crate::engine::tracker::{ObjectTracker, TrackerError};
use crate::engine::world::World;

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
    /// Downward vertical velocity in fixed-point subunits per tick. Negative is
    /// falling; zero is grounded. Not part of the canonical fold.
    player_vertical_velocity: i64,
    /// Whether the player ended the last tick resting on a walkable floor.
    player_grounded: bool,
    view: CullView,
    stats: FrameStats,
    scratch: FrameScratch,
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
            view: CullView::default(),
            stats: FrameStats::default(),
            scratch: FrameScratch::default(),
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

    /// Whether the player ended the last tick resting on a walkable floor.
    ///
    /// This is a diagnostic, not part of the canonical digest. A world with no
    /// floor geometry always reports `false` because `Y` is not resolved there.
    pub fn player_grounded(&self) -> bool {
        self.player_grounded
    }

    /// The player's downward vertical velocity in fixed-point subunits per tick.
    /// Negative is falling; zero is grounded. Diagnostic only.
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
    fn stage_clock(&mut self, tick: ObjectTick) {
        self.tick_index = tick.index;
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
            let visible = within_radius(view.eye, position, view.radius);
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

    /// Stage 9 - player input to movement through the deterministic sweep and
    /// the gravity/ground resolver.
    ///
    /// The sampled input becomes an integer `X`/`Z` delta and is resolved
    /// against the world's [`CollisionMesh`](crate::engine::collision::CollisionMesh).
    /// A zero axis performs no horizontal movement and no collision query, so a
    /// no-input tick cannot change the position on its own. The sweep stops
    /// before walls and slides along them.
    ///
    /// Vertical motion is resolved after the horizontal sweep, at the resulting
    /// `X`/`Z`: gravity accumulates downward velocity, and the player lands
    /// exactly on the highest floor at or below the feet. A world with no floor
    /// triangles leaves `Y` untouched. This is a correct deterministic resolver,
    /// not the original four-step solver.
    fn stage_player_movement(&mut self) {
        let Some(player) = self.player else {
            return;
        };
        self.stats.direct_player_ticks = self.stats.direct_player_ticks.saturating_add(1);

        let [x, y] = self.current_input.move_axis();
        if x != 0 || y != 0 {
            let delta = [
                x.saturating_mul(PLAYER_SPEED_UNITS_PER_TICK),
                0,
                y.saturating_mul(PLAYER_SPEED_UNITS_PER_TICK),
            ];
            let collision = &self.world.collision;
            if let Ok(object) = self.live.get_mut(player) {
                object.position = collision.sweep(object.position, delta, PLAYER_RADIUS_UNITS);
            }
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
            object.position[1] = step.y;
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

    /// Stage 14 - moving-object world/contact pass. Documented no-op until the
    /// contact slice.
    fn stage_moving_contact(&mut self) {}

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
        CullView, FrameState, FrameStats, PLAYER_SPEED_UNITS_PER_TICK, STAGE_ORDER, Stage,
    };
    use crate::engine::actor::ActorProfile;
    use crate::engine::collision::CollisionMesh;
    use crate::engine::determinism::world_state_hash;
    use crate::engine::events::{ContactRecord, EngineEvent, HitRecord, Message};
    use crate::engine::input::{InputButton, InputEvent};
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

    /// The raziel world plus one vertical wall at `x = 100` spanning
    /// `z = -500..500`. The player starts at the origin, 100 units away.
    fn world_with_wall() -> World {
        let mut world = world_with_raziel();
        let vertices = vec![
            [100, -100, -500],
            [100, -100, 500],
            [100, 100, 500],
            [100, 100, -500],
        ];
        let faces = vec![[0u32, 1, 2], [0, 2, 3]];
        world.collision = CollisionMesh::from_room_triangles(&vertices, &faces);
        assert!(!world.collision.is_empty(), "the test wall must be a barrier");
        world
    }

    /// A collision mesh made of upward-facing floor quads. Each tuple is
    /// `(y, x0, x1, z0, z1)`.
    fn floor_mesh(quads: &[(i32, i32, i32, i32, i32)]) -> CollisionMesh {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for &(y, x0, x1, z0, z1) in quads {
            let base = vertices.len() as u32;
            vertices.extend([[x0, y, z0], [x0, y, z1], [x1, y, z1], [x1, y, z0]]);
            faces.push([base, base + 1, base + 2]);
            faces.push([base, base + 2, base + 3]);
        }
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    /// The raziel world plus one walkable floor at `floor_y`, with the player
    /// spawned at `player_y`.
    fn world_with_floor(floor_y: i32, player_y: i16) -> World {
        let report = ObjectReport {
            room_offset: 0,
            count: 1,
            array_offset: 0,
            objects: vec![record("raziel", [0, player_y, 0])],
        };
        let mut world = World::from_report("test".to_string(), 1, &report);
        world.collision = floor_mesh(&[(floor_y, -500, 500, -500, 500)]);
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
        let keyed = live
            .iter()
            .find(|object| object.position == [0, 0, 0])
            .expect("class-keyed spawn");
        let bare = live
            .iter()
            .find(|object| object.position == [1, 0, 0])
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
                position[1] >= 0,
                "the player must never pass through the floor: {position:?}"
            );
            if frame.player_grounded() {
                landed = true;
                break;
            }
        }
        assert!(landed, "gravity must land the player on the floor");
        assert_eq!(frame.player_position().expect("the player is bound")[1], 0);
        assert_eq!(frame.player_vertical_velocity(), 0);
    }

    #[test]
    fn a_grounded_player_rests_without_jitter_or_sinking_over_300_ticks() {
        let mut frame = FrameState::new(world_with_floor(0, 0));
        for index in 0..300 {
            tick(&mut frame, index);
            assert_eq!(
                frame.player_position().expect("the player is bound")[1],
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
            assert!(position[1] >= -100, "fell through the lower floor: {position:?}");
            if !frame.player_grounded() {
                fell = true;
            }
            if fell && frame.player_grounded() {
                landed_low = true;
                assert_eq!(position[1], -100, "must land on the lower floor");
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
            position[2] > 200,
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
        assert_eq!(after[2], before[2] + 60 * PLAYER_SPEED_UNITS_PER_TICK);
        assert_ne!(
            world_state_hash(&frame.world_state()),
            before_hash,
            "moving the player must change the canonical state"
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
}

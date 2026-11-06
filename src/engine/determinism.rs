//! Determinism harness: stable hashing for the simulation state.
//!
//! Determinism is the premise of this engine: the original advances on a fixed
//! 60 Hz rational tick over plain structs, ordered arenas and a fixed 48-slot
//! tracker. A regression still runs, so it is only caught by comparing state.
//! The policy lives in `docs/notes/determinism.md`; the gate is stated in
//! `docs/changes/engine-skeleton/tasks.md`.
//!
//! Four independent golden models live here:
//!
//! - `options_hash` / `error_hash` are **regression gates** over the `--play`
//!   options fold and the typed error text. They never include simulation
//!   state.
//! - [`world_state_hash`] is the **canonical state gate**: FNV-1a 64 over the
//!   tick count, live-list order, per-entry class, position and visibility.
//!   It is independent of the options fold.
//! - [`tracker_hash`] is the **tracker gate**: slot index, state and name in
//!   slot order. It pins acquisition, duplicate-name sharing and lowest-free
//!   reuse without entering the world-state digest.
//! - [`actor_runtime_hash`] is the **actor-runtime gate**: the data-keyed
//!   initial state derived from an `ActorProfile`, with no name in the fold.
//!
//! T1's `state_hash` mixed the options fold with the notion of state; that was
//! a tripwire, not a gate, so T2 retires it instead of re-pinning it.

use std::path::Path;

use crate::actor::{ActorKind, EntryArrayKind};
use crate::engine::actor::ActorRuntime;
use crate::engine::play::{PlayError, PlayOptions};
use crate::engine::tracker::ObjectTracker;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Stable FNV-1a 64 hasher.
///
/// `DefaultHasher` is explicitly unstable across Rust releases; FNV-1a is
/// dependency-free and byte-exact on every platform, which the digest contract
/// requires.
#[derive(Debug, Clone, Copy)]
pub struct StateHasher {
    state: u64,
}

impl StateHasher {
    pub const fn new() -> Self {
        Self {
            state: FNV_OFFSET_BASIS,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    /// Fold a variable-length segment behind its length so concatenation stays
    /// unambiguous: `"ab"+"c"` must not hash like `"a"+"bc"`.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        self.write(bytes);
    }

    pub fn write_u8(&mut self, value: u8) {
        self.write(&[value]);
    }

    pub fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write(&value.to_le_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    pub fn finish(&self) -> u64 {
        self.state
    }
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

fn write_path(hasher: &mut StateHasher, path: &Path) {
    hasher.write_bytes(path.as_os_str().as_encoded_bytes());
}

fn write_options(hasher: &mut StateHasher, options: &PlayOptions) {
    write_path(hasher, &options.bigfile);
    match &options.textures {
        Some(path) => {
            hasher.write_u8(1);
            write_path(hasher, path);
        }
        None => hasher.write_u8(0),
    }
    hasher.write_bytes(options.unit.as_bytes());
    match options.ticks {
        Some(ticks) => {
            hasher.write_u8(1);
            hasher.write_u32(ticks);
        }
        None => hasher.write_u8(0),
    }
    hasher.write_u8(u8::from(options.headless));
    hasher.write_u8(options.camera as u8);
}

fn error_tag(error: &PlayError) -> u8 {
    match error {
        PlayError::UnknownOption(_) => 0,
        PlayError::MissingValue(_) => 1,
        PlayError::MissingArchive => 2,
        PlayError::InvalidTicks(_) => 3,
        PlayError::ZeroTicksHeadless => 4,
        PlayError::InvalidCamera(_) => 5,
        PlayError::NotImplemented => 6,
    }
}

/// Hash the parsed options alone.
pub fn options_hash(options: &PlayOptions) -> u64 {
    let mut hasher = StateHasher::new();
    write_options(&mut hasher, options);
    hasher.finish()
}

/// Hash a typed error by variant and exact message.
pub fn error_hash(error: &PlayError) -> u64 {
    let mut hasher = StateHasher::new();
    hasher.write_u8(error_tag(error));
    hasher.write_bytes(error.to_string().as_bytes());
    hasher.finish()
}

/// Canonical fold shape for the simulation state.
///
/// T3+ fills `live` from the live arena. The contract is fixed now so the
/// determinism gate can land before any world code:
///
/// - `tick` is the number of simulation ticks executed.
/// - `live` is **live-list order**: spawn order, with removals preserving the
///   relative order of survivors. This order is canonical; it is not sorted and
///   never depends on allocation or hash order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorldState {
    pub tick: u64,
    pub live: Vec<WorldEntry>,
}

/// One live object in the canonical fold. `class` is the render class as `u8`,
/// `position` is integer world units and `visible` is the render flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEntry {
    pub class: u8,
    pub position: [i32; 3],
    pub visible: bool,
}

/// Hash the canonical world state: tick count, live-list order, then per entry
/// `class`, `position.x/y/z` and `visible`, all little-endian and fixed width.
///
/// Independent of [`options_hash`]: how the engine was launched is not state.
pub fn world_state_hash(state: &WorldState) -> u64 {
    let mut hasher = StateHasher::new();
    hasher.write_u64(state.tick);
    for entry in &state.live {
        hasher.write_u8(entry.class);
        hasher.write_i32(entry.position[0]);
        hasher.write_i32(entry.position[1]);
        hasher.write_i32(entry.position[2]);
        hasher.write_u8(u8::from(entry.visible));
    }
    hasher.finish()
}

/// Hash an actor's initial runtime state.
///
/// The fold is data-keyed: it includes the profile's kind, the eight state
/// trios, the entry-array kind and the entry ids, then the numeric runtime
/// fields (relation, state, target presence and id, flags). It is a separate,
/// independent gate; it does not enter [`world_state_hash`] and it does not
/// move the options, error, world-state, tracker or classification goldens.
pub fn actor_runtime_hash(runtime: &ActorRuntime) -> u64 {
    let mut hasher = StateHasher::new();
    let profile = runtime.profile();

    hasher.write_u8(actor_kind_code(profile.kind));
    for row in &profile.trios {
        for id in row {
            hasher.write_u8(*id);
        }
    }
    hasher.write_u8(entry_array_code(profile.entry_array));
    hasher.write_u32(profile.entries.len() as u32);
    for entry in &profile.entries {
        hasher.write_u8(entry.id);
    }

    hasher.write_u8(runtime.relation());
    hasher.write_u8(runtime.state());
    match runtime.target() {
        Some(id) => {
            hasher.write_u8(1);
            hasher.write_u32(id.index);
            hasher.write_u32(id.generation);
        }
        None => hasher.write_u8(0),
    }
    hasher.write_u32(runtime.flags());
    hasher.finish()
}

fn actor_kind_code(kind: ActorKind) -> u8 {
    match kind {
        ActorKind::Monster => 0,
        ActorKind::Player => 1,
    }
}

fn entry_array_code(kind: EntryArrayKind) -> u8 {
    match kind {
        EntryArrayKind::Clean => 0,
        EntryArrayKind::Variant => 1,
        EntryArrayKind::None => 2,
    }
}

/// Hash the tracker table: for every slot in index order, the slot index as
/// `u32`, the state code as `u8` (`0` empty, `1` loading, `2` ready) and the
/// name as length-prefixed bytes. Empty slots are folded too, so a release is
/// observable and the hash depends on *where* a name sits. Separate from
/// [`world_state_hash`]: tracker allocation is not world state.
pub fn tracker_hash(tracker: &ObjectTracker) -> u64 {
    let mut hasher = StateHasher::new();
    for (index, slot) in tracker.slots().iter().enumerate() {
        hasher.write_u32(index as u32);
        hasher.write_u8(slot.state().code());
        hasher.write_bytes(slot.name().unwrap_or("").as_bytes());
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{
        StateHasher, WorldEntry, WorldState, actor_runtime_hash, error_hash, options_hash,
        tracker_hash, world_state_hash,
    };
    use crate::actor::{ActorBlock, ActorKind, BehaviourEntry, EntryArrayKind};
    use crate::engine::actor::{ActorProfile, ActorRuntime};
    use crate::engine::class::RenderClass;
    use crate::engine::clock::{ObjectTick, SimClock};
    use crate::engine::frame::{FrameState, STAGE_ORDER, Stage};
    use crate::engine::input::{InputButton, InputEvent};
    use crate::engine::live::{LiveArena, LiveId, LiveObject};
    use crate::engine::play::{PlayError, PlayOptions};
    use crate::engine::tracker::ObjectTracker;
    use crate::engine::world::World;
    use crate::level::{ObjectRecord, ObjectReport};
    use std::time::Duration;

    fn parse(input: &[&str]) -> Result<PlayOptions, PlayError> {
        let argv: Vec<String> = input.iter().map(|token| token.to_string()).collect();
        PlayOptions::parse(&argv)
    }

    #[test]
    fn hasher_is_stable_and_length_prefixed() {
        let mut split = StateHasher::new();
        split.write_bytes(b"ab");
        split.write_bytes(b"c");

        let mut joined = StateHasher::new();
        joined.write_bytes(b"a");
        joined.write_bytes(b"bc");
        assert_ne!(split.finish(), joined.finish());

        let mut first = StateHasher::new();
        first.write(b"same");
        let mut second = StateHasher::new();
        second.write(b"same");
        assert_eq!(first.finish(), second.finish());
    }

    #[test]
    fn same_argv_parses_byte_identical_and_hashes_equal() {
        let input = [
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
        ];
        let first = parse(&input).expect("first parse");
        let second = parse(&input).expect("second parse");
        assert_eq!(first, second);
        assert_eq!(options_hash(&first), options_hash(&second));
    }

    #[test]
    fn same_invalid_argv_repeats_variant_message_and_hash() {
        let input = ["--bigfile", "B", "--camera", "top"];
        let first = parse(&input).expect_err("first parse must fail");
        let second = parse(&input).expect_err("second parse must fail");
        assert_eq!(first, second);
        assert_eq!(first.to_string(), second.to_string());
        assert_eq!(error_hash(&first), error_hash(&second));
    }

    #[test]
    fn different_argv_changes_the_hash() {
        let base = parse(&["--bigfile", "B", "--unit", "under1"]).expect("base parse");
        let other = parse(&["--bigfile", "B", "--unit", "under2"]).expect("other parse");
        assert_ne!(base, other);
        assert_ne!(options_hash(&base), options_hash(&other));
    }

    // Golden values pin the exact byte stream. The tests above compare the
    // harness against itself, so they pass even if the fold order, the field
    // set, the FNV constants or an error tag change. These constants do not.
    // A change here must be deliberate and reviewed.

    // Guards the `write_options` fold order and field set, the FNV-1a offset
    // basis and prime, the length prefix in `write_bytes`, and the little-endian
    // `to_le_bytes` of `--ticks 120`.
    const GOLDEN_OPTIONS_HASH: u64 = 0x27e6_be15_9907_b55c;

    // Guards the error variant tag (`InvalidCamera` = 5), the exact message
    // text and the same FNV constants.
    const GOLDEN_ERROR_HASH: u64 = 0xf7df_8b2c_3041_7acf;

    #[test]
    fn options_hash_matches_the_golden_value() {
        let options = parse(&[
            "--bigfile",
            "BIGFILE.DAT",
            "--unit",
            "under1",
            "--ticks",
            "120",
        ])
        .expect("golden argv must parse");
        assert_eq!(
            options_hash(&options),
            GOLDEN_OPTIONS_HASH,
            "options hash moved: fold order, field set, FNV constants or endianness changed"
        );
    }

    #[test]
    fn error_hash_matches_the_golden_value() {
        let error = parse(&["--bigfile", "B", "--camera", "top"])
            .expect_err("golden argv must be an error");
        assert_eq!(
            error_hash(&error),
            GOLDEN_ERROR_HASH,
            "error hash moved: variant tag, message text or FNV constants changed"
        );
    }

    // Guards the canonical world-state fold: the tick count, the live-list
    // order, and per entry the class, position x/y/z and visible byte, plus the
    // FNV-1a constants and little-endian encoding. It is independent of the
    // options fold above.
    //
    // T5 adds the live arena and the class hooks but does NOT write
    // `WorldState.live`, so this fold and this constant stay unchanged. The
    // live-order replay below is a separate observation, not part of the
    // canonical world golden.
    //
    // T3+ MUST extend the fold and this constant deliberately, in the same PR
    // that updates `docs/changes/engine-skeleton/spec.md`; it must never be
    // "fixed" to make a test go green.
    const GOLDEN_WORLD_STATE_HASH: u64 = 0x82fe_8a38_1b9d_2a6d;

    /// Synthetic, game-free run: advance a fixed clock over `advances` and fold
    /// the resulting world. One entry moves one unit on x per tick so the hash
    /// depends on the tick count, not only on the seed.
    fn synthetic_run(advances: &[Duration]) -> WorldState {
        let mut clock = SimClock::new();
        let mut state = WorldState {
            tick: 0,
            live: vec![
                WorldEntry {
                    class: 1,
                    position: [0, 0, 0],
                    visible: true,
                },
                WorldEntry {
                    class: 2,
                    position: [10, 0, 0],
                    visible: false,
                },
            ],
        };
        for delta in advances {
            for _tick in clock.advance(*delta).ticks() {
                state.tick += 1;
                state.live[0].position[0] += 1;
            }
        }
        state
    }

    #[test]
    fn n_ticks_twice_produce_identical_world_hash() {
        let advances = [Duration::from_millis(16); 120];
        let first = synthetic_run(&advances);
        let second = synthetic_run(&advances);
        assert_eq!(first, second, "same tick sequence must fold identically");
        assert_eq!(world_state_hash(&first), world_state_hash(&second));
        assert_eq!(
            world_state_hash(&first),
            GOLDEN_WORLD_STATE_HASH,
            "world state hash moved: fold order, field set, live order, FNV or endianness changed"
        );
    }

    #[test]
    fn world_hash_is_value_and_order_sensitive() {
        let base = synthetic_run(&[Duration::from_secs(1)]);

        let mut moved = base.clone();
        moved.live[0].position[0] += 1;
        assert_ne!(
            world_state_hash(&base),
            world_state_hash(&moved),
            "a different position must change the hash"
        );

        let mut reordered = base.clone();
        reordered.live.reverse();
        assert_ne!(
            world_state_hash(&base),
            world_state_hash(&reordered),
            "a different live order must change the hash"
        );

        let mut hidden = base.clone();
        hidden.live[0].visible = false;
        assert_ne!(world_state_hash(&base), world_state_hash(&hidden));
    }

    // Guards the tracker fold: for each of the 48 slots in index order, the
    // slot index, the state code and the length-prefixed name, plus the FNV-1a
    // constants. It is independent of the options and world-state goldens.
    const GOLDEN_TRACKER_HASH: u64 = 0x4fb9_0f56_bbe9_c0f9;

    /// Final table: slot 0 `raziel`, slot 1 `paths` (released slot reused) and
    /// slot 2 `particle`, all ready; the rest empty.
    fn tracker_run() -> ObjectTracker {
        let mut tracker = ObjectTracker::new();
        tracker.acquire("raziel").expect("slot 0");
        tracker.complete(0).expect("raziel ready");
        tracker.acquire("soul").expect("slot 1");
        tracker.acquire("raziel").expect("raziel shares slot 0");
        tracker.acquire("particle").expect("slot 2");
        tracker.complete(2).expect("particle ready");
        tracker.release(1).expect("free slot 1");
        tracker.acquire("paths").expect("reuse slot 1");
        tracker.complete(1).expect("paths ready");
        tracker
    }

    #[test]
    fn tracker_sequence_replays_to_the_same_golden_hash() {
        let first = tracker_run();
        let second = tracker_run();
        assert_eq!(
            first, second,
            "same call sequence must build the same table"
        );
        assert_eq!(tracker_hash(&first), tracker_hash(&second));
        assert_eq!(
            tracker_hash(&first),
            GOLDEN_TRACKER_HASH,
            "tracker hash moved: slot order, state code, name bytes, FNV or endianness changed"
        );
    }

    fn reused(free: usize) -> ObjectTracker {
        let mut tracker = ObjectTracker::new();
        for name in ["a", "b", "c"] {
            tracker.acquire(name).expect("acquire");
        }
        tracker.release(free).expect("free");
        tracker.acquire("d").expect("reuse");
        tracker
    }

    #[test]
    fn tracker_hash_is_assignment_and_state_sensitive() {
        let middle = reused(1);
        let first = reused(0);
        assert_eq!((middle.find("d"), first.find("d")), (Some(1), Some(0)));
        assert_ne!(
            tracker_hash(&middle),
            tracker_hash(&first),
            "a different slot assignment must change the hash"
        );

        let mut loading = ObjectTracker::new();
        loading.acquire("a").expect("acquire");
        let mut ready = loading.clone();
        ready.complete(0).expect("complete");
        assert_ne!(
            tracker_hash(&loading),
            tracker_hash(&ready),
            "the state code must be part of the fold"
        );
    }

    // T5 makes the live order observable, but the arena is not wired into
    // `WorldState.live` yet, so this replay is an independent observation and
    // does not touch `GOLDEN_WORLD_STATE_HASH`.
    fn live_object(name: &str) -> LiveObject {
        LiveObject {
            name: name.to_string(),
            class: RenderClass::Unclassified,
            position: [0, 0, 0],
            visible: true,
            slot: None,
            anim_index: 0,
        }
    }

    /// Little-endian bytes for every live id in order: slot index then
    /// generation. The order is the arena's spawn order, never the slot order.
    fn live_order_bytes(arena: &LiveArena) -> Vec<u8> {
        let mut bytes = Vec::new();
        for id in arena.order() {
            bytes.extend_from_slice(&id.index.to_le_bytes());
            bytes.extend_from_slice(&id.generation.to_le_bytes());
        }
        bytes
    }

    fn replay_live() -> LiveArena {
        let mut arena = LiveArena::new();
        let a = arena.spawn(live_object("raziel__"));
        let b = arena.spawn(live_object("monster_wallcr"));
        arena.spawn(live_object("particle"));
        arena.despawn(b).expect("remove the middle object");
        arena.spawn(live_object("sreavr__"));
        arena.despawn(a).expect("remove the first object");
        arena
    }

    #[test]
    fn live_order_replays_byte_identical() {
        let first = replay_live();
        let second = replay_live();
        assert_eq!(
            live_order_bytes(&first),
            live_order_bytes(&second),
            "the same spawn/remove sequence must fold identically"
        );

        let observed: Vec<(u32, u32)> = first
            .order()
            .iter()
            .map(|id| (id.index, id.generation))
            .collect();
        assert_eq!(
            observed,
            vec![(2, 0), (1, 1)],
            "removals keep spawn order and reuse bumps the reused slot generation"
        );
    }

    // Guards the initial actor-runtime fold: profile kind, the eight trios, the
    // entry-array kind, the entry ids, then relation, state, target and flags.
    // It is a sixth, independent gate; it does not move the options, error,
    // world-state, tracker or classification goldens.
    const GOLDEN_ACTOR_RUNTIME_HASH: u64 = 0xe169_695d_0eda_ec99;

    fn synthetic_profile(ids: &[u8]) -> ActorProfile {
        ActorProfile::from_block(&ActorBlock {
            kind: ActorKind::Monster,
            offset: 0x100,
            length: 0x300,
            trios: [
                [13, 31, 31],
                [25, 16, 12],
                [9, 5, 11],
                [7, 13, 27],
                [19, 2, 11],
                [20, 3, 2],
                [9, 0, 55],
                [19, 8, 1],
            ],
            entries: ids.iter().map(|&id| BehaviourEntry { id }).collect(),
            entry_array: EntryArrayKind::Clean,
        })
    }

    fn synthetic_runtime(ids: &[u8]) -> ActorRuntime {
        ActorRuntime::initial(synthetic_profile(ids))
    }

    #[test]
    fn actor_runtime_replay_matches_the_golden() {
        let first = synthetic_runtime(&[0x18, 0x25, 0x26, 0x27, 0x28]);
        let second = synthetic_runtime(&[0x18, 0x25, 0x26, 0x27, 0x28]);
        assert_eq!(first, second, "equal profiles must start identically");

        let observed = actor_runtime_hash(&first);
        assert_eq!(
            observed,
            GOLDEN_ACTOR_RUNTIME_HASH,
            "actor runtime hash moved: profile fold order, runtime init values, FNV or endianness changed (observed {observed:#018x})"
        );
    }

    #[test]
    fn actor_runtime_hash_is_value_and_profile_sensitive() {
        let base = synthetic_runtime(&[0x18, 0x25]);
        let baseline = actor_runtime_hash(&base);

        let mut state = base.clone();
        state.set_state(1);
        assert_ne!(baseline, actor_runtime_hash(&state), "state must be folded");

        let mut relation = base.clone();
        relation.set_relation(1);
        assert_ne!(
            baseline,
            actor_runtime_hash(&relation),
            "relation must be folded"
        );

        let mut flags = base.clone();
        flags.set_flags(1);
        assert_ne!(baseline, actor_runtime_hash(&flags), "flags must be folded");

        let mut target = base.clone();
        target.set_target(Some(LiveId {
            index: 2,
            generation: 1,
        }));
        assert_ne!(
            baseline,
            actor_runtime_hash(&target),
            "the target must be folded"
        );

        let other_profile = synthetic_runtime(&[0x25, 0x26]);
        assert_ne!(
            baseline,
            actor_runtime_hash(&other_profile),
            "the profile entries must be folded"
        );
    }

    // T8 frame pipeline gate. The stage order is behaviour, not documentation:
    // this golden table pins the exact 18 stage numbers and names so swapping
    // two stages fails the gate instead of silently changing the simulation.
    const GOLDEN_FRAME_STAGES: [(u8, &str); 18] = [
        (1, "clock"),
        (2, "scratch-reset"),
        (3, "script-units"),
        (4, "audio-emitters"),
        (5, "time-of-day"),
        (6, "activation-cull"),
        (7, "animation-timers"),
        (8, "player-solver"),
        (9, "player-movement"),
        (10, "pairwise-contact"),
        (11, "hit-list"),
        (12, "per-object-tick"),
        (13, "trigger-query"),
        (14, "moving-contact"),
        (15, "render-submission"),
        (16, "streaming"),
        (17, "particles"),
        (18, "post-frame-changes"),
    ];

    #[test]
    fn frame_stage_order_matches_the_golden_table() {
        assert_eq!(STAGE_ORDER.len(), GOLDEN_FRAME_STAGES.len());
        for (index, stage) in STAGE_ORDER.iter().enumerate() {
            let (code, name) = GOLDEN_FRAME_STAGES[index];
            assert_eq!(
                stage.code(),
                code,
                "stage {} code moved; observed {} ({})",
                index + 1,
                stage.code(),
                stage.name()
            );
            assert_eq!(
                stage.name(),
                name,
                "stage {} name moved; observed {}",
                index + 1,
                stage.name()
            );
        }
    }

    // Final canonical state of the 40-tick scripted replay below. This is a
    // seventh, independent gate: it pins the per-tick stage order, input
    // sampling, direct player translation, culling, render snapshot and
    // post-frame bookkeeping together. It does not move any existing golden.
    const GOLDEN_FRAME_STATE_HASH: u64 = 0x7b7f_16bc_a6af_b9b2;

    fn frame_record(name: &str, position: [i16; 3]) -> ObjectRecord {
        ObjectRecord {
            name: name.to_string(),
            position,
            object_type: 4,
            archetype: 7,
            instance_id: 1,
            flags: 0,
        }
    }

    fn frame_world() -> World {
        let report = ObjectReport {
            room_offset: 0,
            count: 1,
            array_offset: 0,
            objects: vec![frame_record("raziel", [0, 0, 0])],
        };
        World::from_report("test".to_string(), 1, &report)
    }

    /// Same input script and tick sequence, replayed per tick: stage trace, the
    /// canonical world hash and the player position after each tick.
    fn scripted_frame_run() -> Vec<(Vec<Stage>, u64, [i32; 3])> {
        let mut frame = FrameState::new(frame_world());
        let mut observed = Vec::new();
        for index in 0..40u64 {
            match index {
                0 => frame.queue_input(InputEvent::press(InputButton::Forward)),
                10 => frame.queue_input(InputEvent::press(InputButton::Right)),
                20 => {
                    frame.queue_input(InputEvent::release(InputButton::Forward));
                    frame.queue_input(InputEvent::release(InputButton::Right));
                }
                30 => frame.queue_input(InputEvent::press(InputButton::Back)),
                _ => {}
            }
            frame
                .run_tick(ObjectTick::step(index))
                .expect("the scripted frame must tick");
            observed.push((
                frame.stage_trace().to_vec(),
                world_state_hash(&frame.world_state()),
                frame.player_position().expect("the player is bound"),
            ));
        }
        observed
    }

    #[test]
    fn frame_pipeline_replays_stage_order_and_state_identically() {
        let first = scripted_frame_run();
        let second = scripted_frame_run();
        assert_eq!(
            first, second,
            "the same input script and tick sequence must replay byte for byte"
        );
        assert_eq!(first.len(), 40);

        let golden_stages = STAGE_ORDER.to_vec();
        for (trace, _, _) in &first {
            assert_eq!(
                trace, &golden_stages,
                "every tick must invoke the 18 stages in the golden order"
            );
        }

        assert_eq!(first[0].2, [0, 0, 4], "forward moves +Z directly");
        assert_eq!(first[19].2, [40, 0, 80], "right moves +X while forward holds");
        assert_eq!(
            first[20].2, first[19].2,
            "the release is sampled at the tick boundary and stops movement"
        );
        assert_eq!(first[39].2, [40, 0, 40], "back subtracts from the same axis");
        assert_ne!(first[0].1, first[39].1, "the state stream must move");

        let last = first[39].1;
        assert_eq!(
            last, GOLDEN_FRAME_STATE_HASH,
            "frame state hash moved: stage order, input sampling, translation, culling or bookkeeping changed (observed {last:#018x})"
        );
    }
}

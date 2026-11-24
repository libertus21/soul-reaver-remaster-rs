//! SYNTHETIC non-player locomotion for the engine layer.
//!
//! # This is not original behaviour
//!
//! The decoded data gave **no NPC pathing**: `campath` splines are not NPC
//! routes (`docs/notes/campath-route-test.md`), and the original actor AI was
//! never decoded. The original movement of a `monster_` actor is **UNKNOWN**.
//! This module is our own gameplay layer, added so the labelled `walk` clip has
//! a real position delta to react to. Nothing here reproduces retail behaviour
//! and nothing here is derived from a decoded route.
//!
//! # What it does
//!
//! Every eligible actor (a live non-player [`RenderClass::Monster`]) owns a
//! [`SyntheticPatrol`]:
//!
//! - an integer heading into a fixed table of eight directions
//!   ([`SYNTHETIC_DIRECTIONS`]) with a per-tick step of about
//!   [`SYNTHETIC_STEP_UNITS_PER_TICK`] world units, the same scale as the
//!   player's direct translation; there is no float in the position update;
//! - a deterministic duty cycle: walk for [`SYNTHETIC_WALK_TICKS`], stand still
//!   for [`SYNTHETIC_DWELL_TICKS`]. The phase is offset by the live id, so two
//!   actors do not march in lockstep and both the `walk` and the `idle` clip
//!   become observable;
//! - a bounce: each walking tick attempts the step through the existing
//!   [`CollisionMesh`] narrow phase (commit only when
//!   [`CollisionMesh::blocked`] is `false`). On contact the heading turns 90
//!   degrees (two entries in the eight-way table) and the actor tries again on
//!   the next tick, so it never commits a step into a wall;
//! - a room fence: a step whose body-radius footprint leaves
//!   [`Bounds`](crate::engine::collision::Bounds) is treated exactly like a
//!   wall contact.
//!
//! # Determinism
//!
//! The patrol state is initialised from `(live id, authored start position)` and
//! updated by a pure integer step. The collision mesh and its bounds are fixed
//! world data. The whole run is therefore a deterministic fold of
//! `(tick, live id, start position, actor class, mesh)`: the same inputs produce
//! the same byte stream on every platform. No hash-ordered container, float or
//! wall clock enters the path.
//!
//! # Eligibility
//!
//! Only [`RenderClass::Monster`] is moved. The player (`raziel`), the cutscene
//! model (`sreavr`), glyph icons, static physical props, particles and
//! unclassified placements are left untouched: the engine already distinguishes
//! those classes, and moving a marker or a prop would be inventing motion for
//! objects that are not actors.

use crate::engine::axis::{HORIZONTAL_A, HORIZONTAL_B};
use crate::engine::class::RenderClass;
use crate::engine::collision::CollisionMesh;
use crate::engine::live::{LiveId, LiveObject};

/// Horizontal step length of one patrol tick, in world units.
///
/// This matches the player's direct translation scale
/// (`frame::PLAYER_SPEED_UNITS_PER_TICK`), so actors and the player move at a
/// comparable speed. The value is ours, not retail data.
pub const SYNTHETIC_STEP_UNITS_PER_TICK: i32 = 4;

/// The eight integer headings, in `[A, B]` horizontal order.
///
/// Cardinal and diagonal steps are all integers: diagonals use `3` per axis so a
/// diagonal step is not longer than the player's cardinal step by more than a
/// rounding unit. There is no `f32`/`f64` in the update.
pub const SYNTHETIC_DIRECTIONS: [[i32; 2]; 8] = [
    [4, 0],
    [3, 3],
    [0, 4],
    [-3, 3],
    [-4, 0],
    [-3, -3],
    [0, -4],
    [3, -3],
];

/// Collision radius of a synthetic patrol, in world units.
///
/// The same stand-off as the player keeps the patrol clear of walls without
/// re-deriving a second scale.
pub const SYNTHETIC_ACTOR_RADIUS_UNITS: i32 = 16;

/// Walking ticks at the start of one duty cycle.
pub const SYNTHETIC_WALK_TICKS: u64 = 72;

/// Standing ticks at the end of one duty cycle.
pub const SYNTHETIC_DWELL_TICKS: u64 = 24;

/// Length of one walk-plus-dwell cycle, in ticks.
pub const SYNTHETIC_CYCLE_TICKS: u64 = SYNTHETIC_WALK_TICKS + SYNTHETIC_DWELL_TICKS;

/// One actor's deterministic bounce patrol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntheticPatrol {
    heading: usize,
    phase: u64,
}

impl SyntheticPatrol {
    /// Seed the patrol from the actor's identity and authored start position.
    ///
    /// The heading and the dwell phase are integer functions of
    /// `(live id, start position)`, so two runs with the same spawn produce the
    /// same first step.
    pub fn initial(id: LiveId, start: [i32; 3]) -> Self {
        let mixed = i64::from(id.index) * 7
            + i64::from(id.generation) * 3
            + i64::from(start[HORIZONTAL_A])
            + i64::from(start[HORIZONTAL_B]);
        let phase = u64::from(id.index) * 37
            + u64::from(id.generation) * 11
            + u64::from(start[HORIZONTAL_A].unsigned_abs());
        Self {
            heading: mixed.rem_euclid(SYNTHETIC_DIRECTIONS.len() as i64) as usize,
            phase: phase % SYNTHETIC_CYCLE_TICKS,
        }
    }

    /// Current heading index into [`SYNTHETIC_DIRECTIONS`].
    pub fn heading(&self) -> usize {
        self.heading
    }

    /// True when this tick belongs to the walking half of the duty cycle.
    pub fn walking(&self, tick: u64) -> bool {
        tick.wrapping_add(self.phase) % SYNTHETIC_CYCLE_TICKS < SYNTHETIC_WALK_TICKS
    }

    /// Attempt one tick and return the committed position, or `None` when the
    /// actor dwells or the step is blocked by a wall or the room fence.
    ///
    /// A blocked step turns the heading by 90 degrees (two of eight entries) and
    /// commits nothing; the next tick tries the new heading. The actor's height
    /// is unchanged: the patrol is horizontal only.
    pub fn step(
        &mut self,
        tick: u64,
        collision: &CollisionMesh,
        from: [i32; 3],
    ) -> Option<[i32; 3]> {
        if !self.walking(tick) {
            return None;
        }
        let [step_a, step_b] = SYNTHETIC_DIRECTIONS[self.heading];
        let mut delta = [0i32; 3];
        delta[HORIZONTAL_A] = step_a;
        delta[HORIZONTAL_B] = step_b;
        let mut candidate = from;
        candidate[HORIZONTAL_A] = from[HORIZONTAL_A].saturating_add(step_a);
        candidate[HORIZONTAL_B] = from[HORIZONTAL_B].saturating_add(step_b);

        let leaves_room = collision.bounds().is_some_and(|bounds| {
            !bounds.contains_with_radius(candidate, SYNTHETIC_ACTOR_RADIUS_UNITS)
        });
        if leaves_room || collision.blocked(from, delta, SYNTHETIC_ACTOR_RADIUS_UNITS) {
            self.heading = (self.heading + 2) % SYNTHETIC_DIRECTIONS.len();
            return None;
        }
        Some(candidate)
    }
}

/// True when `object` is a live actor this synthetic layer may move.
///
/// The player is excluded explicitly; only the engine's `monster_` render class
/// is included. Marker, inert and static classes never reach this predicate
/// because they are never spawned as live objects, but the class test keeps the
/// rule local and explicit.
pub fn is_moving_actor(object: &LiveObject, id: LiveId, player: Option<LiveId>) -> bool {
    Some(id) != player && object.class == RenderClass::Monster
}

/// One bounded per-actor motion diagnostic line.
///
/// `start` and `end` are the actor's positions before the first movement tick
/// and at report time. The field order is a diagnostic contract only; it never
/// enters the acceptance line or the canonical digest.
pub fn motion_line(name: &str, start: [i32; 3], end: [i32; 3]) -> String {
    format!(
        "npc-motion name={name} start={},{},{} end={},{},{} dx={} dy={} dz={}",
        start[0],
        start[1],
        start[2],
        end[0],
        end[1],
        end[2],
        end[0] - start[0],
        end[1] - start[1],
        end[2] - start[2],
    )
}

#[cfg(test)]
mod tests {
    use super::{
        SYNTHETIC_ACTOR_RADIUS_UNITS, SYNTHETIC_CYCLE_TICKS, SYNTHETIC_DIRECTIONS,
        SYNTHETIC_DWELL_TICKS, SYNTHETIC_STEP_UNITS_PER_TICK, SYNTHETIC_WALK_TICKS,
        SyntheticPatrol, is_moving_actor, motion_line,
    };
    use crate::engine::class::RenderClass;
    use crate::engine::collision::CollisionMesh;
    use crate::engine::live::{LiveId, LiveObject};
    use crate::sra::{ClipMap, ClipSelection, ClipSelector, MotionState};

    fn id(index: u32) -> LiveId {
        LiveId {
            index,
            generation: 0,
        }
    }

    fn wall_mesh() -> CollisionMesh {
        let vertices = vec![
            [100, -500, -100],
            [100, 500, -100],
            [100, 500, 400],
            [100, -500, 400],
        ];
        let faces = vec![[0u32, 1, 2], [0, 2, 3]];
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    fn fenced_mesh() -> CollisionMesh {
        let vertices = vec![
            [-400, -400, 0],
            [400, -400, 0],
            [400, 400, 0],
            [-400, 400, 0],
        ];
        let faces = vec![[0u32, 1, 2], [0, 2, 3]];
        CollisionMesh::from_room_triangles(&vertices, &faces)
    }

    #[test]
    fn the_direction_table_is_integer_and_matches_the_player_scale() {
        assert_eq!(SYNTHETIC_DIRECTIONS.len(), 8);
        for direction in SYNTHETIC_DIRECTIONS {
            assert_ne!(direction, [0, 0]);
            for axis in direction {
                assert!(axis.abs() <= SYNTHETIC_STEP_UNITS_PER_TICK);
            }
        }
    }

    #[test]
    fn the_duty_cycle_walks_then_dwells_and_offsets_by_id() {
        assert_eq!(
            SYNTHETIC_CYCLE_TICKS,
            SYNTHETIC_WALK_TICKS + SYNTHETIC_DWELL_TICKS
        );
        let a = SyntheticPatrol::initial(id(0), [0, 0, 0]);
        let b = SyntheticPatrol::initial(id(1), [0, 0, 0]);
        let walking_a: Vec<bool> = (0..SYNTHETIC_CYCLE_TICKS).map(|t| a.walking(t)).collect();
        let walking_b: Vec<bool> = (0..SYNTHETIC_CYCLE_TICKS).map(|t| b.walking(t)).collect();
        assert_eq!(
            walking_a.iter().filter(|walking| **walking).count() as u64,
            SYNTHETIC_WALK_TICKS
        );
        assert_ne!(
            walking_a, walking_b,
            "different ids must not march in lockstep"
        );
    }

    #[test]
    fn a_patrol_moves_during_walk_ticks_and_rests_during_dwell_ticks() {
        let collision = CollisionMesh::default();
        let mut patrol = SyntheticPatrol::initial(id(3), [0, 0, 0]);
        let mut position = [0, 0, 0];
        let start = position;
        let mut walked = 0;
        let mut dwelled = 0;
        for tick in 0..SYNTHETIC_CYCLE_TICKS {
            if let Some(next) = patrol.step(tick, &collision, position) {
                walked += 1;
                position = next;
            } else {
                dwelled += 1;
            }
        }
        assert_eq!(walked, SYNTHETIC_WALK_TICKS);
        assert_eq!(dwelled, SYNTHETIC_DWELL_TICKS);
        assert_ne!(position, start, "the walk half must produce a real delta");
    }

    #[test]
    fn the_walk_duty_cycle_drives_the_animation_state_to_moving_and_idle() {
        let collision = CollisionMesh::default();
        let mut patrol = SyntheticPatrol::initial(id(5), [0, 0, 0]);
        let mut selector = ClipSelector::new(
            ClipSelection {
                fixed: None,
                map: ClipMap {
                    idle: 0,
                    walk: 1,
                    jump_up: 2,
                    fall: 3,
                    land: 4,
                },
            },
            0,
        );
        let mut position = [0, 0, 0];
        let mut saw_moving = false;
        let mut saw_idle = false;
        for tick in 0..SYNTHETIC_CYCLE_TICKS * 2 {
            let next = patrol.step(tick, &collision, position);
            let moved = next.is_some();
            if let Some(next) = next {
                position = next;
            }
            selector.observe_signals(tick, true, moved, 0);
            match selector.state() {
                Some(MotionState::Moving) => saw_moving = true,
                Some(MotionState::Idle) => saw_idle = true,
                _ => {}
            }
        }
        assert!(saw_moving, "the walk half must select the moving state");
        assert!(saw_idle, "the dwell half must select the idle state");
    }

    #[test]
    fn the_same_start_reproduces_the_same_path() {
        fn run() -> Vec<[i32; 3]> {
            let collision = wall_mesh();
            let mut patrol = SyntheticPatrol::initial(id(9), [0, 0, 0]);
            let mut position = [0, 0, 0];
            let mut path = Vec::new();
            for tick in 0..600 {
                if let Some(next) = patrol.step(tick, &collision, position) {
                    position = next;
                }
                path.push(position);
            }
            path
        }
        assert_eq!(run(), run(), "the patrol must replay byte for byte");
    }

    #[test]
    fn a_wall_turns_the_patrol_and_it_never_crosses_it() {
        let collision = wall_mesh();
        let mut patrol = SyntheticPatrol::initial(id(2), [0, 0, 0]);
        let mut position = [0, 0, 0];
        let safe = 100 - SYNTHETIC_ACTOR_RADIUS_UNITS;
        for tick in 0..600 {
            if let Some(next) = patrol.step(tick, &collision, position) {
                position = next;
            }
            assert!(
                position[0] <= safe,
                "the patrol crossed the wall's radius at tick {tick}: {position:?}"
            );
        }
    }

    #[test]
    fn the_room_fence_is_treated_like_a_wall() {
        let collision = fenced_mesh();
        let mut patrol = SyntheticPatrol::initial(id(4), [0, 0, 0]);
        let mut position = [0, 0, 0];
        for tick in 0..1200 {
            if let Some(next) = patrol.step(tick, &collision, position) {
                position = next;
            }
            let bounds = collision.bounds().expect("the floor must fence the room");
            assert!(
                bounds.contains_with_radius(position, SYNTHETIC_ACTOR_RADIUS_UNITS),
                "the patrol left the room at tick {tick}: {position:?}"
            );
        }
    }

    #[test]
    fn only_a_non_player_monster_is_moved() {
        let mut object = LiveObject {
            name: "monster_wallcr".to_string(),
            class: RenderClass::Monster,
            position: [0, 0, 0],
            visible: true,
            slot: None,
            anim_index: 0,
            actor: None,
        };
        let actor = id(1);
        assert!(is_moving_actor(&object, actor, None));
        assert!(!is_moving_actor(&object, actor, Some(actor)));
        for class in [
            RenderClass::Raziel,
            RenderClass::Sreavr,
            RenderClass::GlyphIcon,
            RenderClass::Physical,
            RenderClass::Particle,
            RenderClass::Unclassified,
        ] {
            object.class = class;
            assert!(
                !is_moving_actor(&object, actor, None),
                "{class:?} is not a moving actor"
            );
        }
    }

    #[test]
    fn the_motion_line_carries_the_delta_and_no_floats() {
        let line = motion_line("hunter", [1, 2, 3], [5, 8, 3]);
        assert_eq!(
            line,
            "npc-motion name=hunter start=1,2,3 end=5,8,3 dx=4 dy=6 dz=0"
        );
        assert_eq!(motion_line("x", [0, 0, 0], [1, 1, 1]), motion_line("x", [0, 0, 0], [1, 1, 1]));
    }
}

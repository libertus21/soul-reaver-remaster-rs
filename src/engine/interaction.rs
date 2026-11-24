//! Player-driven open/closed state for door-like placed objects.
//!
//! The gate answered by `docs/notes/interactions.md` found that doors already
//! render as placed object models but never contributed to collision: the room
//! collision mesh is built only from `level::RoomTriangles` (`world.rs`), and the
//! player sweep only ever queries that mesh (`frame.rs`, stage 9). This module
//! adds the missing interaction state without touching the room parser.
//!
//! # State model
//!
//! A registry entry is created per **live record** (by [`LiveId`]) whose authored
//! name is one of [`INTERACTABLE_NAMES`]. The state is `open: bool` and defaults
//! to `false` (closed), so two doors in one room open independently and the name
//! is only a matching key, never the state key.
//!
//! # Collision
//!
//! All eleven names resolve to their own archived model, so each door's geometry
//! is separable (see the note for the measured vertex/triangle counts). The
//! player-facing entry point (`play.rs`) builds one [`CollisionMesh`] per placed
//! door from that model's rest-pose triangles in world space and installs the
//! map here. While a door is **closed**, its walls are unioned ahead of the room
//! walls for the horizontal player sweep; while it is **open**, they are not.
//! Vertical movement keeps using the room mesh only, so a door's horizontal
//! faces can never become a phantom floor.
//!
//! # Rendering
//!
//! The authored door motion is UNKNOWN. Opening is represented the *synthetic*
//! way: the object is hidden. [`SYNTHETIC_OPEN_REPRESENTATION`] names that
//! placeholder so no call site can mistake it for retail behaviour.

use std::collections::BTreeMap;

use crate::engine::collision::CollisionMesh;
use crate::engine::live::{LiveArena, LiveId};

/// The door-like placed names this layer owns.
///
/// The list is the archive survey set from the gate note; membership is ASCII
/// case-insensitive because placed names are compared the same way elsewhere.
pub const INTERACTABLE_NAMES: [&str; 11] = [
    "sudoor", "catdoor", "wrpdoor", "hatch", "valve", "plswtch", "bellsc", "dblgate", "catdora",
    "skgate", "sugate",
];

/// Radius, in integer world units, within which the `E` key picks the nearest
/// interactable.
///
/// A single documented constant, not a per-object value. 128 units is roughly
/// 32 player steps ([`crate::engine::frame::PLAYER_SPEED_UNITS_PER_TICK`] = 4)
/// and about half a door width, so the player must stand at the doorway.
pub const INTERACT_RADIUS_UNITS: i32 = 128;

/// Name of the synthetic open-door rendering placeholder.
///
/// An open door is hidden rather than animated. The original's door motion is
/// UNKNOWN and is deliberately not reproduced.
pub const SYNTHETIC_OPEN_REPRESENTATION: &str = "hidden";

/// True when `name` is one of [`INTERACTABLE_NAMES`].
pub fn is_interactable(name: &str) -> bool {
    INTERACTABLE_NAMES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

/// Why an activation happened, as the console contract spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractTrigger {
    /// The `E` key in the window's player camera mode.
    Key,
    /// An explicit `--interact-at <TICK>:<slot>[:<generation>]` target.
    Script,
    /// A `--interact-at <TICK>:nearest` target, resolved by proximity.
    Proximity,
}

impl InteractTrigger {
    /// The exact token the `interact` console line prints.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Script => "script",
            Self::Proximity => "proximity",
        }
    }
}

/// One activation, before it is rendered as its console line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractOutcome {
    pub name: String,
    pub position: [i32; 3],
    /// The state **after** the toggle: `true` is open, `false` is closed.
    pub open: bool,
    pub trigger: InteractTrigger,
}

impl InteractOutcome {
    /// The exact one-line console contract.
    pub fn line(&self) -> String {
        format!(
            "interact object={} pos={},{},{} state={} trigger={}",
            self.name,
            self.position[0],
            self.position[1],
            self.position[2],
            if self.open { "open" } else { "closed" },
            self.trigger.as_str(),
        )
    }
}

/// Which record an activation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateTarget {
    /// The nearest interactable within [`INTERACT_RADIUS_UNITS`] of the player.
    Nearest,
    /// A specific live slot, optionally pinned to a generation.
    Id { slot: u32, generation: Option<u32> },
}

impl ActivateTarget {
    /// Parse `nearest` or `<slot>[:<generation>]`.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.eq_ignore_ascii_case("nearest") {
            return Ok(Self::Nearest);
        }
        let mut parts = raw.split(':');
        let slot = parts
            .next()
            .unwrap_or("")
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("'{raw}' is neither 'nearest' nor a live slot"))?;
        let generation = match parts.next() {
            Some(generation) => Some(
                generation
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| format!("'{raw}' has a bad generation"))?,
            ),
            None => None,
        };
        if parts.next().is_some() {
            return Err(format!("'{raw}' has too many ':' parts"));
        }
        Ok(Self::Id { slot, generation })
    }
}

/// One parsed `--interact-at <TICK>:<TARGET>` entry, applied at the tick start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractAt {
    pub tick: u64,
    pub target: ActivateTarget,
}

impl InteractAt {
    /// Parse `TICK:TARGET`; every refusal is a message, never a silent default.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (raw_tick, raw_target) = spec
            .split_once(':')
            .ok_or_else(|| format!("expected TICK:TARGET, got '{spec}'"))?;
        let tick = raw_tick
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("'{}' is not a non-negative tick", raw_tick.trim()))?;
        let target = ActivateTarget::parse(raw_target.trim())?;
        Ok(Self { tick, target })
    }
}

/// Per-unit interaction state: open flags, closed-door collision and the log.
#[derive(Debug, Clone, Default)]
pub struct InteractionLayer {
    enabled: bool,
    /// `open` per live interactable. Presence is registration; a missing id is
    /// not an interactable.
    open: BTreeMap<LiveId, bool>,
    /// Closed-door collision, keyed by live id. Built from the archived model.
    meshes: BTreeMap<LiveId, CollisionMesh>,
    /// Prebuilt world-space door meshes keyed by `(lowercase name, position)`, so
    /// the simulation attaches one when the live object appears.
    base: BTreeMap<(String, [i32; 3]), CollisionMesh>,
    /// Cached room-plus-closed-doors wall set, rebuilt when the state changes.
    combined: Option<CollisionMesh>,
    dirty: bool,
    schedule: Vec<InteractAt>,
    log: Vec<InteractOutcome>,
}

impl InteractionLayer {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Turn the layer on or off. Turning it off clears every open flag and the
    /// collision overlay so `--no-interactions` reproduces the previous world.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.open.clear();
        self.meshes.clear();
        self.combined = None;
        self.dirty = true;
        if !enabled {
            self.log.clear();
        }
    }

    /// Replace the prebuilt per-placement door meshes.
    pub fn set_base(&mut self, base: BTreeMap<(String, [i32; 3]), CollisionMesh>) {
        self.base = base;
        self.dirty = true;
    }

    pub fn set_schedule(&mut self, schedule: Vec<InteractAt>) {
        self.schedule = schedule;
    }

    pub fn schedule(&self) -> &[InteractAt] {
        &self.schedule
    }

    /// Release per-unit state on a world swap. The base is replaced by the
    /// caller once the new unit's placements are known.
    pub fn enter_unit(&mut self) {
        self.open.clear();
        self.meshes.clear();
        self.combined = None;
        self.dirty = true;
        self.log.clear();
    }

    pub fn is_open(&self, id: LiveId) -> bool {
        self.open.get(&id).copied().unwrap_or(false)
    }

    /// Drain the activation log for console printing.
    pub fn take_log(&mut self) -> Vec<InteractOutcome> {
        std::mem::take(&mut self.log)
    }

    /// Live-list ordinals of every open interactable, in live order. These are
    /// what the production digest folds; an empty result is byte-identical to
    /// the pre-interaction digest.
    pub fn open_ordinals(&self, live: &LiveArena) -> Vec<u32> {
        if !self.enabled {
            return Vec::new();
        }
        live.iter()
            .enumerate()
            .filter_map(|(ordinal, (id, _))| self.is_open(id).then_some(ordinal as u32))
            .collect()
    }

    /// Register every live interactable not seen before, attaching its prebuilt
    /// closed-door mesh when the placement matches.
    pub fn register_live(&mut self, live: &LiveArena) {
        if !self.enabled {
            return;
        }
        for (id, object) in live.iter() {
            if self.open.contains_key(&id) || !is_interactable(&object.name) {
                continue;
            }
            self.open.insert(id, false);
            let key = (object.name.to_ascii_lowercase(), object.position);
            if let Some(mesh) = self.base.get(&key) {
                self.meshes.insert(id, mesh.clone());
            }
            self.dirty = true;
        }
    }

    /// Resolve `target`, toggle its `open` flag and log the outcome.
    pub fn activate(
        &mut self,
        live: &LiveArena,
        target: ActivateTarget,
        player: Option<[i32; 3]>,
        trigger: InteractTrigger,
    ) -> Option<InteractOutcome> {
        if !self.enabled {
            return None;
        }
        self.register_live(live);
        let id = self.resolve(live, target, player)?;
        let object = live.get(id).ok()?;
        let name = object.name.clone();
        let position = object.position;
        let open = !self.is_open(id);
        self.open.insert(id, open);
        self.dirty = true;
        let outcome = InteractOutcome {
            name,
            position,
            open,
            trigger,
        };
        self.log.push(outcome.clone());
        Some(outcome)
    }

    fn resolve(
        &self,
        live: &LiveArena,
        target: ActivateTarget,
        player: Option<[i32; 3]>,
    ) -> Option<LiveId> {
        match target {
            ActivateTarget::Nearest => {
                let eye = player?;
                let limit =
                    i64::from(INTERACT_RADIUS_UNITS) * i64::from(INTERACT_RADIUS_UNITS);
                let mut best: Option<(i64, LiveId)> = None;
                for (id, object) in live.iter() {
                    if !self.open.contains_key(&id) {
                        continue;
                    }
                    let distance = squared_distance(eye, object.position);
                    if distance <= limit && best.is_none_or(|(best, _)| distance < best) {
                        best = Some((distance, id));
                    }
                }
                best.map(|(_, id)| id)
            }
            ActivateTarget::Id { slot, generation } => live.iter().find_map(|(id, _)| {
                (id.index == slot
                    && generation.is_none_or(|generation| generation == id.generation)
                    && self.open.contains_key(&id))
                .then_some(id)
            }),
        }
    }

    /// Apply every `--interact-at` entry scheduled for `tick`.
    pub fn apply_schedule(&mut self, live: &LiveArena, player: Option<[i32; 3]>, tick: u64) {
        if !self.enabled {
            return;
        }
        let due: Vec<ActivateTarget> = self
            .schedule
            .iter()
            .filter(|entry| entry.tick == tick)
            .map(|entry| entry.target)
            .collect();
        for target in due {
            let trigger = match target {
                ActivateTarget::Nearest => InteractTrigger::Proximity,
                ActivateTarget::Id { .. } => InteractTrigger::Script,
            };
            let _ = self.activate(live, target, player, trigger);
        }
    }

    /// Rebuild the room-plus-closed-doors wall overlay when the state changed.
    pub fn rebuild(&mut self, room: &CollisionMesh) {
        if !self.enabled {
            self.combined = None;
            self.dirty = false;
            return;
        }
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let mut extra = CollisionMesh::default();
        let mut any = false;
        for (id, mesh) in &self.meshes {
            if self.is_open(*id) || mesh.is_empty() {
                continue;
            }
            extra = extra.union_walls(mesh);
            any = true;
        }
        self.combined = any.then(|| room.union_walls(&extra));
    }

    /// The cached overlay, or `None` when no closed door has walls.
    pub fn combined(&self) -> Option<&CollisionMesh> {
        self.combined.as_ref()
    }
}

fn squared_distance(a: [i32; 3], b: [i32; 3]) -> i64 {
    let dx = i64::from(a[0]) - i64::from(b[0]);
    let dy = i64::from(a[1]) - i64::from(b[1]);
    let dz = i64::from(a[2]) - i64::from(b[2]);
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::{
        ActivateTarget, InteractionLayer, InteractAt, InteractTrigger, is_interactable,
    };
    use crate::engine::class::RenderClass;
    use crate::engine::live::{LiveArena, LiveObject};

    fn object(name: &str, position: [i32; 3]) -> LiveObject {
        LiveObject {
            name: name.to_string(),
            class: RenderClass::Unclassified,
            position,
            visible: true,
            slot: None,
            anim_index: 0,
            actor: None,
        }
    }

    #[test]
    fn the_eleven_names_are_matched_case_insensitively() {
        for name in super::INTERACTABLE_NAMES {
            assert!(is_interactable(name));
            assert!(is_interactable(&name.to_ascii_uppercase()));
        }
        assert!(!is_interactable("raziel"));
        assert!(!is_interactable("doorh"), "doorh is not in the surveyed set");
    }

    #[test]
    fn two_records_with_the_same_name_toggle_independently() {
        let mut live = LiveArena::new();
        let first = live.spawn(object("sudoor", [0, 0, 0]));
        let second = live.spawn(object("sudoor", [400, 0, 0]));
        let mut layer = InteractionLayer::new(true);
        layer.register_live(&live);
        assert!(!layer.is_open(first) && !layer.is_open(second));

        let outcome = layer
            .activate(
                &live,
                ActivateTarget::Id {
                    slot: second.index,
                    generation: Some(second.generation),
                },
                None,
                InteractTrigger::Script,
            )
            .expect("the second door activates");
        assert_eq!(outcome.name, "sudoor");
        assert!(outcome.open);
        assert!(!layer.is_open(first), "the first door must stay closed");
        assert!(layer.is_open(second));
        assert_eq!(
            outcome.line(),
            "interact object=sudoor pos=400,0,0 state=open trigger=script"
        );
    }

    #[test]
    fn nearest_uses_the_fixed_radius_and_live_order() {
        let mut live = LiveArena::new();
        let near = live.spawn(object("hatch", [10, 0, 0]));
        let far = live.spawn(object("hatch", [10_000, 0, 0]));
        let mut layer = InteractionLayer::new(true);
        layer.register_live(&live);
        let outcome = layer
            .activate(
                &live,
                ActivateTarget::Nearest,
                Some([0, 0, 0]),
                InteractTrigger::Key,
            )
            .expect("the near hatch is within the radius");
        assert_eq!(outcome.position, [10, 0, 0]);
        assert!(!layer.is_open(far));
        assert!(layer.is_open(near));
    }

    #[test]
    fn a_disabled_layer_never_activates_and_folds_nothing() {
        let mut live = LiveArena::new();
        let id = live.spawn(object("sudoor", [0, 0, 0]));
        let mut layer = InteractionLayer::new(false);
        layer.register_live(&live);
        assert!(
            layer
                .activate(&live, ActivateTarget::Nearest, Some([0, 0, 0]), InteractTrigger::Key)
                .is_none()
        );
        assert!(layer.open_ordinals(&live).is_empty());
        assert!(!layer.is_open(id));
    }

    #[test]
    fn interact_at_parses_both_target_forms() {
        assert_eq!(
            InteractAt::parse("5:nearest"),
            Ok(InteractAt {
                tick: 5,
                target: ActivateTarget::Nearest
            })
        );
        assert_eq!(
            InteractAt::parse(" 7 : 12 : 3 "),
            Ok(InteractAt {
                tick: 7,
                target: ActivateTarget::Id {
                    slot: 12,
                    generation: Some(3)
                }
            })
        );
        assert_eq!(
            InteractAt::parse("7:12"),
            Ok(InteractAt {
                tick: 7,
                target: ActivateTarget::Id {
                    slot: 12,
                    generation: None
                }
            })
        );
        assert!(InteractAt::parse("7").is_err());
        assert!(InteractAt::parse("x:nearest").is_err());
        assert!(InteractAt::parse("7:").is_err());
        assert!(InteractAt::parse("7:1:2:3").is_err());
    }
}

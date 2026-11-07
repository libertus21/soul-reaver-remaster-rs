//! Data-keyed actor profile and numeric runtime state.
//!
//! This is the **data half** of the actor seam. It reads the shipped per-model
//! actor block through the existing [`crate::actor::parse_actor_block`] parser;
//! no byte parsing is duplicated here. The profile carries the numeric
//! behaviour data (kind, state trios, behaviour entries, entry-array kind) and
//! **no name**, because the name is a spawn-policy key and must never reach the
//! AI layer (`docs/changes/engine-skeleton/design.md`, seam B).
//!
//! [`ActorRuntime`] holds only numeric state: the relation code, the four state
//! bytes the transition-table note identifies (**current**, **queued**,
//! **alternate** and **entry/action**), an optional live target and a flag
//! word. No enum is invented for the state ids: the shipped ids are
//! model-relative bytes, as recorded in `docs/notes/actor-ai.md`,
//! `docs/notes/actor-behaviour-data.md` and
//! `docs/notes/actor-transition-table.md` section 1.
//!
//! Only one state transition is CONFIRMED: the behaviour-set refresh
//! ([`refresh`]). It is frozen here as a pure function and is deliberately not
//! called by [`ActorRuntime::advance`]. The eleven writer bodies remain
//! UNKNOWN (`docs/notes/actor-transition-table.md` section 6).

use crate::actor::{self as parser, ActorBlock, ActorKind, BehaviourEntry, EntryArrayKind};
use crate::engine::clock::ObjectTick;
use crate::engine::events::EngineEvent;
use crate::engine::live::LiveId;
use crate::object::ObjectError;

/// The behaviour data a model carries, without its name.
///
/// Two models whose actor blocks are byte-identical produce equal profiles even
/// when their asset names differ. This is the invariant that keeps the
/// name-keyed spawn policy and the data-keyed AI separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorProfile {
    /// `Monster` or `Player`, from the block signature.
    pub kind: ActorKind,
    /// Eight rows of three one-byte clip-set ids (block `+0x18`).
    pub trios: [[u8; 3]; 8],
    /// The `0x20`-byte behaviour-entry records, in stored order.
    pub entries: Vec<BehaviourEntry>,
    /// `Clean`, `Variant` or `None`, from the entry-array detection.
    pub entry_array: EntryArrayKind,
}

impl ActorProfile {
    /// Copy the behaviour data out of a parsed actor block.
    ///
    /// The block's byte offset and length are deliberately not part of the
    /// profile: only the data is keyed, never the container position.
    pub fn from_block(block: &ActorBlock) -> Self {
        Self {
            kind: block.kind,
            trios: block.trios,
            entries: block.entries.clone(),
            entry_array: block.entry_array,
        }
    }

    /// Parse a model's raw container bytes through the existing actor parser.
    ///
    /// `Ok(None)` means the model is not an actor. A truncated actor-signature
    /// block stays a loud [`ObjectError`], exactly as the parser reports it.
    pub fn parse(bytes: &[u8]) -> Result<Option<Self>, ObjectError> {
        parser::parse_actor_block(bytes)
            .map(|block| block.as_ref().map(Self::from_block))
    }
}

/// The post-action state value.
///
/// CONFIRMED by the shipped behaviour-set refresh: when the current state is
/// this value, the refresh substitutes the pending alternate or queued state.
/// It is a raw byte, not an enum member: no other state value has an
/// established name (`docs/notes/actor-transition-table.md` sections 1 and 6).
pub const POST_ACTION_STATE: u8 = 8;

/// Apply the one CONFIRMED actor-state transition rule.
///
/// ```text
/// refresh(current, queued, alternate):
///     if current != 8: next = current
///     else:            next = alternate if set, else queued
/// ```
///
/// This is the behaviour-set refresh (`docs/notes/actor-transition-table.md`
/// section 2, step 4). The rule is pure: it reads no world state, mutates
/// nothing and returns the next value. It is intentionally **unwired**:
/// [`ActorRuntime::advance`] does not call it, because the activation program,
/// the event layer and the engagement/target data that populate `queued` and
/// `alternate` are not modelled yet (same note, section 6).
///
/// # `alternate` and its UNKNOWN sentinel
///
/// The note records only that "alternate is set" means "any value other than
/// the not-set sentinel", and that the concrete sentinel value is **UNKNOWN**
/// (it was observed as the all-ones byte in one shipped path, not proven as a
/// general sentinel). `Option<u8>` is therefore a Rust representation choice,
/// **not** an evidence claim about the on-disk byte: `None` models "not set"
/// without inventing a sentinel constant, and `Some(value)` models "set to
/// `value`", including `Some(0)` if a shipped path ever sets zero.
pub fn refresh(current: u8, queued: u8, alternate: Option<u8>) -> u8 {
    if current != POST_ACTION_STATE {
        return current;
    }
    alternate.unwrap_or(queued)
}

/// Outcome of one actor transition step.
///
/// The decision body is not implemented, so the only outcome this slice can
/// produce is [`ActorTransition::NotInterpreted`]. The marker is explicit: it is
/// not a plausible default that pretends to be behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorTransition {
    /// The runtime was ticked. No relation change and no action id can be
    /// selected, because perception, the engagement-radius comparison, the melee
    /// gate and the state-to-clip mapping are UNKNOWN (`docs/notes/actor-ai.md`
    /// section 5 and 10).
    NotInterpreted,
}

/// Numeric runtime state for one live actor.
///
/// There is intentionally no name field. The profile is the only data key, so
/// renaming a model cannot change its runtime state. Every state field stays a
/// raw byte: the shipped ids have no stable global meaning and must not be
/// turned into an invented enum.
///
/// The four state bytes are the ones the transition-table note identifies
/// (`docs/notes/actor-transition-table.md` section 1):
///
/// - `current` - the selector the AI tick and the animation set read,
/// - `queued` - a pending state,
/// - `alternate` - a higher-priority pending state,
/// - `entry_action` - the fourth companion byte written from the behaviour
///   entry and copied into `current` by three writers. It is a state source,
///   not the current state.
///
/// `alternate` is `Option<u8>` because the note establishes only that "set"
/// means "not the not-set sentinel" and leaves the concrete sentinel UNKNOWN
/// (see [`refresh`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorRuntime {
    profile: ActorProfile,
    relation: u8,
    current: u8,
    queued: u8,
    alternate: Option<u8>,
    entry_action: u8,
    target: Option<LiveId>,
    flags: u32,
}

impl ActorRuntime {
    /// Build the initial runtime state from a profile.
    ///
    /// The numeric fields start neutral: relation `0` (the first code of the
    /// documented `0..3` relation space) and all four state bytes at their
    /// neutral values (`0`, `0`, not set, `0`). No activation program has run,
    /// so no queued or alternate state exists yet. The profile is embedded, so
    /// two runtimes built from equal profiles are equal regardless of the model
    /// name.
    pub fn initial(profile: ActorProfile) -> Self {
        Self {
            profile,
            relation: 0,
            current: 0,
            queued: 0,
            alternate: None,
            entry_action: 0,
            target: None,
            flags: 0,
        }
    }

    pub fn profile(&self) -> &ActorProfile {
        &self.profile
    }

    pub fn relation(&self) -> u8 {
        self.relation
    }

    /// The current state selector.
    pub fn current(&self) -> u8 {
        self.current
    }

    /// The queued (pending) state.
    pub fn queued(&self) -> u8 {
        self.queued
    }

    /// The alternate (higher-priority pending) state, if set.
    pub fn alternate(&self) -> Option<u8> {
        self.alternate
    }

    /// The behaviour-entry companion byte (a state source, not `current`).
    pub fn entry_action(&self) -> u8 {
        self.entry_action
    }

    pub fn target(&self) -> Option<LiveId> {
        self.target
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// Set the numeric relation code (`0..3` in the documented space).
    pub fn set_relation(&mut self, relation: u8) {
        self.relation = relation;
    }

    /// Set the current state selector.
    pub fn set_current(&mut self, current: u8) {
        self.current = current;
    }

    /// Set the queued (pending) state.
    pub fn set_queued(&mut self, queued: u8) {
        self.queued = queued;
    }

    /// Set or clear the alternate (higher-priority pending) state.
    pub fn set_alternate(&mut self, alternate: Option<u8>) {
        self.alternate = alternate;
    }

    /// Set the behaviour-entry companion byte.
    pub fn set_entry_action(&mut self, entry_action: u8) {
        self.entry_action = entry_action;
    }

    /// The documented transition entry point, called by the per-frame tick.
    ///
    /// This is the only route by which the runtime may advance. `events` is the
    /// FIFO view of the events addressed to this object for the current tick,
    /// already produced by the frame pipeline. The carrier now **receives**
    /// events; it still selects nothing from them.
    ///
    /// The decision body that would choose a relation or an action id is
    /// UNKNOWN: the notes pin the numeric ids but not their meaning, and the
    /// shipped ids are model-relative (`docs/notes/actor-ai.md` section 5,
    /// `docs/notes/actor-behaviour-data.md` section 7). The step therefore
    /// leaves every field untouched regardless of what `events` contains, and
    /// returns an explicit [`ActorTransition::NotInterpreted`] instead of
    /// inventing a transition. In particular it does **not** apply [`refresh`]:
    /// that rule is frozen as a pure function and stays unwired until the
    /// activation and event inputs exist (`docs/notes/actor-transition-table.md`
    /// section 6). Damage, health, knockback and combat resolution are out of
    /// scope and are not read here.
    ///
    /// Later evidence-backed slices replace this body; the dispatch, the carrier,
    /// the event plumbing and the determinism fold do not need to change for
    /// that.
    pub fn advance(&mut self, _step: ObjectTick, _events: &[EngineEvent]) -> ActorTransition {
        ActorTransition::NotInterpreted
    }

    /// Set or clear the live target.
    pub fn set_target(&mut self, target: Option<LiveId>) {
        self.target = target;
    }

    /// Replace the flag word.
    pub fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
    }
}

#[cfg(test)]
mod tests {
    use super::{ActorProfile, ActorRuntime, POST_ACTION_STATE, refresh};
    use crate::actor::{ActorKind, EntryArrayKind};
    use crate::engine::live::LiveId;

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

    fn profile(name: &str, ids: &[u8]) -> ActorProfile {
        ActorProfile::parse(&crate::actor::synthetic_actor(name, ids, trios()))
            .expect("synthetic actor must parse")
            .expect("synthetic actor must carry a block")
    }

    #[test]
    fn actor_model_yields_kind_trios_and_entry_ids() {
        let profile = profile("skinner", &[0x18, 0x25, 0x26, 0x27, 0x28]);
        assert_eq!(profile.kind, ActorKind::Monster);
        assert_eq!(profile.entry_array, EntryArrayKind::Clean);
        assert_eq!(profile.trios, trios());
        assert_eq!(
            profile.entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![0x18, 0x25, 0x26, 0x27, 0x28]
        );
    }

    #[test]
    fn non_actor_model_yields_no_profile() {
        let parsed = ActorProfile::parse(&crate::object::synthetic_model())
            .expect("a non-actor must not be an error");
        assert_eq!(parsed, None);
    }

    #[test]
    fn identical_profiles_from_differently_named_models_start_equal() {
        let skinner = profile("skinner", &[0x18, 0x25, 0x26]);
        let morlock = profile("morlock", &[0x18, 0x25, 0x26]);

        assert_eq!(skinner, morlock, "the name is not part of the profile");
        assert_eq!(
            ActorRuntime::initial(skinner),
            ActorRuntime::initial(morlock),
            "equal profiles must start in equal runtime state"
        );
    }

    #[test]
    fn initial_runtime_is_numeric_and_neutral() {
        let runtime = ActorRuntime::initial(profile("skinner", &[0x25]));
        assert_eq!(runtime.relation(), 0);
        assert_eq!(runtime.current(), 0);
        assert_eq!(runtime.queued(), 0);
        assert_eq!(runtime.alternate(), None);
        assert_eq!(runtime.entry_action(), 0);
        assert_eq!(runtime.target(), None);
        assert_eq!(runtime.flags(), 0);
        assert_eq!(runtime.profile().kind, ActorKind::Monster);
    }

    #[test]
    fn refresh_keeps_a_non_post_action_current_unchanged() {
        for current in [0, 1, 7, 9, 0x25, u8::MAX] {
            for alternate in [None, Some(0), Some(3)] {
                assert_eq!(
                    refresh(current, 4, alternate),
                    current,
                    "current {current} is not the post-action value, so nothing may substitute it"
                );
            }
        }
    }

    #[test]
    fn refresh_selects_alternate_at_the_post_action_value() {
        assert_eq!(refresh(POST_ACTION_STATE, 4, Some(7)), 7);
        assert_eq!(
            refresh(POST_ACTION_STATE, 4, Some(0)),
            0,
            "Some(0) is set, so it wins over queued; only None is not-set"
        );
    }

    #[test]
    fn refresh_falls_back_to_the_queued_state_when_alternate_is_not_set() {
        assert_eq!(refresh(POST_ACTION_STATE, 4, None), 4);
        assert_eq!(refresh(POST_ACTION_STATE, 0, None), 0);
        assert_eq!(refresh(POST_ACTION_STATE, u8::MAX, None), u8::MAX);
    }

    #[test]
    fn refresh_is_pure_repeatable_and_does_not_mutate_the_carrier() {
        let mut runtime = ActorRuntime::initial(profile("skinner", &[0x18, 0x25]));
        runtime.set_current(POST_ACTION_STATE);
        runtime.set_queued(4);
        runtime.set_alternate(Some(7));
        runtime.set_entry_action(2);
        let before = runtime.clone();

        let first = refresh(runtime.current(), runtime.queued(), runtime.alternate());
        let second = refresh(runtime.current(), runtime.queued(), runtime.alternate());

        assert_eq!(first, second, "same inputs must produce the same output");
        assert_eq!(first, 7);
        assert_eq!(
            runtime, before,
            "refresh takes values, not the carrier, and must not mutate it"
        );
        assert_eq!(
            (
                runtime.current(),
                runtime.queued(),
                runtime.alternate(),
                runtime.entry_action()
            ),
            (POST_ACTION_STATE, 4, Some(7), 2)
        );
    }

    #[test]
    fn advance_receives_events_and_still_selects_nothing() {
        use crate::engine::events::{ContactRecord, EngineEvent, HitRecord, Message};

        let mut runtime = ActorRuntime::initial(profile("skinner", &[0x18, 0x25]));
        let before = runtime.clone();
        let events = [
            EngineEvent::Hit(HitRecord {
                attacker: LiveId {
                    index: 1,
                    generation: 0,
                },
                target: LiveId {
                    index: 2,
                    generation: 0,
                },
                bone: 3,
                radius_mask: 0x0001,
            }),
            EngineEvent::Contact(ContactRecord {
                source: LiveId {
                    index: 2,
                    generation: 0,
                },
                target: LiveId {
                    index: 2,
                    generation: 0,
                },
            }),
            EngineEvent::Message(Message {
                target: LiveId {
                    index: 2,
                    generation: 0,
                },
                code: 7,
            }),
        ];

        let outcome = runtime.advance(crate::engine::clock::ObjectTick::step(1), &events);

        assert_eq!(outcome, crate::engine::actor::ActorTransition::NotInterpreted);
        assert_eq!(
            runtime, before,
            "receiving events must not change relation, the four state bytes, target, flags or the profile"
        );
        assert_eq!(
            (
                runtime.relation(),
                runtime.current(),
                runtime.queued(),
                runtime.alternate(),
                runtime.entry_action()
            ),
            (0, 0, 0, None, 0)
        );
    }

    #[test]
    fn advance_does_not_apply_the_confirmed_refresh_rule() {
        use crate::engine::actor::ActorTransition;
        use crate::engine::events::{EngineEvent, Message};

        let mut runtime = ActorRuntime::initial(profile("skinner", &[0x18, 0x25]));
        runtime.set_current(POST_ACTION_STATE);
        runtime.set_queued(4);
        runtime.set_alternate(Some(7));
        runtime.set_entry_action(2);
        let before = runtime.clone();

        let events = [EngineEvent::Message(Message {
            target: LiveId {
                index: 2,
                generation: 0,
            },
            code: 7,
        })];
        let outcome = runtime.advance(crate::engine::clock::ObjectTick::step(1), &events);

        assert_eq!(outcome, ActorTransition::NotInterpreted);
        assert_eq!(
            runtime, before,
            "advance must not call refresh: current stays 8 even though the pure rule would select 7"
        );
        assert_eq!(
            (
                runtime.current(),
                runtime.queued(),
                runtime.alternate(),
                runtime.entry_action()
            ),
            (POST_ACTION_STATE, 4, Some(7), 2)
        );
    }

    #[test]
    fn numeric_fields_are_individually_settable() {
        let mut runtime = ActorRuntime::initial(profile("skinner", &[0x25]));
        let initial = runtime.clone();

        runtime.set_relation(2);
        runtime.set_current(0x25);
        runtime.set_queued(0x26);
        runtime.set_alternate(Some(0x27));
        runtime.set_entry_action(0x28);
        runtime.set_flags(0x04);
        runtime.set_target(Some(LiveId {
            index: 3,
            generation: 1,
        }));

        assert_ne!(runtime, initial);
        assert_eq!(runtime.relation(), 2);
        assert_eq!(runtime.current(), 0x25);
        assert_eq!(runtime.queued(), 0x26);
        assert_eq!(runtime.alternate(), Some(0x27));
        assert_eq!(runtime.entry_action(), 0x28);
        assert_eq!(runtime.flags(), 0x04);
        assert_eq!(
            runtime.target(),
            Some(LiveId {
                index: 3,
                generation: 1,
            })
        );
    }
}

//! Data-keyed actor profile and numeric runtime state.
//!
//! This is the **data half** of the actor seam. It reads the shipped per-model
//! actor block through the existing [`crate::actor::parse_actor_block`] parser;
//! no byte parsing is duplicated here. The profile carries the numeric
//! behaviour data (kind, state trios, behaviour entries, entry-array kind) and
//! **no name**, because the name is a spawn-policy key and must never reach the
//! AI layer (`docs/changes/engine-skeleton/design.md`, seam B).
//!
//! [`ActorRuntime`] holds only numeric state: the relation code, the action
//! state id, an optional live target and a flag word. No enum is invented for
//! the state ids: the shipped ids are model-relative bytes, as recorded in
//! `docs/notes/actor-ai.md` and `docs/notes/actor-behaviour-data.md`.

use crate::actor::{self as parser, ActorBlock, ActorKind, BehaviourEntry, EntryArrayKind};
use crate::engine::clock::ObjectTick;
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
/// renaming a model cannot change its runtime state. `relation` and `state` stay
/// raw bytes: the shipped action ids have no stable global meaning and must not
/// be turned into an invented enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorRuntime {
    profile: ActorProfile,
    relation: u8,
    state: u8,
    target: Option<LiveId>,
    flags: u32,
}

impl ActorRuntime {
    /// Build the initial runtime state from a profile.
    ///
    /// The numeric fields start neutral: relation `0` (the first code of the
    /// documented `0..3` relation space) and action state `0` (no action
    /// selected yet). The profile is embedded, so two runtimes built from equal
    /// profiles are equal regardless of the model name.
    pub fn initial(profile: ActorProfile) -> Self {
        Self {
            profile,
            relation: 0,
            state: 0,
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

    pub fn state(&self) -> u8 {
        self.state
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

    /// Set the numeric action state id.
    pub fn set_state(&mut self, state: u8) {
        self.state = state;
    }

    /// The documented transition entry point, called by the per-frame tick.
    ///
    /// This is the only route by which the runtime may advance. The decision
    /// body that would choose a relation or an action id is UNKNOWN: the notes
    /// pin the numeric ids but not their meaning, and the shipped ids are
    /// model-relative (`docs/notes/actor-ai.md` section 5,
    /// `docs/notes/actor-behaviour-data.md` section 7). The step therefore
    /// selects nothing and leaves every field untouched; it returns an explicit
    /// [`ActorTransition::NotInterpreted`] instead of inventing a transition.
    ///
    /// Later evidence-backed slices replace this body; the dispatch, the carrier
    /// and the determinism fold do not need to change for that.
    pub fn advance(&mut self, _step: ObjectTick) -> ActorTransition {
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
    use super::{ActorProfile, ActorRuntime};
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
        assert_eq!(runtime.state(), 0);
        assert_eq!(runtime.target(), None);
        assert_eq!(runtime.flags(), 0);
        assert_eq!(runtime.profile().kind, ActorKind::Monster);
    }

    #[test]
    fn numeric_fields_are_individually_settable() {
        let mut runtime = ActorRuntime::initial(profile("skinner", &[0x25]));
        let initial = runtime.clone();

        runtime.set_relation(2);
        runtime.set_state(0x25);
        runtime.set_flags(0x04);
        runtime.set_target(Some(LiveId {
            index: 3,
            generation: 1,
        }));

        assert_ne!(runtime, initial);
        assert_eq!(runtime.relation(), 2);
        assert_eq!(runtime.state(), 0x25);
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

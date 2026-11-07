//! Render class table and the three per-object behaviour hooks.
//!
//! The class is a renderer/prototype category, not a spawn rule: it is selected
//! from the loaded model's class-flags word and prototype name and installs up
//! to three function pointers on the live object (event, per-frame tick, query).
//! A missing slot (`None`) is a first-class opt-out, exactly like the original's
//! null hook, and never an error.
//!
//! The table is data: [`CLASS_TABLE`] is a constant array. [`classify_loaded`]
//! reads the forced bits first and only then compares the prototype name
//! exactly, so adding a class is still a row, not another match arm.

use crate::engine::clock::ObjectTick;
use crate::engine::events::EngineEvent;
use crate::engine::live::{LiveArena, LiveId};
use crate::engine::world::World;

/// The six renderer families plus the no-match sentinel.
///
/// The discriminants are the low byte of the original class index (`0..5`, and
/// `0xFF` for the `0xFFFF` no-match value) so the determinism fold can carry the
/// class as one byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RenderClass {
    Raziel = 0,
    Sreavr = 1,
    GlyphIcon = 2,
    Physical = 3,
    Monster = 4,
    Particle = 5,
    Unclassified = 0xFF,
}

impl RenderClass {
    /// Stable byte used by the canonical state fold. Never renumber.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Placeholder payload for the query signature. The query pass fills it later;
/// it exists now so the class table can carry a concrete function pointer
/// without inventing query state.
#[allow(dead_code)]
pub struct Query;
#[allow(dead_code)]
pub struct QueryAnswer;

/// Event/message entry (hook slot 3).
///
/// The evidence says this entry receives the world context and hit payloads.
/// The event carrier is [`EngineEvent`]; the body is not implemented.
pub type EventHook = fn(&mut World, LiveId, &EngineEvent);
/// Per-frame tick (hook slot 6).
///
/// The live arena is passed explicitly because an actor's per-object runtime
/// state lives on its [`crate::engine::live::LiveObject`], not on the authored
/// [`World`]. The original reads and writes the same per-object block from the
/// tick hook; keeping the two stores separate is the engine's existing world /
/// live split.
///
/// The last argument is the FIFO view of the events addressed to this object for
/// the current tick. It is how the carrier receives events; the body still
/// selects nothing.
pub type TickHook = fn(&mut World, &mut LiveArena, LiveId, ObjectTick, &[EngineEvent]);
/// Query/state getter (`+0x1B0`).
pub type QueryHook = fn(&World, LiveId, Query) -> QueryAnswer;

/// The three behaviour slots installed on a live object from the class table.
///
/// `None` is the original's null hook: a deliberate opt-out for classes that do
/// not participate in that pass.
///
/// No `PartialEq`: function-pointer equality is not meaningful, so callers
/// compare `is_some()` per slot instead of whole hook sets.
#[derive(Debug, Clone, Copy)]
pub struct Hooks {
    pub event: Option<EventHook>,
    pub tick: Option<TickHook>,
    pub query: Option<QueryHook>,
}

impl Hooks {
    /// All three slots null.
    pub const NONE: Self = Self {
        event: None,
        tick: None,
        query: None,
    };
}

/// One row of the six-prefix table.
#[derive(Debug, Clone, Copy)]
pub struct ClassEntry {
    pub prefix: &'static str,
    pub class: RenderClass,
    pub hooks: Hooks,
}

/// Shared body of the two installed tick hooks.
///
/// The class table is the **dispatch gate**: only `raziel__` and `monster_`
/// install this body. The body itself does not decide anything. It advances the
/// per-object [`crate::engine::actor::ActorRuntime`] carrier if the live object
/// has one, through the documented
/// [`crate::engine::actor::ActorRuntime::advance`] entry point.
///
/// The documented per-class bodies (the player interaction tick and the monster
/// locomotion thunk) are not implemented; they need animation, input queries and
/// collision. This slice only wires the existing carrier into the frame loop, so
/// both classes share one carriage body. Diverging them later is changing the
/// table row, not the dispatch.
fn actor_runtime_tick(
    _world: &mut World,
    live: &mut LiveArena,
    id: LiveId,
    tick: ObjectTick,
    events: &[EngineEvent],
) {
    let Ok(object) = live.get_mut(id) else {
        return;
    };
    if let Some(actor) = object.actor.as_mut() {
        actor.advance(tick, events);
    }
}

fn raziel_tick(
    world: &mut World,
    live: &mut LiveArena,
    id: LiveId,
    tick: ObjectTick,
    events: &[EngineEvent],
) {
    actor_runtime_tick(world, live, id, tick, events);
}

fn monster_tick(
    world: &mut World,
    live: &mut LiveArena,
    id: LiveId,
    tick: ObjectTick,
    events: &[EngineEvent],
) {
    actor_runtime_tick(world, live, id, tick, events);
}

/// Witness body for the event/message hook slot.
///
/// The evidence pins the slot and that per-actor binders install a body there;
/// it does not pin the body. This function therefore does nothing: it takes the
/// event and changes no world, live or actor state. Its only purpose is to make
/// the delivery seam real and testable without inventing a handler.
fn actor_event(_world: &mut World, _id: LiveId, _event: &EngineEvent) {}

/// The six documented renderer prefixes.
///
/// `raziel__` and `monster_` opt into the generic tick; `sreavr__`, `glphicon`,
/// `physical` and `particle` leave it null. The same two actor rows install the
/// empty event witness on hook slot 3: the evidence attests per-actor event
/// entries, not entries for the other four rows, so those stay null rather than
/// growing an unproven hook. No class carries a query hook in this milestone.
/// Both installed tick bodies share the actor-carriage body: the per-class
/// behaviour (player interaction, monster locomotion) is not implemented, but
/// the dispatch and the carrier are real.
pub const CLASS_TABLE: [ClassEntry; 6] = [
    ClassEntry {
        prefix: "raziel__",
        class: RenderClass::Raziel,
        hooks: Hooks {
            event: Some(actor_event),
            tick: Some(raziel_tick),
            query: None,
        },
    },
    ClassEntry {
        prefix: "sreavr__",
        class: RenderClass::Sreavr,
        hooks: Hooks::NONE,
    },
    ClassEntry {
        prefix: "glphicon",
        class: RenderClass::GlyphIcon,
        hooks: Hooks::NONE,
    },
    ClassEntry {
        prefix: "physical",
        class: RenderClass::Physical,
        hooks: Hooks::NONE,
    },
    ClassEntry {
        prefix: "monster_",
        class: RenderClass::Monster,
        hooks: Hooks {
            event: Some(actor_event),
            tick: Some(monster_tick),
            query: None,
        },
    },
    ClassEntry {
        prefix: "particle",
        class: RenderClass::Particle,
        hooks: Hooks::NONE,
    },
];

/// The table row whose prefix starts `name`, if any.
pub fn classify_entry(name: &str) -> Option<ClassEntry> {
    CLASS_TABLE
        .iter()
        .copied()
        .find(|entry| name.starts_with(entry.prefix))
}

/// Forced `physical` class bit in the loaded object's class-flags word.
pub const CLASS_FLAG_PHYSICAL: u32 = 0x0004_0000;
/// Forced `monster_` class bit in the loaded object's class-flags word.
pub const CLASS_FLAG_MONSTER: u32 = 0x0008_0000;

/// The class for a synthetic/instance name: a linear prefix lookup.
///
/// This is the fallback for spawns that carry no loaded model. The real path is
/// [`classify_loaded`].
pub fn classify(name: &str) -> RenderClass {
    classify_entry(name).map_or(RenderClass::Unclassified, |entry| entry.class)
}

/// The class of a loaded model, from the exact evidence order.
///
/// 1. Bit 18 in the class-flags word forces [`RenderClass::Physical`].
/// 2. Otherwise bit 19 forces [`RenderClass::Monster`]. The physical test runs
///    first, so a word with both bits classifies as physical.
/// 3. Otherwise the padded prototype name is compared **exactly** against the
///    six-row table. No match is [`RenderClass::Unclassified`].
///
/// The flags word must be captured at load-complete, before actor initialisation
/// reuses that slot as the parameter-program pointer. [`crate::object`] captures
/// it while parsing the container, and [`crate::engine::assets`] classifies from
/// that captured word before it parses the actor profile.
pub fn classify_loaded(class_flags: u32, prototype: &str) -> RenderClass {
    if class_flags & CLASS_FLAG_PHYSICAL != 0 {
        return RenderClass::Physical;
    }
    if class_flags & CLASS_FLAG_MONSTER != 0 {
        return RenderClass::Monster;
    }
    CLASS_TABLE
        .iter()
        .find(|entry| entry.prefix == prototype)
        .map_or(RenderClass::Unclassified, |entry| entry.class)
}

#[cfg(test)]
mod tests {
    use super::{
        CLASS_TABLE, ClassEntry, RenderClass, classify, classify_entry, classify_loaded,
    };

    #[test]
    fn class_table_covers_every_prefix_and_declares_every_slot() {
        assert_eq!(CLASS_TABLE.len(), 6, "the six documented prefixes");

        let cases: [(&str, RenderClass, bool, bool); 6] = [
            ("raziel__", RenderClass::Raziel, true, true),
            ("sreavr__", RenderClass::Sreavr, false, false),
            ("glphicon", RenderClass::GlyphIcon, false, false),
            ("physical", RenderClass::Physical, false, false),
            ("monster_", RenderClass::Monster, true, true),
            ("particle", RenderClass::Particle, false, false),
        ];
        for (prefix, class, has_tick, has_event) in cases {
            let entry = classify_entry(prefix)
                .unwrap_or_else(|| panic!("prefix '{prefix}' must be in the table"));
            assert_eq!(entry.prefix, prefix);
            assert_eq!(entry.class, class);
            assert_eq!(classify(prefix), class);
            assert_eq!(
                entry.hooks.tick.is_some(),
                has_tick,
                "tick slot for '{prefix}'"
            );
            assert_eq!(
                entry.hooks.event.is_some(),
                has_event,
                "event slot for '{prefix}'"
            );
            assert!(entry.hooks.query.is_none(), "query slot for '{prefix}'");
        }

        // Every row is reachable through its own prefix.
        for entry in CLASS_TABLE {
            assert_eq!(classify(entry.prefix), entry.class);
        }
    }

    #[test]
    fn only_raziel_and_monster_opt_into_the_tick() {
        for entry in CLASS_TABLE {
            match entry.class {
                RenderClass::Raziel | RenderClass::Monster => {
                    assert!(entry.hooks.tick.is_some(), "{} must tick", entry.prefix)
                }
                RenderClass::Sreavr
                | RenderClass::GlyphIcon
                | RenderClass::Physical
                | RenderClass::Particle => {
                    assert!(entry.hooks.tick.is_none(), "{} must not tick", entry.prefix)
                }
                RenderClass::Unclassified => panic!("no table row may be unclassified"),
            }
        }
    }

    #[test]
    fn only_raziel_and_monster_install_the_event_witness() {
        for entry in CLASS_TABLE {
            match entry.class {
                RenderClass::Raziel | RenderClass::Monster => assert!(
                    entry.hooks.event.is_some(),
                    "{} carries the attested per-actor event entry",
                    entry.prefix
                ),
                RenderClass::Sreavr
                | RenderClass::GlyphIcon
                | RenderClass::Physical
                | RenderClass::Particle => assert!(
                    entry.hooks.event.is_none(),
                    "{} has no attested event entry; the slot stays null",
                    entry.prefix
                ),
                RenderClass::Unclassified => panic!("no table row may be unclassified"),
            }
        }
    }

    #[test]
    fn classification_uses_the_documented_prefixes() {
        assert_eq!(classify("raziel__"), RenderClass::Raziel);
        assert_eq!(classify("raziel__player"), RenderClass::Raziel);
        assert_eq!(classify("sreavr__tail"), RenderClass::Sreavr);
        assert_eq!(classify("glphicon_any"), RenderClass::GlyphIcon);
        assert_eq!(classify("physical_any"), RenderClass::Physical);
        assert_eq!(classify("monster_"), RenderClass::Monster);
        assert_eq!(classify("monster_wallcr"), RenderClass::Monster);
        assert_eq!(classify("particle_any"), RenderClass::Particle);
    }

    #[test]
    fn unknown_prefix_is_unclassified_with_all_hooks_null() {
        for name in [
            "",
            "under1",
            "raziel",
            "RAZIEL__",
            "sreavr",
            "glphico",
            "particl",
        ] {
            assert!(classify_entry(name).is_none(), "'{name}' must not classify");
            assert_eq!(classify(name), RenderClass::Unclassified, "'{name}'");
        }
    }

    #[test]
    fn class_codes_are_stable_single_bytes() {
        assert_eq!(RenderClass::Raziel.code(), 0);
        assert_eq!(RenderClass::Sreavr.code(), 1);
        assert_eq!(RenderClass::GlyphIcon.code(), 2);
        assert_eq!(RenderClass::Physical.code(), 3);
        assert_eq!(RenderClass::Monster.code(), 4);
        assert_eq!(RenderClass::Particle.code(), 5);
        assert_eq!(RenderClass::Unclassified.code(), 0xFF);
    }

    #[test]
    fn no_match_returns_no_entry_rather_than_a_default_row() {
        let entry: Option<ClassEntry> = classify_entry("under1");
        assert!(entry.is_none());
    }

    #[test]
    fn forced_bits_win_in_physical_then_monster_order() {
        assert_eq!(
            classify_loaded(super::CLASS_FLAG_PHYSICAL, "raziel__"),
            RenderClass::Physical,
            "bit 18 forces physical over the prototype name"
        );
        assert_eq!(
            classify_loaded(super::CLASS_FLAG_MONSTER, "raziel__"),
            RenderClass::Monster,
            "bit 19 forces monster over the prototype name"
        );
        assert_eq!(
            classify_loaded(
                super::CLASS_FLAG_PHYSICAL | super::CLASS_FLAG_MONSTER,
                "raziel__"
            ),
            RenderClass::Physical,
            "the physical test runs first, so both bits classify physical"
        );
    }

    #[test]
    fn neither_bit_falls_back_to_an_exact_prototype_name() {
        let cases = [
            ("raziel__", RenderClass::Raziel),
            ("sreavr__", RenderClass::Sreavr),
            ("glphicon", RenderClass::GlyphIcon),
            ("physical", RenderClass::Physical),
            ("monster_", RenderClass::Monster),
            ("particle", RenderClass::Particle),
        ];
        for (prototype, expected) in cases {
            assert_eq!(
                classify_loaded(0, prototype),
                expected,
                "exact prototype '{prototype}'"
            );
        }
    }

    #[test]
    fn neither_bit_and_no_exact_match_is_unclassified() {
        assert_eq!(classify_loaded(0, "skinner_"), RenderClass::Unclassified);
        assert_eq!(classify_loaded(0, "under1"), RenderClass::Unclassified);
        assert_eq!(
            classify_loaded(0, "raziel"),
            RenderClass::Unclassified,
            "the fallback is exact, not a prefix match"
        );
        assert_eq!(
            classify_loaded(0, "raziel__player"),
            RenderClass::Unclassified,
            "a longer name is not an exact row"
        );
    }

    #[test]
    fn a_real_shaped_skinner_classifies_monster_by_bit_19_not_by_name() {
        assert_eq!(
            classify_loaded(super::CLASS_FLAG_MONSTER, "skinner_"),
            RenderClass::Monster
        );
        assert_eq!(
            classify_loaded(0, "skinner_"),
            RenderClass::Unclassified,
            "without the bit the padded actor name never matches the table"
        );
    }
}

//! Render class table and the three per-object behaviour hooks.
//!
//! The class is a renderer/prototype category, not a spawn rule: it is selected
//! from the prototype-name prefix and installs up to three function pointers on
//! the live object (event, per-frame tick, query). A missing slot (`None`) is a
//! first-class opt-out, exactly like the original's null hook, and never an
//! error.
//!
//! The table is data: [`CLASS_TABLE`] is a constant array and [`classify`] is a
//! linear prefix lookup, so adding a class is a row, not another match arm.

use crate::engine::clock::ObjectTick;
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

/// Placeholder payloads for the hook signatures. The frame pipeline (T8) fills
/// their bodies; they exist now so the class table can carry concrete function
/// pointers without inventing AI state.
#[allow(dead_code)]
pub struct Event;
#[allow(dead_code)]
pub struct Query;
#[allow(dead_code)]
pub struct QueryAnswer;

/// Event/message entry (`+0x1A0`).
pub type EventHook = fn(&mut World, LiveId, &Event);
/// Per-frame tick (`+0x1A8`).
///
/// The live arena is passed explicitly because an actor's per-object runtime
/// state lives on its [`crate::engine::live::LiveObject`], not on the authored
/// [`World`]. The original reads and writes the same per-object block from the
/// tick hook; keeping the two stores separate is the engine's existing world /
/// live split.
pub type TickHook = fn(&mut World, &mut LiveArena, LiveId, ObjectTick);
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
fn actor_runtime_tick(_world: &mut World, live: &mut LiveArena, id: LiveId, tick: ObjectTick) {
    let Ok(object) = live.get_mut(id) else {
        return;
    };
    if let Some(actor) = object.actor.as_mut() {
        actor.advance(tick);
    }
}

fn raziel_tick(world: &mut World, live: &mut LiveArena, id: LiveId, tick: ObjectTick) {
    actor_runtime_tick(world, live, id, tick);
}

fn monster_tick(world: &mut World, live: &mut LiveArena, id: LiveId, tick: ObjectTick) {
    actor_runtime_tick(world, live, id, tick);
}

/// The six documented renderer prefixes.
///
/// `raziel__` and `monster_` opt into the generic tick; `sreavr__`, `glphicon`,
/// `physical` and `particle` leave it null. No class carries an event or query
/// hook in this milestone. Both installed tick bodies share the actor-carriage
/// body: the per-class behaviour (player interaction, monster locomotion) is not
/// implemented, but the dispatch and the carrier are real.
pub const CLASS_TABLE: [ClassEntry; 6] = [
    ClassEntry {
        prefix: "raziel__",
        class: RenderClass::Raziel,
        hooks: Hooks {
            event: None,
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
            event: None,
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

/// Render class for a prototype name; an unknown prefix is
/// [`RenderClass::Unclassified`], never a panic.
pub fn classify(name: &str) -> RenderClass {
    classify_entry(name).map_or(RenderClass::Unclassified, |entry| entry.class)
}

#[cfg(test)]
mod tests {
    use super::{CLASS_TABLE, ClassEntry, RenderClass, classify, classify_entry};

    #[test]
    fn class_table_covers_every_prefix_and_declares_every_slot() {
        assert_eq!(CLASS_TABLE.len(), 6, "the six documented prefixes");

        let cases: [(&str, RenderClass, bool); 6] = [
            ("raziel__", RenderClass::Raziel, true),
            ("sreavr__", RenderClass::Sreavr, false),
            ("glphicon", RenderClass::GlyphIcon, false),
            ("physical", RenderClass::Physical, false),
            ("monster_", RenderClass::Monster, true),
            ("particle", RenderClass::Particle, false),
        ];
        for (prefix, class, has_tick) in cases {
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
            assert!(entry.hooks.event.is_none(), "event slot for '{prefix}'");
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
}

//! Name-keyed spawn policy.
//!
//! The original partitions object behaviour by **name lists**, not by the
//! archetype code or the render class. [`EngineNameTable`] owns the five lists
//! recorded in `docs/notes/object-behaviour.md`:
//!
//! | List | Count | Admission |
//! |---|---:|---|
//! | always-spawn system objects | 10 | [`Admission::System`] |
//! | actor roster | 10 | [`Admission::Actor`] (5 overlap boss/special) |
//! | boss/special | 8 | [`Admission::BossSpecial`] |
//! | E3 asset overrides | 22 | [`Admission::PlacedModel`] + E3 path tree |
//! | marker-only | 3 | [`Admission::MarkerOnly`] |
//!
//! The lists overlap: `wallcr`, `aluka`, `ronin`, `sluagh` and `vwraith` are in
//! both the roster and the boss/special set. Boss/special wins, because the
//! special flag is the load-time role recorded for those names. Membership is
//! **ASCII case-insensitive**, matching the level name resolver in
//! [`crate::level::resolve_unit_entry`] and the spawn-point lookup.
//!
//! This module answers only "what do I do with this placed name". It reads the
//! five lists, and it exposes **no actor methods**: no AI, no state, no
//! animation, no asset loading, no tracker. Those seams live in later slices.
//!
//! `Admission::Inert` is part of the documented admission vocabulary but no T6
//! list produces it; every name outside the five lists is a
//! [`Admission::PlacedModel`].

use crate::engine::placed::PlacedObject;

/// Ten system objects spawned unconditionally at level initialization, in the
/// documented order. The order is part of the table contract.
const ALWAYS_SPAWN: [&str; 10] = [
    "raziel", "paths", "glphicon", "sreavr", "soul", "force", "particle", "healths", "eaggot",
    "eaggots",
];

/// Ten-name actor roster. It gates spawning under the "no monsters" debug flag;
/// it is a management key, not an AI key.
const ACTOR_ROSTER: [&str; 10] = [
    "skinner", "morlock", "wallcr", "ronin", "aluka", "sluagh", "vwraith", "vlgra", "vlgrb",
    "vlgrc",
];

/// Eight-name boss/special set. Loading one of these sets the special flag.
const BOSS_SPECIAL: [&str; 8] = [
    "walbos", "alklens", "astat", "wallcr", "aluka", "ronin", "sluagh", "vwraith",
];

/// 22-name E3 override set. Membership switches the asset tree only; it does
/// not change admission.
const E3_OVERRIDES: [&str; 22] = [
    "tripod", "floor", "flor", "flr", "sfxworm", "switch", "torcha", "sfxpipe", "sfxgear",
    "sfxvort", "stair", "sfxfalb", "sfxwinb", "sfxwinc", "fire", "lantern", "sfxfall", "sfxwood",
    "torchd", "coffin", "cptrcha", "sfxrush",
];

/// Names with no model container. They are deliberately not tracked.
const MARKER_ONLY: [&str; 3] = ["campath", "marker", "splob"];

/// What to do with one placed name.
///
/// The discriminants are stable bytes for the classification-table gate. Never
/// renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Admission {
    /// Always-spawned system object (player, path support, renderer/effect
    /// helpers, pickup/effect managers).
    System = 0,
    /// Actor roster member.
    Actor = 1,
    /// Boss/special member. Wins over the roster for the five overlapping names.
    BossSpecial = 2,
    /// Ordinary placed model with no special role.
    PlacedModel = 3,
    /// Marker with no model container; never tracked.
    MarkerOnly = 4,
    /// Reserved for later inert placements. No T6 list produces it.
    Inert = 5,
}

impl Admission {
    /// Stable byte used by the classification-table gate.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// The exact list sizes, so a removal or an addition is visible without reading
/// the private arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameCounts {
    pub always_spawn: usize,
    pub actor_roster: usize,
    pub boss_special: usize,
    pub e3_overrides: usize,
    pub marker_only: usize,
}

/// The five documented name lists, as immutable data.
///
/// This is the only production reader of the lists. It is a plain value with no
/// hooks and no actor-facing API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineNameTable {
    always_spawn: &'static [&'static str],
    actor_roster: &'static [&'static str],
    boss_special: &'static [&'static str],
    e3_overrides: &'static [&'static str],
    marker_only: &'static [&'static str],
}

impl EngineNameTable {
    /// The documented table.
    pub const fn documented() -> Self {
        Self {
            always_spawn: &ALWAYS_SPAWN,
            actor_roster: &ACTOR_ROSTER,
            boss_special: &BOSS_SPECIAL,
            e3_overrides: &E3_OVERRIDES,
            marker_only: &MARKER_ONLY,
        }
    }

    /// Exact list sizes.
    pub const fn counts(&self) -> NameCounts {
        NameCounts {
            always_spawn: self.always_spawn.len(),
            actor_roster: self.actor_roster.len(),
            boss_special: self.boss_special.len(),
            e3_overrides: self.e3_overrides.len(),
            marker_only: self.marker_only.len(),
        }
    }

    /// Classify a raw name.
    ///
    /// Precedence: system, boss/special, actor roster, marker-only, placed
    /// model. Boss/special before the roster is deliberate: the five shared
    /// names take the special role.
    pub fn admission_for_name(&self, name: &str) -> Admission {
        if contains_ignore_ascii_case(self.always_spawn, name) {
            Admission::System
        } else if contains_ignore_ascii_case(self.boss_special, name) {
            Admission::BossSpecial
        } else if contains_ignore_ascii_case(self.actor_roster, name) {
            Admission::Actor
        } else if contains_ignore_ascii_case(self.marker_only, name) {
            Admission::MarkerOnly
        } else {
            Admission::PlacedModel
        }
    }

    /// Classify one placed record by its authored name.
    pub fn admit(&self, placed: &PlacedObject) -> Admission {
        self.admission_for_name(&placed.name)
    }
}

impl Default for EngineNameTable {
    fn default() -> Self {
        Self::documented()
    }
}

fn contains_ignore_ascii_case(list: &[&str], name: &str) -> bool {
    list.iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::{
        ACTOR_ROSTER, ALWAYS_SPAWN, Admission, BOSS_SPECIAL, E3_OVERRIDES, EngineNameTable,
        MARKER_ONLY, NameCounts,
    };
    use crate::engine::placed::PlacedObject;

    fn placed(name: &str) -> PlacedObject {
        PlacedObject {
            name: name.to_string(),
            position: [0, 0, 0],
            object_type: 0,
            archetype: 0,
            flags: 0,
            instance_id: 0,
        }
    }

    #[test]
    fn the_five_lists_keep_the_documented_counts() {
        assert_eq!(
            EngineNameTable::documented().counts(),
            NameCounts {
                always_spawn: 10,
                actor_roster: 10,
                boss_special: 8,
                e3_overrides: 22,
                marker_only: 3,
            }
        );
    }

    #[test]
    fn always_spawn_names_classify_as_system_in_documented_order() {
        assert_eq!(
            ALWAYS_SPAWN,
            [
                "raziel", "paths", "glphicon", "sreavr", "soul", "force", "particle", "healths",
                "eaggot", "eaggots",
            ]
        );
        let table = EngineNameTable::documented();
        for name in ALWAYS_SPAWN {
            assert_eq!(table.admission_for_name(name), Admission::System, "{name}");
            assert_eq!(
                table.admit(&placed(name)),
                Admission::System,
                "{name} via admit"
            );
        }
    }

    #[test]
    fn actor_roster_names_classify_exactly() {
        assert_eq!(
            ACTOR_ROSTER,
            [
                "skinner", "morlock", "wallcr", "ronin", "aluka", "sluagh", "vwraith", "vlgra",
                "vlgrb", "vlgrc",
            ]
        );
        let expected = [
            ("skinner", Admission::Actor),
            ("morlock", Admission::Actor),
            ("wallcr", Admission::BossSpecial),
            ("ronin", Admission::BossSpecial),
            ("aluka", Admission::BossSpecial),
            ("sluagh", Admission::BossSpecial),
            ("vwraith", Admission::BossSpecial),
            ("vlgra", Admission::Actor),
            ("vlgrb", Admission::Actor),
            ("vlgrc", Admission::Actor),
        ];
        let table = EngineNameTable::documented();
        for (name, admission) in expected {
            assert_eq!(table.admission_for_name(name), admission, "{name}");
        }
    }

    #[test]
    fn boss_special_names_classify_as_boss_special() {
        assert_eq!(
            BOSS_SPECIAL,
            [
                "walbos", "alklens", "astat", "wallcr", "aluka", "ronin", "sluagh", "vwraith",
            ]
        );
        let table = EngineNameTable::documented();
        for name in BOSS_SPECIAL {
            assert_eq!(
                table.admission_for_name(name),
                Admission::BossSpecial,
                "{name}"
            );
        }
    }

    #[test]
    fn e3_override_names_are_placed_models() {
        assert_eq!(E3_OVERRIDES.len(), 22);
        let table = EngineNameTable::documented();
        for name in E3_OVERRIDES {
            assert_eq!(
                table.admission_for_name(name),
                Admission::PlacedModel,
                "{name}"
            );
        }
    }

    #[test]
    fn marker_only_names_classify_as_marker_only() {
        assert_eq!(MARKER_ONLY, ["campath", "marker", "splob"]);
        let table = EngineNameTable::documented();
        for name in MARKER_ONLY {
            assert_eq!(
                table.admission_for_name(name),
                Admission::MarkerOnly,
                "{name}"
            );
        }
    }

    #[test]
    fn unknown_name_is_a_placed_model() {
        let table = EngineNameTable::documented();
        // `warpg`/`wrpface` are load-time warp/link markers, not spawner
        // markers, so they are outside this three-name list.
        for name in [
            "", "under1", "raziel__", "splo", "MORLOCK_", "warpg", "wrpface",
        ] {
            assert_eq!(
                table.admission_for_name(name),
                Admission::PlacedModel,
                "{name}"
            );
        }
    }

    #[test]
    fn list_membership_is_ascii_case_insensitive() {
        let table = EngineNameTable::documented();
        assert_eq!(table.admission_for_name("RAZIEL"), Admission::System);
        assert_eq!(table.admission_for_name("PaThS"), Admission::System);
        assert_eq!(table.admission_for_name("Morlock"), Admission::Actor);
        assert_eq!(table.admission_for_name("WALLCR"), Admission::BossSpecial);
        assert_eq!(table.admission_for_name("CAMpaTH"), Admission::MarkerOnly);
        assert_eq!(table.admission_for_name("TRIPOD"), Admission::PlacedModel);
        assert_eq!(
            table.admission_for_name("no-such-name"),
            Admission::PlacedModel
        );
    }
}

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
//!
//! [`SpawnPolicy`] is the seam. It answers three questions only: admission,
//! load-complete role and asset paths. It exposes no actor method and performs
//! no asset loading; the tracker and the AI module never call it.
//!
//! The four exact load-complete literals (`MORLOCK`, `RONINBSS`, `ALUKABSS`,
//! `SKINBOS`) are compared **including case**, because the evidence records
//! `MORLOCK` as exact upper-case. This is deliberately narrower than the
//! case-insensitive membership of the five lists: `morlock` (the roster
//! spelling) takes the generic load path, while `MORLOCK` takes the actor-flag
//! role.

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

/// Load-complete special: sets the actor flag and adjusts the spawn counter.
/// Compared exactly, including case.
const LOAD_ACTOR_FLAG: [&str; 1] = ["MORLOCK"];

/// Load-complete specials that adjust the same spawn counter. Compared exactly,
/// including case.
const LOAD_SPAWN_COUNTER: [&str; 3] = ["RONINBSS", "ALUKABSS", "SKINBOS"];

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

/// What the load-complete callback does with a name.
///
/// This is an engine role, not an AI hook: it only records which counter or
/// flag the original adjusts when a model finishes loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadRole {
    /// Model + skeleton + optional sound + animation.
    Generic = 0,
    /// Adjust the spawn counter (`RONINBSS`, `ALUKABSS`, `SKINBOS`).
    SpawnCounter = 1,
    /// Set the actor flag and adjust the spawn counter (`MORLOCK`).
    ActorFlag = 2,
    /// Set the special flag (boss/special list).
    SpecialFlag = 3,
}

impl LoadRole {
    /// Stable byte used by the classification-table gate.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Which object tree the asset paths come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AssetTree {
    /// `\kain2\object`.
    Standard = 0,
    /// `\kain2\object_e3`, used by the 22-name override list.
    E3 = 1,
}

/// The three paths the generic loader builds from a name.
///
/// `tree: None` means the name has no model container (the marker-only list).
/// The name is echoed exactly as supplied; case normalisation belongs to the
/// archive layer, not to the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPaths {
    pub name: String,
    pub tree: Option<AssetTree>,
}

impl AssetPaths {
    /// Paths in the standard object tree.
    pub fn standard(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tree: Some(AssetTree::Standard),
        }
    }

    /// Paths in the E3 override tree.
    pub fn e3(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tree: Some(AssetTree::E3),
        }
    }

    /// No container: the marker-only names.
    pub fn none(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tree: None,
        }
    }

    /// Whether this name has any model container.
    pub fn has_model_container(&self) -> bool {
        self.tree.is_some()
    }

    fn root(&self) -> Option<&'static str> {
        match self.tree {
            Some(AssetTree::Standard) => Some(r"\kain2\object"),
            Some(AssetTree::E3) => Some(r"\kain2\object_e3"),
            None => None,
        }
    }

    /// `\kain2\object\<name>.sra`.
    pub fn skeleton(&self) -> Option<String> {
        self.root().map(|root| {
            let name = &self.name;
            format!(r"{root}\{name}.sra")
        })
    }

    /// `\kain2\object\<name>\<name>.crm`.
    pub fn mesh(&self) -> Option<String> {
        self.root().map(|root| {
            let name = &self.name;
            format!(r"{root}\{name}\{name}.crm")
        })
    }

    /// `\kain2\object\<name>\<name>.pcm`.
    pub fn model(&self) -> Option<String> {
        self.root().map(|root| {
            let name = &self.name;
            format!(r"{root}\{name}\{name}.pcm")
        })
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

    /// Load-complete role for a name.
    ///
    /// The four exact literals are checked first and case-sensitively. The
    /// boss/special list is then checked case-insensitively; every other name
    /// takes the generic path.
    pub fn load_role_for_name(&self, name: &str) -> LoadRole {
        if contains_exact(&LOAD_ACTOR_FLAG, name) {
            LoadRole::ActorFlag
        } else if contains_exact(&LOAD_SPAWN_COUNTER, name) {
            LoadRole::SpawnCounter
        } else if contains_ignore_ascii_case(self.boss_special, name) {
            LoadRole::SpecialFlag
        } else {
            LoadRole::Generic
        }
    }

    /// Asset paths for a name.
    ///
    /// Marker-only names have no container. The 22 E3 names switch tree and
    /// keep their normal admission; every other name uses the standard tree.
    pub fn asset_paths_for_name(&self, name: &str) -> AssetPaths {
        if contains_ignore_ascii_case(self.marker_only, name) {
            AssetPaths::none(name)
        } else if contains_ignore_ascii_case(self.e3_overrides, name) {
            AssetPaths::e3(name)
        } else {
            AssetPaths::standard(name)
        }
    }
}

/// The name-keyed spawn seam.
///
/// Exactly three questions. No actor method, no AI, no state, no animation and
/// no asset loading. The implementation is [`EngineNameTable`]; the tracker and
/// the AI module never call this trait.
pub trait SpawnPolicy {
    /// What to do with one placed record.
    fn admit(&self, placed: &PlacedObject) -> Admission;
    /// What the load-complete callback does with a name.
    fn load_complete(&self, name: &str) -> LoadRole;
    /// The three asset paths the loader builds from a name.
    fn asset_paths(&self, name: &str) -> AssetPaths;
}

impl SpawnPolicy for EngineNameTable {
    fn admit(&self, placed: &PlacedObject) -> Admission {
        self.admission_for_name(&placed.name)
    }

    fn load_complete(&self, name: &str) -> LoadRole {
        self.load_role_for_name(name)
    }

    fn asset_paths(&self, name: &str) -> AssetPaths {
        self.asset_paths_for_name(name)
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

fn contains_exact(list: &[&str], name: &str) -> bool {
    list.iter().any(|candidate| *candidate == name)
}

#[cfg(test)]
mod tests {
    use super::{
        ACTOR_ROSTER, ALWAYS_SPAWN, Admission, AssetTree, BOSS_SPECIAL, E3_OVERRIDES,
        EngineNameTable, LOAD_ACTOR_FLAG, LOAD_SPAWN_COUNTER, LoadRole, MARKER_ONLY, NameCounts,
        SpawnPolicy,
    };
    use crate::engine::determinism::StateHasher;
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

    #[test]
    fn load_complete_roles_follow_the_documented_partition() {
        assert_eq!(LOAD_ACTOR_FLAG, ["MORLOCK"]);
        assert_eq!(LOAD_SPAWN_COUNTER, ["RONINBSS", "ALUKABSS", "SKINBOS"]);

        let table = EngineNameTable::documented();
        assert_eq!(table.load_role_for_name("MORLOCK"), LoadRole::ActorFlag);
        for name in LOAD_SPAWN_COUNTER {
            assert_eq!(
                table.load_role_for_name(name),
                LoadRole::SpawnCounter,
                "{name}"
            );
        }
        for name in BOSS_SPECIAL {
            assert_eq!(
                table.load_role_for_name(name),
                LoadRole::SpecialFlag,
                "{name}"
            );
        }
        for name in ["raziel", "skinner", "campath", "tripod", "under1"] {
            assert_eq!(table.load_role_for_name(name), LoadRole::Generic, "{name}");
        }
    }

    #[test]
    fn load_complete_exact_literals_are_case_sensitive() {
        let table = EngineNameTable::documented();
        assert_eq!(table.load_complete("MORLOCK"), LoadRole::ActorFlag);
        assert_eq!(table.load_complete("morlock"), LoadRole::Generic);
        assert_eq!(table.load_complete("RONINBSS"), LoadRole::SpawnCounter);
        assert_eq!(table.load_complete("roninbss"), LoadRole::Generic);
        assert_eq!(table.load_complete("ALUKABSS"), LoadRole::SpawnCounter);
        assert_eq!(table.load_complete("alukabss"), LoadRole::Generic);
        assert_eq!(table.load_complete("SKINBOS"), LoadRole::SpawnCounter);
        assert_eq!(table.load_complete("skinbos"), LoadRole::Generic);
        // The boss/special list is case-insensitive, unlike the exact literals.
        assert_eq!(table.load_complete("WALLCR"), LoadRole::SpecialFlag);
    }

    #[test]
    fn asset_paths_match_the_documented_layout() {
        let table = EngineNameTable::documented();

        let raziel = table.asset_paths("raziel");
        assert_eq!(raziel.tree, Some(AssetTree::Standard));
        assert_eq!(
            raziel.skeleton().as_deref(),
            Some(r"\kain2\object\raziel.sra")
        );
        assert_eq!(
            raziel.mesh().as_deref(),
            Some(r"\kain2\object\raziel\raziel.crm")
        );
        assert_eq!(
            raziel.model().as_deref(),
            Some(r"\kain2\object\raziel\raziel.pcm")
        );
        assert!(raziel.has_model_container());

        let tripod = table.asset_paths("tripod");
        assert_eq!(tripod.tree, Some(AssetTree::E3));
        assert_eq!(
            tripod.skeleton().as_deref(),
            Some(r"\kain2\object_e3\tripod.sra")
        );
        assert_eq!(
            tripod.mesh().as_deref(),
            Some(r"\kain2\object_e3\tripod\tripod.crm")
        );
        assert_eq!(
            tripod.model().as_deref(),
            Some(r"\kain2\object_e3\tripod\tripod.pcm")
        );

        let marker = table.asset_paths("marker");
        assert_eq!(marker.tree, None);
        assert_eq!(marker.skeleton(), None);
        assert_eq!(marker.mesh(), None);
        assert_eq!(marker.model(), None);
        assert!(!marker.has_model_container());

        let unknown = table.asset_paths("under1");
        assert_eq!(unknown.tree, Some(AssetTree::Standard));
    }

    #[test]
    fn asset_paths_switch_tree_case_insensitively_and_echo_the_name() {
        let table = EngineNameTable::documented();

        let e3 = table.asset_paths("TRIPOD");
        assert_eq!(e3.tree, Some(AssetTree::E3));
        assert_eq!(
            e3.model().as_deref(),
            Some(r"\kain2\object_e3\TRIPOD\TRIPOD.pcm")
        );

        let standard = table.asset_paths("Raziel");
        assert_eq!(standard.tree, Some(AssetTree::Standard));
        assert_eq!(
            standard.model().as_deref(),
            Some(r"\kain2\object\Raziel\Raziel.pcm")
        );

        let marker = table.asset_paths("CAMpaTH");
        assert_eq!(marker.tree, None);
    }

    #[test]
    fn trait_object_answers_the_three_seam_questions() {
        let policy: &dyn SpawnPolicy = &EngineNameTable::documented();
        assert_eq!(policy.admit(&placed("raziel")), Admission::System);
        assert_eq!(policy.load_complete("MORLOCK"), LoadRole::ActorFlag);
        assert_eq!(policy.asset_paths("tripod").tree, Some(AssetTree::E3));
        assert_eq!(policy.asset_paths("marker").tree, None);
    }

    // Guards the classification table itself: for each of the five lists in
    // documented order, the list id, its exact count, and per name the name
    // bytes, the admission code, the load-role code and the asset-tree code.
    // Renaming, removing, adding or reordering any entry moves the digest.
    const GOLDEN_CLASSIFICATION_HASH: u64 = 0x9bec_a441_2d97_8281;

    fn asset_tree_code(tree: Option<AssetTree>) -> u8 {
        match tree {
            Some(AssetTree::Standard) => 0,
            Some(AssetTree::E3) => 1,
            None => 2,
        }
    }

    fn classification_digest() -> u64 {
        let table = EngineNameTable::documented();
        let lists: [(u8, &'static [&'static str]); 5] = [
            (0, &ALWAYS_SPAWN),
            (1, &ACTOR_ROSTER),
            (2, &BOSS_SPECIAL),
            (3, &E3_OVERRIDES),
            (4, &MARKER_ONLY),
        ];
        let mut hasher = StateHasher::new();
        for (list_id, names) in lists {
            hasher.write_u8(list_id);
            hasher.write_u32(names.len() as u32);
            for name in names {
                hasher.write_bytes(name.as_bytes());
                hasher.write_u8(table.admission_for_name(name).code());
                hasher.write_u8(table.load_role_for_name(name).code());
                hasher.write_u8(asset_tree_code(table.asset_paths_for_name(name).tree));
            }
        }
        hasher.finish()
    }

    #[test]
    fn classification_table_matches_the_golden() {
        let observed = classification_digest();
        assert_eq!(
            observed, GOLDEN_CLASSIFICATION_HASH,
            "name-table classification moved: list order, count, name bytes, admission, load role or asset tree changed (observed {observed:#018x})"
        );
    }
}

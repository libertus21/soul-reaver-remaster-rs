//! Level-transition data: the target, the reason, the console line and the
//! placement rule for a world swap.
//!
//! The swap itself is [`crate::engine::play::swap_unit`], the single seam every
//! entry point calls. This module owns only the pure pieces: the target unit as
//! a value, the trigger reason token, the schedule entry an argv flag parses to,
//! and the deterministic rule that picks where the player appears.
//!
//! # The object-driven trigger is UNKNOWN
//!
//! `warpg` objects exist in only nine units (`cathy58 cathy62 under3 city6
//! oracle1 aluka43 out10 chrono17 stone15`), two per unit, and none are reachable
//! from the game's debug level menu. The decoded unit link graph (855 edges)
//! carries no per-object binding, so which `warpg` instance points at which unit
//! is **UNKNOWN**. Nothing here guesses that binding: a warp target is always
//! supplied explicitly by a flag (today) or by a real trigger (later), and the
//! future trigger would call the same seam with the unit it resolved.

use std::path::Path;

use crate::engine::frame::PLAYER_NAME;
use crate::engine::world::World;
use crate::level;

/// Why a swap happened, as the console contract spells it.
///
/// `SpawnMissing` takes precedence over the trigger token: when the target unit
/// places no `raziel`, the player used the deterministic fallback and the line
/// records that fact instead of the flag that requested the swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpReason {
    /// An explicit `--warp-to <UNIT>` after the first completed tick.
    Flag,
    /// The windowed `--warp-cycle` list advanced by a `G` key press.
    Key,
    /// A `--warp-at <TICK>:<UNIT>` schedule entry applied at the tick start.
    Script,
    /// The target places no `raziel`, so the fallback placement was used.
    SpawnMissing,
}

impl WarpReason {
    /// The exact token the `warp` console line prints.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Key => "key",
            Self::Script => "script",
            Self::SpawnMissing => "spawn-missing",
        }
    }
}

/// The result of one swap, including the `placed`/`admitted` counters the
/// headless segment report needs.
///
/// The console contract is [`WarpOutcome::line`]; the counters are the same
/// one-time boot counts a fresh run of the target unit reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpOutcome {
    /// Unit the swap left.
    pub from: String,
    /// Unit the swap entered, as the target declares its own name.
    pub to: String,
    /// Where the player was placed, in integer world units.
    pub player: [i32; 3],
    /// The effective reason token (see [`WarpReason`]).
    pub reason: WarpReason,
    pub placed: usize,
    pub admitted: usize,
    pub skipped_markers: usize,
    pub inert: usize,
    pub unresolved: usize,
}

impl WarpOutcome {
    /// The exact one-line console contract for a swap.
    pub fn line(&self) -> String {
        format!(
            "warp from={} to={} player={},{},{} reason={}",
            self.from,
            self.to,
            self.player[0],
            self.player[1],
            self.player[2],
            self.reason.as_str(),
        )
    }
}

/// One parsed `--warp-at <TICK>:<UNIT>` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpAt {
    pub tick: u64,
    pub unit: String,
}

impl WarpAt {
    /// Parse `TICK:UNIT`; every refusal is a message the typed option error
    /// wraps, never a silent default.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let (raw_tick, raw_unit) = spec
            .split_once(':')
            .ok_or_else(|| format!("expected TICK:UNIT, got '{spec}'"))?;
        let unit = raw_unit.trim();
        if unit.is_empty() {
            return Err(format!("'{spec}' names no unit after ':'"));
        }
        let tick = raw_tick
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("'{}' is not a non-negative tick", raw_tick.trim()))?;
        Ok(Self {
            tick,
            unit: unit.to_string(),
        })
    }
}

/// One swap scheduled at a global tick, sorted and applied at that tick's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledWarp {
    pub tick: u64,
    pub unit: String,
    pub reason: WarpReason,
}

/// Parse a comma-separated `--warp-cycle` list into its units.
///
/// An empty entry is refused rather than skipped: a trailing comma would
/// otherwise silently drop a room from the browse list.
pub fn parse_cycle(spec: &str) -> Result<Vec<String>, String> {
    let mut units = Vec::new();
    for part in spec.split(',') {
        let unit = part.trim();
        if unit.is_empty() {
            return Err(format!("'{spec}' contains an empty unit name"));
        }
        units.push(unit.to_string());
    }
    Ok(units)
}

/// The deterministic player placement for a freshly loaded world.
///
/// The rule is:
/// 1. the first `raziel` placed record's position, when the unit places one;
/// 2. otherwise the first placed record's position in placed-array order;
/// 3. otherwise world origin `[0, 0, 0]`.
///
/// The returned flag says whether rule 1 applied. A `false` flag is what turns
/// the console reason into [`WarpReason::SpawnMissing`].
pub fn placement(world: &World) -> ([i32; 3], bool) {
    if let Some(slot) = world.placed.by_name(PLAYER_NAME).first().copied()
        && let Some(record) = world.placed.record(slot)
    {
        return (
            [
                i32::from(record.position[0]),
                i32::from(record.position[1]),
                i32::from(record.position[2]),
            ],
            true,
        );
    }
    if let Some(record) = world.placed.records_for_boot().first() {
        return (
            [
                i32::from(record.position[0]),
                i32::from(record.position[1]),
                i32::from(record.position[2]),
            ],
            false,
        );
    }
    ([0, 0, 0], false)
}

/// A few discovered unit names, for an unknown-target error.
///
/// The list is the archive scan's order, capped so the error stays one line.
/// A failed scan yields no examples instead of failing the error path.
pub fn valid_examples(bigfile: &Path) -> Vec<String> {
    level::scan_archive_levels(bigfile)
        .map(|scan| {
            scan.units
                .into_iter()
                .take(5)
                .map(|unit| unit.name)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{WarpAt, WarpOutcome, WarpReason, parse_cycle, placement};
    use crate::engine::world::World;
    use crate::level::{ObjectRecord, ObjectReport};

    fn record(name: &str, position: [i16; 3]) -> ObjectRecord {
        ObjectRecord {
            name: name.to_string(),
            position,
            object_type: 4,
            archetype: 0,
            instance_id: 1,
            flags: 0,
        }
    }

    fn world(objects: Vec<ObjectRecord>) -> World {
        World::from_report(
            "under1".to_string(),
            1,
            &ObjectReport {
                room_offset: 0,
                count: objects.len() as u32,
                array_offset: 0,
                objects,
            },
        )
    }

    #[test]
    fn reason_tokens_match_the_console_contract() {
        assert_eq!(WarpReason::Flag.as_str(), "flag");
        assert_eq!(WarpReason::Key.as_str(), "key");
        assert_eq!(WarpReason::Script.as_str(), "script");
        assert_eq!(WarpReason::SpawnMissing.as_str(), "spawn-missing");
    }

    #[test]
    fn outcome_line_is_the_documented_shape() {
        let outcome = WarpOutcome {
            from: "under1".to_string(),
            to: "cathy28".to_string(),
            player: [10, -20, 30],
            reason: WarpReason::Script,
            placed: 1,
            admitted: 1,
            skipped_markers: 0,
            inert: 0,
            unresolved: 0,
        };
        assert_eq!(
            outcome.line(),
            "warp from=under1 to=cathy28 player=10,-20,30 reason=script"
        );
    }

    #[test]
    fn warp_at_parses_a_tick_and_a_unit() {
        assert_eq!(
            WarpAt::parse("60:cathy28"),
            Ok(WarpAt {
                tick: 60,
                unit: "cathy28".to_string()
            })
        );
        assert_eq!(
            WarpAt::parse(" 7 : under2 "),
            Ok(WarpAt {
                tick: 7,
                unit: "under2".to_string()
            })
        );
        assert!(WarpAt::parse("60").is_err(), "a missing colon is refused");
        assert!(WarpAt::parse("x:cathy28").is_err(), "a bad tick is refused");
        assert!(WarpAt::parse("60:").is_err(), "an empty unit is refused");
    }

    #[test]
    fn cycle_lists_split_and_keep_order() {
        assert_eq!(
            parse_cycle("under1,cathy28, skinnr4").expect("valid list"),
            vec![
                "under1".to_string(),
                "cathy28".to_string(),
                "skinnr4".to_string()
            ]
        );
        assert!(parse_cycle("under1,,cathy28").is_err(), "an empty entry is refused");
        assert!(parse_cycle("").is_err(), "an empty list is refused");
    }

    #[test]
    fn placement_prefers_the_authored_raziel_record() {
        let world = world(vec![
            record("capsule0", [50, 0, 0]),
            record("raziel", [1, 2, 3]),
        ]);
        assert_eq!(placement(&world), ([1, 2, 3], true));
    }

    #[test]
    fn placement_falls_back_to_the_first_record_then_origin() {
        let spawned = world(vec![record("capsule0", [50, 60, 70])]);
        assert_eq!(placement(&spawned), ([50, 60, 70], false));

        let empty = world(Vec::new());
        assert_eq!(placement(&empty), ([0, 0, 0], false));
    }
}

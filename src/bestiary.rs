//! Offline actor bestiary: one row per actor type, its `.SRA` clips and the
//! top **heuristic** idle/walk candidates, plus the units that place each type.
//!
//! This module is pure: it renders rows, classifies a type's animation source
//! and scores the known runtime fixtures. It never reads the game, never writes
//! a label store and never calls the classifier validated. The `source=heuristic`
//! header is the contract: a candidate is a clip to confirm, never a label.

use std::collections::BTreeMap;

use crate::clip_metrics::{ClipMetricsReport, LocomotionState};

/// Header line for every text table; a candidate is never presented as measured.
pub const HEADER: &str = "bestiary source=heuristic";

/// Why a type is or is not labelable offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The `.SRA` decoded a clip table with the model's bone count.
    Labelable,
    /// The `.SRA` exists but yields no packed frame table for that bone count.
    NoTable,
    /// No `.SRA` file resolved for the actor name.
    NoSra,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Labelable => "labelable",
            Self::NoTable => "no-table",
            Self::NoSra => "no-sra",
        }
    }
}

/// The pre-decoded animation source injected into [`Row::new`].
///
/// Keeping the decode outside the row builder lets the status and no-table
/// paths be unit-tested without a game file.
#[derive(Debug, Clone, PartialEq)]
pub enum SraOutcome {
    /// No file was resolved for the actor.
    Missing,
    /// Every clip decoded with the model's bone count.
    Decoded(ClipMetricsReport),
    /// The file resolved but no packed frame table decoded.
    NoTable,
}

/// A ranked clip candidate: file clip index and duration in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub clip: usize,
    pub duration_s: f64,
}

impl Candidate {
    fn text(self) -> String {
        format!("{}({:.3}s)", self.clip, self.duration_s)
    }
}

/// One actor type: identity, decode counts, top candidates and status.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub actor: String,
    pub entry: usize,
    pub bones: usize,
    pub sra: Option<String>,
    pub clips: usize,
    pub idle: Option<Candidate>,
    pub walk: Option<Candidate>,
    pub status: Status,
}

impl Row {
    /// Build one row from an actor identity and an injected animation outcome.
    ///
    /// `sra` is the resolved file name (`None` when nothing resolved). The
    /// status follows from the pair: no file is `no-sra`, a decoded report is
    /// `labelable`, and a file that did not decode is `no-table`.
    pub fn new(
        actor: impl Into<String>,
        entry: usize,
        bones: usize,
        sra: Option<&str>,
        outcome: &SraOutcome,
    ) -> Self {
        let status = match (sra, outcome) {
            (None, _) => Status::NoSra,
            (Some(_), SraOutcome::Decoded(_)) => Status::Labelable,
            (Some(_), _) => Status::NoTable,
        };
        let (clips, idle, walk) = match outcome {
            SraOutcome::Decoded(report) if status == Status::Labelable => (
                report.clips.len(),
                top(report, LocomotionState::Idle),
                top(report, LocomotionState::Walk),
            ),
            _ => (0, None, None),
        };
        Self {
            actor: actor.into(),
            entry,
            bones,
            sra: sra.map(str::to_string),
            clips,
            idle,
            walk,
            status,
        }
    }

    /// The documented one-line row shape.
    pub fn render(&self) -> String {
        format!(
            "actor={} entry={} bones={} sra={} clips={} idle={} walk={} status={}",
            self.actor,
            self.entry,
            self.bones,
            self.sra.as_deref().unwrap_or("-"),
            self.clips,
            self.idle.map_or_else(|| "-".to_string(), Candidate::text),
            self.walk.map_or_else(|| "-".to_string(), Candidate::text),
            self.status.label(),
        )
    }
}

fn top(report: &ClipMetricsReport, state: LocomotionState) -> Option<Candidate> {
    let clip = *report.candidates(state).first()?;
    let duration_s = report
        .clips
        .iter()
        .find(|motion| motion.index == clip)
        .map_or(0.0, |motion| motion.duration_s);
    Some(Candidate { clip, duration_s })
}

/// The header plus one row per actor type, in the caller's order.
pub fn table_text(rows: &[Row]) -> String {
    let mut text = String::from(HEADER);
    for row in rows {
        text.push('\n');
        text.push_str(&row.render());
    }
    text
}

/// Totals across every row, including how many units were scanned for sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    pub actors: usize,
    pub labelable: usize,
    pub no_table: usize,
    pub no_sra: usize,
    pub units_scanned: usize,
}

impl Summary {
    pub fn of(rows: &[Row], units_scanned: usize) -> Self {
        let mut summary = Self { actors: rows.len(), units_scanned, ..Self::default() };
        for row in rows {
            match row.status {
                Status::Labelable => summary.labelable += 1,
                Status::NoTable => summary.no_table += 1,
                Status::NoSra => summary.no_sra += 1,
            }
        }
        summary
    }

    pub fn render(&self) -> String {
        format!(
            "BESTIARY actors={} labelable={} no-table={} no-sra={} units_scanned={}",
            self.actors, self.labelable, self.no_table, self.no_sra, self.units_scanned
        )
    }
}

/// What the placement scan covered, so coverage is never implied.
pub fn scan_line(units_scanned: usize, units_failed: usize) -> String {
    format!(
        "bestiary-sites scope=archive-index source=placed units_scanned={units_scanned} units_failed={units_failed}"
    )
}

/// `actor=<name> units=<u1,u2,...>` rows in the caller's order; `units=-` when
/// no discovered unit places that type.
pub fn sites_text(order: &[String], sites: &BTreeMap<String, Vec<String>>) -> String {
    let mut lines = Vec::with_capacity(order.len());
    for actor in order {
        let units = sites.get(actor).map(|units| units.join(",")).unwrap_or_else(|| "-".to_string());
        lines.push(format!("actor={actor} units={units}"));
    }
    lines.join("\n")
}

/// The acceptance check for the two types with known fixtures.
///
/// `hunter` was confirmed visually (`idle=2`, `walk=7`); `sluagh` has a
/// runtime-motion idle (`8`) and a walk (`1`) that ranks fifth offline, so a
/// miss states the rank instead of claiming a pass.
pub fn check_text(reports: &BTreeMap<String, ClipMetricsReport>) -> String {
    let mut lines = Vec::with_capacity(2);
    for (actor, idle, walk) in [
        ("hunter", Some(2usize), Some(7usize)),
        ("sluagh", Some(8usize), Some(1usize)),
    ] {
        let Some(report) = reports.get(actor) else {
            lines.push(format!("BESTIARY CHECK {actor} unavailable"));
            continue;
        };
        lines.push(format!(
            "BESTIARY CHECK {actor} {}  {}",
            field("idle", idle, report, LocomotionState::Idle),
            field("walk", walk, report, LocomotionState::Walk),
        ));
    }
    lines.join("\n")
}

fn field(
    label: &str,
    expected: Option<usize>,
    report: &ClipMetricsReport,
    state: LocomotionState,
) -> String {
    let candidates = report.candidates(state);
    let top = candidates.first().copied();
    let Some(expected) = expected else {
        return format!("{label}=- expected=- SKIP");
    };
    if top == Some(expected) {
        return format!("{label}={expected} expected={expected} PASS");
    }
    let top_text = top.map_or_else(|| "-".to_string(), |clip| clip.to_string());
    let rank = match candidates.iter().position(|&clip| clip == expected) {
        Some(position) => format!("rank={}", position + 1),
        None => "not-a-candidate".to_string(),
    };
    format!("{label}={top_text} expected={expected} MISS {rank}")
}

/// One deterministic JSON document for `--json`. Hand-rolled so the tool adds
/// no dependency and stays byte-for-byte repeatable.
pub fn json_text(
    rows: &[Row],
    sites: &BTreeMap<String, Vec<String>>,
    summary: &Summary,
    checks: &[String],
) -> String {
    let actors: Vec<String> = rows
        .iter()
        .map(|row| {
            format!(
                "{{\"actor\":{},\"entry\":{},\"bones\":{},\"sra\":{},\"clips\":{},\"idle\":{},\"walk\":{},\"status\":{}}}",
                json_string(&row.actor),
                row.entry,
                row.bones,
                row.sra.as_deref().map_or_else(|| "null".to_string(), json_string),
                row.clips,
                candidate_json(row.idle),
                candidate_json(row.walk),
                json_string(row.status.label()),
            )
        })
        .collect();
    let sites: Vec<String> = rows
        .iter()
        .map(|row| {
            let units = sites.get(&row.actor).map(Vec::as_slice).unwrap_or(&[]);
            format!("{{\"actor\":{},\"units\":{}}}", json_string(&row.actor), json_strings(units))
        })
        .collect();
    format!(
        "{{\"source\":\"heuristic\",\"actors\":[{}],\"sites\":[{}],\"summary\":{{\"actors\":{},\"labelable\":{},\"no_table\":{},\"no_sra\":{},\"units_scanned\":{}}},\"check\":{}}}",
        actors.join(","),
        sites.join(","),
        summary.actors,
        summary.labelable,
        summary.no_table,
        summary.no_sra,
        summary.units_scanned,
        json_strings(checks),
    )
}

fn candidate_json(candidate: Option<Candidate>) -> String {
    candidate.map_or_else(|| "null".to_string(), |candidate| {
        format!(
            "{{\"clip\":{},\"duration_s\":{:.3}}}",
            candidate.clip, candidate.duration_s
        )
    })
}

fn json_strings(values: &[String]) -> String {
    let rendered: Vec<String> = values.iter().map(|value| json_string(value)).collect();
    format!("[{}]", rendered.join(","))
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip_metrics::{ClipMotion, RegionPlan};

    /// One clip whose root slot travels `hdisp` units horizontally.
    fn clip(index: usize, frames: usize, hdisp: f32) -> ClipMotion {
        let step = if frames > 1 { hdisp / (frames as f32 - 1.0) } else { 0.0 };
        let data: Vec<Vec<[f32; 12]>> = (0..frames)
            .map(|frame| {
                let mut lanes = [0.0f32; 12];
                lanes[8] = frame as f32 * step;
                vec![lanes]
            })
            .collect();
        ClipMotion::from_lanes(index, 100, &data, &RegionPlan::Proxy)
    }

    fn hunter_report() -> ClipMetricsReport {
        ClipMetricsReport::from_clips(vec![clip(2, 8, 0.0), clip(7, 8, 50.0)], RegionPlan::Proxy)
    }

    fn sluagh_report() -> ClipMetricsReport {
        // Idle clip 8 has no travel; the five walk clips order 17,2,3,4,1 by
        // horizontal travel, so the runtime walk `1` lands at rank 5.
        ClipMetricsReport::from_clips(
            vec![
                clip(8, 8, 0.0),
                clip(17, 8, 50.0),
                clip(2, 8, 40.0),
                clip(3, 8, 30.0),
                clip(4, 8, 20.0),
                clip(1, 8, 10.0),
            ],
            RegionPlan::Proxy,
        )
    }

    #[test]
    fn a_decoded_row_renders_the_documented_shape() {
        let report = hunter_report();
        assert_eq!(report.candidates(LocomotionState::Idle), &[2, 7]);
        assert_eq!(report.candidates(LocomotionState::Walk), &[7]);
        let row = Row::new("hunter", 231, 1, Some("HUNTER.SRA"), &SraOutcome::Decoded(report));
        assert_eq!(row.status, Status::Labelable);
        assert_eq!(row.clips, 2);
        assert_eq!(
            row.render(),
            "actor=hunter entry=231 bones=1 sra=HUNTER.SRA clips=2 idle=2(0.160s) walk=7(0.160s) status=labelable"
        );
    }

    #[test]
    fn status_separates_labelable_no_table_and_no_sra_and_the_summary_counts_them() {
        let labelable =
            Row::new("hunter", 1, 44, Some("HUNTER.SRA"), &SraOutcome::Decoded(hunter_report()));
        let no_table = Row::new("soul", 72, 2, Some("SOUL.SRA"), &SraOutcome::NoTable);
        let no_sra = Row::new("ghost", 9, 5, None, &SraOutcome::Missing);
        assert_eq!(labelable.status, Status::Labelable);
        assert_eq!(no_table.status, Status::NoTable);
        assert_eq!(no_sra.status, Status::NoSra);
        let rows = vec![labelable, no_table, no_sra];
        assert_eq!(
            Summary::of(&rows, 372).render(),
            "BESTIARY actors=3 labelable=1 no-table=1 no-sra=1 units_scanned=372"
        );
    }

    #[test]
    fn the_no_table_path_has_no_clips_or_candidates() {
        let row = Row::new("soul", 72, 2, Some("SOUL.SRA"), &SraOutcome::NoTable);
        assert_eq!(row.clips, 0);
        assert_eq!(row.idle, None);
        assert_eq!(row.walk, None);
        assert_eq!(
            row.render(),
            "actor=soul entry=72 bones=2 sra=SOUL.SRA clips=0 idle=- walk=- status=no-table"
        );
        let missing = Row::new("ghost", 9, 5, None, &SraOutcome::Missing);
        assert_eq!(
            missing.render(),
            "actor=ghost entry=9 bones=5 sra=- clips=0 idle=- walk=- status=no-sra"
        );
    }

    #[test]
    fn the_check_passes_hunter_and_states_the_sluagh_walk_rank() {
        let mut reports = BTreeMap::new();
        reports.insert("hunter".to_string(), hunter_report());
        reports.insert("sluagh".to_string(), sluagh_report());
        assert_eq!(
            check_text(&reports),
            "BESTIARY CHECK hunter idle=2 expected=2 PASS  walk=7 expected=7 PASS\nBESTIARY CHECK sluagh idle=8 expected=8 PASS  walk=17 expected=1 MISS rank=5"
        );
    }

    #[test]
    fn the_sites_table_is_ordered_and_marks_unplaced_types() {
        let order = vec!["hunter".to_string(), "soul".to_string()];
        let mut sites = BTreeMap::new();
        sites.insert("hunter".to_string(), vec!["cath1".to_string(), "cath2".to_string()]);
        assert_eq!(
            sites_text(&order, &sites),
            "actor=hunter units=cath1,cath2\nactor=soul units=-"
        );
    }

    #[test]
    fn json_is_one_parseable_document_with_escaped_strings() {
        let rows = vec![Row::new("soul", 72, 2, Some("SOUL.SRA"), &SraOutcome::NoTable)];
        let summary = Summary::of(&rows, 0);
        let json = json_text(&rows, &BTreeMap::new(), &summary, &["BESTIARY CHECK soul unavailable".to_string()]);
        assert!(json.starts_with("{\"source\":\"heuristic\",\"actors\":["));
        assert!(json.contains("\"status\":\"no-table\""));
        assert!(json.contains("\"sra\":\"SOUL.SRA\""));
        assert!(json.contains("\"idle\":null"));
        assert!(json.contains("\"check\":[\"BESTIARY CHECK soul unavailable\"]"));
        assert!(json.contains("\"units_scanned\":0"));
        assert!(json.ends_with('}'));
        assert!(json_string("a\"b\\c").contains("a\\\"b\\\\c"));
    }
}

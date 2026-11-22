//! Offline motion metrics for identifier-less `.SRA` clips.
//!
//! The packed tables carry no name, state byte or clip index, so a clip's
//! semantics must be inferred from how its bones move. This module decodes every
//! frame of every clip, measures root translation, angular velocity, periodicity
//! and per-region motion, and ranks a shortlist of candidates for five
//! locomotion states.
//!
//! It is a **measurement table and a ranking**, never ground truth. Every guess
//! is labelled `heuristic`; a state whose candidates cannot be separated is
//! left empty. The validation gate is documented in
//! `docs/notes/clip-metrics.md`: offline motion alone cannot give a clip its
//! runtime-selected identity, and this module says so instead of inventing one.

use crate::object::{bone_world_offsets, ObjectModel};
use crate::pose::BINARY_ANGLE_UNITS_PER_TURN;
use crate::sra::{SraAnimation, ANIMATION_UNITS_PER_SECOND};

/// The three region columns printed for every clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// Feet and legs: low rest height, not far from the body axis.
    Leg,
    /// Pelvis and torso: middle rest height, close to the body axis.
    Hip,
    /// Head and arms/hands: high rest height close to the axis, or any bone
    /// whose lateral rest offset is far from the axis.
    Upper,
}

impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Self::Leg => "leg",
            Self::Hip => "hip",
            Self::Upper => "upper",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Leg => 0,
            Self::Hip => 1,
            Self::Upper => 2,
        }
    }
}

/// One bone's rest geometry and derived region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneRegion {
    pub bone: usize,
    pub region: Region,
    /// Accumulated rest translation along the model's vertical axis (`+Z`).
    pub rest_z: f64,
    /// Horizontal distance from the body's vertical axis in the rest pose.
    pub lateral: f64,
}

/// Regions used by every clip in one file.
///
/// `Skeleton` assigns a region per bone from the archived model's rest offsets.
/// `Proxy` is the documented slot-third stand-in used when no model is loaded.
/// `Unavailable` means a model was supplied but its bone count disagrees with
/// the animation's slot count; the shares then fall back to the proxy and the
/// output says why.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionPlan {
    Skeleton {
        bones: Vec<BoneRegion>,
        low_z: f64,
        high_z: f64,
        lateral: f64,
    },
    Proxy,
    Unavailable { slots: usize, bones: usize },
}

impl RegionPlan {
    /// Derive regions from `model`'s rest skeleton for `slots` animation slots.
    ///
    /// The slot count and the model bone count must agree exactly; a mismatch is
    /// reported, never padded. The vertical bands are fractions of the bones'
    /// rest `Z` extent and the lateral threshold is the median horizontal
    /// radius, so every skeleton supplies its own scale and no absolute world
    /// unit is hardcoded.
    pub fn from_model(model: &ObjectModel, slots: usize) -> Self {
        let bones = model.bones.len();
        if bones != slots || bones == 0 {
            return Self::Unavailable { slots, bones };
        }
        let offsets = bone_world_offsets(&model.bones);
        let mut zs = Vec::with_capacity(bones);
        let mut laterals = Vec::with_capacity(bones);
        let mut rest = Vec::with_capacity(bones);
        for offset in &offsets {
            let rest_z = f64::from(offset[2]);
            let x = f64::from(offset[0]);
            let y = f64::from(offset[1]);
            let lateral = (x * x + y * y).sqrt();
            zs.push(rest_z);
            laterals.push(lateral);
            rest.push((rest_z, lateral));
        }
        zs.sort_by(f64::total_cmp);
        laterals.sort_by(f64::total_cmp);
        let (low_z, high_z) = match (zs.first().copied(), zs.last().copied()) {
            (Some(min), Some(max)) if max - min > f64::EPSILON => {
                (min + (max - min) * LOW_BAND, min + (max - min) * HIGH_BAND)
            }
            _ => (f64::NEG_INFINITY, f64::INFINITY),
        };
        let median_lateral = quantile(&laterals, 0.5);
        let lateral = if median_lateral > f64::EPSILON {
            median_lateral * LATERAL_MARGIN
        } else {
            f64::INFINITY
        };
        let bones = rest
            .into_iter()
            .enumerate()
            .map(|(bone, (rest_z, side))| BoneRegion {
                bone,
                region: classify(rest_z, side, low_z, high_z, lateral),
                rest_z,
                lateral: side,
            })
            .collect();
        Self::Skeleton {
            bones,
            low_z,
            high_z,
            lateral,
        }
    }

    /// Region of one animation slot, or `None` when the proxy applies.
    pub fn region_of(&self, slot: usize) -> Option<Region> {
        match self {
            Self::Skeleton { bones, .. } => bones.get(slot).map(|bone| bone.region),
            Self::Proxy | Self::Unavailable { .. } => None,
        }
    }

    /// Header marker: the region basis, never presented as more than it is.
    pub fn marker(&self) -> String {
        match self {
            Self::Proxy => "region=proxy".to_string(),
            Self::Skeleton { .. } => "region=skeleton".to_string(),
            Self::Unavailable { slots, bones } => format!(
                "region=unavailable reason=bone-slot-mismatch slots={slots} bones={bones}"
            ),
        }
    }

    /// Assignment lines for inspection: the derived thresholds and one line per
    /// bone. Empty when no skeleton was used.
    pub fn assignment_lines(&self) -> Vec<String> {
        let Self::Skeleton {
            bones,
            low_z,
            high_z,
            lateral,
        } = self
        else {
            return Vec::new();
        };
        let mut lines = Vec::with_capacity(bones.len() + 1);
        lines.push(format!(
            "region-basis skeleton low_z={low_z:.1} high_z={high_z:.1} lateral={lateral:.1}"
        ));
        lines.extend(bones.iter().map(|bone| {
            format!(
                "region-bone slot={} zone={} rest_z={:.1} lateral={:.1}",
                bone.bone,
                bone.region.label(),
                bone.rest_z,
                bone.lateral
            )
        }));
        lines
    }
}

fn classify(rest_z: f64, lateral: f64, low_z: f64, high_z: f64, threshold: f64) -> Region {
    if lateral >= threshold {
        Region::Upper
    } else if rest_z <= low_z {
        Region::Leg
    } else if rest_z >= high_z {
        Region::Upper
    } else {
        Region::Hip
    }
}

/// Linear-interpolated quantile of an ascending slice.
fn quantile(sorted: &[f64], fraction: f64) -> f64 {
    match sorted.len() {
        0 => 0.0,
        1 => sorted[0],
        len => {
            let position = fraction.clamp(0.0, 1.0) * (len - 1) as f64;
            let lower = position.floor() as usize;
            let upper = position.ceil() as usize;
            let weight = position - lower as f64;
            sorted[lower] * (1.0 - weight) + sorted[upper] * weight
        }
    }
}

/// Degrees per binary-angle unit (`4096` units per turn).
pub const DEGREES_PER_BINARY_UNIT: f64 = 360.0 / BINARY_ANGLE_UNITS_PER_TURN as f64;

/// A clip needs at least this many frames to be an idle loop candidate.
pub const IDLE_MIN_FRAMES: usize = 4;
/// Maximum mean per-frame rotation for an idle candidate, in degrees.
/// **Arbitrary** ceiling chosen from the observed low-motion band.
pub const IDLE_MAX_ANG_MEAN: f64 = 200.0;
/// A clip needs at least this many frames to be a walk candidate.
pub const WALK_MIN_FRAMES: usize = 4;
/// Minimum root horizontal travel for a walk candidate. **Arbitrary.**
pub const WALK_MIN_HORIZ: f64 = 10.0;
/// At most this many frames for an airborne (jump/fall/land) candidate.
pub const AIR_MAX_FRAMES: usize = 3;
/// At least this much root vertical travel for a land candidate. **Arbitrary.**
pub const LAND_MIN_VARC: f64 = 1.0;
/// Default number of candidates shown per state.
pub const DEFAULT_TOP: usize = 5;
/// Lower vertical band as a fraction of the skeleton's rest `Z` extent.
pub const LOW_BAND: f64 = 0.4;
/// Upper vertical band as a fraction of the skeleton's rest `Z` extent.
pub const HIGH_BAND: f64 = 0.7;
/// Fraction of the median bone radius that counts as a lateral (arm) offset.
/// Slightly below `1.0` so a bone sitting on the median still counts as far
/// instead of splitting on floating-point noise.
pub const LATERAL_MARGIN: f64 = 0.95;

/// The five locomotion states this table ranks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotionState {
    Idle,
    Walk,
    JumpUp,
    Fall,
    Land,
}

impl LocomotionState {
    pub const ALL: [Self; 5] = [Self::Idle, Self::Walk, Self::JumpUp, Self::Fall, Self::Land];

    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::JumpUp => "jump_up",
            Self::Fall => "fall",
            Self::Land => "land",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.label() == label)
    }
}

/// Confidence of a state's top candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// Deterministic motion metrics for one clip.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipMotion {
    pub index: usize,
    pub frames: usize,
    pub divisor: i16,
    pub duration_s: f64,
    /// `max - min` of the root slot's world Z translation.
    pub varc: f64,
    /// `max - min` of the root slot's world X translation.
    pub hdisp_x: f64,
    /// `max - min` of the root slot's world Y translation.
    pub hdisp_y: f64,
    /// Mean per-frame total rotation delta across all slots, in degrees.
    pub ang_mean: f64,
    /// Largest single-frame total rotation delta, in degrees.
    pub ang_max: f64,
    /// Best autocorrelation lag over `1..=frames/2`, or `0`.
    pub period_lag: usize,
    /// `1 / (1 + residual_degrees)` at the best lag (`1.0` = exact repeat).
    pub period_score: f64,
    /// Share of total motion on bones assigned to the leg region. Under the
    /// proxy this is the lowest slot third; under the skeleton it is real
    /// low-height geometry.
    pub leg: f64,
    /// Share of total motion on bones assigned to the hip/torso region.
    pub hip: f64,
    /// Share of total motion on bones assigned to the head/arm region.
    pub upper: f64,
    /// The state this clip is the top candidate for, if any.
    pub guess: Option<LocomotionState>,
    /// Confidence of that guess.
    pub confidence: Confidence,
}

impl ClipMotion {
    /// Horizontal root travel: the larger of the X and Y ranges.
    pub fn hdisp(&self) -> f64 {
        self.hdisp_x.max(self.hdisp_y)
    }

    /// Mean angular velocity normalised by the clip's own duration, in degrees
    /// per second. A short but busy clip and a long slow loop separate here.
    pub fn ang_per_second(&self) -> f64 {
        if self.duration_s <= f64::EPSILON {
            0.0
        } else {
            self.ang_mean / self.duration_s
        }
    }

    /// Derive every metric from decoded twelve-lane frames.
    ///
    /// `frames[k][slot]` is one decoded frame. Lanes `0..=2` are angles and
    /// `8..=10` are translation. Frame count is the slice length, so a caller
    /// cannot disagree with the data it passes. A skeleton plan assigns each
    /// slot to its real body region and adds the per-frame translation
    /// displacement to that slot's motion; the proxy plan keeps the historical
    /// angular-only slot-third split.
    pub fn from_lanes(
        index: usize,
        divisor: i16,
        frames: &[Vec<[f32; 12]>],
        regions: &RegionPlan,
    ) -> Self {
        let frame_count = frames.len();
        let slots = frames.first().map_or(0, Vec::len);
        let duration_s = frame_count as f64 * f64::from(divisor) / ANIMATION_UNITS_PER_SECOND as f64;
        let empty = Self {
            index,
            frames: frame_count,
            divisor,
            duration_s,
            varc: 0.0,
            hdisp_x: 0.0,
            hdisp_y: 0.0,
            ang_mean: 0.0,
            ang_max: 0.0,
            period_lag: 0,
            period_score: 0.0,
            leg: 0.0,
            hip: 0.0,
            upper: 0.0,
            guess: None,
            confidence: Confidence::Low,
        };
        if frame_count == 0 || slots == 0 {
            return empty;
        }

        let root = root_slot(frames, slots);
        let (varc, hdisp_x, hdisp_y) = root_translation(frames, root);
        let (ang_mean, ang_max) = angular_velocity(frames, slots);
        let (period_lag, period_score) = periodicity(frames, slots);
        let (leg, hip, upper) = region_shares(frames, slots, regions);

        Self {
            varc,
            hdisp_x,
            hdisp_y,
            ang_mean,
            ang_max,
            period_lag,
            period_score,
            leg,
            hip,
            upper,
            ..empty
        }
    }
}

/// The lowest-index slot that animates any translation lane, else slot `0`.
///
/// The archived descriptor rest translations are not in `.SRA`; only authored
/// translation overrides are. This picks the first bone whose motion the file
/// actually carries (the pelvis for the measured bodies).
fn root_slot(frames: &[Vec<[f32; 12]>], slots: usize) -> usize {
    for slot in 0..slots {
        for frame in frames {
            for lane in 8..=10 {
                if frame[slot][lane] != 0.0 {
                    return slot;
                }
            }
        }
    }
    0
}

fn root_translation(frames: &[Vec<[f32; 12]>], root: usize) -> (f64, f64, f64) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for frame in frames {
        for (axis, lane) in (8..=10).enumerate() {
            let value = f64::from(frame[root][lane]);
            min[axis] = min[axis].min(value);
            max[axis] = max[axis].max(value);
        }
    }
    if min[0] == f64::INFINITY {
        (0.0, 0.0, 0.0)
    } else {
        (max[2] - min[2], max[0] - min[0], max[1] - min[1])
    }
}

fn angular_velocity(frames: &[Vec<[f32; 12]>], slots: usize) -> (f64, f64) {
    if frames.len() < 2 {
        return (0.0, 0.0);
    }
    let mut sum = 0.0f64;
    let mut max = 0.0f64;
    let mut samples = 0usize;
    for pair in frames.windows(2) {
        let mut total = 0.0f64;
        for slot in 0..slots {
            for axis in 0..3 {
                total += angular_delta(pair[1][slot][axis], pair[0][slot][axis]).abs();
            }
        }
        let degrees = total * DEGREES_PER_BINARY_UNIT;
        sum += degrees;
        max = max.max(degrees);
        samples += 1;
    }
    (sum / samples as f64, max)
}

fn periodicity(frames: &[Vec<[f32; 12]>], slots: usize) -> (usize, f64) {
    let frame_count = frames.len();
    if frame_count < 2 {
        return (0, 0.0);
    }
    let mut best_lag = 0usize;
    let mut best_score = -1.0f64;
    for lag in 1..=frame_count / 2 {
        let mut sum = 0.0f64;
        let mut samples = 0usize;
        for at in 0..frame_count - lag {
            for slot in 0..slots {
                for axis in 0..3 {
                    sum += angular_delta(
                        frames[at + lag][slot][axis],
                        frames[at][slot][axis],
                    )
                    .abs();
                    samples += 1;
                }
            }
        }
        if samples == 0 {
            continue;
        }
        let residual_deg = sum / samples as f64 * DEGREES_PER_BINARY_UNIT;
        let score = 1.0 / (1.0 + residual_deg);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    if best_lag == 0 { (0, 0.0) } else { (best_lag, best_score) }
}

/// Per-region share of total motion.
///
/// With a [`RegionPlan::Skeleton`] each slot's motion is the per-frame angular
/// delta plus the per-frame translation displacement, summed over the clip;
/// the region owns the bones assigned by [`RegionPlan::from_model`]. With no
/// skeleton the slot-third proxy is kept exactly as before: angular motion only,
/// lowest third `leg`, middle third `hip`, highest third `upper`. A proxy is
/// never presented as a skeleton measurement.
fn region_shares(
    frames: &[Vec<[f32; 12]>],
    slots: usize,
    plan: &RegionPlan,
) -> (f64, f64, f64) {
    let skeleton = matches!(plan, RegionPlan::Skeleton { .. });
    let mut region = [0.0f64; 3];
    let mut total = 0.0f64;
    if frames.len() >= 2 {
        for slot in 0..slots {
            let zone = plan
                .region_of(slot)
                .map_or_else(|| (slot * 3 / slots).min(2), Region::index);
            let mut angular = 0.0f64;
            let mut translation = 0.0f64;
            for pair in frames.windows(2) {
                for axis in 0..3 {
                    angular += angular_delta(pair[1][slot][axis], pair[0][slot][axis]).abs();
                }
                if skeleton {
                    let distance_squared: f64 = (8..=10)
                        .map(|lane| {
                            let delta = f64::from(pair[1][slot][lane]) - f64::from(pair[0][slot][lane]);
                            delta * delta
                        })
                        .sum();
                    translation += distance_squared.sqrt();
                }
            }
            let motion = angular * DEGREES_PER_BINARY_UNIT + translation;
            region[zone] += motion;
            total += motion;
        }
    }
    if total <= f64::EPSILON {
        (0.0, 0.0, 0.0)
    } else {
        (region[0] / total, region[1] / total, region[2] / total)
    }
}

/// Shortest signed per-axis angular difference in binary units.
fn angular_delta(to: f32, from: f32) -> f64 {
    let turn = f64::from(BINARY_ANGLE_UNITS_PER_TURN);
    let half = turn / 2.0;
    let mut delta = (f64::from(to) - f64::from(from)) % turn;
    if delta > half {
        delta -= turn;
    } else if delta < -half {
        delta += turn;
    }
    delta
}

/// Every clip's metrics plus the per-state shortlist.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipMetricsReport {
    pub clips: Vec<ClipMotion>,
    pub shortlist: [(LocomotionState, Vec<usize>, Confidence); 5],
    /// Region basis that produced every clip's `leg`/`hip`/`upper` shares.
    pub regions: RegionPlan,
}

impl ClipMetricsReport {
    /// Build a report and record which region basis produced its shares.
    pub fn from_clips(mut clips: Vec<ClipMotion>, regions: RegionPlan) -> Self {
        let mut shortlist = LocomotionState::ALL.map(|state| {
            let ranked = rank(&clips, state);
            let indices: Vec<usize> = ranked.iter().map(|clip| clip.index).collect();
            let confidence = confidence(state, &ranked);
            (state, indices, confidence)
        });
        // A single clip cannot be the top candidate for two airborne phases: a
        // two-frame clip carries no phase split. When jump/fall/land collapse
        // onto the same clip, the metrics cannot separate them, so all three
        // states stay `-` instead of naming one clip three times.
        let air_collision = {
            let mut tops = Vec::new();
            for state in [LocomotionState::JumpUp, LocomotionState::Fall, LocomotionState::Land] {
                if let Some((_, indices, _)) =
                    shortlist.iter().find(|(candidate, _, _)| *candidate == state)
                {
                    if let Some(&top) = indices.first() {
                        tops.push(top);
                    }
                }
            }
            tops.iter()
                .any(|top| tops.iter().filter(|other| *other == top).count() > 1)
        };
        if air_collision {
            for (state, indices, confidence) in &mut shortlist {
                if matches!(
                    state,
                    LocomotionState::JumpUp | LocomotionState::Fall | LocomotionState::Land
                ) {
                    indices.clear();
                    *confidence = Confidence::Low;
                }
            }
        }
        for (state, indices, confidence) in &shortlist {
            if let Some(&top) = indices.first() {
                if let Some(clip) = clips.iter_mut().find(|clip| clip.index == top) {
                    clip.guess = Some(*state);
                    clip.confidence = *confidence;
                }
            }
        }
        Self {
            clips,
            shortlist,
            regions,
        }
    }

    /// Indices ranked for `state`, best first.
    pub fn candidates(&self, state: LocomotionState) -> &[usize] {
        self.shortlist
            .iter()
            .find(|(candidate, _, _)| *candidate == state)
            .map_or(&[], |(_, indices, _)| indices.as_slice())
    }
}

/// Clips eligible for `state`, best first.
fn rank<'a>(clips: &'a [ClipMotion], state: LocomotionState) -> Vec<&'a ClipMotion> {
    let mut rows: Vec<&ClipMotion> = clips
        .iter()
        .filter(|clip| eligible(clip, state))
        .collect();
    rows.sort_by(|a, b| compare(a, b, state));
    rows
}

fn eligible(clip: &ClipMotion, state: LocomotionState) -> bool {
    match state {
        LocomotionState::Idle => {
            clip.frames >= IDLE_MIN_FRAMES && clip.ang_mean <= IDLE_MAX_ANG_MEAN
        }
        LocomotionState::Walk => {
            clip.frames >= WALK_MIN_FRAMES && clip.hdisp() >= WALK_MIN_HORIZ
        }
        LocomotionState::JumpUp => clip.frames <= AIR_MAX_FRAMES,
        LocomotionState::Fall => clip.frames <= 2,
        LocomotionState::Land => clip.frames <= AIR_MAX_FRAMES && clip.varc >= LAND_MIN_VARC,
    }
}

fn compare(a: &ClipMotion, b: &ClipMotion, state: LocomotionState) -> std::cmp::Ordering {
    let index = a.index.cmp(&b.index);
    match state {
        LocomotionState::Idle => a
            .ang_per_second()
            .total_cmp(&b.ang_per_second())
            .then_with(|| a.hdisp().total_cmp(&b.hdisp()))
            .then(index),
        LocomotionState::Walk => b
            .hdisp()
            .total_cmp(&a.hdisp())
            .then_with(|| b.ang_mean.total_cmp(&a.ang_mean))
            .then(index),
        LocomotionState::JumpUp => b
            .ang_mean
            .total_cmp(&a.ang_mean)
            .then_with(|| b.varc.total_cmp(&a.varc))
            .then_with(|| b.hdisp().total_cmp(&a.hdisp()))
            .then(index),
        LocomotionState::Fall => a
            .ang_mean
            .total_cmp(&b.ang_mean)
            .then_with(|| a.hdisp().total_cmp(&b.hdisp()))
            .then_with(|| a.varc.total_cmp(&b.varc))
            .then(index),
        LocomotionState::Land => b
            .varc
            .total_cmp(&a.varc)
            .then_with(|| b.ang_mean.total_cmp(&a.ang_mean))
            .then(index),
    }
}

fn confidence(state: LocomotionState, ranked: &[&ClipMotion]) -> Confidence {
    let Some(first) = ranked.first() else {
        return Confidence::Low;
    };
    let Some(second) = ranked.get(1) else {
        return Confidence::High;
    };
    let (top, next) = match state {
        LocomotionState::Idle => {
            let first_speed = first.ang_per_second();
            let second_speed = second.ang_per_second();
            (second_speed - first_speed, first_speed.max(1.0))
        }
        LocomotionState::Fall => (second.ang_mean - first.ang_mean, first.ang_mean.max(1.0)),
        LocomotionState::Walk => (first.hdisp() - second.hdisp(), first.hdisp().max(1.0)),
        LocomotionState::JumpUp => (first.ang_mean - second.ang_mean, first.ang_mean.max(1.0)),
        LocomotionState::Land => (first.varc - second.varc, first.varc.max(1.0)),
    };
    if top <= 0.0 {
        // A tie at the top: the metrics cannot separate the leading clips.
        Confidence::Low
    } else if top / next >= 0.25 {
        Confidence::High
    } else if top / next >= 0.05 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

/// Decode every clip of `animation` and build its report with the slot-third
/// proxy.
pub fn analyze_animation(animation: &SraAnimation) -> Result<ClipMetricsReport, String> {
    analyze_animation_with_regions(animation, &RegionPlan::Proxy)
}

/// Decode every clip and derive its region shares from `regions`.
pub fn analyze_animation_with_regions(
    animation: &SraAnimation,
    regions: &RegionPlan,
) -> Result<ClipMetricsReport, String> {
    let mut clips = Vec::with_capacity(animation.clip_count());
    for clip in 0..animation.clip_count() {
        let frame_count = animation
            .frame_count_of(clip)
            .map_err(|error| error.to_string())?;
        let divisor = animation.divisor_of(clip).map_err(|error| error.to_string())?;
        let mut frames = Vec::with_capacity(frame_count);
        for frame in 0..frame_count {
            frames.push(animation.decode_clip_lanes(clip, frame)?);
        }
        clips.push(ClipMotion::from_lanes(clip, divisor, &frames, regions));
    }
    Ok(ClipMetricsReport::from_clips(clips, regions.clone()))
}

fn format_seconds(value: f64) -> String {
    format!("{value:.3}")
}

/// One row per clip, in file order.
pub fn rows_text(report: &ClipMetricsReport) -> String {
    let mut lines = Vec::with_capacity(report.clips.len());
    for clip in &report.clips {
        let guess = clip.guess.map_or("-", LocomotionState::label);
        lines.push(format!(
            "clip index={} frames={} divisor={} dur={}s varc={:.1} hdisp={:.1} ang_mean={:.1} ang_max={:.1} period_lag={} score={:.2} leg={:.2} hip={:.2} upper={:.2} guess={} conf={} heuristic",
            clip.index,
            clip.frames,
            clip.divisor,
            format_seconds(clip.duration_s),
            clip.varc,
            clip.hdisp(),
            clip.ang_mean,
            clip.ang_max,
            clip.period_lag,
            clip.period_score,
            clip.leg,
            clip.hip,
            clip.upper,
            guess,
            clip.confidence.label(),
        ));
    }
    lines.join("\n")
}

/// The trailing ranked shortlist. An empty state is `-`.
pub fn shortlist_text(report: &ClipMetricsReport, top: usize) -> String {
    let mut parts = Vec::new();
    for (state, indices, _) in &report.shortlist {
        if indices.is_empty() {
            parts.push(format!("{}=-", state.label()));
            continue;
        }
        let mut rendered = Vec::new();
        for (position, &index) in indices.iter().take(top).enumerate() {
            if position == 0 && *state == LocomotionState::Idle {
                let duration = report
                    .clips
                    .iter()
                    .find(|clip| clip.index == index)
                    .map_or(0.0, |clip| clip.duration_s);
                rendered.push(format!("{index}({}s)", format_seconds(duration)));
            } else {
                rendered.push(index.to_string());
            }
        }
        parts.push(format!("{}={}", state.label(), rendered.join(",")));
    }
    format!("SHORTLIST {}", parts.join(" "))
}

/// Full report: a header, the region basis, one row per clip, then the
/// shortlist. A skeleton basis also prints the bone-to-region assignment.
pub fn report_text(
    sra_label: &str,
    slots: usize,
    report: &ClipMetricsReport,
    top: usize,
) -> String {
    let mut text = format!(
        "clip-metrics sra={sra_label} slots={slots} clips={} source=heuristic {} (ranking, not ground truth)\n",
        report.clips.len(),
        report.regions.marker(),
    );
    let assignment = report.regions.assignment_lines();
    if !assignment.is_empty() {
        text.push_str(&assignment.join("\n"));
        text.push('\n');
    }
    text.push_str(&rows_text(report));
    if !report.clips.is_empty() {
        text.push('\n');
    }
    text.push_str(&shortlist_text(report, top));
    text
}

/// One validation expectation: a state and the clip index measured at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expectation {
    pub state: LocomotionState,
    pub index: usize,
}

impl Expectation {
    /// Build expectations from `(state_label, clip_index)` pairs, skipping an
    /// unknown label rather than panicking.
    pub fn from_labels(labels: &[(&str, usize)]) -> Vec<Self> {
        labels
            .iter()
            .filter_map(|(label, index)| {
                LocomotionState::from_label(label).map(|state| Self {
                    state,
                    index: *index,
                })
            })
            .collect()
    }
}

/// Validate the report's top candidate per expected state.
///
/// Returns the pass count and one human-readable line per expectation naming
/// the top candidate and, on a miss, the metric that ranked it first.
pub fn validate(report: &ClipMetricsReport, expected: &[Expectation]) -> (usize, String) {
    let mut passed = 0usize;
    let mut lines = Vec::new();
    for expectation in expected {
        let candidates = report.candidates(expectation.state);
        let top = candidates.first().copied();
        let pass = top == Some(expectation.index);
        if pass {
            passed += 1;
        }
        let top_text = top.map_or("-".to_string(), |index| index.to_string());
        let reason = if pass {
            "top candidate".to_string()
        } else {
            format!(
                "top is clip {top_text}; expected clip {} is {}",
                expectation.index,
                candidate_rank(candidates, expectation.index)
            )
        };
        lines.push(format!(
            "state={} expected={} top={} result={} ({reason})",
            expectation.state.label(),
            expectation.index,
            top_text,
            if pass { "pass" } else { "fail" },
        ));
    }
    (
        passed,
        format!(
            "CLASSIFIER VALIDATION pass={}/{} heuristic\n{}",
            passed,
            expected.len(),
            lines.join("\n")
        ),
    )
}

fn candidate_rank(candidates: &[usize], index: usize) -> String {
    match candidates.iter().position(|&candidate| candidate == index) {
        Some(rank) => format!("rank {}", rank + 1),
        None => "not a candidate".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lane(angle: f32, translation: [f32; 3]) -> [f32; 12] {
        let mut lanes = [0.0f32; 12];
        lanes[0] = angle;
        lanes[8] = translation[0];
        lanes[9] = translation[1];
        lanes[10] = translation[2];
        lanes
    }

    fn constant(frames: usize, slots: usize, angle: f32) -> Vec<Vec<[f32; 12]>> {
        vec![vec![lane(angle, [0.0; 3]); slots]; frames]
    }

    fn ramping(frames: usize, step: f32) -> Vec<Vec<[f32; 12]>> {
        (0..frames)
            .map(|k| vec![lane(step * k as f32, [0.0; 3])])
            .collect()
    }

    #[test]
    fn a_still_clip_has_zero_motion_and_full_period_score() {
        let frames = constant(8, 2, 10.0);
        let metrics = ClipMotion::from_lanes(3, 100, &frames, &RegionPlan::Proxy);
        assert_eq!(metrics.frames, 8);
        assert_eq!(metrics.ang_mean, 0.0);
        assert_eq!(metrics.ang_max, 0.0);
        assert_eq!(metrics.period_lag, 1);
        assert_eq!(metrics.period_score, 1.0);
        assert_eq!(metrics.leg + metrics.hip + metrics.upper, 0.0);
        assert!((metrics.duration_s - 8.0 * 100.0 / 5000.0).abs() < 1e-9);
    }

    #[test]
    fn root_translation_ranges_are_measured_not_signed() {
        let mut frames = constant(4, 2, 0.0);
        frames[0][0] = lane(0.0, [1.0, -2.0, 10.0]);
        frames[1][0] = lane(0.0, [4.0, 3.0, 15.0]);
        frames[2][0] = lane(0.0, [2.0, 0.0, 12.0]);
        frames[3][0] = lane(0.0, [3.0, 1.0, 16.0]);
        let metrics = ClipMotion::from_lanes(0, 100, &frames, &RegionPlan::Proxy);
        assert!((metrics.varc - 6.0).abs() < 1e-9);
        assert!((metrics.hdisp_x - 3.0).abs() < 1e-9);
        assert!((metrics.hdisp_y - 5.0).abs() < 1e-9);
        assert!((metrics.hdisp() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn angular_velocity_wraps_at_the_turn_boundary() {
        let mut frames = constant(3, 1, 4090.0);
        frames[1][0] = lane(10.0, [0.0; 3]);
        let metrics = ClipMotion::from_lanes(0, 100, &frames, &RegionPlan::Proxy);
        let expected = 16.0 * DEGREES_PER_BINARY_UNIT;
        assert!((metrics.ang_max - expected).abs() < 1e-9, "{metrics:?}");
    }

    #[test]
    fn region_shares_follow_the_slot_third_proxy() {
        let mut frames = constant(3, 3, 0.0);
        // Only the last slot moves, so the upper third owns all motion.
        frames[0][2] = lane(0.0, [0.0; 3]);
        frames[1][2] = lane(100.0, [0.0; 3]);
        frames[2][2] = lane(200.0, [0.0; 3]);
        let metrics = ClipMotion::from_lanes(0, 100, &frames, &RegionPlan::Proxy);
        assert_eq!(metrics.leg, 0.0);
        assert_eq!(metrics.hip, 0.0);
        assert_eq!(metrics.upper, 1.0);
    }

    #[test]
    fn shortlist_orders_are_total_and_deterministic() {
        let clips = vec![
            ClipMotion::from_lanes(0, 100, &constant(1, 1, 0.0), &RegionPlan::Proxy),
            ClipMotion::from_lanes(1, 100, &constant(1, 1, 0.0), &RegionPlan::Proxy),
        ];
        let report = ClipMetricsReport::from_clips(clips, RegionPlan::Proxy);
        // Jump and fall collapse onto the same one-frame clip, so the metrics
        // refuse to name either air state.
        assert!(report.candidates(LocomotionState::Fall).is_empty());
        assert!(report.candidates(LocomotionState::JumpUp).is_empty());
        let text = report_text("SYNTH.SRA", 1, &report, 5);
        assert!(text.starts_with("clip-metrics sra=SYNTH.SRA slots=1 clips=2 source=heuristic"));
        assert!(
            text.ends_with("SHORTLIST idle=- walk=- jump_up=- fall=- land=-"),
            "{text}"
        );
    }

    #[test]
    fn validation_names_the_top_and_the_miss() {
        let report = ClipMetricsReport::from_clips(vec![
            ClipMotion::from_lanes(0, 100, &ramping(4, 1000.0), &RegionPlan::Proxy),
            ClipMotion::from_lanes(5, 100, &ramping(8, 50.0), &RegionPlan::Proxy),
        ], RegionPlan::Proxy);
        let (passed, text) = validate(
            &report,
            &[Expectation { state: LocomotionState::Idle, index: 0 }],
        );
        assert_eq!(passed, 0);
        assert!(text.contains("CLASSIFIER VALIDATION pass=0/1"));
        assert!(text.contains("expected=0 top=5 result=fail"), "{text}");
    }

    fn bone(local: [i16; 3]) -> crate::object::Bone {
        crate::object::Bone {
            first_vertex: 0,
            last_vertex: 0,
            local,
            parent: u16::MAX,
        }
    }

    fn geometric_model() -> ObjectModel {
        let mut bones = Vec::new();
        // Eight low legs close to the axis.
        for z in [0, 2, 4, 6, 8, 10, 12, 14] {
            bones.push(bone([1, 0, z]));
        }
        // Two hips at mid height.
        bones.push(bone([0, 0, 50]));
        bones.push(bone([0, 0, 60]));
        // Ten lateral arms at every height band.
        for z in [40, 45, 50, 55, 60, 65, 70, 75, 80, 90] {
            bones.push(bone([60, 0, z]));
        }
        ObjectModel {
            bones,
            ..ObjectModel::default()
        }
    }

    fn core_model() -> ObjectModel {
        ObjectModel {
            bones: vec![bone([0, 0, 0]), bone([0, 0, 50]), bone([0, 0, 100])],
            ..ObjectModel::default()
        }
    }

    #[test]
    fn skeleton_regions_come_from_rest_geometry_not_slot_order() {
        let plan = RegionPlan::from_model(&geometric_model(), 20);
        for slot in 0..8 {
            assert_eq!(plan.region_of(slot), Some(Region::Leg), "slot {slot}");
        }
        for slot in 8..10 {
            assert_eq!(plan.region_of(slot), Some(Region::Hip), "slot {slot}");
        }
        for slot in 10..20 {
            assert_eq!(plan.region_of(slot), Some(Region::Upper), "slot {slot}");
        }
        let lines = plan.assignment_lines();
        assert_eq!(lines.len(), 21, "one threshold line plus one line per bone");
        assert!(lines[0].starts_with("region-basis skeleton low_z=36.0 high_z=63.0"));
        assert!(plan.marker().contains("region=skeleton"));

        // With no lateral spread the vertical bands alone drive the regions.
        let core = RegionPlan::from_model(&core_model(), 3);
        assert_eq!(core.region_of(0), Some(Region::Leg));
        assert_eq!(core.region_of(1), Some(Region::Hip));
        assert_eq!(core.region_of(2), Some(Region::Upper));
    }

    #[test]
    fn skeleton_shares_use_the_bone_region_and_count_translation() {
        let plan = RegionPlan::from_model(&geometric_model(), 20);
        // Only an arm slot carries translation; every angle is constant.
        let mut frames = constant(3, 20, 0.0);
        frames[1][12] = lane(0.0, [10.0, 0.0, 0.0]);
        let metrics = ClipMotion::from_lanes(0, 100, &frames, &plan);
        assert_eq!((metrics.leg, metrics.hip), (0.0, 0.0));
        assert_eq!(metrics.upper, 1.0);
        // The same frames under the proxy ignore translation, so the proxy sees
        // no motion at all and reports zeros.
        let proxy = ClipMotion::from_lanes(0, 100, &frames, &RegionPlan::Proxy);
        assert_eq!((proxy.leg, proxy.hip, proxy.upper), (0.0, 0.0, 0.0));
    }

    #[test]
    fn a_bone_slot_mismatch_is_named_and_falls_back_to_the_proxy() {
        let plan = RegionPlan::from_model(&geometric_model(), 21);
        assert_eq!(
            plan.marker(),
            "region=unavailable reason=bone-slot-mismatch slots=21 bones=20"
        );
        assert!(plan.assignment_lines().is_empty());
        let frames = constant(3, 20, 0.0);
        let metrics = ClipMotion::from_lanes(0, 100, &frames, &plan);
        let report = ClipMetricsReport::from_clips(vec![metrics], plan);
        let text = report_text("SYNTH.SRA", 21, &report, 5);
        assert!(text.contains("region=unavailable reason=bone-slot-mismatch slots=21 bones=20"));
        assert!(!text.contains("region-bone"));
    }
}

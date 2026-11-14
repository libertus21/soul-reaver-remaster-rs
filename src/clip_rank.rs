//! Offline heuristic ranking for identifier-less `.SRA` clips.
//!
//! The packed tables carry no name, id or state byte, so clip identity is file
//! order only. This module decodes every located clip, derives deterministic
//! motion metrics and sorts the clips into four buckets. It is a **shortlist**,
//! not ground truth: the thresholds are explicit, a bucket can be empty, and an
//! UNKNOWN bucket split is reported as such rather than guessed.

use crate::pose::BINARY_ANGLE_UNITS_PER_TURN;
use crate::sra::{ANIMATION_UNITS_PER_SECOND, SraAnimation};

/// Degrees per binary-angle unit (`4096` units per turn).
pub const DEGREES_PER_BINARY_UNIT: f64 = 360.0 / BINARY_ANGLE_UNITS_PER_TURN as f64;

/// A clip whose mean per-axis consecutive-frame change is at most this is
/// classified `STILL`.
pub const STILL_MAX_MEAN_DEG: f64 = 0.50;
/// `LOCOMOTION` requires at least this much mean motion (it is the complement
/// of the `STILL` ceiling) and at most [`LOCOMOTION_MAX_MEAN_DEG`].
pub const LOCOMOTION_MIN_MEAN_DEG: f64 = STILL_MAX_MEAN_DEG;
/// Upper mean-motion bound for `LOCOMOTION`.
pub const LOCOMOTION_MAX_MEAN_DEG: f64 = 8.00;
/// A detected period only supports `LOCOMOTION` when its residual is at most
/// this many degrees.
pub const LOCOMOTION_MAX_RESIDUAL_DEG: f64 = 1.00;
/// `IMPACT` requires a single consecutive-frame change of at least this many
/// degrees.
pub const IMPACT_MIN_MAX_DEG: f64 = 5.00;
/// `IMPACT` requires `max_delta / mean_delta` to be at least this large.
pub const IMPACT_MIN_SPIKE_RATIO: f64 = 4.00;
/// A slot counts as moving when its largest per-axis consecutive-frame change
/// exceeds this many degrees.
pub const MOVING_SLOT_DEG: f64 = 0.50;
/// Default number of clips shown per bucket.
pub const DEFAULT_TOP: usize = 8;

/// The four heuristic buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipBucket {
    Still,
    Locomotion,
    Impact,
    Other,
}

impl ClipBucket {
    pub const ALL: [Self; 4] = [Self::Still, Self::Locomotion, Self::Impact, Self::Other];

    pub fn label(self) -> &'static str {
        match self {
            Self::Still => "STILL",
            Self::Locomotion => "LOCOMOTION",
            Self::Impact => "IMPACT",
            Self::Other => "OTHER",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Still => "mean_delta_deg asc",
            Self::Locomotion => "residual_deg asc",
            Self::Impact => "spike_ratio desc",
            Self::Other => "max_delta_deg desc",
        }
    }
}

/// Deterministic motion metrics for one clip.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipMetrics {
    pub index: usize,
    pub frames: usize,
    pub divisor: i16,
    pub duration_ms: u64,
    pub mean_delta_deg: f64,
    pub max_delta_deg: f64,
    pub loop_closure_deg: f64,
    pub moving_slots: usize,
    pub period: Option<usize>,
    pub period_residual_deg: f64,
}

impl ClipMetrics {
    /// Derive every metric from decoded per-frame angle triples.
    ///
    /// `frames[k][slot][axis]` is a binary-angle value. Frame count is the
    /// slice length, so a caller cannot disagree with the data it passes.
    pub fn from_frames(index: usize, divisor: i16, frames: &[Vec<[f32; 3]>]) -> Self {
        let frame_count = frames.len();
        let duration_ms = duration_ms(frame_count, divisor);
        let slots = frames.first().map_or(0, Vec::len);
        if frame_count == 0 || slots == 0 {
            return Self {
                index,
                frames: frame_count,
                divisor,
                duration_ms,
                mean_delta_deg: 0.0,
                max_delta_deg: 0.0,
                loop_closure_deg: 0.0,
                moving_slots: 0,
                period: None,
                period_residual_deg: 0.0,
            };
        }

        let mut delta_sum = 0.0f64;
        let mut delta_max = 0.0f64;
        let mut delta_samples = 0usize;
        for k in 1..frame_count {
            for slot in 0..slots {
                for axis in 0..3 {
                    let delta = angular_delta(
                        units(frames[k][slot][axis]),
                        units(frames[k - 1][slot][axis]),
                    );
                    let delta = delta.abs();
                    delta_sum += delta;
                    delta_max = delta_max.max(delta);
                    delta_samples += 1;
                }
            }
        }
        let mean_delta_deg = if delta_samples == 0 {
            0.0
        } else {
            delta_sum / delta_samples as f64 * DEGREES_PER_BINARY_UNIT
        };
        let max_delta_deg = delta_max * DEGREES_PER_BINARY_UNIT;

        let mut closure_sum = 0.0f64;
        let closure_samples = slots * 3;
        for slot in 0..slots {
            for axis in 0..3 {
                let delta = angular_delta(
                    units(frames[0][slot][axis]),
                    units(frames[frame_count - 1][slot][axis]),
                );
                closure_sum += delta.abs();
            }
        }
        let loop_closure_deg = closure_sum / closure_samples as f64 * DEGREES_PER_BINARY_UNIT;

        let mut moving_slots = 0usize;
        for slot in 0..slots {
            let mut slot_max = 0.0f64;
            for k in 1..frame_count {
                for axis in 0..3 {
                    let delta = angular_delta(
                        units(frames[k][slot][axis]),
                        units(frames[k - 1][slot][axis]),
                    );
                    slot_max = slot_max.max(delta.abs());
                }
            }
            if slot_max * DEGREES_PER_BINARY_UNIT > MOVING_SLOT_DEG {
                moving_slots += 1;
            }
        }

        let (period, period_residual_deg) = detect_period(frames);

        Self {
            index,
            frames: frame_count,
            divisor,
            duration_ms,
            mean_delta_deg,
            max_delta_deg,
            loop_closure_deg,
            moving_slots,
            period,
            period_residual_deg,
        }
    }

    /// `max_delta / mean_delta`, or `0` when the mean is zero.
    pub fn spike_ratio(&self) -> f64 {
        if self.mean_delta_deg <= f64::EPSILON {
            0.0
        } else {
            self.max_delta_deg / self.mean_delta_deg
        }
    }

    /// Apply the explicit thresholds. Earlier rules win.
    pub fn classify(&self) -> ClipBucket {
        if self.mean_delta_deg <= STILL_MAX_MEAN_DEG {
            return ClipBucket::Still;
        }
        // A period of 1 is the trivial adjacent-frame shift: for a smooth
        // trajectory it is always the smallest residual and it says nothing
        // about repetition. It is reported as `period=1`, but it is not
        // periodicity evidence for `LOCOMOTION` or `IMPACT`.
        let real_period = self.period.is_some_and(|period| period >= 2);
        let weak_period = !real_period || self.period_residual_deg > LOCOMOTION_MAX_RESIDUAL_DEG;
        if self.max_delta_deg >= IMPACT_MIN_MAX_DEG
            && self.spike_ratio() >= IMPACT_MIN_SPIKE_RATIO
            && weak_period
        {
            return ClipBucket::Impact;
        }
        if self.mean_delta_deg > LOCOMOTION_MIN_MEAN_DEG
            && self.mean_delta_deg <= LOCOMOTION_MAX_MEAN_DEG
            && real_period
            && self.period_residual_deg <= LOCOMOTION_MAX_RESIDUAL_DEG
        {
            return ClipBucket::Locomotion;
        }
        ClipBucket::Other
    }
}

/// One clip plus its bucket and spike ratio.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedClip {
    pub metrics: ClipMetrics,
    pub bucket: ClipBucket,
    pub spike_ratio: f64,
}

/// Every ranked clip, in file order. Ordering is applied per bucket on demand.
#[derive(Debug, Clone, PartialEq)]
pub struct RankReport {
    pub clips: Vec<RankedClip>,
}

impl RankReport {
    pub fn from_metrics(metrics: Vec<ClipMetrics>) -> Self {
        let clips = metrics
            .into_iter()
            .map(|metrics| {
                let bucket = metrics.classify();
                let spike_ratio = metrics.spike_ratio();
                RankedClip {
                    metrics,
                    bucket,
                    spike_ratio,
                }
            })
            .collect();
        Self { clips }
    }

    /// Counts in [`ClipBucket::ALL`] order.
    pub fn counts(&self) -> [usize; 4] {
        let mut counts = [0usize; 4];
        for clip in &self.clips {
            let slot = match clip.bucket {
                ClipBucket::Still => 0,
                ClipBucket::Locomotion => 1,
                ClipBucket::Impact => 2,
                ClipBucket::Other => 3,
            };
            counts[slot] += 1;
        }
        counts
    }

    /// Bucket members in the bucket's own order.
    pub fn ordered(&self, bucket: ClipBucket) -> Vec<&RankedClip> {
        let mut rows: Vec<&RankedClip> = self
            .clips
            .iter()
            .filter(|clip| clip.bucket == bucket)
            .collect();
        match bucket {
            ClipBucket::Still => rows.sort_by(|a, b| {
                a.metrics
                    .mean_delta_deg
                    .total_cmp(&b.metrics.mean_delta_deg)
                    .then_with(|| a.metrics.index.cmp(&b.metrics.index))
            }),
            ClipBucket::Locomotion => rows.sort_by(|a, b| {
                a.metrics
                    .period_residual_deg
                    .total_cmp(&b.metrics.period_residual_deg)
                    .then_with(|| {
                        a.metrics
                            .mean_delta_deg
                            .total_cmp(&b.metrics.mean_delta_deg)
                    })
                    .then_with(|| a.metrics.index.cmp(&b.metrics.index))
            }),
            ClipBucket::Impact => rows.sort_by(|a, b| {
                b.spike_ratio
                    .total_cmp(&a.spike_ratio)
                    .then_with(|| b.metrics.max_delta_deg.total_cmp(&a.metrics.max_delta_deg))
                    .then_with(|| a.metrics.index.cmp(&b.metrics.index))
            }),
            ClipBucket::Other => rows.sort_by(|a, b| {
                b.metrics
                    .max_delta_deg
                    .total_cmp(&a.metrics.max_delta_deg)
                    .then_with(|| a.metrics.index.cmp(&b.metrics.index))
            }),
        }
        rows
    }

    fn sorted_by_mean(&self) -> Vec<&RankedClip> {
        let mut rows: Vec<&RankedClip> = self.clips.iter().collect();
        rows.sort_by(|a, b| {
            a.metrics
                .mean_delta_deg
                .total_cmp(&b.metrics.mean_delta_deg)
                .then_with(|| a.metrics.index.cmp(&b.metrics.index))
        });
        rows
    }

    fn sorted_by_max(&self) -> Vec<&RankedClip> {
        let mut rows: Vec<&RankedClip> = self.clips.iter().collect();
        rows.sort_by(|a, b| {
            b.metrics
                .max_delta_deg
                .total_cmp(&a.metrics.max_delta_deg)
                .then_with(|| a.metrics.index.cmp(&b.metrics.index))
        });
        rows
    }

    fn sorted_by_residual(&self) -> Vec<&RankedClip> {
        let mut rows: Vec<&RankedClip> = self
            .clips
            .iter()
            .filter(|clip| clip.metrics.period.is_some_and(|period| period >= 2))
            .collect();
        rows.sort_by(|a, b| {
            a.metrics
                .period_residual_deg
                .total_cmp(&b.metrics.period_residual_deg)
                .then_with(|| a.metrics.index.cmp(&b.metrics.index))
        });
        rows
    }
}

/// Decode every clip of `animation` and rank it. No cache outlives the call.
pub fn rank_animation(animation: &SraAnimation) -> Result<RankReport, String> {
    let mut metrics = Vec::with_capacity(animation.clip_count());
    for clip in 0..animation.clip_count() {
        let frame_count = animation
            .frame_count_of(clip)
            .map_err(|error| error.to_string())?;
        let divisor = animation
            .divisor_of(clip)
            .map_err(|error| error.to_string())?;
        let mut frames = Vec::with_capacity(frame_count);
        for index in 0..frame_count {
            frames.push(animation.decode_clip_index(clip, index)?.angles);
        }
        metrics.push(ClipMetrics::from_frames(clip, divisor, &frames));
    }
    Ok(RankReport::from_metrics(metrics))
}

/// Hard line bound for [`report_text`] at a given `top`.
///
/// One summary, one threshold line, four bucket headers, at most `top` rows per
/// bucket, one suggestion and three justification lines.
pub fn max_report_lines(top: usize) -> usize {
    10 + 4 * top
}

/// Render the bounded, parseable shortlist. Every line is `key=value` or a
/// three-word header, so a caller can grep the summary and the suggestion.
pub fn report_text(sra_label: &str, report: &RankReport, top: usize) -> String {
    let counts = report.counts();
    let mut lines = Vec::new();
    lines.push(format!(
        "rank sra={sra_label} clips={} still={} locomotion={} impact={} other={}",
        report.clips.len(),
        counts[0],
        counts[1],
        counts[2],
        counts[3]
    ));
    lines.push(format!(
        "heuristic thresholds still_mean<={STILL_MAX_MEAN_DEG:.2}deg locomotion_mean<={LOCOMOTION_MAX_MEAN_DEG:.2}deg locomotion_residual<={LOCOMOTION_MAX_RESIDUAL_DEG:.2}deg impact_max>={IMPACT_MIN_MAX_DEG:.2}deg impact_ratio>={IMPACT_MIN_SPIKE_RATIO:.2}x moving_slot>{MOVING_SLOT_DEG:.2}deg period_search=1..frames/2 period1=adjacent_shift_not_periodicity (shortlist, not ground truth)"
    ));
    for bucket in ClipBucket::ALL {
        let rows = report.ordered(bucket);
        let shown = rows.len().min(top);
        let note = if rows.is_empty() { " note=empty" } else { "" };
        lines.push(format!(
            "bucket {} shown={} total={} key={}{note}",
            bucket.label(),
            shown,
            rows.len(),
            bucket.key()
        ));
        for ranked in rows.into_iter().take(top) {
            lines.push(row_line(ranked));
        }
    }
    let (suggestion, reasons) = suggest(report);
    lines.push(suggestion);
    lines.extend(reasons);
    debug_assert!(
        lines.len() <= max_report_lines(top),
        "rank report exceeded its line bound"
    );
    lines.join("\n")
}

fn row_line(ranked: &RankedClip) -> String {
    let metrics = &ranked.metrics;
    let period = metrics
        .period
        .map(|period| period.to_string())
        .unwrap_or_else(|| "none".to_string());
    let residual = if metrics.period.is_some() {
        format!("{:.2}", metrics.period_residual_deg)
    } else {
        "-".to_string()
    };
    format!(
        "clip index={} frames={} divisor={} duration_ms={} mean_delta_deg={:.2} max_delta_deg={:.2} loop_closure_deg={:.2} moving_slots={} period={} residual_deg={}",
        metrics.index,
        metrics.frames,
        metrics.divisor,
        metrics.duration_ms,
        metrics.mean_delta_deg,
        metrics.max_delta_deg,
        metrics.loop_closure_deg,
        metrics.moving_slots,
        period,
        residual
    )
}

fn suggest(report: &RankReport) -> (String, Vec<String>) {
    if report.clips.is_empty() {
        return (
            "suggest --clip-idle 0 --clip-walk 0 --clip-jump 0".to_string(),
            vec![
                "idle: no clips decoded".to_string(),
                "walk: no clips decoded".to_string(),
                "jump: no clips decoded".to_string(),
            ],
        );
    }

    let still = report.ordered(ClipBucket::Still);
    let locomotion = report.ordered(ClipBucket::Locomotion);
    let impact = report.ordered(ClipBucket::Impact);
    let by_mean = report.sorted_by_mean();
    let by_residual = report.sorted_by_residual();
    let by_max = report.sorted_by_max();
    let motion_by_mean: Vec<&RankedClip> = by_mean
        .iter()
        .copied()
        .filter(|clip| clip.bucket != ClipBucket::Still)
        .collect();
    let mut used = Vec::new();

    let (idle, idle_is_still) = choose(&still, &used)
        .map(|clip| (clip, true))
        .or_else(|| choose(&by_mean, &used).map(|clip| (clip, false)))
        .expect("a non-empty report always yields a fallback clip");
    used.push(idle.metrics.index);
    let idle_reason = if idle_is_still {
        format!(
            "idle: lowest mean motion ({:.2}deg) with loop closure ({:.2}deg)",
            idle.metrics.mean_delta_deg, idle.metrics.loop_closure_deg
        )
    } else {
        format!(
            "idle: no STILL clip; lowest mean motion ({:.2}deg)",
            idle.metrics.mean_delta_deg
        )
    };

    let (walk, walk_source) = choose(&locomotion, &used)
        .map(|clip| (clip, WalkSource::Locomotion))
        .or_else(|| choose(&by_residual, &used).map(|clip| (clip, WalkSource::Periodic)))
        .or_else(|| choose(&motion_by_mean, &used).map(|clip| (clip, WalkSource::Motion)))
        .or_else(|| choose(&by_mean, &used).map(|clip| (clip, WalkSource::Fallback)))
        .expect("a non-empty report always yields a fallback clip");
    used.push(walk.metrics.index);
    let walk_reason = match walk_source {
        WalkSource::Locomotion => format!(
            "walk: moderate motion ({:.2}deg) with detected period {} frames (residual {:.2}deg)",
            walk.metrics.mean_delta_deg,
            walk.metrics.period.unwrap_or(0),
            walk.metrics.period_residual_deg
        ),
        WalkSource::Periodic => format!(
            "walk: no LOCOMOTION clip; cleanest detected period {} frames (residual {:.2}deg)",
            walk.metrics.period.unwrap_or(0),
            walk.metrics.period_residual_deg
        ),
        WalkSource::Motion => format!(
            "walk: no LOCOMOTION clip; lowest non-still motion ({:.2}deg)",
            walk.metrics.mean_delta_deg
        ),
        WalkSource::Fallback => format!(
            "walk: no moving clip; lowest remaining motion ({:.2}deg)",
            walk.metrics.mean_delta_deg
        ),
    };

    let (jump, jump_is_impact) = choose(&impact, &used)
        .map(|clip| (clip, true))
        .or_else(|| choose(&by_max, &used).map(|clip| (clip, false)))
        .expect("a non-empty report always yields a fallback clip");
    let jump_reason = if jump_is_impact {
        format!(
            "jump: largest spike ({:.2}deg, {:.2}x mean)",
            jump.metrics.max_delta_deg, jump.spike_ratio
        )
    } else {
        format!(
            "jump: no IMPACT clip; largest remaining single-frame change ({:.2}deg)",
            jump.metrics.max_delta_deg
        )
    };

    (
        format!(
            "suggest --clip-idle {} --clip-walk {} --clip-jump {}",
            idle.metrics.index, walk.metrics.index, jump.metrics.index
        ),
        vec![idle_reason, walk_reason, jump_reason],
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalkSource {
    Locomotion,
    Periodic,
    Motion,
    Fallback,
}

fn choose<'a>(rows: &[&'a RankedClip], used: &[usize]) -> Option<&'a RankedClip> {
    rows.iter()
        .copied()
        .find(|clip| !used.contains(&clip.metrics.index))
        .or_else(|| rows.first().copied())
}

fn detect_period(frames: &[Vec<[f32; 3]>]) -> (Option<usize>, f64) {
    let frame_count = frames.len();
    let slots = frames.first().map_or(0, Vec::len);
    if frame_count < 4 || slots == 0 {
        return (None, 0.0);
    }
    let mut best_period = None;
    let mut best_units = f64::INFINITY;
    for period in 1..frame_count / 2 {
        let mut sum = 0.0f64;
        let mut samples = 0usize;
        for k in 0..frame_count - period {
            for slot in 0..slots {
                for axis in 0..3 {
                    let delta = angular_delta(
                        units(frames[k + period][slot][axis]),
                        units(frames[k][slot][axis]),
                    );
                    sum += delta.abs();
                    samples += 1;
                }
            }
        }
        let residual = sum / samples as f64;
        if residual < best_units {
            best_units = residual;
            best_period = Some(period);
        }
    }
    (
        best_period,
        if best_period.is_some() {
            best_units * DEGREES_PER_BINARY_UNIT
        } else {
            0.0
        },
    )
}

fn duration_ms(frames: usize, divisor: i16) -> u64 {
    let divisor = u64::try_from(divisor).unwrap_or(0);
    (frames as u64).saturating_mul(divisor).saturating_mul(1000) / ANIMATION_UNITS_PER_SECOND
}

fn units(value: f32) -> f64 {
    f64::from(value)
}

/// Shortest signed per-axis angular difference in binary units.
///
/// Stored binary angles are not normalized, so a raw subtraction across the
/// turn boundary would turn a small physical rotation into a full-turn spike.
/// Wrapping the difference into `[-half_turn, half_turn]` keeps the metric
/// angular. The reported degrees still use `value * 360 / 4096`.
fn angular_delta(to: f64, from: f64) -> f64 {
    let turn = f64::from(BINARY_ANGLE_UNITS_PER_TURN);
    let half = turn / 2.0;
    let mut delta = (to - from) % turn;
    if delta > half {
        delta -= turn;
    } else if delta < -half {
        delta += turn;
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 20.0;

    fn clip(frames: Vec<[f32; 3]>) -> Vec<Vec<[f32; 3]>> {
        frames.into_iter().map(|angles| vec![angles]).collect()
    }

    fn identical(frames: usize, angle: [f32; 3]) -> Vec<Vec<[f32; 3]>> {
        vec![vec![angle]; frames]
    }

    fn metrics(frames: &[Vec<[f32; 3]>], divisor: i16) -> ClipMetrics {
        ClipMetrics::from_frames(0, divisor, frames)
    }

    #[test]
    fn identical_frames_rank_still() {
        let frames = identical(10, [512.0, -256.0, 128.0]);
        let metrics = metrics(&frames, 100);
        assert_eq!(metrics.mean_delta_deg, 0.0);
        assert_eq!(metrics.max_delta_deg, 0.0);
        assert_eq!(metrics.moving_slots, 0);
        assert_eq!(metrics.classify(), ClipBucket::Still);
        let report = RankReport::from_metrics(vec![metrics]);
        assert_eq!(report.counts(), [1, 0, 0, 0]);
    }

    #[test]
    fn periodic_clip_detects_period_and_ranks_locomotion() {
        let values = [0.0f32, STEP, STEP * 2.0, STEP * 3.0];
        let frames: Vec<[f32; 3]> = (0..10)
            .map(|k| [values[k % values.len()], 0.0, 0.0])
            .collect();
        let frames = clip(frames);
        let metrics = metrics(&frames, 100);
        assert_eq!(metrics.period, Some(4), "{metrics:?}");
        assert_eq!(metrics.period_residual_deg, 0.0);
        assert!(metrics.mean_delta_deg > STILL_MAX_MEAN_DEG);
        assert!(metrics.mean_delta_deg <= LOCOMOTION_MAX_MEAN_DEG);
        assert_eq!(metrics.classify(), ClipBucket::Locomotion);
    }

    #[test]
    fn differences_wrap_at_the_turn_boundary_instead_of_spiking() {
        assert_eq!(angular_delta(10.0, 4090.0), 16.0);
        assert_eq!(angular_delta(4090.0, 10.0), -16.0);
        let frames = clip(vec![[4090.0, 0.0, 0.0], [10.0, 0.0, 0.0]]);
        let metrics = metrics(&frames, 100);
        let expected = 16.0 * DEGREES_PER_BINARY_UNIT;
        assert!(
            (metrics.max_delta_deg - expected).abs() < 1e-9,
            "{metrics:?}"
        );
    }

    #[test]
    fn a_trivial_period_one_is_not_locomotion_evidence() {
        let frames: Vec<[f32; 3]> = (0..10).map(|k| [STEP * k as f32, 0.0, 0.0]).collect();
        let frames = clip(frames);
        let metrics = metrics(&frames, 100);
        assert_eq!(metrics.period, Some(1), "adjacent shift wins the search");
        assert!(metrics.mean_delta_deg <= LOCOMOTION_MAX_MEAN_DEG);
        assert_eq!(metrics.classify(), ClipBucket::Other);
    }

    #[test]
    fn single_spike_ranks_impact() {
        let mut frames: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0]; 10];
        frames[5][0] = 2048.0;
        let frames = clip(frames);
        let metrics = metrics(&frames, 100);
        assert!(metrics.max_delta_deg >= IMPACT_MIN_MAX_DEG, "{metrics:?}");
        assert!(
            metrics.spike_ratio() >= IMPACT_MIN_SPIKE_RATIO,
            "{metrics:?}"
        );
        assert_eq!(metrics.classify(), ClipBucket::Impact);
    }

    #[test]
    fn loop_closure_is_zero_for_a_clean_loop_and_non_zero_otherwise() {
        let clean = clip(vec![
            [0.0, 0.0, 0.0],
            [STEP, 0.0, 0.0],
            [STEP * 2.0, 0.0, 0.0],
            [STEP, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ]);
        assert_eq!(metrics(&clean, 100).loop_closure_deg, 0.0);

        let open = clip(vec![
            [0.0, 0.0, 0.0],
            [STEP, 0.0, 0.0],
            [STEP * 2.0, 0.0, 0.0],
            [STEP, 0.0, 0.0],
        ]);
        let closure = metrics(&open, 100).loop_closure_deg;
        assert!(closure > 0.0, "{closure}");
        let expected = STEP as f64 / 3.0 * DEGREES_PER_BINARY_UNIT;
        assert!((closure - expected).abs() < 1e-6, "{closure} != {expected}");
    }

    #[test]
    fn duration_ms_is_correct_for_a_known_frames_and_divisor_pair() {
        assert_eq!(duration_ms(50, 100), 1000);
        assert_eq!(duration_ms(30, 50), 300);
        assert_eq!(duration_ms(0, 100), 0);
        let metrics = ClipMetrics::from_frames(0, 100, &identical(50, [0.0; 3]));
        assert_eq!(metrics.duration_ms, 1000);
    }

    #[test]
    fn report_respects_the_line_bound_and_the_table_parses() {
        let metrics = vec![
            ClipMetrics::from_frames(0, 100, &identical(10, [0.0; 3])),
            ClipMetrics::from_frames(
                1,
                100,
                &clip((0..10).map(|k| [STEP * (k % 4) as f32, 0.0, 0.0]).collect()),
            ),
            ClipMetrics::from_frames(2, 100, &{
                let mut frames = vec![[0.0f32, 0.0, 0.0]; 10];
                frames[5][0] = 2048.0;
                clip(frames)
            }),
            ClipMetrics::from_frames(
                3,
                100,
                &clip(
                    (0..10)
                        .map(|k| [STEP * 10.0 * k as f32, 0.0, 0.0])
                        .collect(),
                ),
            ),
        ];
        let report = RankReport::from_metrics(metrics);
        let text = report_text("SYNTH.SRA", &report, 2);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines.len() <= max_report_lines(2),
            "{} > {}",
            lines.len(),
            max_report_lines(2)
        );
        assert!(lines[0].starts_with("rank sra=SYNTH.SRA clips=4 still="));
        assert!(lines[1].starts_with("heuristic thresholds "));
        let suggestion = lines
            .iter()
            .find(|line| line.starts_with("suggest "))
            .expect("a suggestion line");
        let fields: Vec<&str> = suggestion.split_whitespace().collect();
        assert_eq!(fields.len(), 7, "{suggestion}");
        assert_eq!(fields[1], "--clip-idle");
        assert_eq!(fields[3], "--clip-walk");
        assert_eq!(fields[5], "--clip-jump");
        assert!(lines.iter().any(|line| line.starts_with("bucket STILL ")));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("clip index=0 frames=10 divisor=100 duration_ms=200 "))
        );
    }
}

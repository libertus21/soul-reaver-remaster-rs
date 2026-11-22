//! Read-only `.SRA` key-shape detector.
//!
//! The confirmed file-side key records from `docs/notes/spline-animation.md`
//! are implemented as byte-shape predicates:
//!
//! - rotation: `i16` frame delta + 4 x `i16` quaternion at stride `0x0a`
//!   (file) or `0x1c` (runtime, with a `f32` mirror), fixed-point scale
//!   `4096` (`>>12`);
//! - position: `i16` frame delta + 3 x `i16` position + 3 x `i32`
//!   tangent-out + 3 x `i32` tangent-in at stride `0x20` (file) or `0x30`
//!   (runtime, with a `f32` mirror).
//!
//! `.SRA` carries no channel descriptor, so [`find_*_runs`] scans raw payload
//! bytes for consecutive keys. This is a detector, not a decoder: it never
//! assigns a clip, joint or time axis, and it is not wired into the engine.
//!
//! The container parser is the two-level layout recorded in
//! `docs/notes/sra-animation.md`: a padded preamble, then a `0xFFFFFFFF`-closed
//! `u32` payload directory whose offsets are relative to the payload boundary.

use std::collections::HashSet;
use std::ops::Range;

use crate::packed_channel::{
    self, FRAME_INDEX_POLICY, FrameHeader, PackedFrameTable, TableForm,
};
use crate::pose::{SlotPose, rotation_from_binary_angles};

pub const ROTATION_SOURCE_STRIDE: usize = 0x0a;
pub const ROTATION_RUNTIME_STRIDE: usize = 0x1c;
pub const POSITION_SOURCE_STRIDE: usize = 0x20;
pub const POSITION_RUNTIME_STRIDE: usize = 0x30;
pub const FIXED_ONE: f64 = 4096.0;
pub const FRAME_DELTA_MAX: i32 = 4096;
pub const QUAT_COMPONENT_MAX: i32 = 8192;
pub const QUAT_LENGTH_TOLERANCE: f64 = 0.10;
pub const MIRROR_TOLERANCE: f64 = 0.02;
pub const TANGENT_MAX: i64 = 1_000_000;
const PAYLOAD_ALIGN: usize = 2048;
const MAX_COUNT: usize = 1 << 20;

/// Constant presentation clock rate, in animation time units per second.
///
/// The authored wall-clock rate is **UNKNOWN**. The live capture fixes only the
/// source-side unit: `frame_index = floor(frame_time / divisor)`, with
/// `frame_time = 2400` and divisor `100` selecting frame `24`. It says nothing
/// about how many time units the game advances per second. This value is
/// therefore an explicit presentation choice, not a measured engine constant.
///
/// The clock advances this many time units per simulated second, and each clip's
/// `divisor` turns those units into palette frames. That makes the divisor
/// load-bearing: a clip with a larger divisor plays proportionally slower.
///
/// `5000` is derived from the validated reference clip. It has divisor `100` and
/// 50 frames, and the previous presentation choice played it at `50` palette
/// frames per second. `50 frames/s * 100 units/frame = 5000 units/s`, so the
/// reference clip still makes one full 50-frame pass in one simulated second.
/// Change the reference speed with `--anim-fps <N>`; the clock itself stays
/// integer-only and deterministic.
pub const ANIMATION_UNITS_PER_SECOND: u64 = 5_000;

/// Divisor of the validated reference clip used to interpret `--anim-fps`.
pub const REFERENCE_DIVISOR: u64 = 100;

/// Presentation playback rate of the reference clip, in palette frames/second.
///
/// This is the `--anim-fps` unit: frames per second for a divisor-100 clip. It
/// is derived from [`ANIMATION_UNITS_PER_SECOND`] so the two cannot drift. A clip
/// with another divisor plays at `ANIMATION_UNITS_PER_SECOND / divisor` frames
/// per second.
pub const ANIMATION_FPS: u32 = (ANIMATION_UNITS_PER_SECOND / REFERENCE_DIVISOR) as u32;

/// Convert the `--anim-fps` reference speed to the clock's time units per second.
///
/// `fps` counts frames per second for a divisor-[`REFERENCE_DIVISOR`] clip, so
/// the equivalent constant rate is `fps * REFERENCE_DIVISOR` time units per
/// second.
pub fn units_per_second_for_fps(fps: u32) -> u64 {
    u64::from(fps) * REFERENCE_DIVISOR
}

/// How a playback clock handles a frame index past the last palette frame.
///
/// This is a presentation choice, separate from the decoder's conservative
/// [`FRAME_INDEX_POLICY`] (`Saturate`). Playback passes its own policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackLoop {
    /// Wrap modulo `frame_count`: gameplay looping. The default.
    #[default]
    Loop,
    /// Saturate at `frame_count - 1`: one pass, then hold the last frame.
    Once,
}

/// Map one canonical 60 Hz tick to a palette frame index for presentation.
///
/// The mapping is integer-only and deterministic:
///
/// ```text
/// units_elapsed = tick * units_per_second / 60
/// frame_index   = units_elapsed / divisor
/// index         = frame_index % frame_count           (PlaybackLoop::Loop)
/// index         = min(frame_index, frame_count - 1)   (PlaybackLoop::Once)
/// ```
///
/// `divisor` is the table's source-side frame duration, so it sets the clip's
/// relative speed: at the same constant rate, a clip with divisor `544` advances
/// `100 / 544` as fast as the validated divisor-`100` reference. The absolute
/// wall-clock rate stays UNKNOWN (see [`ANIMATION_UNITS_PER_SECOND`]).
///
/// The arithmetic is `u64` throughout, so large tick counts cannot hit the
/// `u16` frame-time ceiling that used to freeze playback.
pub fn frame_index_for_playback(
    tick: u64,
    units_per_second: u64,
    divisor: i16,
    frame_count: usize,
    policy: PlaybackLoop,
) -> Result<usize, String> {
    if units_per_second == 0 {
        return Err(
            "sra playback: rate 0 units per second cannot advance the animation".to_string(),
        );
    }
    let divisor = match u64::try_from(divisor) {
        Ok(value) if value > 0 => value,
        _ => {
            return Err(format!(
                "sra playback: divisor {divisor} must be positive to select a frame"
            ));
        }
    };
    if frame_count == 0 {
        return Err("sra playback: frame count 0 cannot select a frame".to_string());
    }
    let frame_count = u64::try_from(frame_count)
        .map_err(|_| format!("sra playback: frame count {frame_count} does not fit u64"))?;
    let units_elapsed = tick.saturating_mul(units_per_second) / 60;
    let frame_index = units_elapsed / divisor;
    let index = match policy {
        PlaybackLoop::Loop => frame_index % frame_count,
        PlaybackLoop::Once => frame_index.min(frame_count - 1),
    };
    usize::try_from(index)
        .map_err(|_| format!("sra playback: frame index {index} does not fit this platform"))
}

/// One maximal run of consecutive key-shaped records at a fixed stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRun {
    /// Byte offset of the first key in the scanned slice.
    pub start: usize,
    /// Number of consecutive keys.
    pub keys: usize,
    /// Sum of the frame deltas (`u32`; each delta is non-negative).
    pub span: u32,
}

/// Decoded file-side rotation key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationKey {
    pub delta: i16,
    /// Stored order w, x, y, z; components are 12-bit fixed point.
    pub quat: [i16; 4],
}

/// Decoded file-side position key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionKey {
    pub delta: i16,
    pub position: [i16; 3],
    pub out_tangent: [i32; 3],
    pub in_tangent: [i32; 3],
}

/// One payload record span, parsed from the two-level container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub payload_base: usize,
    pub records: Vec<Range<usize>>,
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at.checked_add(4)?)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_i16(bytes: &[u8], at: usize) -> Option<i16> {
    bytes
        .get(at..at.checked_add(2)?)
        .map(|s| i16::from_le_bytes([s[0], s[1]]))
}

fn read_i32(bytes: &[u8], at: usize) -> Option<i32> {
    bytes
        .get(at..at.checked_add(4)?)
        .map(|s| i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_f32(bytes: &[u8], at: usize) -> Option<f32> {
    bytes
        .get(at..at.checked_add(4)?)
        .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Parse the `.SRA` container: preamble, 2 KiB-aligned payload boundary, and
/// the `0xFFFFFFFF`-closed payload directory. Returns `None` on any structural
/// violation (never a partial read).
pub fn parse_container(bytes: &[u8]) -> Option<Container> {
    let count = read_u32(bytes, 0)? as usize;
    if !(2..=MAX_COUNT).contains(&count) {
        return None;
    }
    // [count][0][count-1 offsets][0]
    let preamble_size = 8usize.checked_add(4 * count)?;
    if preamble_size > bytes.len() {
        return None;
    }
    let payload_base = preamble_size.div_ceil(PAYLOAD_ALIGN).max(1) * PAYLOAD_ALIGN;
    if payload_base.checked_add(4)? > bytes.len() {
        return None;
    }
    let mut offsets = Vec::new();
    let mut at = payload_base;
    loop {
        let value = read_u32(bytes, at)?;
        at = at.checked_add(4)?;
        if value == u32::MAX {
            break;
        }
        offsets.push(value as usize);
        if offsets.len() > MAX_COUNT {
            return None;
        }
    }
    if offsets.is_empty() || offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }
    let mut records = Vec::with_capacity(offsets.len());
    for (index, offset) in offsets.iter().enumerate() {
        let start = payload_base.checked_add(*offset)?;
        let end = match offsets.get(index + 1) {
            Some(next) => payload_base.checked_add(*next)?,
            None => bytes.len(),
        };
        if start < payload_base || end > bytes.len() || start >= end {
            return None;
        }
        records.push(start..end);
    }
    Some(Container { payload_base, records })
}

/// The normalized length of a stored quaternion after `>>12`, if it is inside
/// the confirmed component bound.
pub fn quaternion_length(quat: [i16; 4]) -> Option<f64> {
    if quat.iter().any(|&c| i32::from(c).abs() > QUAT_COMPONENT_MAX) {
        return None;
    }
    let sum: f64 = quat
        .iter()
        .map(|&c| {
            let v = f64::from(c) / FIXED_ONE;
            v * v
        })
        .sum();
    Some(sum.sqrt())
}

/// File-side rotation key: `i16` delta + 4 x `i16` quaternion.
pub fn rotation_key(bytes: &[u8], at: usize) -> Option<RotationKey> {
    let delta = read_i16(bytes, at)?;
    if !(0..=FRAME_DELTA_MAX as i16).contains(&delta) {
        return None;
    }
    let mut quat = [0i16; 4];
    for (index, component) in quat.iter_mut().enumerate() {
        *component = read_i16(bytes, at.checked_add(2 + 2 * index)?)?;
    }
    let length = quaternion_length(quat)?;
    if (length - 1.0).abs() > QUAT_LENGTH_TOLERANCE {
        return None;
    }
    Some(RotationKey { delta, quat })
}

/// Runtime rotation key at stride `0x1c`: the file-side fields plus the four
/// `f32` mirrors of the same quaternion (fixed-point / 4096).
pub fn rotation_runtime_key(bytes: &[u8], at: usize) -> Option<RotationKey> {
    // `delta, 2-byte pad, quat at +4, four f32 mirrors at +12` == 0x1c.
    let delta = read_i16(bytes, at)?;
    if !(0..=FRAME_DELTA_MAX as i16).contains(&delta) {
        return None;
    }
    let mut quat = [0i16; 4];
    for (index, component) in quat.iter_mut().enumerate() {
        *component = read_i16(bytes, at.checked_add(4 + 2 * index)?)?;
    }
    let length = quaternion_length(quat)?;
    if (length - 1.0).abs() > QUAT_LENGTH_TOLERANCE {
        return None;
    }
    for (index, &component) in quat.iter().enumerate() {
        let mirror = read_f32(bytes, at.checked_add(12 + 4 * index)?)?;
        if !mirror.is_finite() {
            return None;
        }
        let expected = f64::from(component) / FIXED_ONE;
        if (f64::from(mirror) - expected).abs() > MIRROR_TOLERANCE {
            return None;
        }
    }
    Some(RotationKey { delta, quat })
}

/// File-side position key: `i16` delta + 3 x `i16` position + 3 x `i32`
/// tangent-out + 3 x `i32` tangent-in.
pub fn position_key(bytes: &[u8], at: usize) -> Option<PositionKey> {
    let delta = read_i16(bytes, at)?;
    if !(0..=FRAME_DELTA_MAX as i16).contains(&delta) {
        return None;
    }
    let mut position = [0i16; 3];
    for (index, value) in position.iter_mut().enumerate() {
        *value = read_i16(bytes, at.checked_add(2 + 2 * index)?)?;
    }
    let mut out_tangent = [0i32; 3];
    let mut in_tangent = [0i32; 3];
    for index in 0..3 {
        out_tangent[index] = read_i32(bytes, at.checked_add(8 + 4 * index)?)?;
        in_tangent[index] = read_i32(bytes, at.checked_add(20 + 4 * index)?)?;
    }
    if out_tangent
        .iter()
        .chain(in_tangent.iter())
        .any(|&value| i64::from(value).abs() > TANGENT_MAX)
    {
        return None;
    }
    Some(PositionKey {
        delta,
        position,
        out_tangent,
        in_tangent,
    })
}

/// Runtime position key at stride `0x30`: delta, 2-byte pad, three `i16`
/// positions, 2-byte pad, three `f32` mirrors, then the six `i32` tangents.
pub fn position_runtime_key(bytes: &[u8], at: usize) -> Option<PositionKey> {
    let delta = read_i16(bytes, at)?;
    if !(0..=FRAME_DELTA_MAX as i16).contains(&delta) {
        return None;
    }
    let mut position = [0i16; 3];
    for (index, value) in position.iter_mut().enumerate() {
        *value = read_i16(bytes, at.checked_add(4 + 2 * index)?)?;
    }
    for (index, &value) in position.iter().enumerate() {
        let mirror = read_f32(bytes, at.checked_add(12 + 4 * index)?)?;
        if !mirror.is_finite() {
            return None;
        }
        let expected = f64::from(value) / FIXED_ONE;
        if (f64::from(mirror) - expected).abs() > MIRROR_TOLERANCE {
            return None;
        }
    }
    let mut out_tangent = [0i32; 3];
    let mut in_tangent = [0i32; 3];
    for index in 0..3 {
        out_tangent[index] = read_i32(bytes, at.checked_add(24 + 4 * index)?)?;
        in_tangent[index] = read_i32(bytes, at.checked_add(36 + 4 * index)?)?;
    }
    if out_tangent
        .iter()
        .chain(in_tangent.iter())
        .any(|&value| i64::from(value).abs() > TANGENT_MAX)
    {
        return None;
    }
    Some(PositionKey {
        delta,
        position,
        out_tangent,
        in_tangent,
    })
}

/// Maximal runs of consecutive keys at `stride`, scanning `range` byte by
/// byte (keys are not assumed to be aligned). `delta` returns the key's frame
/// delta when the record at `at` is key-shaped.
fn find_runs<F>(bytes: &[u8], range: Range<usize>, stride: usize, min_keys: usize, delta: F) -> Vec<KeyRun>
where
    F: Fn(&[u8], usize) -> Option<i16>,
{
    let start = range.start.min(bytes.len());
    let end = range.end.min(bytes.len());
    let mut runs = Vec::new();
    let mut at = start;
    while at.saturating_add(stride) <= end {
        let Some(first) = delta(bytes, at) else {
            at += 1;
            continue;
        };
        let mut keys = 1usize;
        let mut span = u32::from(first as u16);
        let mut next = at + stride;
        while next.saturating_add(stride) <= end {
            let Some(key) = delta(bytes, next) else {
                break;
            };
            keys += 1;
            span = span.saturating_add(u32::from(key as u16));
            next += stride;
        }
        if keys >= min_keys && span > 0 {
            runs.push(KeyRun {
                start: at,
                keys,
                span,
            });
        }
        at = if keys >= min_keys { next } else { at + 1 };
    }
    runs
}

/// Runs of file-side rotation keys (stride `0x0a`).
pub fn rotation_runs(bytes: &[u8], range: Range<usize>, min_keys: usize) -> Vec<KeyRun> {
    find_runs(bytes, range, ROTATION_SOURCE_STRIDE, min_keys, |b, at| {
        rotation_key(b, at).map(|key| key.delta)
    })
}

/// Runs of runtime rotation keys (stride `0x1c`).
pub fn rotation_runtime_runs(bytes: &[u8], range: Range<usize>, min_keys: usize) -> Vec<KeyRun> {
    find_runs(bytes, range, ROTATION_RUNTIME_STRIDE, min_keys, |b, at| {
        rotation_runtime_key(b, at).map(|key| key.delta)
    })
}

/// Runs of file-side position keys (stride `0x20`).
pub fn position_runs(bytes: &[u8], range: Range<usize>, min_keys: usize) -> Vec<KeyRun> {
    find_runs(bytes, range, POSITION_SOURCE_STRIDE, min_keys, |b, at| {
        position_key(b, at).map(|key| key.delta)
    })
}

/// Runs of runtime position keys (stride `0x30`).
pub fn position_runtime_runs(bytes: &[u8], range: Range<usize>, min_keys: usize) -> Vec<KeyRun> {
    find_runs(bytes, range, POSITION_RUNTIME_STRIDE, min_keys, |b, at| {
        position_runtime_key(b, at).map(|key| key.delta)
    })
}

/// Encode one file-side rotation key. Test and fixture helper.
pub fn encode_rotation_key(delta: i16, quat: [i16; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ROTATION_SOURCE_STRIDE);
    out.extend_from_slice(&delta.to_le_bytes());
    for component in quat {
        out.extend_from_slice(&component.to_le_bytes());
    }
    out
}

/// Encode one runtime rotation key (`0x1c`). Test and fixture helper.
pub fn encode_rotation_runtime_key(delta: i16, quat: [i16; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ROTATION_RUNTIME_STRIDE);
    out.extend_from_slice(&delta.to_le_bytes());
    out.extend_from_slice(&[0u8; 2]);
    for component in quat {
        out.extend_from_slice(&component.to_le_bytes());
    }
    for component in quat {
        let mirror = f32::from(component) / FIXED_ONE as f32;
        out.extend_from_slice(&mirror.to_le_bytes());
    }
    out
}

/// Wrap `record` bytes in a valid two-level `.SRA` container with one payload
/// record. Test and fixture helper.
pub fn encode_single_record_container(record: &[u8]) -> Vec<u8> {
    let count = 2usize;
    let preamble_size = 8 + 4 * count;
    let payload_base = preamble_size.div_ceil(PAYLOAD_ALIGN).max(1) * PAYLOAD_ALIGN;
    let mut bytes = vec![0u8; payload_base + 8];
    bytes[0..4].copy_from_slice(&(count as u32).to_le_bytes());
    // Directory: one offset (8, right after the 8-byte directory) then 0xFFFFFFFF.
    bytes[payload_base..payload_base + 4].copy_from_slice(&8u32.to_le_bytes());
    bytes[payload_base + 4..payload_base + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(record);
    bytes
}

/// One decoded animation frame: per-slot binary-angle triples (`X`, `Y`, `Z`).
#[derive(Debug, Clone, PartialEq)]
pub struct SraFrame {
    /// Palette frame index used for this decode.
    pub frame_index: usize,
    /// Number of frames in the table.
    pub frame_count: usize,
    /// Number of value streams the table header declares.
    pub channel_count: usize,
    /// One angle triple per slot, in slot order.
    pub angles: Vec<[f32; 3]>,
}

/// Public descriptor of one located clip (packed frame table).
///
/// The index is the clip's position among the located tables, in payload-record
/// file order. `record` is the payload-directory record it came from and
/// `offset` is the table's byte offset relative to that record's start. The
/// table itself carries no name or id (`docs/notes/packed-channel-format.md`,
/// "Clip enumeration and state selection"): these fields are the only stable
/// identifiers, and they are structural, not semantic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SraClipInfo {
    pub index: usize,
    pub record: usize,
    pub offset: usize,
    pub frame_count: usize,
    pub divisor: i16,
    pub channel_count: usize,
    pub flags: u8,
    /// Content-only table fingerprint (`frame_count:divisor:hash` hash part).
    /// This is the form-invariant digest a runtime observation maps on.
    pub fingerprint: u32,
    /// Legacy 20-byte-header digest. Diagnostic only: in memory the extra
    /// field is a relocated pointer, so this digest is neither form- nor
    /// session-invariant.
    pub legacy_fingerprint: u32,
    /// Whether this table was found after the first table in its record.
    pub additional: bool,
}

impl SraClipInfo {
    /// The labelling probe's human-readable fingerprint key.
    pub fn fingerprint_text(&self) -> String {
        format!(
            "{}:{}:{:08x}",
            self.frame_count, self.divisor, self.fingerprint
        )
    }

    /// The diagnostic-only legacy key.
    pub fn legacy_fingerprint_text(&self) -> String {
        format!(
            "{}:{}:{:08x}",
            self.frame_count, self.divisor, self.legacy_fingerprint
        )
    }
}

/// One candidate table rejected during the extended scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RejectedTable {
    /// Payload record index.
    pub record: usize,
    /// Byte offset relative to the record start.
    pub offset: usize,
    pub frame_count: u16,
    pub divisor: i16,
    pub channel_count: u8,
}

/// How the extended enumeration classified a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipScanStats {
    /// First structurally valid table per payload record (indices `0..base`).
    pub base_count: usize,
    /// Additional non-duplicate tables appended after `base_count`.
    pub additional_distinct: usize,
    /// Valid additional tables dropped because their fingerprint was indexed.
    pub additional_duplicates: usize,
    /// Distinct fingerprints across base plus additional tables.
    pub distinct_fingerprints: usize,
    /// `+0x04`/`+0x08` header-word aliases: rejected, never indexed.
    pub rejected_aliases: Vec<RejectedTable>,
    /// Header-shaped candidates past a table's extent that failed full validity.
    pub rejected_invalid: usize,
}

/// A clip index outside the located set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipError {
    OutOfRange { clip: usize, clip_count: usize },
}

impl std::fmt::Display for ClipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange { clip, clip_count } => {
                write!(f, "clip {clip} is outside the {clip_count} decoded clips")
            }
        }
    }
}

impl std::error::Error for ClipError {}

/// One located packed-channel table inside one `.SRA` payload record.
#[derive(Debug, Clone)]
struct ClipTable {
    record: usize,
    offset: usize,
    table: Range<usize>,
    header: FrameHeader,
    flags: u8,
    fingerprint: u32,
    legacy_fingerprint: u32,
    additional: bool,
}

/// All packed-channel tables in one `.SRA` file, bound to a model slot count.
///
/// [`SraAnimation::load`] scans the container's payload records in file order.
/// It keeps the **first** structurally valid table in each record as the base
/// clip (`0..base_count`), exactly as the shipped enumeration always did, then
/// continues past that table's inline value stream to append any further
/// structurally valid, non-duplicate tables after the base indices.
///
/// The `+0x04`/`+0x08` duplicate header word parses as a bogus one-stream table
/// (`docs/notes/packed-channel-format.md`, "Clip enumeration and state
/// selection"). A candidate nested in the base table's 20-byte header with the
/// base frame count and divisor is therefore rejected and reported, never
/// indexed.
///
/// The slot count is the model's bone count, not an `.SRA` field
/// (`docs/notes/sra-slot-pose.md`).
///
/// Decoding is on demand, so a frame is a pure function of a table and the
/// requested index; no clock or wall time is stored here. The zero-argument
/// accessors keep addressing clip 0 so existing callers are unchanged.
#[derive(Debug, Clone)]
pub struct SraAnimation {
    bytes: Vec<u8>,
    clips: Vec<ClipTable>,
    slot_count: usize,
    stats: ClipScanStats,
}

/// One table located by the structural scan, with its byte extent and hash.
#[derive(Debug, Clone, Copy)]
struct LocatedTable {
    header: FrameHeader,
    flags: u8,
    /// First byte after the inline value stream, relative to the table start.
    end: usize,
    fingerprint: u32,
    legacy_fingerprint: u32,
}

/// Structural validity the loader has always used: parse the header and value
/// offsets, validate the bit planes, require the angle plane, and decode the
/// whole inline stream for `slot_count` slots.
fn located_table(slice: &[u8], slot_count: usize) -> Option<LocatedTable> {
    let table = PackedFrameTable::parse(slice, TableForm::File).ok()?;
    let planes = table.bit_planes(slot_count).ok()?;
    if planes.flags() & 0x01 == 0 {
        return None;
    }
    let end = packed_channel::inline_stream_end(&table, slot_count).ok()?;
    let fingerprint = table.fingerprint(slot_count).ok()?;
    let legacy_fingerprint = table.legacy_fingerprint(slot_count).ok()?;
    Some(LocatedTable {
        header: table.header(),
        flags: planes.flags(),
        end,
        fingerprint,
        legacy_fingerprint,
    })
}

/// The first structurally valid table in one payload record and its offset.
fn first_table(slice: &[u8], slot_count: usize) -> Option<(usize, LocatedTable)> {
    (0..slice.len()).find_map(|offset| {
        located_table(&slice[offset..], slot_count).map(|table| (offset, table))
    })
}

/// The `+0x04`/`+0x08` duplicate header word aliases inside the base header.
///
/// The header stores `(divisor << 16) | frame_count` at `+0x08` and `+0x0C`.
/// Read one word earlier those bytes look like a one-stream table with the same
/// frame count and divisor, so a byte scan would index the same clip twice.
/// These candidates are rejected and reported, never appended.
fn header_aliases(
    slice: &[u8],
    record: usize,
    base_offset: usize,
    base: &FrameHeader,
) -> Vec<RejectedTable> {
    let end = base_offset
        .saturating_add(packed_channel::FINGERPRINT_HEADER_BYTES)
        .min(slice.len());
    let mut rejected = Vec::new();
    for candidate in (base_offset + 1)..end {
        let Ok(header) = packed_channel::parse_header(&slice[candidate..]) else {
            continue;
        };
        if header.frame_count == base.frame_count && header.divisor == base.divisor {
            rejected.push(RejectedTable {
                record,
                offset: candidate,
                frame_count: header.frame_count,
                divisor: header.divisor,
                channel_count: header.table_count,
            });
        }
    }
    rejected
}

/// Continue the byte scan after a table's inline stream and append any further
/// structurally valid, non-duplicate tables.
///
/// `start` is the first byte after the table just accepted. A structural match
/// advances past its own stream so nested matches inside it are never counted.
fn scan_additional(
    slice: &[u8],
    record: usize,
    range: &Range<usize>,
    start: usize,
    slot_count: usize,
    fingerprints: &mut HashSet<u32>,
    clips: &mut Vec<ClipTable>,
    stats: &mut ClipScanStats,
) {
    let mut cursor = start.min(slice.len());
    while cursor < slice.len() {
        if let Some(located) = located_table(&slice[cursor..], slot_count) {
            if fingerprints.insert(located.fingerprint) {
                clips.push(ClipTable {
                    record,
                    offset: cursor,
                    table: range.start + cursor..range.end,
                    header: located.header,
                    flags: located.flags,
                    fingerprint: located.fingerprint,
                    legacy_fingerprint: located.legacy_fingerprint,
                    additional: true,
                });
                stats.additional_distinct += 1;
            } else {
                stats.additional_duplicates += 1;
            }
            cursor = cursor.saturating_add(located.end.max(1));
        } else {
            if packed_channel::parse_header(&slice[cursor..]).is_ok() {
                stats.rejected_invalid += 1;
            }
            cursor += 1;
        }
    }
}

impl SraAnimation {
    /// Locate every clip and bind them to a model's slot count.
    ///
    /// A container that does not parse, or a payload with no table that decodes
    /// for that many slots, is a loud error. Base indices are stable: they are
    /// the first valid table of each record in file order, so a new table in a
    /// record never shifts an earlier index.
    pub fn load(bytes: Vec<u8>, slot_count: usize) -> Result<Self, String> {
        if slot_count == 0 {
            return Err("sra animation: slot count 0 cannot size the bit planes".to_string());
        }
        let container = parse_container(&bytes).ok_or_else(|| {
            "sra animation: the .SRA container directory did not parse".to_string()
        })?;
        let mut clips = Vec::new();
        let mut fingerprints = HashSet::new();
        let mut stats = ClipScanStats::default();
        for (record, range) in container.records.iter().enumerate() {
            let slice = &bytes[range.clone()];
            let Some((offset, base)) = first_table(slice, slot_count) else {
                continue;
            };
            clips.push(ClipTable {
                record,
                offset,
                table: range.start + offset..range.end,
                header: base.header,
                flags: base.flags,
                fingerprint: base.fingerprint,
                legacy_fingerprint: base.legacy_fingerprint,
                additional: false,
            });
            fingerprints.insert(base.fingerprint);
            stats.base_count += 1;
            stats.rejected_aliases.extend(header_aliases(
                slice,
                record,
                offset,
                &base.header,
            ));
            scan_additional(
                slice,
                record,
                range,
                offset + base.end,
                slot_count,
                &mut fingerprints,
                &mut clips,
                &mut stats,
            );
        }
        if clips.is_empty() {
            return Err(format!(
                "sra animation: no packed frame table decoded for {slot_count} slots"
            ));
        }
        stats.distinct_fingerprints = fingerprints.len();
        Ok(Self {
            bytes,
            clips,
            slot_count,
            stats,
        })
    }

    /// How the extended enumeration classified this file.
    pub fn scan_stats(&self) -> &ClipScanStats {
        &self.stats
    }

    /// Number of base clips (one per payload record, the stable `0..` prefix).
    pub fn base_clip_count(&self) -> usize {
        self.stats.base_count
    }

    /// Number of slots this animation decodes.
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Number of clips (base plus appended additional tables), in file order.
    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }

    /// Descriptors of every located clip, in file order.
    pub fn clips(&self) -> Vec<SraClipInfo> {
        (0..self.clips.len())
            .filter_map(|index| self.clip_info(index).ok())
            .collect()
    }

    /// Descriptor of one clip.
    pub fn clip_info(&self, clip: usize) -> Result<SraClipInfo, ClipError> {
        let located = self.clip_at(clip)?;
        Ok(SraClipInfo {
            index: clip,
            record: located.record,
            offset: located.offset,
            frame_count: usize::from(located.header.frame_count),
            divisor: located.header.divisor,
            channel_count: usize::from(located.header.table_count),
            flags: located.flags,
            fingerprint: located.fingerprint,
            legacy_fingerprint: located.legacy_fingerprint,
            additional: located.additional,
        })
    }

    /// Reject a clip index that is outside the located set.
    pub fn validate_clip(&self, clip: usize) -> Result<(), ClipError> {
        if clip < self.clips.len() {
            Ok(())
        } else {
            Err(ClipError::OutOfRange {
                clip,
                clip_count: self.clips.len(),
            })
        }
    }

    /// Number of frames in one clip.
    pub fn frame_count_of(&self, clip: usize) -> Result<usize, ClipError> {
        self.clip_at(clip).map(|clip| usize::from(clip.header.frame_count))
    }

    /// Frame-clock divisor of one clip.
    pub fn divisor_of(&self, clip: usize) -> Result<i16, ClipError> {
        self.clip_at(clip).map(|clip| clip.header.divisor)
    }

    /// Number of value streams one clip declares.
    pub fn channel_count_of(&self, clip: usize) -> Result<usize, ClipError> {
        self.clip_at(clip).map(|clip| usize::from(clip.header.table_count))
    }

    /// Number of frames in the default (first) clip.
    pub fn frame_count(&self) -> usize {
        self.clips
            .first()
            .map_or(0, |clip| usize::from(clip.header.frame_count))
    }

    /// Number of value streams the default clip declares.
    pub fn channel_count(&self) -> usize {
        self.clips
            .first()
            .map_or(0, |clip| usize::from(clip.header.table_count))
    }

    /// The default clip's frame-clock divisor.
    pub fn divisor(&self) -> i16 {
        self.clips.first().map_or(1, |clip| clip.header.divisor)
    }

    /// Palette frame index for one canonical tick in one clip.
    ///
    /// See [`frame_index_for_playback`]. The source divisor comes from that
    /// clip; the rate and wrap choice are supplied by the caller, so the
    /// presentation clock is never a silent guess.
    pub fn frame_index_for_clip_tick(
        &self,
        clip: usize,
        tick: u64,
        units_per_second: u64,
        policy: PlaybackLoop,
    ) -> Result<usize, String> {
        let table = self.clip_at(clip).map_err(|error| error.to_string())?;
        frame_index_for_playback(
            tick,
            units_per_second,
            table.header.divisor,
            usize::from(table.header.frame_count),
            policy,
        )
    }

    /// Palette frame index for one explicit frame-time unit in one clip.
    pub fn frame_index_for_clip_time(
        &self,
        clip: usize,
        frame_time: u16,
    ) -> Result<usize, String> {
        let table = self.clip_at(clip).map_err(|error| error.to_string())?;
        packed_channel::select_frame_index(
            frame_time,
            table.header.divisor,
            table.header.frame_count,
            FRAME_INDEX_POLICY,
        )
    }

    /// Palette frame index for one canonical tick in the default clip.
    ///
    /// See [`frame_index_for_playback`]. The source divisor comes from the
    /// located table; the rate and wrap choice are supplied by the caller, so
    /// the presentation clock is never a silent guess.
    pub fn frame_index_for_tick(
        &self,
        tick: u64,
        units_per_second: u64,
        policy: PlaybackLoop,
    ) -> Result<usize, String> {
        self.frame_index_for_clip_tick(0, tick, units_per_second, policy)
    }

    /// Palette frame index for one explicit frame-time unit in the default clip.
    pub fn frame_index_for_time(&self, frame_time: u16) -> Result<usize, String> {
        self.frame_index_for_clip_time(0, frame_time)
    }

    /// Decode one palette frame of one clip.
    pub fn decode_clip_index(&self, clip: usize, frame_index: usize) -> Result<SraFrame, String> {
        let table = self.clip_at(clip).map_err(|error| error.to_string())?;
        let packed = self.table(clip)?;
        let decoded = packed_channel::decode_inline_channel(&packed, self.slot_count, frame_index)?;
        Ok(SraFrame {
            frame_index: decoded.frame_index,
            frame_count: usize::from(table.header.frame_count),
            channel_count: usize::from(table.header.table_count),
            angles: decoded.slots.iter().map(|slot| slot.angles).collect(),
        })
    }

    /// Decode one palette frame of one clip, keeping all twelve lanes.
    ///
    /// Offline metrics need the translation lanes the angle-only
    /// [`Self::decode_clip_index`] discards. The lane order is angle `0..=2`,
    /// scale `4..=6`, translation `8..=10` (`docs/notes/packed-channel-format.md`).
    pub fn decode_clip_lanes(
        &self,
        clip: usize,
        frame_index: usize,
    ) -> Result<Vec<[f32; 12]>, String> {
        let packed = self.table(clip)?;
        packed_channel::decode_inline_lanes(&packed, self.slot_count, frame_index)
    }

    /// Decode the frame selected by one frame-time unit in one clip.
    pub fn decode_clip_time(&self, clip: usize, frame_time: u16) -> Result<SraFrame, String> {
        self.decode_clip_index(clip, self.frame_index_for_clip_time(clip, frame_time)?)
    }

    /// Decode the frame selected by one canonical tick in one clip.
    pub fn decode_clip_tick(
        &self,
        clip: usize,
        tick: u64,
        units_per_second: u64,
        policy: PlaybackLoop,
    ) -> Result<SraFrame, String> {
        self.decode_clip_index(
            clip,
            self.frame_index_for_clip_tick(clip, tick, units_per_second, policy)?,
        )
    }

    /// Decode one palette frame of the default clip.
    pub fn decode_index(&self, frame_index: usize) -> Result<SraFrame, String> {
        self.decode_clip_index(0, frame_index)
    }

    /// Decode the frame selected by one frame-time unit in the default clip.
    pub fn decode_time(&self, frame_time: u16) -> Result<SraFrame, String> {
        self.decode_clip_time(0, frame_time)
    }

    /// Decode the frame selected by one canonical tick in the default clip.
    pub fn decode_tick(
        &self,
        tick: u64,
        units_per_second: u64,
        policy: PlaybackLoop,
    ) -> Result<SraFrame, String> {
        self.decode_clip_tick(0, tick, units_per_second, policy)
    }

    /// The slot-pose list for `frame`, ready for the hierarchical skinner.
    pub fn poses(&self, frame: &SraFrame) -> Vec<SlotPose> {
        poses_from_angles(&frame.angles)
    }

    /// Decode the tick's frame of one clip under the playback clock and build
    /// its poses.
    pub fn poses_for_clip_tick(
        &self,
        clip: usize,
        tick: u64,
        units_per_second: u64,
        policy: PlaybackLoop,
    ) -> Result<Vec<SlotPose>, String> {
        let frame = self.decode_clip_tick(clip, tick, units_per_second, policy)?;
        Ok(self.poses(&frame))
    }

    /// Decode the default clip's tick frame and build its poses.
    pub fn poses_for_tick(
        &self,
        tick: u64,
        units_per_second: u64,
        policy: PlaybackLoop,
    ) -> Result<Vec<SlotPose>, String> {
        self.poses_for_clip_tick(0, tick, units_per_second, policy)
    }

    fn clip_at(&self, clip: usize) -> Result<&ClipTable, ClipError> {
        self.clips.get(clip).ok_or(ClipError::OutOfRange {
            clip,
            clip_count: self.clips.len(),
        })
    }

    fn table(&self, clip: usize) -> Result<PackedFrameTable<'_>, String> {
        let located = self.clip_at(clip).map_err(|error| error.to_string())?;
        PackedFrameTable::parse(&self.bytes[located.table.clone()], TableForm::File)
    }
}

/// The deterministic player motion state animation selection reads.
///
/// Derived from the grounded flag, the committed horizontal delta and the
/// signed vertical velocity; `Landing` also uses the touchdown edge and window
/// owned by [`MotionTracker`]. Presentation state only: never in the canonical
/// fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionState {
    /// Grounded with no committed horizontal movement.
    Idle,
    /// Grounded with committed horizontal movement.
    Moving,
    /// Not grounded with a positive (rising) vertical velocity.
    Ascending,
    /// Not grounded with a zero or negative (falling) vertical velocity.
    Falling,
    /// Grounded inside the landing window that opened on the touchdown tick.
    Landing,
}

impl MotionState {
    /// Classify one sample from the existing player signals.
    ///
    /// This pure half never sees the landing window. `vertical_velocity` is the
    /// signed fixed-point velocity of
    /// [`FrameState::player_vertical_velocity`](crate::engine::frame::FrameState::player_vertical_velocity);
    /// exactly zero counts as `Falling`: a non-grounded player with no upward
    /// motion is on its way down (or at the apex).
    pub fn from_player_signals(grounded: bool, moving: bool, vertical_velocity: i64) -> Self {
        if grounded {
            if moving {
                Self::Moving
            } else {
                Self::Idle
            }
        } else if vertical_velocity > 0 {
            Self::Ascending
        } else {
            Self::Falling
        }
    }

    /// The label-store field name for this state.
    pub fn label_field(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Moving => "walk",
            Self::Ascending => "jump_up",
            Self::Falling => "fall",
            Self::Landing => "land",
        }
    }
}

/// Shortest landing window, in 60 Hz ticks.
///
/// Four ticks (`66.7 ms`) is the shortest hold visible as a distinct pose; a
/// one-frame, low-divisor land clip would otherwise derive a zero- or one-tick
/// window and never present its own first frame.
pub const LANDING_WINDOW_MIN_TICKS: u64 = 4;

/// Longest landing window, in 60 Hz ticks.
///
/// Landing is a recovery phase, not a loop: `30` ticks (`0.5 s`) covers a slow
/// two-frame land clip with room, and stops a very long clip from holding the
/// state for seconds.
pub const LANDING_WINDOW_MAX_TICKS: u64 = 30;

/// Derive the landing window from the `land` clip's own duration.
///
/// One pass takes `frames * divisor / units_per_second` seconds at
/// [`ANIMATION_UNITS_PER_SECOND`], so at the fixed 60 Hz clock the hold is
/// `floor(frames * divisor * 60 / units_per_second)` ticks, clamped to
/// [`LANDING_WINDOW_MIN_TICKS`]..=[`LANDING_WINDOW_MAX_TICKS`]. A zero frame
/// count, non-positive divisor or zero rate falls back to the minimum instead
/// of dividing by zero. The shipped land clip (2 frames, divisor 400) gives
/// `floor(2 * 400 * 60 / 5000) = 9` ticks (`0.15 s`).
pub fn landing_window_ticks(frame_count: usize, divisor: i16, units_per_second: u64) -> u64 {
    let (Ok(frames), Ok(divisor)) = (u64::try_from(frame_count), u64::try_from(divisor)) else {
        return LANDING_WINDOW_MIN_TICKS;
    };
    if frames == 0 || divisor == 0 || units_per_second == 0 {
        return LANDING_WINDOW_MIN_TICKS;
    }
    let units = frames.saturating_mul(divisor).saturating_mul(60);
    (units / units_per_second).clamp(LANDING_WINDOW_MIN_TICKS, LANDING_WINDOW_MAX_TICKS)
}

/// Stateful half of the motion machine: the touchdown edge and its window.
///
/// [`MotionState::from_player_signals`] is memoryless, so it cannot express
/// `Landing`. This tracker keeps only the previous grounded sample and the
/// exclusive tick at which the open window closes. A touchdown is `grounded`
/// now with a previous observation of `false`; a first observation has no
/// previous sample, so a spawn already resting on the floor does not play a
/// landing. The window is `[touchdown, touchdown + landing_ticks)`: the
/// touchdown tick is `Landing` and the exclusive bound releases to
/// `Idle`/`Moving`. Leaving the ground clears the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionTracker {
    landing_ticks: u64,
    landing_until: Option<u64>,
    last_grounded: Option<bool>,
}

impl MotionTracker {
    /// Build a tracker with an explicit landing window in ticks.
    pub fn new(landing_ticks: u64) -> Self {
        Self {
            landing_ticks,
            landing_until: None,
            last_grounded: None,
        }
    }

    /// Observe one rendered frame's player signals and return its motion state.
    pub fn observe(
        &mut self,
        tick: u64,
        grounded: bool,
        moving: bool,
        vertical_velocity: i64,
    ) -> MotionState {
        let touchdown = grounded && self.last_grounded == Some(false);
        self.last_grounded = Some(grounded);
        if !grounded {
            self.landing_until = None;
            return MotionState::from_player_signals(grounded, moving, vertical_velocity);
        }
        if touchdown {
            self.landing_until = Some(tick.saturating_add(self.landing_ticks));
        }
        if self.landing_until.is_some_and(|until| tick < until) {
            return MotionState::Landing;
        }
        self.landing_until = None;
        MotionState::from_player_signals(grounded, moving, vertical_velocity)
    }
}

/// Default idle clip: **CONFIRMED** by runtime observation.
///
/// Clip `55` (`6:580`) held coverage `1.00` in three of four still-window runs;
/// one earlier run saw clip `0` (`50:100`) at the same coverage, so idle has a
/// variant. `55` is the most frequent observation and is the default. See
/// `docs/changes/runtime-capture/clip-labelling.md`.
pub const DEFAULT_IDLE_CLIP: usize = 55;

/// Default moving clip: **CONFIRMED** by runtime observation.
///
/// The walk window selected clip `2` (`12:209:3acaf951`) with coverage `0.91`.
/// That invariant fingerprint was predicted from the shipped file before the
/// run and then matched, so the file-side prediction and the runtime selection
/// agree.
pub const DEFAULT_WALK_CLIP: usize = 2;

/// Default ascending clip: **CONFIRMED** by runtime observation.
///
/// The aligned per-jump template placed clip `35` first in the ordered
/// `35 -> 36 -> 37` sequence. Two independent runs agreed on the phase at
/// `0.83`, rising to `1.00` once the templates were aligned, and the shipped
/// `RAZIEL.SRA` holds a `2:700` two-frame table at that index. See
/// `docs/notes/packed-channel-format.md`, "State defaults".
pub const DEFAULT_JUMP_UP_CLIP: usize = 35;

/// Default falling clip: **CONFIRMED** by runtime observation.
///
/// The middle phase of the same aligned sequence was clip `36` (template
/// agreement `0.83 -> 1.00` across two runs).
pub const DEFAULT_FALL_CLIP: usize = 36;

/// Default landing clip: **CONFIRMED** by runtime observation, and the source
/// of the landing-window length.
///
/// The last phase of the aligned sequence was clip `37` (agreement
/// `0.83 -> 1.00`). Its shipped table is `2:400`, so [`landing_window_ticks`]
/// derives `floor(2 * 400 * 60 / 5000) = 9` ticks for the `Landing` hold.
pub const DEFAULT_LAND_CLIP: usize = 37;

/// Explicit state-to-clip mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipMap {
    pub idle: usize,
    pub walk: usize,
    pub jump_up: usize,
    pub fall: usize,
    pub land: usize,
}

impl ClipMap {
    /// Default presentation mapping for `clip_count` clips.
    ///
    /// The tables carry no clip identity, so every index is a runtime
    /// observation of the table the engine selected, not a decoded field. All
    /// five are **CONFIRMED** by two independent runtime runs of our build.
    ///
    /// Each default is clamped to the last decoded clip, so a short animation
    /// falls back to its last clip instead of erroring; a zero-clip animation
    /// maps every state to `0`. Override with `--clip-idle`, `--clip-walk`,
    /// `--clip-jump-up`, `--clip-fall`, `--clip-land` or the `--clip-jump`
    /// all-three shortcut, or freeze one clip with `--clip <N>`.
    pub fn for_clip_count(clip_count: usize) -> Self {
        let last = clip_count.saturating_sub(1);
        Self {
            idle: DEFAULT_IDLE_CLIP.min(last),
            walk: DEFAULT_WALK_CLIP.min(last),
            jump_up: DEFAULT_JUMP_UP_CLIP.min(last),
            fall: DEFAULT_FALL_CLIP.min(last),
            land: DEFAULT_LAND_CLIP.min(last),
        }
    }

    /// The clip index for one motion state.
    pub fn clip_for(&self, state: MotionState) -> usize {
        match state {
            MotionState::Idle => self.idle,
            MotionState::Moving => self.walk,
            MotionState::Ascending => self.jump_up,
            MotionState::Falling => self.fall,
            MotionState::Landing => self.land,
        }
    }

    /// Derive this map's landing window from the configured `land` clip.
    ///
    /// See [`landing_window_ticks`]. A resolved map is always in range against
    /// its animation, so a missing clip falls back to the minimum window: the
    /// window is presentation timing, not a decode.
    pub fn landing_window(&self, animation: &SraAnimation) -> u64 {
        let frames = animation.frame_count_of(self.land).unwrap_or(0);
        let divisor = animation.divisor_of(self.land).unwrap_or(0);
        landing_window_ticks(frames, divisor, ANIMATION_UNITS_PER_SECOND)
    }
}

/// A fixed clip override plus a state map, as resolved from the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipSelection {
    pub fixed: Option<usize>,
    pub map: ClipMap,
}

/// Per-state clip slots where a missing slot is the actor's **rest pose**.
///
/// This is the runtime-label form: an entry comes from the persistent label
/// store and names only the states that were actually measured. A missing
/// state plays no clip at all; it never borrows another actor's index and never
/// falls back to an arbitrary table. Indices are validated against the loaded
/// clip count by [`ClipLabels::validated`], so a stale store cannot address a
/// table the file does not contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClipLabels {
    pub idle: Option<usize>,
    pub walk: Option<usize>,
    pub jump_up: Option<usize>,
    pub fall: Option<usize>,
    pub land: Option<usize>,
}

impl ClipLabels {
    /// No measured label: every state stays at rest.
    pub const NONE: Self = Self {
        idle: None,
        walk: None,
        jump_up: None,
        fall: None,
        land: None,
    };

    /// The measured clip index for one motion state, or `None` for rest.
    pub fn clip_for(&self, state: MotionState) -> Option<usize> {
        match state {
            MotionState::Idle => self.idle,
            MotionState::Moving => self.walk,
            MotionState::Ascending => self.jump_up,
            MotionState::Falling => self.fall,
            MotionState::Landing => self.land,
        }
    }

    /// Set or clear the clip index for one motion state.
    pub fn set(&mut self, state: MotionState, clip: Option<usize>) {
        match state {
            MotionState::Idle => self.idle = clip,
            MotionState::Moving => self.walk = clip,
            MotionState::Ascending => self.jump_up = clip,
            MotionState::Falling => self.fall = clip,
            MotionState::Landing => self.land = clip,
        }
    }

    /// Number of states that carry a measured label.
    pub fn labelled(&self) -> usize {
        [
            self.idle,
            self.walk,
            self.jump_up,
            self.fall,
            self.land,
        ]
        .into_iter()
        .filter(Option::is_some)
        .count()
    }

    /// Drop every index that is outside `0..clip_count`.
    ///
    /// A stored label is evidence from one run of one file. If the file now
    /// decodes fewer tables, the label is stale: the state stays at rest rather
    /// than clamping onto the last table, which would name a different clip.
    pub fn validated(self, clip_count: usize) -> Self {
        let keep = |clip: Option<usize>| clip.filter(|&index| index < clip_count);
        Self {
            idle: keep(self.idle),
            walk: keep(self.walk),
            jump_up: keep(self.jump_up),
            fall: keep(self.fall),
            land: keep(self.land),
        }
    }

    /// Landing window derived from this map's `land` clip.
    ///
    /// With no `land` label there is no land clip to measure, so the minimum
    /// window is used; it is presentation timing and never a decode.
    pub fn landing_window(&self, animation: &SraAnimation) -> u64 {
        self.land.map_or(LANDING_WINDOW_MIN_TICKS, |clip| {
            let frames = animation.frame_count_of(clip).unwrap_or(0);
            let divisor = animation.divisor_of(clip).unwrap_or(0);
            landing_window_ticks(frames, divisor, ANIMATION_UNITS_PER_SECOND)
        })
    }
}

/// The clip source behind a [`ClipSelector`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipPlan {
    /// `--clip <N>`: one clip, global tick.
    Fixed(usize),
    /// The confirmed default/CLI map: every state has an index.
    Mapped(ClipMap),
    /// Per-actor stored labels: a missing state is the rest pose.
    Labelled(ClipLabels),
}

/// State-driven clip selector.
///
/// Rule: the clip changes only when the observed motion state changes. On a
/// change the clip restarts at local tick 0 (`start_tick = tick`), so a state
/// change starts the new clip at its first frame instead of inheriting a
/// mid-clip index from the previous clip. The local tick is the tick count since
/// that restart, and is what the playback clock consumes.
///
/// The selector owns the [`MotionTracker`], so one call per rendered frame
/// classifies the signals and applies the switching rule atomically. Because
/// `Landing` is held for the whole derived window, a touchdown cannot alternate
/// `Landing -> Idle -> Landing`: the state only changes when the window closes.
///
/// A fixed override (`--clip <N>`) ignores the state and uses the global tick,
/// so it starts at frame 0 and never resets. A labelled selector
/// ([`ClipSelector::from_labels`]) may resolve a state to `None`, which means
/// the actor keeps its rest pose for that state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipSelector {
    plan: ClipPlan,
    tracker: MotionTracker,
    clip: Option<usize>,
    start_tick: u64,
    state: Option<MotionState>,
}

impl ClipSelector {
    /// Build a selector with an explicit landing window in ticks.
    pub fn new(selection: ClipSelection, landing_ticks: u64) -> Self {
        let plan = match selection.fixed {
            Some(fixed) => ClipPlan::Fixed(fixed),
            None => ClipPlan::Mapped(selection.map),
        };
        let clip = match plan {
            ClipPlan::Fixed(fixed) => Some(fixed),
            ClipPlan::Mapped(map) => Some(map.idle),
            ClipPlan::Labelled(labels) => labels.idle,
        };
        Self {
            plan,
            tracker: MotionTracker::new(landing_ticks),
            clip,
            start_tick: 0,
            state: None,
        }
    }

    /// Build a per-actor selector from stored labels.
    ///
    /// A missing label is `None` (rest), never a default index. The caller
    /// validates the labels against the file's clip count before this call.
    pub fn from_labels(labels: ClipLabels, landing_ticks: u64) -> Self {
        Self {
            plan: ClipPlan::Labelled(labels),
            tracker: MotionTracker::new(landing_ticks),
            clip: labels.idle,
            start_tick: 0,
            state: None,
        }
    }

    /// The active clip index.
    ///
    /// Kept for the existing callers: a labelled selector with no clip for the
    /// current state reports `0`, but those callers must use
    /// [`ClipSelector::clip_opt`] when "no clip" is meaningful.
    pub fn clip(&self) -> usize {
        self.clip.unwrap_or(0)
    }

    /// The active clip index, or `None` when the state is the rest pose.
    pub fn clip_opt(&self) -> Option<usize> {
        self.clip
    }

    /// The last observed motion state, if any.
    pub fn state(&self) -> Option<MotionState> {
        self.state
    }

    /// Observe one rendered frame's raw player signals and switch if needed.
    ///
    /// Classifies the sample through the [`MotionTracker`] first, then applies
    /// [`ClipSelector::observe`] to the resulting state.
    pub fn observe_signals(
        &mut self,
        tick: u64,
        grounded: bool,
        moving: bool,
        vertical_velocity: i64,
    ) -> bool {
        let state = self
            .tracker
            .observe(tick, grounded, moving, vertical_velocity);
        self.observe(tick, state)
    }

    /// Observe one tick's motion state. Returns `true` when the clip switched.
    pub fn observe(&mut self, tick: u64, state: MotionState) -> bool {
        if matches!(self.plan, ClipPlan::Fixed(_)) || self.state == Some(state) {
            return false;
        }
        self.state = Some(state);
        self.clip = match &self.plan {
            ClipPlan::Fixed(fixed) => Some(*fixed),
            ClipPlan::Mapped(map) => Some(map.clip_for(state)),
            ClipPlan::Labelled(labels) => labels.clip_for(state),
        };
        self.start_tick = tick;
        true
    }

    /// The tick the playback clock should use for this clip.
    pub fn local_tick(&self, tick: u64) -> u64 {
        if matches!(self.plan, ClipPlan::Fixed(_)) {
            tick
        } else {
            tick.saturating_sub(self.start_tick)
        }
    }
}

/// Build the 13-float angle-form slot pose list from decoded binary angles.
///
/// The nine rotation floats are [`rotation_from_binary_angles`]'s
/// `Rz(c) * Ry(b) * Rx(a)` matrix. Translation stays zero and the scale marker
/// stays zero; the packed translation lanes are consumed by the decoder but not
/// applied by this presentation path yet.
pub fn poses_from_angles(angles: &[[f32; 3]]) -> Vec<SlotPose> {
    angles
        .iter()
        .map(|angle| {
            let rotation = rotation_from_binary_angles(angle[0], angle[1], angle[2]);
            [
                rotation[0][0],
                rotation[0][1],
                rotation[0][2],
                rotation[1][0],
                rotation[1][1],
                rotation[1][2],
                rotation[2][0],
                rotation[2][1],
                rotation[2][2],
                0.0,
                0.0,
                0.0,
                0.0,
            ]
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn synthetic_animation_container(
    masks: &[u16],
    frame_count: u16,
    divisor: i16,
    values: &[f32],
) -> Vec<u8> {
    encode_single_record_container(&synthetic_record(masks, frame_count, divisor, values))
}

/// Build one payload record with one table, for tests.
#[cfg(test)]
fn synthetic_record(masks: &[u16], frame_count: u16, divisor: i16, values: &[f32]) -> Vec<u8> {
    let slot_count = masks.len();
    let stride_bits = ((3 * slot_count + 7) / 8) * 8;
    let block_base = 0x20;
    let mut record = vec![0u8; block_base + 1 + stride_bits / 8];
    record[0] = 3;
    record[4..6].copy_from_slice(&frame_count.to_le_bytes());
    record[6..8].copy_from_slice(&divisor.to_le_bytes());
    for pair in 0..2 {
        record[8 + pair * 4..10 + pair * 4].copy_from_slice(&frame_count.to_le_bytes());
        record[10 + pair * 4..12 + pair * 4].copy_from_slice(&divisor.to_le_bytes());
    }
    record[block_base] = 0x01;
    for (slot, mask) in masks.iter().enumerate() {
        for lane in 0..3 {
            if mask & (1 << lane) != 0 {
                let bit = block_base * 8 + 8 + 3 * slot + lane;
                record[bit / 8] |= 1 << (bit % 8);
            }
        }
    }
    let stream_at = (record.len() + 3) & !3;
    record.resize(stream_at, 0);
    let mut next = values.iter().copied();
    for mask in masks {
        for lane in 0..12 {
            if mask & (1 << lane) != 0 {
                record.extend_from_slice(&1i32.to_le_bytes());
                record.extend_from_slice(&next.next().unwrap_or(0.0).to_le_bytes());
            }
        }
    }
    record
}

/// Build a multi-clip animation container, one clip per payload record.
#[cfg(test)]
pub(crate) fn synthetic_multi_clip_container(
    masks: &[u16],
    clips: &[(u16, i16)],
) -> Vec<u8> {
    let records: Vec<Vec<u8>> = clips
        .iter()
        .map(|&(frames, divisor)| synthetic_record(masks, frames, divisor, &[0.0]))
        .collect();
    let count = records.len() + 1;
    let preamble_size = 8 + 4 * count;
    let payload_base = preamble_size.div_ceil(PAYLOAD_ALIGN).max(1) * PAYLOAD_ALIGN;
    let directory_len = 4 * (records.len() + 1);
    let mut bytes = vec![0u8; payload_base + directory_len];
    bytes[0..4].copy_from_slice(&(count as u32).to_le_bytes());
    let mut offset = directory_len;
    for (index, record) in records.iter().enumerate() {
        let at = payload_base + index * 4;
        bytes[at..at + 4].copy_from_slice(&(offset as u32).to_le_bytes());
        offset += record.len();
    }
    let terminator = payload_base + records.len() * 4;
    bytes[terminator..terminator + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    for record in &records {
        bytes.extend_from_slice(record);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented `font` rotation channel: three keys, deltas 75/75/0,
    /// quaternions `(0,0,0,-4096)`, `(0,0,-4096,0)`, `(0,0,0,4096)`.
    fn font_channel() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(encode_rotation_key(75, [0, 0, 0, -4096]));
        out.extend(encode_rotation_key(75, [0, 0, -4096, 0]));
        out.extend(encode_rotation_key(0, [0, 0, 0, 4096]));
        out
    }

    #[test]
    fn font_calibration_channel_is_found() {
        let mut record = vec![0u8; 13];
        record.extend(font_channel());
        record.extend_from_slice(&[0xFFu8; 17]);
        let container = encode_single_record_container(&record);
        let parsed = parse_container(&container).expect("synthetic container must parse");
        assert_eq!(parsed.records.len(), 1);
        let range = parsed.records[0].clone();
        let runs = rotation_runs(&container, range.clone(), 2);
        assert_eq!(
            runs,
            vec![KeyRun {
                start: range.start + 13,
                keys: 3,
                span: 150
            }],
            "the detector must re-find the documented font channel"
        );
        let key = rotation_key(&container, range.start + 13).expect("first font key");
        assert_eq!(key, RotationKey { delta: 75, quat: [0, 0, 0, -4096] });
        let key = rotation_key(&container, range.start + 33).expect("third font key");
        assert_eq!(key, RotationKey { delta: 0, quat: [0, 0, 0, 4096] });
    }

    #[test]
    fn noise_is_rejected() {
        let mut state: u32 = 0x1234_5678;
        let mut bytes = vec![0u8; 4096];
        for byte in &mut bytes {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (state >> 24) as u8;
        }
        assert!(rotation_runs(&bytes, 0..bytes.len(), 3).is_empty());
        assert!(position_runs(&bytes, 0..bytes.len(), 3).is_empty());
    }

    #[test]
    fn runtime_rotation_channel_requires_the_float_mirror() {
        let mut record = Vec::new();
        for key in [
            RotationKey { delta: 10, quat: [0, 0, 0, 4096] },
            RotationKey { delta: 10, quat: [0, 0, 2896, 2896] },
            RotationKey { delta: 0, quat: [0, 0, 4096, 0] },
        ] {
            record.extend(encode_rotation_runtime_key(key.delta, key.quat));
        }
        let runs = rotation_runtime_runs(&record, 0..record.len(), 2);
        assert_eq!(runs.len(), 1);
        assert_eq!((runs[0].keys, runs[0].span), (3, 20));
        // The same bytes do not satisfy the compact file stride.
        assert!(rotation_runs(&record, 0..record.len(), 3).is_empty());
    }

    #[test]
    fn position_channel_shape_is_decoded() {
        let mut record = Vec::new();
        for index in 0..4i16 {
            record.extend_from_slice(&(12i16).to_le_bytes());
            for axis in 0..3i16 {
                record.extend_from_slice(&(index * 100 + axis * 10 + 7).to_le_bytes());
            }
            for tangent in 0..3 {
                record.extend_from_slice(&(1000i32 + i32::from(tangent)).to_le_bytes());
            }
            for tangent in 0..3 {
                record.extend_from_slice(&(-1000i32 - i32::from(tangent)).to_le_bytes());
            }
        }
        let runs = position_runs(&record, 0..record.len(), 3);
        assert_eq!(runs.len(), 1);
        assert_eq!((runs[0].keys, runs[0].span), (4, 48));
        let key = position_key(&record, 0).expect("first position key");
        assert_eq!(key.delta, 12);
        assert_eq!(key.position, [7, 17, 27]);
        assert_eq!(key.out_tangent, [1000, 1001, 1002]);
    }

    #[test]
    fn container_rejects_a_broken_directory() {
        let mut record = vec![0u8; 32];
        record.extend(font_channel());
        let mut container = encode_single_record_container(&record);
        let base = parse_container(&container).expect("baseline must parse").payload_base;
        // Replacing the terminator with a smaller offset breaks ascending order.
        container[base..base + 4].copy_from_slice(&16u32.to_le_bytes());
        container[base + 4..base + 8].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse_container(&container).is_none());
    }

    fn palette_record(
        masks: &[u16],
        frame_count: u16,
        divisor: i16,
        palettes: &[&[f32]],
    ) -> Vec<u8> {
        let slot_count = masks.len();
        let stride_bits = ((3 * slot_count + 7) / 8) * 8;
        let block_base = 0x20;
        let mut record = vec![0u8; block_base + 1 + stride_bits / 8];
        record[0] = 3;
        record[4..6].copy_from_slice(&frame_count.to_le_bytes());
        record[6..8].copy_from_slice(&divisor.to_le_bytes());
        for pair in 0..2 {
            record[8 + pair * 4..10 + pair * 4].copy_from_slice(&frame_count.to_le_bytes());
            record[10 + pair * 4..12 + pair * 4].copy_from_slice(&divisor.to_le_bytes());
        }
        record[block_base] = 0x01;
        for (slot, mask) in masks.iter().enumerate() {
            for lane in 0..3 {
                if mask & (1 << lane) != 0 {
                    let bit = block_base * 8 + 8 + 3 * slot + lane;
                    record[bit / 8] |= 1 << (bit % 8);
                }
            }
        }
        let stream_at = (record.len() + 3) & !3;
        record.resize(stream_at, 0);
        let mut next = palettes.iter();
        for mask in masks {
            for lane in 0..12 {
                if mask & (1 << lane) == 0 {
                    continue;
                }
                let palette = next.next().expect("one palette per set lane");
                assert_eq!(palette.len(), usize::from(frame_count));
                record.extend_from_slice(&0i32.to_le_bytes());
                for value in *palette {
                    record.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        record
    }

    fn palette_container(
        masks: &[u16],
        frame_count: u16,
        divisor: i16,
        palettes: &[&[f32]],
    ) -> Vec<u8> {
        encode_single_record_container(&palette_record(masks, frame_count, divisor, palettes))
    }

    /// Wrap several payload records in one valid container, in file order.
    fn multi_record_container(records: &[Vec<u8>]) -> Vec<u8> {
        let count = records.len() + 1;
        let preamble_size = 8 + 4 * count;
        let payload_base = preamble_size.div_ceil(PAYLOAD_ALIGN).max(1) * PAYLOAD_ALIGN;
        let directory_len = 4 * (records.len() + 1);
        let mut bytes = vec![0u8; payload_base + directory_len];
        bytes[0..4].copy_from_slice(&(count as u32).to_le_bytes());
        let mut offset = directory_len;
        for (index, record) in records.iter().enumerate() {
            let at = payload_base + index * 4;
            bytes[at..at + 4].copy_from_slice(&(offset as u32).to_le_bytes());
            offset += record.len();
        }
        let terminator = payload_base + records.len() * 4;
        bytes[terminator..terminator + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        for record in records {
            bytes.extend_from_slice(record);
        }
        bytes
    }

    #[test]
    fn a_constructed_table_decodes_known_angles_for_a_known_frame() {
        let container = palette_container(
            &[0x007, 0x001],
            2,
            100,
            &[&[10.0, 20.0], &[30.0, 40.0], &[50.0, 60.0], &[70.0, 80.0]],
        );
        let animation = SraAnimation::load(container, 2).expect("synthetic table must locate");
        assert_eq!(animation.slot_count(), 2);
        assert_eq!(animation.frame_count(), 2);
        assert_eq!(animation.channel_count(), 3);
        assert_eq!(animation.divisor(), 100);

        let first = animation.decode_index(0).expect("frame 0");
        assert_eq!(first.frame_index, 0);
        assert_eq!(first.angles, vec![[10.0, 30.0, 50.0], [70.0, 0.0, 0.0]]);

        let second = animation.decode_time(100).expect("frame 1");
        assert_eq!(second.frame_index, 1);
        assert_eq!(second.angles, vec![[20.0, 40.0, 60.0], [80.0, 0.0, 0.0]]);
        assert_eq!(animation.poses(&second).len(), 2);
    }

    #[test]
    fn playback_clock_advances_about_fifty_times_per_second_at_the_default_rate() {
        let mut previous = 0;
        for tick in 0..=120u64 {
            let index = frame_index_for_playback(
                tick,
                ANIMATION_UNITS_PER_SECOND,
                100,
                1000,
                PlaybackLoop::Loop,
            )
            .expect("clock");
            assert!(index >= previous, "index went backwards at tick {tick}");
            previous = index;
        }
        assert_eq!(
            frame_index_for_playback(
                60,
                ANIMATION_UNITS_PER_SECOND,
                100,
                1000,
                PlaybackLoop::Loop
            ),
            Ok(50),
            "60 ticks at the default rate is 50 divisor-100 palette frames"
        );
        let advances = (1..=60u64)
            .filter(|&tick| {
                let now = frame_index_for_playback(
                    tick,
                    ANIMATION_UNITS_PER_SECOND,
                    100,
                    1000,
                    PlaybackLoop::Loop,
                )
                .expect("clock");
                let before = frame_index_for_playback(
                    tick - 1,
                    ANIMATION_UNITS_PER_SECOND,
                    100,
                    1000,
                    PlaybackLoop::Loop,
                )
                .expect("clock");
                now > before
            })
            .count();
        assert_eq!(advances, 50, "one advance per palette frame in one second");
    }

    #[test]
    fn playback_clock_with_divisor_100_and_50_frames_matches_first_transitions() {
        let indices: Vec<usize> = (0..=8u64)
            .map(|tick| {
                frame_index_for_playback(
                    tick,
                    ANIMATION_UNITS_PER_SECOND,
                    100,
                    50,
                    PlaybackLoop::Loop,
                )
                .expect("clock")
            })
            .collect();
        assert_eq!(indices, vec![0, 0, 1, 2, 3, 4, 5, 5, 6]);
        assert_eq!(
            frame_index_for_playback(
                60,
                ANIMATION_UNITS_PER_SECOND,
                100,
                50,
                PlaybackLoop::Loop
            ),
            Ok(0),
            "the 50th advance wraps back to frame 0"
        );
    }

    #[test]
    fn the_divisor_sets_each_clips_relative_playback_rate() {
        let slider = |tick: u64, divisor: i16| {
            frame_index_for_playback(
                tick,
                ANIMATION_UNITS_PER_SECOND,
                divisor,
                1000,
                PlaybackLoop::Loop,
            )
            .expect("clock")
        };
        assert_eq!(
            (slider(0, 100), slider(0, 544)),
            (0, 0),
            "both clips start at frame 0"
        );
        assert_eq!(
            (slider(6, 100), slider(6, 544)),
            (5, 0),
            "the reference clip advances while the slower clip is still on frame 0"
        );
        assert_eq!(
            (slider(7, 100), slider(7, 544)),
            (5, 1),
            "the divisor-544 clip advances one tick later"
        );
        assert_eq!(
            (slider(60, 100), slider(60, 544)),
            (50, 9),
            "one second advances 50 reference frames and 9 divisor-544 frames"
        );
        assert_eq!(
            (slider(120, 100), slider(120, 544)),
            (100, 18),
            "two seconds preserve the same relative rate"
        );
        let reference = slider(490, 100);
        let slow = slider(490, 544);
        assert_eq!((reference, slow), (408, 75));
        assert_eq!(
            reference * REFERENCE_DIVISOR as usize,
            slow * 544,
            "the frame counts are exactly in the ratio 544:100"
        );
    }

    #[test]
    fn playback_loop_wraps_at_frame_count_and_does_not_saturate() {
        assert_eq!(
            frame_index_for_playback(600, ANIMATION_UNITS_PER_SECOND, 100, 50, PlaybackLoop::Loop),
            Ok(0)
        );
        assert_eq!(
            frame_index_for_playback(601, ANIMATION_UNITS_PER_SECOND, 100, 50, PlaybackLoop::Loop),
            Ok(0)
        );
        assert_eq!(
            frame_index_for_playback(602, ANIMATION_UNITS_PER_SECOND, 100, 50, PlaybackLoop::Loop),
            Ok(1)
        );
    }

    #[test]
    fn playback_once_saturates_at_the_last_frame() {
        assert_eq!(
            frame_index_for_playback(59, ANIMATION_UNITS_PER_SECOND, 100, 50, PlaybackLoop::Once),
            Ok(49)
        );
        assert_eq!(
            frame_index_for_playback(60, ANIMATION_UNITS_PER_SECOND, 100, 50, PlaybackLoop::Once),
            Ok(49)
        );
        assert_eq!(
            frame_index_for_playback(
                10_000_000,
                ANIMATION_UNITS_PER_SECOND,
                100,
                50,
                PlaybackLoop::Once
            ),
            Ok(49)
        );
    }

    #[test]
    fn loop_and_once_honor_a_divisor_other_than_100() {
        let at = |tick: u64, policy: PlaybackLoop| {
            frame_index_for_playback(tick, ANIMATION_UNITS_PER_SECOND, 544, 10, policy)
                .expect("clock")
        };
        assert_eq!(at(65, PlaybackLoop::Loop), 9);
        assert_eq!(at(66, PlaybackLoop::Loop), 0, "frame 10 wraps to 0");
        assert_eq!(at(71, PlaybackLoop::Loop), 0);
        assert_eq!(at(72, PlaybackLoop::Loop), 1);
        assert_eq!(at(65, PlaybackLoop::Once), 9);
        assert_eq!(at(66, PlaybackLoop::Once), 9, "Once holds the last frame");
        assert_eq!(
            at(10_000_000, PlaybackLoop::Once),
            9,
            "a huge tick still saturates at the divisor-544 last frame"
        );
        assert_eq!(
            at(10_000_000, PlaybackLoop::Loop),
            2,
            "a huge tick wraps instead of saturating"
        );
    }

    #[test]
    fn large_tick_counts_keep_advancing_past_the_u16_ceiling() {
        let huge = frame_index_for_playback(
            10_000_000,
            ANIMATION_UNITS_PER_SECOND,
            100,
            50,
            PlaybackLoop::Loop,
        )
        .expect("clock");
        let next = frame_index_for_playback(
            10_000_001,
            ANIMATION_UNITS_PER_SECOND,
            100,
            50,
            PlaybackLoop::Loop,
        )
        .expect("clock");
        assert_eq!((huge, next), (33, 34));
        assert_ne!(huge, 49, "a large tick must not freeze on the last frame");
        assert_ne!(
            frame_index_for_playback(
                65_535,
                ANIMATION_UNITS_PER_SECOND,
                100,
                50,
                PlaybackLoop::Loop
            ),
            frame_index_for_playback(
                10_000_000,
                ANIMATION_UNITS_PER_SECOND,
                100,
                50,
                PlaybackLoop::Loop
            ),
            "the clock does not saturate at the u16 frame-time ceiling"
        );
        assert!(frame_index_for_playback(0, 0, 100, 50, PlaybackLoop::Loop)
            .unwrap_err()
            .contains("rate 0"));
    }

    #[test]
    fn animation_playback_clock_does_not_freeze_on_a_large_tick() {
        let container = synthetic_animation_container(&[0x001], 50, 100, &[0.0]);
        let animation = SraAnimation::load(container, 1).expect("synthetic table must locate");
        let frame = animation
            .decode_tick(
                10_000_000,
                ANIMATION_UNITS_PER_SECOND,
                PlaybackLoop::Loop
            )
            .expect("a large tick still decodes");
        assert_eq!(frame.frame_index, 33);
        assert_eq!(
            animation.frame_index_for_tick(
                10_000_000,
                ANIMATION_UNITS_PER_SECOND,
                PlaybackLoop::Loop
            ),
            frame_index_for_playback(
                10_000_000,
                ANIMATION_UNITS_PER_SECOND,
                100,
                50,
                PlaybackLoop::Loop
            )
        );
    }

    #[test]
    fn a_decoded_pose_moves_more_than_one_bone_through_the_skinner() {
        use crate::bone_hierarchy::BoneHierarchy;
        use crate::object::{Bone, ObjectModel, rest_positions};

        // Slot 0 rotates X, slot 1 rotates Y, slot 2 carries one angle lane only.
        let container = synthetic_animation_container(
            &[0x007, 0x007, 0x001],
            1,
            100,
            &[1024.0, 0.0, 0.0, 0.0, 1024.0, 0.0, 0.0],
        );
        let animation = SraAnimation::load(container, 3).expect("synthetic table must locate");
        let frame = animation.decode_index(0).expect("frame 0");
        assert_eq!(frame.angles[0], [1024.0, 0.0, 0.0]);
        assert_eq!(frame.angles[1], [0.0, 1024.0, 0.0]);

        let model = ObjectModel {
            vertices: vec![
                ([0, 1, 0], 0),
                ([0, 2, 0], 0),
                ([1, 0, 0], 0),
                ([2, 0, 0], 0),
                ([0, 0, 1], 0),
                ([0, 0, 2], 0),
            ],
            bones: vec![
                Bone { first_vertex: 0, last_vertex: 1, local: [0, 0, 0], parent: u16::MAX },
                Bone { first_vertex: 2, last_vertex: 3, local: [10, 0, 0], parent: u16::MAX },
                Bone { first_vertex: 4, last_vertex: 5, local: [0, 10, 0], parent: u16::MAX },
            ],
            ..ObjectModel::default()
        };
        let hierarchy = BoneHierarchy::from_parents(vec![-1, -1, -1]);
        let poses = animation.poses(&frame);
        let positions =
            crate::pose::skin_positions_hierarchical(&model, &hierarchy, &poses).expect("skin");
        let rest = rest_positions(&model);
        let moved_ranges = model
            .bones
            .iter()
            .filter(|bone| {
                (bone.first_vertex..=bone.last_vertex)
                    .any(|index| positions[index as usize] != rest[index as usize])
            })
            .count();
        assert!(
            moved_ranges >= 2,
            "the decoded pose must move more than one bone, moved {moved_ranges}"
        );
    }

    #[test]
    fn the_enumerator_finds_one_clip_per_constructed_record_in_file_order() {
        let first = palette_record(
            &[0x007],
            2,
            100,
            &[&[10.0, 20.0], &[30.0, 40.0], &[50.0, 60.0]],
        );
        let second = palette_record(&[0x001], 3, 200, &[&[1.0, 2.0, 3.0]]);
        let mut third = vec![0u8; 5];
        third.extend(palette_record(
            &[0x001],
            4,
            50,
            &[&[7.0, 8.0, 9.0, 10.0]],
        ));
        let container = multi_record_container(&[first, second, third]);

        let animation = SraAnimation::load(container, 1).expect("three synthetic clips");
        assert_eq!(animation.clip_count(), 3);
        let clips = animation.clips();
        assert_eq!(
            clips
                .iter()
                .map(|clip| (clip.index, clip.record, clip.offset))
                .collect::<Vec<_>>(),
            vec![(0, 0, 0), (1, 1, 0), (2, 2, 5)],
            "clips are numbered in file order and keep their record-relative offset"
        );
        assert_eq!(
            clips
                .iter()
                .map(|clip| (clip.frame_count, clip.divisor))
                .collect::<Vec<_>>(),
            vec![(2, 100), (3, 200), (4, 50)]
        );
        assert_eq!(
            animation
                .decode_clip_index(1, 0)
                .expect("second clip frame 0")
                .angles,
            vec![[1.0, 0.0, 0.0]],
            "clip 1 is the second record, not the first"
        );
        assert_eq!(
            animation
                .decode_clip_index(0, 1)
                .expect("first clip frame 1")
                .angles,
            vec![[20.0, 40.0, 60.0]]
        );
    }

    #[test]
    fn clip_selection_maps_each_motion_state_to_the_configured_index() {
        let map = ClipMap {
            idle: 2,
            walk: 5,
            jump_up: 7,
            fall: 8,
            land: 9,
        };
        assert_eq!(map.clip_for(MotionState::Idle), 2);
        assert_eq!(map.clip_for(MotionState::Moving), 5);
        assert_eq!(map.clip_for(MotionState::Ascending), 7);
        assert_eq!(map.clip_for(MotionState::Falling), 8);
        assert_eq!(map.clip_for(MotionState::Landing), 9);
    }

    #[test]
    fn vertical_velocity_sign_separates_ascending_from_falling() {
        assert_eq!(
            MotionState::from_player_signals(false, false, 1),
            MotionState::Ascending,
            "a positive signed velocity is a rise"
        );
        assert_eq!(
            MotionState::from_player_signals(false, false, 0),
            MotionState::Falling,
            "exactly zero is not a rise, so it falls"
        );
        assert_eq!(
            MotionState::from_player_signals(false, true, -1),
            MotionState::Falling,
            "a negative velocity falls even while moving horizontally"
        );
        assert_eq!(
            MotionState::from_player_signals(true, false, 1234),
            MotionState::Idle,
            "grounded wins over a stale velocity"
        );
        assert_eq!(
            MotionState::from_player_signals(true, true, -99),
            MotionState::Moving
        );
    }

    #[test]
    fn landing_window_is_derived_from_the_land_clip_duration() {
        // frames * divisor / units_per_second seconds, then * 60 ticks.
        assert_eq!(landing_window_ticks(2, 400, ANIMATION_UNITS_PER_SECOND), 9);
        assert_eq!(landing_window_ticks(6, 240, ANIMATION_UNITS_PER_SECOND), 17);
        assert_eq!(
            landing_window_ticks(1, 100, ANIMATION_UNITS_PER_SECOND),
            LANDING_WINDOW_MIN_TICKS,
            "a sub-frame duration clamps up to the visible minimum"
        );
        assert_eq!(
            landing_window_ticks(50, 100, ANIMATION_UNITS_PER_SECOND),
            LANDING_WINDOW_MAX_TICKS,
            "a one-second clip clamps down to the recovery maximum"
        );
        for (frames, divisor, rate) in [(0usize, 400i16, 5_000u64), (2, 0, 5_000), (2, 400, 0)] {
            assert_eq!(
                landing_window_ticks(frames, divisor, rate),
                LANDING_WINDOW_MIN_TICKS,
                "an undefined duration falls back to the minimum"
            );
        }
    }

    #[test]
    fn the_landing_window_reads_the_land_clips_own_frames_and_divisor() {
        let first = palette_record(&[0x001], 3, 100, &[&[1.0, 2.0, 3.0]]);
        let land = palette_record(&[0x001], 2, 400, &[&[4.0, 5.0]]);
        let animation = SraAnimation::load(multi_record_container(&[first, land]), 1)
            .expect("two records, two clips");
        let mut map = ClipMap::for_clip_count(10);
        map.land = 1;
        assert_eq!(map.landing_window(&animation), 9);
    }

    #[test]
    fn landing_holds_for_the_window_then_releases_to_the_grounded_state() {
        let mut tracker = MotionTracker::new(9);
        assert_eq!(
            tracker.observe(0, true, false, 0),
            MotionState::Idle,
            "a spawn sample with no previous observation is not a touchdown"
        );
        assert_eq!(tracker.observe(1, false, false, 800), MotionState::Ascending);
        assert_eq!(tracker.observe(2, false, false, -100), MotionState::Falling);
        for tick in 3..12 {
            assert_eq!(
                tracker.observe(tick, true, false, 0),
                MotionState::Landing,
                "tick {tick} is inside the 9-tick window that opened at 3"
            );
        }
        assert_eq!(
            tracker.observe(12, true, false, 0),
            MotionState::Idle,
            "the exclusive end of the window releases to idle"
        );
        assert_eq!(
            tracker.observe(13, true, true, 0),
            MotionState::Moving,
            "the grounded state then follows the horizontal delta"
        );
    }

    #[test]
    fn touchdown_does_not_flap_on_the_landing_boundary() {
        let selection = ClipSelection {
            fixed: None,
            map: ClipMap {
                idle: 55,
                walk: 2,
                jump_up: 35,
                fall: 36,
                land: 37,
            },
        };
        let mut selector = ClipSelector::new(selection, 6);
        let mut switches = 0;
        let mut switches_after_touchdown = 0;
        // Ground, rise, fall, then land while the committed horizontal delta
        // alternates every tick. A flapping machine would toggle Landing/Idle
        // or Landing/Moving inside the window.
        let samples = [
            (0u64, true, false, 0i64),
            (1, false, false, 500),
            (2, false, false, -50),
            (3, true, true, 0),
            (4, true, false, 0),
            (5, true, true, 0),
            (6, true, false, 0),
            (7, true, true, 0),
            (8, true, false, 0),
            (9, true, true, 0),
            (10, true, false, 0),
        ];
        for (tick, grounded, moving, velocity) in samples {
            let switched = selector.observe_signals(tick, grounded, moving, velocity);
            if switched {
                switches += 1;
                if tick >= 3 {
                    switches_after_touchdown += 1;
                }
            }
            let state = selector.state().expect("observed");
            if (3..9).contains(&tick) {
                assert_eq!(
                    state,
                    MotionState::Landing,
                    "tick {tick} must hold Landing across the alternating delta"
                );
                assert_eq!(selector.clip(), 37);
            }
        }
        assert_eq!(
            switches, 6,
            "Idle, Ascending, Falling, Landing, Moving and Idle; nothing repeats"
        );
        assert_eq!(
            switches_after_touchdown, 3,
            "after touchdown only entering Landing, leaving it to Moving and \
             then following the delta to Idle may switch"
        );
        assert_eq!(
            selector.state(),
            Some(MotionState::Idle),
            "the window closes at tick 9; the committed delta then decides"
        );
        assert_eq!(selector.clip(), 55);
    }

    #[test]
    fn default_map_uses_the_five_confirmed_indices() {
        let map = ClipMap::for_clip_count(230);
        assert_eq!(map.idle, DEFAULT_IDLE_CLIP);
        assert_eq!(map.walk, DEFAULT_WALK_CLIP);
        assert_eq!(map.jump_up, DEFAULT_JUMP_UP_CLIP);
        assert_eq!(map.fall, DEFAULT_FALL_CLIP);
        assert_eq!(map.land, DEFAULT_LAND_CLIP);
        assert_eq!(
            (map.idle, map.walk, map.jump_up, map.fall, map.land),
            (55, 2, 35, 36, 37),
            "the defaults are the measured five-state sequence"
        );
    }

    #[test]
    fn default_map_clamps_indices_past_the_clip_count() {
        let single = ClipMap::for_clip_count(1);
        assert_eq!(
            (
                single.idle,
                single.walk,
                single.jump_up,
                single.fall,
                single.land
            ),
            (0, 0, 0, 0, 0)
        );

        let short = ClipMap::for_clip_count(10);
        assert_eq!(
            (
                short.idle,
                short.walk,
                short.jump_up,
                short.fall,
                short.land
            ),
            (9, 2, 9, 9, 9),
            "each default past the last clip falls back to the last clip"
        );

        let empty = ClipMap::for_clip_count(0);
        assert_eq!(
            (
                empty.idle,
                empty.walk,
                empty.jump_up,
                empty.fall,
                empty.land
            ),
            (0, 0, 0, 0, 0)
        );
    }

    #[test]
    fn a_partial_label_map_leaves_missing_states_at_rest() {
        let labels = ClipLabels {
            idle: Some(4),
            walk: Some(2),
            ..ClipLabels::NONE
        };
        assert_eq!(labels.clip_for(MotionState::Idle), Some(4));
        assert_eq!(labels.clip_for(MotionState::Moving), Some(2));
        assert_eq!(labels.clip_for(MotionState::Ascending), None);
        assert_eq!(labels.clip_for(MotionState::Falling), None);
        assert_eq!(labels.clip_for(MotionState::Landing), None);
        assert_eq!(labels.labelled(), 2);

        let mut selector = ClipSelector::from_labels(labels, 4);
        assert_eq!(selector.clip_opt(), Some(4));
        assert!(selector.observe(0, MotionState::Idle));
        assert_eq!(selector.clip_opt(), Some(4));
        assert!(selector.observe(1, MotionState::Moving));
        assert_eq!(selector.clip_opt(), Some(2));
        assert!(selector.observe(2, MotionState::Ascending));
        assert_eq!(selector.clip_opt(), None, "an unmeasured state is rest");
        assert_eq!(selector.clip(), 0, "legacy callers keep the zero fallback");
    }

    #[test]
    fn stale_labels_outside_the_short_animation_are_dropped_not_clamped() {
        let labels = ClipLabels {
            idle: Some(55),
            walk: Some(0),
            land: Some(7),
            ..ClipLabels::NONE
        };
        let validated = labels.validated(1);
        assert_eq!(validated.idle, None, "the stale index is not clamped");
        assert_eq!(validated.walk, Some(0), "an in-range index is kept");
        assert_eq!(validated.land, None);
        assert_eq!(validated.labelled(), 1);
    }

    #[test]
    fn default_map_selection_switches_on_state_change_and_restarts_at_local_zero() {
        let mut selector = ClipSelector::new(
            ClipSelection {
                fixed: None,
                map: ClipMap::for_clip_count(230),
            },
            9,
        );
        assert_eq!(selector.clip(), DEFAULT_IDLE_CLIP);

        assert!(selector.observe(5, MotionState::Idle));
        assert_eq!(selector.clip(), DEFAULT_IDLE_CLIP);
        assert_eq!(selector.local_tick(9), 4);
        assert!(!selector.observe(10, MotionState::Idle));
        assert_eq!(selector.clip(), DEFAULT_IDLE_CLIP);
        assert_eq!(selector.local_tick(10), 5);

        assert!(selector.observe(11, MotionState::Moving));
        assert_eq!(selector.clip(), DEFAULT_WALK_CLIP);
        assert_eq!(
            selector.local_tick(11),
            0,
            "the walk clip restarts at local tick 0"
        );

        assert!(selector.observe(12, MotionState::Ascending));
        assert_eq!(selector.clip(), DEFAULT_JUMP_UP_CLIP);
        assert_eq!(selector.local_tick(12), 0);

        assert!(selector.observe(13, MotionState::Falling));
        assert_eq!(selector.clip(), DEFAULT_FALL_CLIP);
        assert_eq!(selector.local_tick(13), 0);

        assert!(selector.observe(14, MotionState::Landing));
        assert_eq!(selector.clip(), DEFAULT_LAND_CLIP);
        assert_eq!(selector.local_tick(14), 0);
    }

    #[test]
    fn a_state_change_restarts_the_clip_and_only_changes_on_a_new_state() {
        let mut selector = ClipSelector::new(
            ClipSelection {
                fixed: None,
                map: ClipMap {
                    idle: 2,
                    walk: 5,
                    jump_up: 7,
                    fall: 8,
                    land: 9,
                },
            },
            9,
        );

        assert!(selector.observe(10, MotionState::Idle));
        assert_eq!(selector.clip(), 2);
        assert_eq!(selector.local_tick(10), 0, "the clip starts at frame 0");
        assert_eq!(selector.local_tick(15), 5);
        assert!(
            !selector.observe(16, MotionState::Idle),
            "the same state must not restart the clip"
        );
        assert_eq!(selector.local_tick(16), 6);

        assert!(selector.observe(17, MotionState::Moving));
        assert_eq!(selector.clip(), 5);
        assert_eq!(
            selector.local_tick(17),
            0,
            "a state change restarts the new clip"
        );
        assert!(!selector.observe(18, MotionState::Moving));
        assert_eq!(selector.local_tick(18), 1);

        assert!(selector.observe(20, MotionState::Ascending));
        assert_eq!(selector.clip(), 7);
        assert_eq!(selector.local_tick(20), 0);

        assert!(selector.observe(21, MotionState::Falling));
        assert_eq!(selector.clip(), 8);
        assert_eq!(selector.local_tick(21), 0);

        assert!(selector.observe(22, MotionState::Landing));
        assert_eq!(selector.clip(), 9);
        assert_eq!(selector.local_tick(22), 0);
    }

    #[test]
    fn a_fixed_clip_ignores_state_and_uses_the_global_tick() {
        let mut selector = ClipSelector::new(
            ClipSelection {
                fixed: Some(3),
                map: ClipMap {
                    idle: 0,
                    walk: 1,
                    jump_up: 2,
                    fall: 4,
                    land: 5,
                },
            },
            9,
        );
        assert!(!selector.observe(42, MotionState::Moving));
        assert!(!selector.observe(43, MotionState::Ascending));
        assert!(!selector.observe(44, MotionState::Falling));
        assert!(!selector.observe(45, MotionState::Landing));
        assert_eq!(selector.clip(), 3);
        assert_eq!(selector.local_tick(42), 42);
        assert_eq!(selector.state(), None);
    }

    #[test]
    fn out_of_range_clip_indices_are_typed_errors() {
        let animation = SraAnimation::load(
            palette_container(&[0x001], 2, 100, &[&[1.0, 2.0]]),
            1,
        )
        .expect("one synthetic clip");
        assert_eq!(animation.validate_clip(0), Ok(()));
        assert_eq!(
            animation.validate_clip(1),
            Err(ClipError::OutOfRange {
                clip: 1,
                clip_count: 1
            })
        );
        assert!(animation.frame_count_of(1).is_err());
        assert!(animation
            .frame_index_for_clip_tick(1, 0, ANIMATION_UNITS_PER_SECOND, PlaybackLoop::Loop)
            .is_err());
    }

    #[test]
    fn a_record_with_two_tables_appends_the_second_after_the_base_index() {
        let first = palette_record(&[0x001], 2, 100, &[&[10.0, 20.0]]);
        let second = palette_record(&[0x001], 3, 200, &[&[1.0, 2.0, 3.0]]);
        let mut record = first.clone();
        record.extend_from_slice(&second);
        let animation = SraAnimation::load(encode_single_record_container(&record), 1)
            .expect("two tables in one record");

        assert_eq!(animation.base_clip_count(), 1);
        assert_eq!(animation.clip_count(), 2);
        assert_eq!(animation.scan_stats().additional_distinct, 1);
        assert_eq!(animation.scan_stats().additional_duplicates, 0);

        let clips = animation.clips();
        assert!(!clips[0].additional, "the first table keeps the base index");
        assert!(clips[1].additional, "the second table is marked additional");
        assert_eq!(
            (clips[1].index, clips[1].record, clips[1].offset),
            (1, 0, first.len()),
            "the second table is appended after the base index at its real offset"
        );
        assert_eq!((clips[1].frame_count, clips[1].divisor), (3, 200));
        assert_eq!(
            animation
                .decode_clip_index(1, 0)
                .expect("second table frame 0")
                .angles,
            vec![[1.0, 0.0, 0.0]],
            "the appended index addresses the second table, not the first"
        );
    }

    #[test]
    fn the_plus_0x08_duplicate_header_word_is_rejected_as_a_bogus_table() {
        let container = palette_container(
            &[0x001, 0x001],
            2,
            100,
            &[&[1024.0, 0.0], &[0.0, 0.0]],
        );
        let animation = SraAnimation::load(container, 2).expect("one real table");
        assert_eq!(animation.clip_count(), 1, "the alias is never indexed");
        let stats = animation.scan_stats();
        assert!(
            stats
                .rejected_aliases
                .iter()
                .any(|rejected| rejected.offset == 8
                    && rejected.frame_count == 2
                    && rejected.divisor == 100),
            "the +0x08 header word must be reported: {:?}",
            stats.rejected_aliases
        );
        assert!(stats.rejected_aliases.iter().all(|rejected| rejected.record == 0));
    }

    #[test]
    fn duplicate_additional_tables_are_deduplicated_by_fingerprint() {
        let a = palette_record(&[0x001], 1, 100, &[&[5.0]]);
        let b = palette_record(&[0x001], 2, 200, &[&[7.0, 8.0]]);
        let c = palette_record(&[0x001], 3, 300, &[&[9.0, 10.0, 11.0]]);

        // Record 0: A. Record 1: B + A (A repeats record 0). Record 2: C + B
        // (B repeats an earlier record's base). Both repeats are dropped.
        let mut record_one = b.clone();
        record_one.extend_from_slice(&a);
        let mut record_two = c.clone();
        record_two.extend_from_slice(&b);
        let container = multi_record_container(&[a, record_one, record_two]);

        let animation = SraAnimation::load(container, 1).expect("three base tables");
        assert_eq!(animation.base_clip_count(), 3);
        assert_eq!(animation.clip_count(), 3, "no duplicate is appended");
        assert_eq!(animation.scan_stats().additional_distinct, 0);
        assert_eq!(animation.scan_stats().additional_duplicates, 2);
    }

    #[test]
    fn base_indices_do_not_move_when_a_record_gains_a_second_table() {
        let first = palette_record(&[0x001], 2, 100, &[&[10.0, 20.0]]);
        let second = palette_record(&[0x001], 3, 200, &[&[1.0, 2.0, 3.0]]);
        let single = SraAnimation::load(encode_single_record_container(&first), 1)
            .expect("single table");
        let mut paired_record = first.clone();
        paired_record.extend_from_slice(&second);
        let paired = SraAnimation::load(encode_single_record_container(&paired_record), 1)
            .expect("paired record");

        assert_eq!(paired.clip_count(), 2);
        assert_eq!(
            single.clips()[0],
            paired.clips()[0],
            "index 0 is unchanged by the appended table"
        );
        assert_eq!((paired.clips()[0].index, paired.clips()[0].offset), (0, 0));
        assert_eq!(
            (paired.clips()[1].index, paired.clips()[1].offset),
            (1, first.len())
        );
    }

    #[test]
    fn identical_tables_share_a_fingerprint_while_different_tables_do_not() {
        let a = palette_record(&[0x001], 2, 100, &[&[10.0, 20.0]]);
        let a_twin = a.clone();
        let b = palette_record(&[0x001], 3, 200, &[&[1.0, 2.0, 3.0]]);
        let one = SraAnimation::load(encode_single_record_container(&a), 1).expect("A");
        let twin = SraAnimation::load(encode_single_record_container(&a_twin), 1).expect("A'");
        let other = SraAnimation::load(encode_single_record_container(&b), 1).expect("B");
        assert_eq!(one.clips()[0].fingerprint, twin.clips()[0].fingerprint);
        assert_ne!(one.clips()[0].fingerprint, other.clips()[0].fingerprint);
        assert_eq!(
            one.clips()[0].fingerprint_text(),
            format!(
                "2:100:{:08x}",
                one.clips()[0].fingerprint
            )
        );
    }

    #[test]
    fn clip_descriptors_expose_the_invariant_and_legacy_fingerprints() {
        let record = palette_record(&[0x001], 2, 100, &[&[10.0, 20.0]]);
        let animation =
            SraAnimation::load(encode_single_record_container(&record), 1).expect("one clip");
        let clip = animation.clip_info(0).expect("clip 0");
        assert_eq!(
            clip.fingerprint_text(),
            format!("2:100:{:08x}", clip.fingerprint),
            "the mapping key keeps the pair readable"
        );
        assert_eq!(
            clip.legacy_fingerprint_text(),
            format!("2:100:{:08x}", clip.legacy_fingerprint)
        );
        assert_ne!(
            clip.fingerprint, clip.legacy_fingerprint,
            "the 16-byte and 20-byte views are different hash inputs"
        );
        assert_ne!(clip.fingerprint, 0, "a real table has a non-zero digest");
    }
}

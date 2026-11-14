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
}

/// All packed-channel tables in one `.SRA` file, bound to a model slot count.
///
/// [`SraAnimation::load`] scans the container's payload records in file order
/// and keeps the **first** structurally valid table in each record whose
/// whole-slot inline stream decodes for `slot_count` slots. That "one table per
/// record" rule is what the shipped files show: every payload record starts a
/// table, and scanning every byte again would also match nested false positives
/// inside a header. The validated `RAZIEL.SRA` body table is clip 0.
///
/// Decoding is on demand, so a frame is a pure function of a table and the
/// requested index; no clock or wall time is stored here. The zero-argument
/// accessors keep addressing clip 0 so existing callers are unchanged.
#[derive(Debug, Clone)]
pub struct SraAnimation {
    bytes: Vec<u8>,
    clips: Vec<ClipTable>,
    slot_count: usize,
}

fn located_table(slice: &[u8], slot_count: usize) -> Option<(FrameHeader, u8)> {
    let table = PackedFrameTable::parse(slice, TableForm::File).ok()?;
    let planes = table.bit_planes(slot_count).ok()?;
    if planes.flags() & 0x01 == 0 {
        return None;
    }
    if packed_channel::decode_inline_channel(&table, slot_count, 0).is_err() {
        return None;
    }
    Some((table.header(), planes.flags()))
}

impl SraAnimation {
    /// Locate every clip and bind them to a model's slot count.
    ///
    /// The slot count is the model's bone count, not an `.SRA` field
    /// (`docs/notes/sra-slot-pose.md`). A container that does not parse, or a
    /// payload with no table that decodes for that many slots, is a loud error.
    pub fn load(bytes: Vec<u8>, slot_count: usize) -> Result<Self, String> {
        if slot_count == 0 {
            return Err("sra animation: slot count 0 cannot size the bit planes".to_string());
        }
        let container = parse_container(&bytes).ok_or_else(|| {
            "sra animation: the .SRA container directory did not parse".to_string()
        })?;
        let mut clips = Vec::new();
        for (record, range) in container.records.iter().enumerate() {
            let slice = &bytes[range.clone()];
            for offset in 0..slice.len() {
                if let Some((header, flags)) = located_table(&slice[offset..], slot_count) {
                    clips.push(ClipTable {
                        record,
                        offset,
                        table: range.start + offset..range.end,
                        header,
                        flags,
                    });
                    break;
                }
            }
        }
        if clips.is_empty() {
            return Err(format!(
                "sra animation: no packed frame table decoded for {slot_count} slots"
            ));
        }
        Ok(Self {
            bytes,
            clips,
            slot_count,
        })
    }

    /// Number of slots this animation decodes.
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Number of clips (located tables), in file order.
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
/// Derived from existing signals only: the grounded flag and whether stage 9
/// committed a non-zero horizontal delta this tick. It is presentation state
/// and never enters the canonical fold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionState {
    /// Grounded with no committed horizontal movement.
    Idle,
    /// Grounded with committed horizontal movement.
    Moving,
    /// Not grounded (rising or falling).
    Airborne,
}

impl MotionState {
    /// Map the two existing player signals to one motion state.
    pub fn from_player_signals(grounded: bool, moving: bool) -> Self {
        if !grounded {
            Self::Airborne
        } else if moving {
            Self::Moving
        } else {
            Self::Idle
        }
    }
}

/// Explicit state-to-clip mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipMap {
    pub idle: usize,
    pub walk: usize,
    pub jump: usize,
}

impl ClipMap {
    /// Default presentation mapping for `clip_count` clips.
    ///
    /// The shipped tables carry no clip identity, so this is an explicit
    /// presentation choice, not a recovered mapping: idle is the first clip,
    /// walk the middle clip and jump the last clip. It is deterministic and
    /// always in range; override it with `--clip-idle/--clip-walk/--clip-jump`
    /// or freeze one clip with `--clip <N>`.
    pub fn for_clip_count(clip_count: usize) -> Self {
        let last = clip_count.saturating_sub(1);
        Self {
            idle: 0,
            walk: last / 2,
            jump: last,
        }
    }

    /// The clip index for one motion state.
    pub fn clip_for(&self, state: MotionState) -> usize {
        match state {
            MotionState::Idle => self.idle,
            MotionState::Moving => self.walk,
            MotionState::Airborne => self.jump,
        }
    }
}

/// A fixed clip override plus a state map, as resolved from the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipSelection {
    pub fixed: Option<usize>,
    pub map: ClipMap,
}

/// State-driven clip selector.
///
/// Rule: the clip changes only when the observed motion state changes. On a
/// change the clip restarts at local tick 0 (`start_tick = tick`), so a state
/// change starts the new clip at its first frame instead of inheriting a
/// mid-clip index from the previous clip. The local tick is the tick count since
/// that restart, and is what the playback clock consumes.
///
/// A fixed override (`--clip <N>`) ignores the state and uses the global tick,
/// so it starts at frame 0 and never resets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipSelector {
    selection: ClipSelection,
    clip: usize,
    start_tick: u64,
    state: Option<MotionState>,
}

impl ClipSelector {
    /// Build a selector. The initial clip is the fixed override or the idle
    /// clip; the first [`ClipSelector::observe`] establishes the start tick.
    pub fn new(selection: ClipSelection) -> Self {
        let clip = selection.fixed.unwrap_or(selection.map.idle);
        Self {
            selection,
            clip,
            start_tick: 0,
            state: None,
        }
    }

    /// The active clip index.
    pub fn clip(&self) -> usize {
        self.clip
    }

    /// The last observed motion state, if any.
    pub fn state(&self) -> Option<MotionState> {
        self.state
    }

    /// Observe one tick's motion state. Returns `true` when the clip switched.
    pub fn observe(&mut self, tick: u64, state: MotionState) -> bool {
        if self.selection.fixed.is_some() || self.state == Some(state) {
            return false;
        }
        self.state = Some(state);
        self.clip = self.selection.map.clip_for(state);
        self.start_tick = tick;
        true
    }

    /// The tick the playback clock should use for this clip.
    pub fn local_tick(&self, tick: u64) -> u64 {
        if self.selection.fixed.is_some() {
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
    encode_single_record_container(&record)
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
            jump: 7,
        };
        assert_eq!(map.clip_for(MotionState::Idle), 2);
        assert_eq!(map.clip_for(MotionState::Moving), 5);
        assert_eq!(map.clip_for(MotionState::Airborne), 7);
        assert_eq!(
            MotionState::from_player_signals(true, false),
            MotionState::Idle
        );
        assert_eq!(
            MotionState::from_player_signals(true, true),
            MotionState::Moving
        );
        assert_eq!(
            MotionState::from_player_signals(false, true),
            MotionState::Airborne,
            "airborne wins over the committed horizontal delta"
        );
    }

    #[test]
    fn a_state_change_restarts_the_clip_and_only_changes_on_a_new_state() {
        let mut selector = ClipSelector::new(ClipSelection {
            fixed: None,
            map: ClipMap {
                idle: 2,
                walk: 5,
                jump: 7,
            },
        });

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

        assert!(selector.observe(20, MotionState::Airborne));
        assert_eq!(selector.clip(), 7);
        assert_eq!(selector.local_tick(20), 0);
    }

    #[test]
    fn a_fixed_clip_ignores_state_and_uses_the_global_tick() {
        let mut selector = ClipSelector::new(ClipSelection {
            fixed: Some(3),
            map: ClipMap {
                idle: 0,
                walk: 1,
                jump: 2,
            },
        });
        assert!(!selector.observe(42, MotionState::Moving));
        assert!(!selector.observe(43, MotionState::Airborne));
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
}

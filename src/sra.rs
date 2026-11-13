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

/// A located packed-channel table inside one `.SRA` payload record.
///
/// [`SraAnimation::load`] scans the container's payload records in file order
/// for the first structurally valid table whose whole-slot inline stream
/// decodes for `slot_count` slots. The validated `RAZIEL.SRA` body table is the
/// first such record. Decoding is on demand, so the animation frame is a pure
/// function of the table and the requested index; no clock or wall time is
/// stored here.
#[derive(Debug, Clone)]
pub struct SraAnimation {
    bytes: Vec<u8>,
    table: Range<usize>,
    header: FrameHeader,
    slot_count: usize,
}

impl SraAnimation {
    /// Locate the table and bind it to a model's slot count.
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
        for record in &container.records {
            let slice = &bytes[record.clone()];
            for at in 0..slice.len() {
                let Ok(table) = PackedFrameTable::parse(&slice[at..], TableForm::File) else {
                    continue;
                };
                let Ok(planes) = table.bit_planes(slot_count) else {
                    continue;
                };
                if planes.flags() & 0x01 == 0 {
                    continue;
                }
                if packed_channel::decode_inline_channel(&table, slot_count, 0).is_err() {
                    continue;
                }
                let header = table.header();
                let table = record.start + at..record.end;
                return Ok(Self {
                    bytes,
                    table,
                    header,
                    slot_count,
                });
            }
        }
        Err(format!(
            "sra animation: no packed frame table decoded for {slot_count} slots"
        ))
    }

    /// Number of slots this animation decodes.
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// Number of frames in the located table.
    pub fn frame_count(&self) -> usize {
        usize::from(self.header.frame_count)
    }

    /// Number of value streams the table header declares.
    pub fn channel_count(&self) -> usize {
        usize::from(self.header.table_count)
    }

    /// The table's frame-clock divisor.
    pub fn divisor(&self) -> i16 {
        self.header.divisor
    }

    /// Palette frame index for one canonical tick.
    ///
    /// One tick advances the frame time by one unit, which is the unit the live
    /// capture recorded (`time = 2400` selected frame `24` at divisor `100`).
    /// The mapping is saturated at `u16::MAX` and the index follows
    /// [`FRAME_INDEX_POLICY`] (`Saturate`), so it is monotonic and never wraps
    /// past the evidence.
    pub fn frame_index_for_tick(&self, tick: u64) -> Result<usize, String> {
        let frame_time = u16::try_from(tick.min(u64::from(u16::MAX))).unwrap_or(u16::MAX);
        self.frame_index_for_time(frame_time)
    }

    /// Palette frame index for one explicit frame-time unit.
    pub fn frame_index_for_time(&self, frame_time: u16) -> Result<usize, String> {
        packed_channel::select_frame_index(
            frame_time,
            self.header.divisor,
            self.header.frame_count,
            FRAME_INDEX_POLICY,
        )
    }

    /// Decode one palette frame.
    pub fn decode_index(&self, frame_index: usize) -> Result<SraFrame, String> {
        let table = self.table()?;
        let decoded = packed_channel::decode_inline_channel(&table, self.slot_count, frame_index)?;
        Ok(SraFrame {
            frame_index: decoded.frame_index,
            frame_count: self.frame_count(),
            channel_count: self.channel_count(),
            angles: decoded.slots.iter().map(|slot| slot.angles).collect(),
        })
    }

    /// Decode the frame selected by one frame-time unit.
    pub fn decode_time(&self, frame_time: u16) -> Result<SraFrame, String> {
        self.decode_index(self.frame_index_for_time(frame_time)?)
    }

    /// Decode the frame selected by one canonical tick.
    pub fn decode_tick(&self, tick: u64) -> Result<SraFrame, String> {
        self.decode_index(self.frame_index_for_tick(tick)?)
    }

    /// The slot-pose list for `frame`, ready for the hierarchical skinner.
    pub fn poses(&self, frame: &SraFrame) -> Vec<SlotPose> {
        poses_from_angles(&frame.angles)
    }

    /// Decode the frame selected by one canonical tick and build its poses.
    pub fn poses_for_tick(&self, tick: u64) -> Result<Vec<SlotPose>, String> {
        let frame = self.decode_tick(tick)?;
        Ok(self.poses(&frame))
    }

    fn table(&self) -> Result<PackedFrameTable<'_>, String> {
        PackedFrameTable::parse(&self.bytes[self.table.clone()], TableForm::File)
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

    fn palette_container(
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
        encode_single_record_container(&record)
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
    fn frame_index_advances_monotonically_with_tick_count_and_saturates() {
        let container = synthetic_animation_container(&[0x001], 50, 100, &[0.0]);
        let animation = SraAnimation::load(container, 1).expect("synthetic table must locate");
        assert_eq!(animation.frame_index_for_tick(0), Ok(0));
        assert_eq!(animation.frame_index_for_tick(99), Ok(0));
        assert_eq!(animation.frame_index_for_tick(100), Ok(1));
        assert_eq!(animation.frame_index_for_tick(2400), Ok(24));
        assert_eq!(animation.frame_index_for_tick(4900), Ok(49));
        assert_eq!(animation.frame_index_for_tick(5000), Ok(49));
        assert_eq!(animation.frame_index_for_tick(u64::MAX), Ok(49));
        let mut previous = 0;
        for tick in (0..=5000u64).step_by(37) {
            let index = animation.frame_index_for_tick(tick).expect("clock");
            assert!(index >= previous, "index went backwards at tick {tick}");
            assert!(index < animation.frame_count());
            previous = index;
        }
        assert_eq!(
            animation.decode_tick(2400).expect("frame 24").frame_index,
            24
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
}

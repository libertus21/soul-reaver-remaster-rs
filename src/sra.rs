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
}

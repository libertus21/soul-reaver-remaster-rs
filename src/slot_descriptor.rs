//! SR1 per-slot animation descriptor array, file form.
//!
//! The array is a run of 24-byte records inside an archived object-container
//! entry. It is *not* generated from the bone table: it is a stored block whose
//! element count equals the model bone count. The object loader expands each
//! file record to 32 bytes in memory and relocates the base-relative aim offset
//! at `+20` into an absolute pointer, so this module parses the file form only.
//!
//! File record (`24` bytes, little-endian):
//!
//! ```text
//! +0  u32     fixed zero (observed on every slot)
//! +4  u16     fixed zero (observed on every slot)
//! +6  u16     slot word; 0xFFFF on inactive slots, a small value on active slots
//! +8  u16     slot range start (observed <= +10)
//! +10 u16     slot range end
//! +12 u16     raw word
//! +14 u16     raw word
//! +16 u16     raw word
//! +18 u16     slot parent; 0xFFFF or a smaller slot index
//! +20 u16     base-relative aim offset (0 = none)
//! +22 u16     fixed zero (observed on every slot)
//! ```
//!
//! Only `aim_offset` is named by the descriptor spec; the other words stay raw
//! except for the observed shape described above. On a live raziel dump the
//! `0xFFFF` words at `+6`/`+10` appear only on the first three slots, while all
//! 66 records share the fixed-zero words and the range/parent shape. The locator
//! therefore reads the model header's own bone-table base first (no hardcoded
//! offset) and keeps the `0xFFFF` run as the headerless signature path.
//!
//! The record stride is fixed at the file level even though the referenced
//! per-slot aim data has no established length, so [`aim_record`] returns an
//! explicit [`AimRecord::UnknownLength`] instead of inventing one.
//!
//! Pure in-memory parsing: no I/O, no engine wiring, no rotation math.

/// Byte length of one descriptor record in the archived file form.
pub const FILE_RECORD_LEN: usize = 24;
/// Byte length of one descriptor record after the loader expands and relocates it.
pub const MEMORY_RECORD_LEN: usize = 32;
/// The word observed at `+6`/`+10` on inactive slots; the headerless signature.
pub const SENTINEL: u16 = 0xFFFF;

const SENTINEL_AT_6: usize = 6;
const SENTINEL_AT_10: usize = 10;
const AIM_OFFSET_AT: usize = 20;
const MAX_RELOCATIONS: u32 = 1 << 20;
const MODEL_HEADER_LEN: usize = 0x34;
const BONE_COUNT_AT: usize = 0x18;
const BONE_START_AT: usize = 0x1C;
const TABLE_AT: usize = 0x0C;
const TABLE_LEN_AT: usize = 0x0A;
const OBJECT_HEADER_LEN: usize = 0x64;
const OFFSET_ALIGN: usize = 512;

/// One 24-byte descriptor record. `word0`..`word18` and `word22` are raw; only
/// `aim_offset` carries a descriptor-spec name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotDescriptor {
    pub word0: u32,
    pub word4: u16,
    pub word6: u16,
    pub word8: u16,
    pub word10: u16,
    pub word12: u16,
    pub word14: u16,
    pub word16: u16,
    pub word18: u16,
    pub aim_offset: u16,
    pub word22: u16,
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..at.checked_add(2)?)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at.checked_add(4)?)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Parse one 24-byte record. The words are raw evidence, so a record is not
/// refused for carrying an active (`!= 0xFFFF`) slot word.
fn parse_record(record: &[u8]) -> Option<SlotDescriptor> {
    if record.len() != FILE_RECORD_LEN {
        return None;
    }
    Some(SlotDescriptor {
        word0: read_u32(record, 0)?,
        word4: read_u16(record, 4)?,
        word6: read_u16(record, 6)?,
        word8: read_u16(record, 8)?,
        word10: read_u16(record, 10)?,
        word12: read_u16(record, 12)?,
        word14: read_u16(record, 14)?,
        word16: read_u16(record, 16)?,
        word18: read_u16(record, 18)?,
        aim_offset: read_u16(record, AIM_OFFSET_AT)?,
        word22: read_u16(record, 22)?,
    })
}

/// The object-container data base, mirroring the level/object loader:
/// `4 * roundup512(record_count + 1)`.
fn object_data_base(records: u32) -> usize {
    let words = (records as usize).saturating_add(1);
    let rounded = words.saturating_add(OFFSET_ALIGN - 1) & !(OFFSET_ALIGN - 1);
    rounded.saturating_mul(4)
}

/// Read the model header's declared bone-table base and count.
///
/// Returns `(entry_relative_base, declared_count)` when the payload is an
/// object container with a single-model table and a bone table that fits.
fn header_bone_table(payload: &[u8]) -> Option<(usize, usize)> {
    let records = read_u32(payload, 0)?;
    if !(1..=MAX_RELOCATIONS).contains(&records)
        || 4 + 4 * records as usize > payload.len()
    {
        return None;
    }
    let data_base = object_data_base(records);
    if data_base > payload.len() {
        return None;
    }
    let data = &payload[data_base..];
    let header = data.get(..OBJECT_HEADER_LEN)?;
    let table_at = read_u32(header, TABLE_AT)? as usize;
    let table_len = usize::from(read_u16(header, TABLE_LEN_AT)?) + 1;
    let table_end = table_at.checked_add(4 * table_len)?;
    let table = data.get(table_at..table_end)?;
    let model_at = read_u32(table, 0)? as usize;
    if model_at == 0 {
        return None;
    }
    let model = data.get(model_at..model_at.checked_add(MODEL_HEADER_LEN)?)?;
    let bone_count = read_u32(model, BONE_COUNT_AT)? as usize;
    let bone_start = read_u32(model, BONE_START_AT)? as usize;
    if bone_count == 0 || bone_start == 0 {
        return None;
    }
    let bone_end = bone_start.checked_add(bone_count.checked_mul(FILE_RECORD_LEN)?)?;
    if bone_end > data.len() {
        return None;
    }
    Some((data_base + bone_start, bone_count))
}

/// Whether the record starting at `at` carries the inactive-slot signature at
/// `+6` and `+10`.
fn has_inactive_signature(payload: &[u8], at: usize) -> bool {
    read_u16(payload, at.saturating_add(SENTINEL_AT_6)) == Some(SENTINEL)
        && read_u16(payload, at.saturating_add(SENTINEL_AT_10)) == Some(SENTINEL)
}

/// First start of a run of at least `bone_count` records carrying the inactive
/// `0xFFFF` signature, spaced by the file stride.
fn first_inactive_run(payload: &[u8], bone_count: usize) -> Option<usize> {
    if bone_count == 0 || payload.len() < bone_count.saturating_mul(FILE_RECORD_LEN) {
        return None;
    }
    let len = payload.len();
    // `run[at]` is the number of consecutive signed records spaced by the file
    // stride starting at `at`; the backwards fold keeps the scan linear.
    let mut run = vec![0usize; len];
    for at in (0..len).rev() {
        if has_inactive_signature(payload, at) {
            run[at] = 1 + run.get(at + FILE_RECORD_LEN).copied().unwrap_or(0);
        }
    }
    run.iter().position(|&count| count >= bone_count)
}

/// Locate the descriptor array without a hardcoded offset.
///
/// The primary path reads the payload as an object container and returns the
/// model header's own bone-table base when its declared count equals
/// `bone_count`. This is generic: the header, not this module, supplies the
/// position. When the payload is not an object container (synthetic fixtures,
/// headerless blocks), the function falls back to the requested signature: a
/// run of `bone_count` records with `0xFFFF` at `+6` and `+10`.
pub fn locate_descriptor_array(payload: &[u8], bone_count: usize) -> Option<usize> {
    if let Some((base, declared)) = header_bone_table(payload)
        && declared == bone_count
    {
        return Some(base);
    }
    first_inactive_run(payload, bone_count)
}

/// Parse the `bone_count` records starting at `array_base`.
///
/// Refuses a truncated or out-of-range block. Records are returned raw, so an
/// active slot word does not make the block invalid.
pub fn parse_descriptor_array(
    payload: &[u8],
    array_base: usize,
    bone_count: usize,
) -> Option<Vec<SlotDescriptor>> {
    let total = bone_count.checked_mul(FILE_RECORD_LEN)?;
    let end = array_base.checked_add(total)?;
    let block = payload.get(array_base..end)?;
    block.chunks_exact(FILE_RECORD_LEN).map(parse_record).collect()
}

/// A per-slot aim reference and its bounded raw bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AimRecord<'a> {
    /// The slot carries no per-slot aim data (`+20 == 0`).
    Absent,
    /// The slot references data at `array_base + offset`, but no evidence
    /// establishes the record length. `bytes` is bounded by the next larger
    /// aim offset in `descriptors`, or by the end of `payload`. An empty
    /// `bytes` with a non-zero `offset` means the offset lies outside
    /// `payload`; it is not an invented length.
    UnknownLength { offset: u16, bytes: &'a [u8] },
}

/// Return the raw, bounded aim slice for `slot`. Never invents a record length.
///
/// The known offsets do not share a fixed stride and the referenced data has no
/// length prefix, count byte, or `0xFFFF`/`0` terminator, so the length stays
/// explicitly unknown. The slice stops at the next known aim offset, or at the
/// end of `payload`.
pub fn aim_record<'a>(
    payload: &'a [u8],
    descriptors: &[SlotDescriptor],
    array_base: usize,
    slot: usize,
) -> AimRecord<'a> {
    let Some(descriptor) = descriptors.get(slot) else {
        return AimRecord::Absent;
    };
    let offset = descriptor.aim_offset;
    if offset == 0 {
        return AimRecord::Absent;
    }
    let start = array_base.saturating_add(usize::from(offset));
    let next = descriptors
        .iter()
        .filter_map(|other| {
            (other.aim_offset > offset)
                .then_some(array_base.saturating_add(usize::from(other.aim_offset)))
        })
        .filter(|&bound| bound > start)
        .min()
        .unwrap_or(payload.len())
        .min(payload.len());
    let end = if start < payload.len() && next > start {
        next
    } else {
        start
    };
    let bytes = payload.get(start..end).unwrap_or(&[]);
    AimRecord::UnknownLength { offset, bytes }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One record with the inactive-slot `0xFFFF` words at `+6`/`+10`.
    fn inactive_record(aim_offset: u16) -> [u8; FILE_RECORD_LEN] {
        let mut record = [0u8; FILE_RECORD_LEN];
        record[6..8].copy_from_slice(&SENTINEL.to_le_bytes());
        record[10..12].copy_from_slice(&SENTINEL.to_le_bytes());
        record[20..22].copy_from_slice(&aim_offset.to_le_bytes());
        record
    }

    /// One active record: non-sentinel words but the observed raw shape.
    fn active_record(index: u16, aim_offset: u16) -> [u8; FILE_RECORD_LEN] {
        let mut record = [0u8; FILE_RECORD_LEN];
        record[6..8].copy_from_slice(&index.to_le_bytes());
        record[8..10].copy_from_slice(&index.to_le_bytes());
        record[10..12].copy_from_slice(&(index + 1).to_le_bytes());
        record[18..20].copy_from_slice(&0u16.to_le_bytes());
        record[20..22].copy_from_slice(&aim_offset.to_le_bytes());
        record
    }

    fn signature_fixture(junk: usize, count: usize) -> Vec<u8> {
        let mut payload = vec![0xAA; junk];
        for index in 0..count {
            payload.extend_from_slice(&inactive_record(index as u16));
        }
        payload
    }

    /// Synthetic object container with a model header pointing at a bone table.
    /// Values are invented, never game bytes.
    fn container_fixture(bone_count: usize) -> (Vec<u8>, usize) {
        let mut payload = 1u32.to_le_bytes().to_vec();
        payload.extend_from_slice(&12u32.to_le_bytes());
        payload.resize(object_data_base(1), 0);
        let data_at = payload.len();
        let mut data = vec![0u8; 0x200];
        data[0x0A..0x0C].copy_from_slice(&0u16.to_le_bytes());
        data[TABLE_AT..TABLE_AT + 4].copy_from_slice(&100u32.to_le_bytes());
        data[0x4C..0x54].copy_from_slice(b"raziel__");
        let model_at = 120usize;
        data[100..104].copy_from_slice(&(model_at as u32).to_le_bytes());
        data[model_at + BONE_COUNT_AT..model_at + BONE_COUNT_AT + 4]
            .copy_from_slice(&(bone_count as u32).to_le_bytes());
        data[model_at + BONE_START_AT..model_at + BONE_START_AT + 4]
            .copy_from_slice(&200u32.to_le_bytes());
        for index in 0..bone_count {
            let at = 200 + index * FILE_RECORD_LEN;
            data[at..at + FILE_RECORD_LEN].copy_from_slice(&active_record(index as u16, 0));
        }
        payload.extend_from_slice(&data);
        (payload, data_at + 200)
    }

    #[test]
    fn parses_the_exact_24_byte_layout() {
        let mut record = inactive_record(0x0080);
        record[0..4].copy_from_slice(&0x1122_3300u32.to_le_bytes());
        record[4..6].copy_from_slice(&10u16.to_le_bytes());
        record[12..14].copy_from_slice(&11u16.to_le_bytes());
        record[14..16].copy_from_slice(&12u16.to_le_bytes());
        record[16..18].copy_from_slice(&13u16.to_le_bytes());
        record[18..20].copy_from_slice(&14u16.to_le_bytes());

        let descriptors = parse_descriptor_array(&record, 0, 1).expect("one record must parse");
        assert_eq!(descriptors.len(), 1);
        let slot = descriptors[0];
        assert_eq!(slot.word0, 0x1122_3300);
        assert_eq!(slot.word4, 10);
        assert_eq!(slot.word6, SENTINEL);
        assert_eq!(slot.word8, 0);
        assert_eq!(slot.word10, SENTINEL);
        assert_eq!(slot.word12, 11);
        assert_eq!(slot.word14, 12);
        assert_eq!(slot.word16, 13);
        assert_eq!(slot.word18, 14);
        assert_eq!(slot.aim_offset, 0x0080);
        assert_eq!(slot.word22, 0);
    }

    #[test]
    fn locates_the_inactive_run_without_a_fixed_offset() {
        let payload = signature_fixture(37, 4);
        assert_eq!(locate_descriptor_array(&payload, 4), Some(37));
        assert_eq!(parse_descriptor_array(&payload, 37, 4).map(|d| d.len()), Some(4));
    }

    #[test]
    fn header_anchor_finds_the_bone_table_and_ignores_the_signature() {
        let (container, expected) = container_fixture(3);
        // The first three records are active, so the 0xFFFF run cannot match.
        assert_eq!(locate_descriptor_array(&container, 3), Some(expected));
        let parsed = parse_descriptor_array(&container, expected, 3).expect("records must parse");
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].word6, 0);
        assert_eq!(parsed[2].word10, 3);
    }

    #[test]
    fn header_anchor_requires_the_declared_count_to_match() {
        let (container, _) = container_fixture(3);
        // No inactive run exists, so a mismatched requested count is refused.
        assert_eq!(locate_descriptor_array(&container, 4), None);
    }

    #[test]
    fn parses_active_records_without_the_inactive_signature() {
        let payload: Vec<u8> = (0..3)
            .flat_map(|index| active_record(index, 0))
            .collect();
        let parsed = parse_descriptor_array(&payload, 0, 3).expect("raw records must parse");
        assert_eq!(parsed[0].word6, 0);
        assert_eq!(parsed[2].word10, 3);
    }

    #[test]
    fn bone_count_gate_rejects_a_short_run() {
        let payload = signature_fixture(0, 3);
        assert_eq!(locate_descriptor_array(&payload, 4), None);
        assert_eq!(locate_descriptor_array(&payload, 3), Some(0));
        assert_eq!(locate_descriptor_array(&payload, 0), None);
    }

    #[test]
    fn a_record_that_breaks_the_signature_ends_the_run() {
        let mut payload = signature_fixture(0, 3);
        payload[FILE_RECORD_LEN + 10] = 0;
        assert_eq!(locate_descriptor_array(&payload, 3), None);
        assert_eq!(locate_descriptor_array(&payload, 1), Some(0));
    }

    #[test]
    fn offset_zero_is_absent_and_nonzero_is_unknown_length() {
        let mut payload: Vec<u8> = (0..3).flat_map(|index| active_record(index, 0)).collect();
        payload[FILE_RECORD_LEN + 20..FILE_RECORD_LEN + 22].copy_from_slice(&0x20u16.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 3).expect("records must parse");
        assert_eq!(aim_record(&payload, &descriptors, 0, 0), AimRecord::Absent);
        assert_eq!(aim_record(&payload, &descriptors, 0, 2), AimRecord::Absent);
        match aim_record(&payload, &descriptors, 0, 1) {
            AimRecord::UnknownLength { offset, bytes } => {
                assert_eq!(offset, 0x20);
                assert_eq!(bytes.len(), payload.len() - 0x20);
            }
            other => panic!("expected UnknownLength, got {other:?}"),
        }
    }

    #[test]
    fn aim_slice_stops_at_the_next_known_offset() {
        let mut payload: Vec<u8> = (0..3).flat_map(|index| active_record(index, 0)).collect();
        payload[20..22].copy_from_slice(&0x20u16.to_le_bytes());
        payload[FILE_RECORD_LEN + 20..FILE_RECORD_LEN + 22]
            .copy_from_slice(&0x30u16.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 3).expect("records must parse");
        match aim_record(&payload, &descriptors, 0, 0) {
            AimRecord::UnknownLength { offset, bytes } => {
                assert_eq!(offset, 0x20);
                assert_eq!(bytes.len(), 0x10);
            }
            other => panic!("expected UnknownLength, got {other:?}"),
        }
    }

    #[test]
    fn out_of_range_slot_and_offset_are_refused_without_inventing_data() {
        let mut payload: Vec<u8> = active_record(0, 0).to_vec();
        payload[20..22].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 1).expect("record must parse");
        assert_eq!(aim_record(&payload, &descriptors, 0, 9), AimRecord::Absent);
        match aim_record(&payload, &descriptors, 0, 0) {
            AimRecord::UnknownLength { offset, bytes } => {
                assert_eq!(offset, 0xFFFF);
                assert!(bytes.is_empty(), "an out-of-range offset yields no bytes");
            }
            other => panic!("expected UnknownLength, got {other:?}"),
        }
    }

    #[test]
    fn truncated_and_malformed_payloads_are_refused() {
        let payload = signature_fixture(0, 2);
        assert!(parse_descriptor_array(&payload, 0, 2).is_some());
        assert!(parse_descriptor_array(&payload[..payload.len() - 1], 0, 2).is_none());
        assert!(parse_descriptor_array(&payload[..FILE_RECORD_LEN], 0, 2).is_none());
        assert!(parse_descriptor_array(&payload, 1, 2).is_none());
        assert!(parse_descriptor_array(&payload, 0, 0).is_some_and(|d| d.is_empty()));
        assert_eq!(locate_descriptor_array(&payload[..FILE_RECORD_LEN], 2), None);
        assert_eq!(locate_descriptor_array(&payload[..10], 1), None);
    }
}

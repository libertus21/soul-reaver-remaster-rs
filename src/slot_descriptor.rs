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
//! Each slot with a non-zero `aim_offset` owns one contiguous sub-slice of a
//! shared aim stream. The sub-slice starts with a 24-byte header and is followed
//! by `frame_count` 20-byte frames; the count is read from the per-slot track
//! header that sits in the record region immediately after the descriptor array.
//! [`aim_slices`] returns those real bounds. The corpus-wide gap distribution is
//! reported in `docs/notes/aim-stream-bounds.md`; a sub-slice is **not** a whole
//! number of 3-i16 triples.
//!
//! The aim shorts are fixed-point. [`FixedPoint`] uses the remaster's documented
//! animation scale (`4096 == 1.0`, `>>12`); the aim-specific magnitude and the
//! absolute direction-vs-angle meaning remain PARTLY and are documented.
//!
//! Pure in-memory parsing: no I/O, no engine wiring, no rotation math.

/// Byte length of one descriptor record in the archived file form.
pub const FILE_RECORD_LEN: usize = 24;
/// Byte length of one descriptor record after the loader expands and relocates it.
pub const MEMORY_RECORD_LEN: usize = 32;
/// The word observed at `+6`/`+10` on inactive slots; the headerless signature.
pub const SENTINEL: u16 = 0xFFFF;
/// Byte length of the per-slot aim header at the start of every aim sub-slice.
pub const AIM_HEADER_LEN: usize = 24;
/// Byte length of one aim keyframe. Each sub-slice holds
/// `AIM_HEADER_LEN + frame_count * AIM_FRAME_LEN` bytes.
pub const AIM_FRAME_LEN: usize = 20;
/// Signed 16-bit words in one 20-byte aim keyframe.
///
/// This is the evidence-backed element count: the frame is `20 / 2` signed
/// shorts, all ten carrying small fixed-point-like values (see [`Frame`]).
/// The internal grouping of the ten words stays UNKNOWN.
pub const AIM_FRAME_WORDS: usize = 10;
/// Byte length of one per-slot track header (count, destination offset, tail).
pub const TRACK_ENTRY_LEN: usize = 16;
/// Byte length of the unresolved tail of a [`TrackEntry`].
pub const TRACK_TAIL_LEN: usize = 8;
/// Highest frame count accepted from a per-slot track header (corpus max 62).
pub const MAX_TRACK_FRAMES: u32 = 4096;
/// The remaster's documented animation fixed-point scale: `4096 == 1.0`, `>>12`.
///
/// `docs/notes/spline-animation.md` section 2.1 documents the only animation
/// fixed-point shift in the remaster: every animation short is converted with a
/// right shift of 12. The aim shorts are described as fixed-point in
/// `docs/notes/animation-loader-trace.md`; the aim-specific magnitude and the
/// absolute meaning stay PARTLY (see `docs/notes/aim-stream-bounds.md`).
pub const Q12_SHIFT: u32 = 12;
/// Raw value of `1.0` in the documented animation fixed-point scale.
pub const Q12_ONE: i32 = 1 << Q12_SHIFT;

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

/// How the end of an [`AimSlice`] was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AimTermination {
    /// Bounded by the next larger aim offset of the same model.
    NextOffset,
    /// Last slot: bounded by its track header's frame count.
    FrameCount(u32),
    /// Last slot with no resolvable track header: only the 24-byte header is
    /// bounded. The frame region stays UNKNOWN rather than invented.
    HeaderOnly,
}

/// One delimited sub-slice of the shared aim stream.
///
/// `bytes` covers `offset .. offset + AIM_HEADER_LEN + frame_count * AIM_FRAME_LEN`
/// when the end is established, or the next offset when it is not the last slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AimSlice<'a> {
    /// Descriptor index of the slot that owns this sub-slice.
    pub slot: usize,
    /// Base-relative aim offset carried by the descriptor (`+20`).
    pub offset: u16,
    /// How the end bound was established.
    pub termination: AimTermination,
    /// The bounded raw bytes.
    pub bytes: &'a [u8],
}

/// A per-slot aim reference and its bounded raw bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AimRecord<'a> {
    /// The slot carries no per-slot aim data (`+20 == 0`).
    Absent,
    /// The slot references a bounded aim sub-slice.
    Bounded(AimSlice<'a>),
}

/// Active aim slots, sorted by ascending offset. Descriptor order is monotonic
/// across the corpus, but sorting keeps the bound rule independent of order.
fn active_aim_slots(descriptors: &[SlotDescriptor]) -> Vec<(usize, u16)> {
    let mut slots: Vec<(usize, u16)> = descriptors
        .iter()
        .enumerate()
        .filter_map(|(slot, descriptor)| {
            (descriptor.aim_offset != 0).then_some((slot, descriptor.aim_offset))
        })
        .collect();
    slots.sort_by_key(|&(_, offset)| offset);
    slots
}

/// One 16-byte per-slot track header.
///
/// The header is 16 bytes, but headers are **not** a fixed-stride array: each
/// header sits at the start of its slot's record block, whose length equals the
/// slot's aim sub-slice length. Only the first two fields are evidence-backed;
/// the tail stays UNKNOWN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackEntry {
    /// Number of `AIM_FRAME_LEN` frames. Corpus test: for every non-last slot,
    /// `aim_next - aim == AIM_HEADER_LEN + frame_count * AIM_FRAME_LEN`.
    pub frame_count: u32,
    /// Base-relative start of the first frame (`aim_offset + AIM_HEADER_LEN`).
    pub frame_offset: u32,
    /// UNKNOWN: the remaining 8 bytes. Zero on some entries and non-zero on
    /// others; no stable role (candidate divisor) is established.
    pub raw_tail: [u8; TRACK_TAIL_LEN],
}

/// The engine's animation fixed-point value, Q12 (`4096 == 1.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPoint(i32);

impl FixedPoint {
    /// Zero.
    pub const ZERO: Self = Self(0);
    /// One (`4096` raw).
    pub const ONE: Self = Self(Q12_ONE);

    /// Wrap a raw signed short from the aim stream.
    pub fn from_raw(raw: i16) -> Self {
        Self(i32::from(raw))
    }

    /// Wrap an already-scaled raw value.
    pub fn from_q12(raw: i32) -> Self {
        Self(raw)
    }

    /// The raw Q12 integer.
    pub fn raw(self) -> i32 {
        self.0
    }

    /// Convert to `f32` with the documented `>>12` scale.
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / Q12_ONE as f32
    }

    /// Convert from `f32`, rounding to the nearest Q12 integer.
    pub fn from_f32(value: f32) -> Self {
        Self((value * Q12_ONE as f32).round() as i32)
    }
}

/// The best-supported aim scale: Q12 (`4096 == 1.0`), see the module docs.
pub const AIM_UNIT: FixedPoint = FixedPoint::ONE;

/// One 20-byte aim keyframe, read as ten signed 16-bit words.
///
/// # Evidence
///
/// - **CONFIRMED (shape):** the frame is [`AIM_FRAME_WORDS`] little-endian
///   `i16` words. On the corpus, every word position carries the same small
///   bidirectional band; reading the bytes as five `u32` mixes a negative high
///   half (`0xFFFF....`) and is not a component reading. The tenth word is
///   active, so the frame is not "three triples plus two padding bytes".
/// - **INFERRED (unit):** the words are Q12 fixed point via [`FixedPoint`]; the
///   measured magnitudes sit inside the aim band (`|word| << 4096`).
/// - **UNKNOWN (meaning):** how the ten words group (e.g. axis triples), and
///   whether any word is a time, frame index or weight. No word position is a
///   monotone counter and no lane is a bounded weight, but that is a negative
///   observation, not a decoded field.
///
/// Only the raw word positions are named; no semantic field is invented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// The ten signed words in file order.
    pub words: [i16; AIM_FRAME_WORDS],
}

impl Frame {
    /// Parse one exactly-20-byte frame; a shorter or longer slice is refused.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != AIM_FRAME_LEN {
            return None;
        }
        let mut words = [0i16; AIM_FRAME_WORDS];
        for (index, word) in words.iter_mut().enumerate() {
            let at = index * 2;
            *word = i16::from_le_bytes([bytes[at], bytes[at + 1]]);
        }
        Some(Self { words })
    }

    /// The ten words converted with the Q12 scale (`INFERRED`, see the type
    /// docs). The conversion itself is exact; the *unit* is inferred.
    pub fn units(&self) -> [f32; AIM_FRAME_WORDS] {
        self.words.map(|word| FixedPoint::from_raw(word).to_f32())
    }

    /// The leading three words as a Q12 triplet.
    ///
    /// The animation trace names a per-slot "three signed shorts" aim vector,
    /// so the first three words are the natural candidate. Their grouping stays
    /// `INFERRED`; this helper is a candidate accessor, not a decoded field.
    pub fn leading_triplet(&self) -> [FixedPoint; 3] {
        [
            FixedPoint::from_raw(self.words[0]),
            FixedPoint::from_raw(self.words[1]),
            FixedPoint::from_raw(self.words[2]),
        ]
    }
}

/// Parse the whole frames of one slot out of its bounded aim sub-slice.
///
/// `payload` must begin at the descriptor-array base, exactly like
/// [`aim_slices`]. A slot with no aim data, a `HeaderOnly` slice, or an
/// incomplete trailing frame yields no frame for that remainder (the tail is
/// ignored, never padded into a fabricated frame). The `24 + count * 20` layout
/// is CONFIRMED; the field meanings of each [`Frame`] are not.
pub fn frames(descriptors: &[SlotDescriptor], payload: &[u8], slot: usize) -> Vec<Frame> {
    let Some(descriptor) = descriptors.get(slot) else {
        return Vec::new();
    };
    if descriptor.aim_offset == 0 {
        return Vec::new();
    }
    let Some(slice) = aim_slices(descriptors, payload)
        .into_iter()
        .find(|slice| slice.slot == slot)
    else {
        return Vec::new();
    };
    let body = slice.bytes.get(AIM_HEADER_LEN..).unwrap_or(&[]);
    body.chunks_exact(AIM_FRAME_LEN)
        .filter_map(Frame::parse)
        .collect()
}

/// Read the per-slot track headers that follow the descriptor array.
///
/// `payload` must begin at the descriptor-array base. The header block starts
/// after the array plus a 0/8/16-byte preamble; each header then sits at the
/// start of its slot's block, and blocks advance by the slot's aim gap. Returns
/// one entry per active aim slot, in ascending offset order, stopping at the
/// first malformed header (a partial list is honest, not padded).
pub fn track_entries(descriptors: &[SlotDescriptor], payload: &[u8]) -> Vec<TrackEntry> {
    let active = active_aim_slots(descriptors);
    let Some(&(_, first_aim)) = active.first() else {
        return Vec::new();
    };
    let Some(region_start) = descriptors.len().checked_mul(FILE_RECORD_LEN) else {
        return Vec::new();
    };
    let Some(region) = payload.get(region_start..) else {
        return Vec::new();
    };
    let expected = u32::from(first_aim).saturating_add(AIM_HEADER_LEN as u32);
    let scan_end = region.len().saturating_sub(TRACK_ENTRY_LEN).min(64);
    let Some(first) = (0..scan_end).step_by(4).find(|&at| {
        read_u32(region, at + 4) == Some(expected)
            && read_u32(region, at).is_some_and(|count| (1..=MAX_TRACK_FRAMES).contains(&count))
    }) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    let mut pos = region_start.saturating_add(first);
    for (index, &(_, aim)) in active.iter().enumerate() {
        let Some(count) = read_u32(payload, pos) else {
            break;
        };
        let Some(frame_offset) = read_u32(payload, pos + 4) else {
            break;
        };
        if frame_offset != u32::from(aim).saturating_add(AIM_HEADER_LEN as u32)
            || !(1..=MAX_TRACK_FRAMES).contains(&count)
        {
            break;
        }
        let Some(tail) = payload.get(pos + 8..pos + TRACK_ENTRY_LEN) else {
            break;
        };
        let mut raw_tail = [0u8; TRACK_TAIL_LEN];
        raw_tail.copy_from_slice(tail);
        entries.push(TrackEntry {
            frame_count: count,
            frame_offset,
            raw_tail,
        });
        let Some(&(_, next_aim)) = active.get(index + 1) else {
            break;
        };
        pos = pos.saturating_add(usize::from(next_aim).saturating_sub(usize::from(aim)));
    }
    entries
}

/// Frame counts aligned with [`active_aim_slots`].
fn aim_frame_counts(descriptors: &[SlotDescriptor], payload: &[u8]) -> Vec<u32> {
    track_entries(descriptors, payload)
        .into_iter()
        .map(|entry| entry.frame_count)
        .collect()
}

/// Delimit every per-slot aim sub-slice.
///
/// `payload` must begin at the descriptor-array base, so `aim_offset` indexes
/// into it directly. Bounds:
///
/// - non-last slot: `[offset, next larger offset)`;
/// - last slot with a track header: `[offset, offset + 24 + count * 20)`;
/// - last slot without a resolvable header: `[offset, offset + 24)`.
///
/// A slice whose `offset` lies outside `payload` yields empty `bytes`; no length
/// is invented.
pub fn aim_slices<'a>(descriptors: &[SlotDescriptor], payload: &'a [u8]) -> Vec<AimSlice<'a>> {
    let active = active_aim_slots(descriptors);
    let counts = aim_frame_counts(descriptors, payload);
    let mut slices = Vec::with_capacity(active.len());
    for (index, &(slot, offset)) in active.iter().enumerate() {
        let start = usize::from(offset);
        let (end, termination) = match active.get(index + 1) {
            Some(&(_, next)) => (usize::from(next), AimTermination::NextOffset),
            None => match counts.get(index) {
                Some(&count) if count > 0 => {
                    let len = AIM_HEADER_LEN.saturating_add(count as usize * AIM_FRAME_LEN);
                    (start.saturating_add(len), AimTermination::FrameCount(count))
                }
                _ => (
                    start.saturating_add(AIM_HEADER_LEN),
                    AimTermination::HeaderOnly,
                ),
            },
        };
        let end = end.min(payload.len()).max(start.min(payload.len()));
        let bytes = payload.get(start..end).unwrap_or(&[]);
        slices.push(AimSlice {
            slot,
            offset,
            termination,
            bytes,
        });
    }
    slices
}

/// Return the bounded aim slice for `slot`.
///
/// Thin wrapper over [`aim_slices`] for callers that hold the whole entry and
/// the located `array_base`.
pub fn aim_record<'a>(
    payload: &'a [u8],
    descriptors: &[SlotDescriptor],
    array_base: usize,
    slot: usize,
) -> AimRecord<'a> {
    if descriptors.get(slot).is_none_or(|descriptor| descriptor.aim_offset == 0) {
        return AimRecord::Absent;
    }
    let Some(relative) = payload.get(array_base..) else {
        return AimRecord::Absent;
    };
    aim_slices(descriptors, relative)
        .into_iter()
        .find(|slice| slice.slot == slot)
        .map(AimRecord::Bounded)
        .unwrap_or(AimRecord::Absent)
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

    /// Build an array-relative payload: `n` descriptors, a track-header region
    /// with `preamble` leading bytes, and one contiguous aim sub-slice per slot.
    /// Own values only; never game bytes.
    fn aim_fixture(counts: &[u32], preamble: usize) -> (Vec<u8>, Vec<u16>) {
        let n = counts.len();
        let region_start = n * FILE_RECORD_LEN;
        let block_lens: Vec<usize> = counts
            .iter()
            .map(|&count| AIM_HEADER_LEN + count as usize * AIM_FRAME_LEN)
            .collect();
        let table_end = region_start + preamble + block_lens.iter().sum::<usize>();
        let mut aims = Vec::with_capacity(n);
        let mut cursor = table_end + 16;
        for &len in &block_lens {
            aims.push(cursor as u16);
            cursor += len;
        }
        let mut payload = vec![0u8; cursor];
        for (index, &aim) in aims.iter().enumerate() {
            let at = index * FILE_RECORD_LEN;
            payload[at..at + FILE_RECORD_LEN].copy_from_slice(&active_record(index as u16, aim));
        }
        let mut pos = region_start + preamble;
        for (index, &count) in counts.iter().enumerate() {
            payload[pos..pos + 4].copy_from_slice(&count.to_le_bytes());
            payload[pos + 4..pos + 8]
                .copy_from_slice(&(u32::from(aims[index]) + 24).to_le_bytes());
            for byte in 0..8 {
                payload[pos + 8 + byte] = (index as u8).wrapping_add(byte as u8);
            }
            pos += block_lens[index];
        }
        for (index, &aim) in aims.iter().enumerate() {
            let start = aim as usize;
            for byte in 0..AIM_HEADER_LEN {
                payload[start + byte] = 0x40 ^ (index as u8);
            }
            for frame in 0..counts[index] as usize {
                for byte in 0..AIM_FRAME_LEN {
                    payload[start + AIM_HEADER_LEN + frame * AIM_FRAME_LEN + byte] =
                        (index as u8).wrapping_mul(7).wrapping_add(byte as u8);
                }
            }
        }
        (payload, aims)
    }

    #[test]
    fn offset_zero_is_absent_and_nonzero_is_bounded() {
        let mut payload: Vec<u8> = (0..3).flat_map(|index| active_record(index, 0)).collect();
        payload[FILE_RECORD_LEN + 20..FILE_RECORD_LEN + 22].copy_from_slice(&0x20u16.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 3).expect("records must parse");
        assert_eq!(aim_record(&payload, &descriptors, 0, 0), AimRecord::Absent);
        assert_eq!(aim_record(&payload, &descriptors, 0, 2), AimRecord::Absent);
        match aim_record(&payload, &descriptors, 0, 1) {
            AimRecord::Bounded(slice) => {
                assert_eq!(slice.offset, 0x20);
                assert_eq!(slice.termination, AimTermination::HeaderOnly);
                assert_eq!(slice.bytes.len(), AIM_HEADER_LEN);
            }
            other => panic!("expected Bounded, got {other:?}"),
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
            AimRecord::Bounded(slice) => {
                assert_eq!(slice.offset, 0x20);
                assert_eq!(slice.termination, AimTermination::NextOffset);
                assert_eq!(slice.bytes.len(), 0x10);
            }
            other => panic!("expected Bounded, got {other:?}"),
        }
    }

    #[test]
    fn out_of_range_slot_and_offset_are_refused_without_inventing_data() {
        let mut payload: Vec<u8> = active_record(0, 0).to_vec();
        payload[20..22].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 1).expect("record must parse");
        assert_eq!(aim_record(&payload, &descriptors, 0, 9), AimRecord::Absent);
        match aim_record(&payload, &descriptors, 0, 0) {
            AimRecord::Bounded(slice) => {
                assert_eq!(slice.offset, 0xFFFF);
                assert_eq!(slice.termination, AimTermination::HeaderOnly);
                assert!(slice.bytes.is_empty(), "an out-of-range offset yields no bytes");
            }
            other => panic!("expected Bounded, got {other:?}"),
        }
    }

    #[test]
    fn aim_slices_use_next_offset_then_frame_count() {
        let counts = [2u32, 1, 3];
        let (payload, aims) = aim_fixture(&counts, 8);
        let descriptors = parse_descriptor_array(&payload, 0, counts.len()).expect("records");
        let slices = aim_slices(&descriptors, &payload);
        assert_eq!(slices.len(), 3);
        for (index, slice) in slices.iter().enumerate() {
            assert_eq!(slice.slot, index);
            assert_eq!(slice.offset, aims[index]);
            let expected = AIM_HEADER_LEN + counts[index] as usize * AIM_FRAME_LEN;
            assert_eq!(slice.bytes.len(), expected, "slice {index}");
        }
        assert_eq!(slices[0].termination, AimTermination::NextOffset);
        assert_eq!(slices[1].termination, AimTermination::NextOffset);
        assert_eq!(slices[2].termination, AimTermination::FrameCount(3));
        // The non-last bound is the next offset, not the payload end.
        assert_eq!(slices[0].bytes.len(), usize::from(aims[1] - aims[0]));
        assert_eq!(slices[1].bytes.len(), usize::from(aims[2] - aims[1]));
    }

    #[test]
    fn last_slice_is_bounded_by_its_frame_count_not_the_payload_end() {
        let counts = [1u32, 1];
        let (payload, aims) = aim_fixture(&counts, 8);
        let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records");
        let slices = aim_slices(&descriptors, &payload);
        let last = slices.last().expect("two slices");
        assert_eq!(last.offset, aims[1]);
        assert_eq!(last.bytes.len(), AIM_HEADER_LEN + AIM_FRAME_LEN);
        assert_eq!(last.termination, AimTermination::FrameCount(1));
        assert!(aims[1] as usize + last.bytes.len() <= payload.len());
    }

    #[test]
    fn frame_count_bound_tolerates_the_observed_preambles() {
        for preamble in [0usize, 8, 16] {
            let (payload, _aims) = aim_fixture(&[1u32, 2], preamble);
            let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records");
            let slices = aim_slices(&descriptors, &payload);
            assert_eq!(slices.len(), 2, "preamble {preamble}");
            assert_eq!(slices[1].termination, AimTermination::FrameCount(2));
            assert_eq!(
                slices[1].bytes.len(),
                AIM_HEADER_LEN + 2 * AIM_FRAME_LEN
            );
        }
    }

    #[test]
    fn track_headers_expose_count_offset_and_raw_tail() {
        let (payload, aims) = aim_fixture(&[1u32, 2], 8);
        let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records");
        let entries = track_entries(&descriptors, &payload);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].frame_count, 1);
        assert_eq!(entries[1].frame_count, 2);
        assert_eq!(entries[0].frame_offset, u32::from(aims[0]) + 24);
        assert_eq!(entries[1].frame_offset, u32::from(aims[1]) + 24);
        assert_eq!(entries[0].raw_tail[0], 0);
        assert_eq!(entries[1].raw_tail[0], 1);
    }

    #[test]
    fn track_entries_stop_at_a_malformed_header() {
        let counts = [1u32, 1, 1];
        let (mut payload, _aims) = aim_fixture(&counts, 8);
        let region_start = 3 * FILE_RECORD_LEN;
        let second = region_start + 8 + AIM_HEADER_LEN + AIM_FRAME_LEN;
        payload[second + 4..second + 8].copy_from_slice(&0u32.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 3).expect("records");
        let entries = track_entries(&descriptors, &payload);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].frame_count, 1);
    }

    #[test]
    fn last_slice_falls_back_to_the_header_without_a_track_header() {
        let counts = [1u32, 2];
        let (mut payload, _aims) = aim_fixture(&counts, 8);
        let region_start = 2 * FILE_RECORD_LEN;
        payload[region_start + 8 + 4..region_start + 8 + 8].copy_from_slice(&0u32.to_le_bytes());
        let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records");
        let slices = aim_slices(&descriptors, &payload);
        assert_eq!(slices[0].termination, AimTermination::NextOffset);
        assert_eq!(slices[1].termination, AimTermination::HeaderOnly);
        assert_eq!(slices[1].bytes.len(), AIM_HEADER_LEN);
    }

    #[test]
    fn fixed_point_uses_the_documented_q12_scale() {
        assert_eq!(Q12_SHIFT, 12);
        assert_eq!(Q12_ONE, 4096);
        assert_eq!(FixedPoint::from_raw(4096).to_f32(), 1.0);
        assert_eq!(FixedPoint::from_raw(2048).to_f32(), 0.5);
        assert_eq!(FixedPoint::from_raw(-2048).to_f32(), -0.5);
        assert_eq!(FixedPoint::from_f32(1.0).raw(), Q12_ONE);
        assert_eq!(FixedPoint::from_f32(-0.5).raw(), -Q12_ONE / 2);
        assert_eq!(AIM_UNIT.to_f32(), 1.0);
        assert_eq!(FixedPoint::ZERO.raw(), 0);
        assert_eq!(FixedPoint::from_q12(123).raw(), 123);
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

    /// Encode one frame's ten words back into its little-endian 20 bytes.
    fn frame_bytes(words: [i16; AIM_FRAME_WORDS]) -> [u8; AIM_FRAME_LEN] {
        let mut bytes = [0u8; AIM_FRAME_LEN];
        for (index, word) in words.iter().enumerate() {
            bytes[index * 2..index * 2 + 2].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn frame_parse_reads_ten_signed_words_and_the_q12_units() {
        let words = [-4096, 2048, -1, 0, 1, -2, 3, -4, 5, 4096];
        let frame = Frame::parse(&frame_bytes(words)).expect("20 bytes must parse");
        assert_eq!(frame.words, words);
        let units = frame.units();
        assert_eq!(units[0], -1.0);
        assert_eq!(units[1], 0.5);
        assert_eq!(units[9], 1.0);
        assert_eq!(units[2], -1.0 / Q12_ONE as f32);
        assert_eq!(frame.leading_triplet().map(FixedPoint::raw), [-4096, 2048, -1]);
    }

    #[test]
    fn frame_parse_refuses_any_other_length() {
        assert!(Frame::parse(&[0u8; 19]).is_none());
        assert!(Frame::parse(&[0u8; 21]).is_none());
        assert!(Frame::parse(&[]).is_none());
        assert!(Frame::parse(&[0u8; AIM_FRAME_LEN]).is_some());
    }

    #[test]
    fn frames_parse_two_whole_frames_after_the_24_byte_header() {
        let mut payload = vec![0u8; 152];
        payload[..FILE_RECORD_LEN].copy_from_slice(&active_record(0, 64));
        payload[FILE_RECORD_LEN..2 * FILE_RECORD_LEN].copy_from_slice(&active_record(1, 128));
        let first = [1i16, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let second = [11i16, 12, 13, 14, 15, 16, 17, 18, 19, 20];
        // Slot 0's sub-slice is [64, 128): a 24-byte header then two 20-byte frames.
        payload[64 + AIM_HEADER_LEN..64 + AIM_HEADER_LEN + AIM_FRAME_LEN]
            .copy_from_slice(&frame_bytes(first));
        payload[64 + AIM_HEADER_LEN + AIM_FRAME_LEN..64 + AIM_HEADER_LEN + 2 * AIM_FRAME_LEN]
            .copy_from_slice(&frame_bytes(second));

        let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records must parse");
        let parsed = frames(&descriptors, &payload, 0);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].words, first);
        assert_eq!(parsed[1].words, second);
    }

    #[test]
    fn frames_are_empty_for_an_absent_slot() {
        let payload: Vec<u8> = (0..2).flat_map(|index| active_record(index, 0)).collect();
        let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records must parse");
        assert!(frames(&descriptors, &payload, 0).is_empty());
        assert!(frames(&descriptors, &payload, 9).is_empty());
    }

    #[test]
    fn frames_ignore_an_incomplete_trailing_frame() {
        let mut payload = vec![0u8; 124];
        payload[..FILE_RECORD_LEN].copy_from_slice(&active_record(0, 48));
        payload[FILE_RECORD_LEN..2 * FILE_RECORD_LEN].copy_from_slice(&active_record(1, 100));
        let descriptors = parse_descriptor_array(&payload, 0, 2).expect("records must parse");
        // Slot 0 is bounded by the next offset at 100: [48, 100) = 52 bytes, i.e.
        // the 24-byte header plus one whole 20-byte frame and an 8-byte tail.
        assert_eq!(aim_slices(&descriptors, &payload)[0].bytes.len(), 52);
        assert_eq!(frames(&descriptors, &payload, 0).len(), 1, "the 8-byte tail is not a frame");
    }

}

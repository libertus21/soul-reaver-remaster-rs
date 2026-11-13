//! Packed-channel frame-table decoder, file form.
//!
//! `docs/notes/packed-channel-format.md` establishes the table the channel
//! record at `+0x38` points at: a standalone frame table whose header holds a
//! `u16` frame count (`+0x04`) and an `s16` divisor (`+0x06`), followed by an
//! array of value-stream offsets/pointers and a per-frame bit-plane block.
//!
//! This module decodes that table into per-slot binary-angle triples and the
//! rotation each triple produces through
//! [`crate::pose::rotation_from_binary_angles`]. It is pure in-memory parsing:
//! no I/O, no engine wiring, no canonical-state change.
//!
//! The layout implemented here (file form, `docs/notes/packed-channel-format.md`
//! "The frame table" and "The per-frame bit planes"):
//!
//! ```text
//! +0x00  u8   table count (number of value streams)
//! +0x04  u16  frame count
//! +0x06  i16  divisor
//! +0x14 + i*4   i32 value-stream offset   (file form, 32-bit)
//! +0x18 + i*8   u64 value-stream pointer  (memory form, 64-bit)
//! block base    flags byte, then present planes
//! ```
//!
//! The memory form shifts the value-offset array and the bit-plane block by
//! `0x10`; [`TableForm`] selects the width. Only the file form carries offsets
//! in the validated `RAZIEL.SRA` replay, but both shapes are parsed so the
//! pointer-width difference is explicit rather than hidden in an offset.
//!
//! Present planes are concatenated at `align8(3 * slot_count)` bits in the
//! fixed order angle (`bit 0`), scale (`bit 1`), translation (`bit 2`). Each
//! slot owns three consecutive bits per plane, one per component (`X`, `Y`,
//! `Z`), and those groups form the 12-bit lane mask
//! `angles | scale << 4 | translation << 8`. The value stream is one block per
//! set bit in ascending lane order: `i32 0` plus a `frame_count` palette, or
//! `i32 1` plus one literal.
//!
//! ## Frame-index policy
//!
//! `frame_index = floor(frame_time / divisor)` is CONFIRMED. What the engine
//! does when the index reaches `frame_count` is **UNKNOWN** (UNKNOWN 1 in the
//! note): the wrap branch was reached in code but never exercised by the
//! validated frame. This module therefore does not guess silently: the policy
//! is a named choice, [`FrameIndexPolicy`], and the default used by
//! [`decode_channel`] is [`FRAME_INDEX_POLICY`] = [`FrameIndexPolicy::Saturate`].
//! Saturating to the last frame is conservative and deterministic; it never
//! invents a wrap the evidence does not establish.

use crate::pose::{Mat3, rotation_from_binary_angles};

/// Byte offset of the table count.
pub const TABLE_COUNT_AT: usize = 0x00;
/// Byte offset of the `u16` frame count.
pub const FRAME_COUNT_AT: usize = 0x04;
/// Byte offset of the `s16` divisor.
pub const DIVISOR_AT: usize = 0x06;
/// Byte offset of the value-offset array in the file form.
pub const FILE_VALUE_OFFSETS_AT: usize = 0x14;
/// Byte offset of the value-pointer array in the memory form.
pub const MEMORY_VALUE_OFFSETS_AT: usize = 0x18;
/// Highest table count accepted from `+0x00`.
pub const MAX_TABLE_COUNT: u8 = 16;
/// Highest frame count accepted from `+0x04`.
pub const MAX_FRAME_COUNT: u16 = 4096;
/// Highest slot count accepted by the bit-plane layout.
pub const MAX_SLOT_COUNT: usize = 4096;

const FLAG_ANGLE: u8 = 0x01;
const FLAG_SCALE: u8 = 0x02;
const FLAG_TRANSLATION: u8 = 0x04;
const DEFINED_FLAGS: u8 = FLAG_ANGLE | FLAG_SCALE | FLAG_TRANSLATION;
const LANES: usize = 12;
const ANGLE_LANES: usize = 3;

/// Pointer width of the frame table's offset array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableForm {
    /// 32-bit value offsets; the shipped `.SRA` table form.
    File,
    /// 64-bit value pointers; the runtime form, shifted by `0x10`.
    Memory,
}

impl TableForm {
    /// Byte offset of the value-offset array for this form.
    pub const fn value_offsets_at(self) -> usize {
        match self {
            Self::File => FILE_VALUE_OFFSETS_AT,
            Self::Memory => MEMORY_VALUE_OFFSETS_AT,
        }
    }

    /// Width of one value offset in bytes.
    pub const fn pointer_width(self) -> usize {
        match self {
            Self::File => 4,
            Self::Memory => 8,
        }
    }
}

/// Parsed frame-table header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Number of value streams at `+0x00`.
    pub table_count: u8,
    /// Number of frames at `+0x04`.
    pub frame_count: u16,
    /// Positive clock divisor at `+0x06`.
    pub divisor: i16,
}

/// How [`select_frame_index`] handles an index at or past `frame_count`.
///
/// This is a **named choice**, not a measured engine behaviour: the wrap branch
/// is UNKNOWN 1 in `docs/notes/packed-channel-format.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameIndexPolicy {
    /// Clamp to `frame_count - 1`. The default ([`FRAME_INDEX_POLICY`]).
    Saturate,
    /// Map the index modulo `frame_count`. Offered for the unvalidated wrap
    /// branch; do not assume it matches the engine.
    Wrap,
}

/// The policy [`decode_channel`] applies when the clock overruns the table.
///
/// [`FrameIndexPolicy::Saturate`] is chosen because the validated replay never
/// reached the wrap branch. Selecting either variant is explicit at the call
/// site through [`decode_channel_range`].
pub const FRAME_INDEX_POLICY: FrameIndexPolicy = FrameIndexPolicy::Saturate;

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    let end = at
        .checked_add(2)
        .ok_or_else(|| format!("packed table: u16 offset +0x{at:X} overflows"))?;
    let slice = bytes.get(at..end).ok_or_else(|| {
        format!(
            "packed table: u16 at byte +0x{at:X} is outside the {}-byte record",
            bytes.len()
        )
    })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_i16(bytes: &[u8], at: usize) -> Result<i16, String> {
    Ok(read_u16(bytes, at)? as i16)
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    let end = at
        .checked_add(4)
        .ok_or_else(|| format!("packed table: u32 offset +0x{at:X} overflows"))?;
    let slice = bytes.get(at..end).ok_or_else(|| {
        format!(
            "packed table: u32 at byte +0x{at:X} is outside the {}-byte record",
            bytes.len()
        )
    })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, String> {
    let end = at
        .checked_add(8)
        .ok_or_else(|| format!("packed table: u64 offset +0x{at:X} overflows"))?;
    let slice = bytes.get(at..end).ok_or_else(|| {
        format!(
            "packed table: u64 at byte +0x{at:X} is outside the {}-byte record",
            bytes.len()
        )
    })?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_i32(bytes: &[u8], at: usize) -> Result<i32, String> {
    Ok(read_u32(bytes, at)? as i32)
}

fn read_f32(bytes: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(read_u32(bytes, at)?))
}

/// Parse the `frame_count`/`divisor` header and validate its bounds.
///
/// Rejects a zero or oversized table count, a zero frame count, an oversized
/// frame count and a non-positive divisor. Every failure names the field.
pub fn parse_header(bytes: &[u8]) -> Result<FrameHeader, String> {
    let table_count = *bytes.get(TABLE_COUNT_AT).ok_or_else(|| {
        format!(
            "packed table: missing table count at byte +0x{TABLE_COUNT_AT:02X} (record is {} bytes)",
            bytes.len()
        )
    })?;
    if table_count == 0 || table_count > MAX_TABLE_COUNT {
        return Err(format!(
            "packed table: table count {table_count} is outside 1..={MAX_TABLE_COUNT}"
        ));
    }
    let frame_count = read_u16(bytes, FRAME_COUNT_AT)?;
    if frame_count == 0 || frame_count > MAX_FRAME_COUNT {
        return Err(format!(
            "packed table: frame count {frame_count} is outside 1..={MAX_FRAME_COUNT}"
        ));
    }
    let divisor = read_i16(bytes, DIVISOR_AT)?;
    if divisor <= 0 {
        return Err(format!(
            "packed table: divisor {divisor} must be positive (frame time is unsigned)"
        ));
    }
    Ok(FrameHeader {
        table_count,
        frame_count,
        divisor,
    })
}

/// Select a palette frame index under an explicit [`FrameIndexPolicy`].
///
/// `frame_time / divisor` floors because both operands are non-negative. An
/// index at or past `frame_count` is clamped ([`FrameIndexPolicy::Saturate`]) or
/// wrapped ([`FrameIndexPolicy::Wrap`]). Zero/negative divisors and a zero
/// frame count are loud errors.
pub fn select_frame_index(
    frame_time: u16,
    divisor: i16,
    frame_count: u16,
    policy: FrameIndexPolicy,
) -> Result<usize, String> {
    if divisor <= 0 {
        return Err(format!(
            "packed table: divisor {divisor} must be positive to select a frame"
        ));
    }
    if frame_count == 0 {
        return Err("packed table: frame count 0 cannot select a frame".to_string());
    }
    let divisor = usize::from(u16::try_from(divisor).map_err(|_| {
        format!("packed table: divisor {divisor} must be positive to select a frame")
    })?);
    let frame_count = usize::from(frame_count);
    let raw = usize::from(frame_time) / divisor;
    Ok(match policy {
        FrameIndexPolicy::Saturate => raw.min(frame_count - 1),
        FrameIndexPolicy::Wrap => raw % frame_count,
    })
}

/// A parsed frame table borrowing the record bytes.
#[derive(Debug, Clone, Copy)]
pub struct PackedFrameTable<'a> {
    bytes: &'a [u8],
    header: FrameHeader,
    form: TableForm,
}

impl<'a> PackedFrameTable<'a> {
    /// Parse the header and validate the value-offset array.
    pub fn parse(bytes: &'a [u8], form: TableForm) -> Result<Self, String> {
        let header = parse_header(bytes)?;
        let offsets_at = form.value_offsets_at();
        let offsets_end = usize::from(header.table_count)
            .checked_mul(form.pointer_width())
            .and_then(|width| offsets_at.checked_add(width))
            .ok_or_else(|| "packed table: value-offset array overflows".to_string())?;
        if offsets_end > bytes.len() {
            return Err(format!(
                "packed table: value-offset array ends at byte +0x{offsets_end:X}, past the {}-byte record",
                bytes.len()
            ));
        }
        Ok(Self {
            bytes,
            header,
            form,
        })
    }

    /// The parsed header.
    pub fn header(&self) -> FrameHeader {
        self.header
    }

    /// The borrowed record bytes.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Byte offset of the bit-plane block's flags byte.
    pub fn bit_plane_base(&self) -> usize {
        self.form.value_offsets_at()
            + usize::from(self.header.table_count) * self.form.pointer_width()
    }

    /// Byte offset of the value stream selected by `selector`.
    pub fn value_offset(&self, selector: usize) -> Result<usize, String> {
        if selector >= usize::from(self.header.table_count) {
            return Err(format!(
                "packed table: value selector {selector} is outside the {} recorded streams",
                self.header.table_count
            ));
        }
        let at = self.form.value_offsets_at() + selector * self.form.pointer_width();
        let offset = match self.form {
            TableForm::File => usize::try_from(read_u32(self.bytes, at)?)
                .map_err(|_| format!("packed table: value offset at +0x{at:X} does not fit usize"))?,
            TableForm::Memory => usize::try_from(read_u64(self.bytes, at)?)
                .map_err(|_| format!("packed table: value pointer at +0x{at:X} does not fit usize"))?,
        };
        if offset >= self.bytes.len() {
            return Err(format!(
                "packed table: value stream offset +0x{offset:X} is past the {}-byte record",
                self.bytes.len()
            ));
        }
        Ok(offset)
    }

    /// Parse the per-frame bit-plane block for `slot_count` slots.
    pub fn bit_planes(&self, slot_count: usize) -> Result<BitPlanes<'a>, String> {
        BitPlanes::parse(self.bytes, self.bit_plane_base(), slot_count)
    }

    /// Byte offset of the file-form **inline** value stream for `slot_count`
    /// slots.
    ///
    /// The validated `RAZIEL.SRA` replay stores every set lane's block in one
    /// contiguous stream directly after the present planes, 4-byte aligned.
    /// The table's own `+0x14` offset array points at float arrays that are not
    /// this stream (UNKNOWN 3 in `docs/notes/packed-channel-format.md`), so
    /// file-form callers that decode a whole record use this offset with
    /// [`ValueStream::new`].
    pub fn inline_value_stream_offset(&self, slot_count: usize) -> Result<usize, String> {
        let planes = self.bit_planes(slot_count)?;
        let present = (planes.flags() & DEFINED_FLAGS).count_ones() as usize;
        let stride_bits = align8_bits(
            slot_count
                .checked_mul(3)
                .ok_or_else(|| "packed table: 3 * slot_count overflows".to_string())?,
        )?;
        let plane_bytes = present
            .checked_mul(stride_bits / 8)
            .ok_or_else(|| "packed table: bit-plane block overflows".to_string())?;
        let raw = self
            .bit_plane_base()
            .checked_add(1)
            .and_then(|value| value.checked_add(plane_bytes))
            .ok_or_else(|| "packed table: inline value stream overflows".to_string())?;
        Ok((raw + 3) & !3)
    }
}

/// The per-frame flag byte and 3-bit-per-slot plane layout.
#[derive(Debug, Clone, Copy)]
pub struct BitPlanes<'a> {
    bytes: &'a [u8],
    base: usize,
    slot_count: usize,
    flags: u8,
    stride_bits: usize,
}

impl<'a> BitPlanes<'a> {
    /// Parse the flags byte and validate that every present plane fits.
    pub fn parse(bytes: &'a [u8], base: usize, slot_count: usize) -> Result<Self, String> {
        if slot_count == 0 || slot_count > MAX_SLOT_COUNT {
            return Err(format!(
                "packed table: slot count {slot_count} is outside 1..={MAX_SLOT_COUNT}"
            ));
        }
        let flags = *bytes.get(base).ok_or_else(|| {
            format!(
                "packed table: missing bit-plane flags at byte +0x{base:X} (record is {} bytes)",
                bytes.len()
            )
        })?;
        let undefined = flags & !DEFINED_FLAGS;
        if undefined != 0 {
            return Err(format!(
                "packed table: flags 0x{flags:02X} set undefined plane bits 0x{undefined:02X}; only 0x01 angle, 0x02 scale and 0x04 translation are defined"
            ));
        }
        let stride_bits = align8_bits(
            slot_count
                .checked_mul(3)
                .ok_or_else(|| "packed table: 3 * slot_count overflows".to_string())?,
        )?;
        let present = (flags & DEFINED_FLAGS).count_ones() as usize;
        let plane_bytes = present
            .checked_mul(stride_bits / 8)
            .ok_or_else(|| "packed table: bit-plane block overflows".to_string())?;
        let end = base
            .checked_add(1)
            .and_then(|value| value.checked_add(plane_bytes))
            .ok_or_else(|| "packed table: bit-plane block overflows".to_string())?;
        if end > bytes.len() {
            return Err(format!(
                "packed table: bit-plane block needs {end} bytes, record has {}",
                bytes.len()
            ));
        }
        Ok(Self {
            bytes,
            base,
            slot_count,
            flags,
            stride_bits,
        })
    }

    /// The raw flags byte.
    pub fn flags(&self) -> u8 {
        self.flags
    }

    /// Whether the plane selected by `plane` (0 angle, 1 scale, 2 translation)
    /// is present.
    pub fn has_plane(&self, plane: u8) -> bool {
        plane < 3 && self.flags & (1 << plane) != 0
    }

    /// The 12-bit lane mask for `slot`: `angles | scale << 4 | translation << 8`.
    pub fn slot_mask(&self, slot: usize) -> Result<u16, String> {
        if slot >= self.slot_count {
            return Err(format!(
                "packed table: slot {slot} is outside the {}-slot plane layout",
                self.slot_count
            ));
        }
        let mut mask = 0u16;
        let mut present = 0usize;
        for plane in 0..3u8 {
            if !self.has_plane(plane) {
                continue;
            }
            let first_bit = self.base * 8 + 8 + 3 * slot + present * self.stride_bits;
            let mut group = 0u16;
            for lane in 0..3usize {
                if self.bit(first_bit + lane)? {
                    group |= 1 << lane;
                }
            }
            mask |= group << (4 * u16::from(plane));
            present += 1;
        }
        Ok(mask)
    }

    fn bit(&self, at: usize) -> Result<bool, String> {
        let byte_at = at / 8;
        let byte = *self.bytes.get(byte_at).ok_or_else(|| {
            format!(
                "packed table: bit {at} (byte +0x{byte_at:X}) is outside the {}-byte record",
                self.bytes.len()
            )
        })?;
        Ok(byte & (1 << (at % 8)) != 0)
    }
}

fn align8_bits(bits: usize) -> Result<usize, String> {
    let padded = bits
        .checked_add(7)
        .ok_or_else(|| "packed table: bit-plane stride overflows".to_string())?;
    Ok((padded / 8) * 8)
}

/// A linear palette/literal value stream.
#[derive(Debug, Clone, Copy)]
pub struct ValueStream<'a> {
    bytes: &'a [u8],
    cursor: usize,
    frame_count: usize,
    frame_index: usize,
}

impl<'a> ValueStream<'a> {
    /// Start a stream at `offset` for a table with `frame_count` frames.
    pub fn new(
        bytes: &'a [u8],
        offset: usize,
        frame_count: usize,
        frame_index: usize,
    ) -> Result<Self, String> {
        if frame_count == 0 {
            return Err("packed value stream: frame count 0 cannot index a palette".to_string());
        }
        if frame_index >= frame_count {
            return Err(format!(
                "packed value stream: frame index {frame_index} is outside the {frame_count}-frame palette"
            ));
        }
        if offset >= bytes.len() {
            return Err(format!(
                "packed value stream: start +0x{offset:X} is past the {}-byte record",
                bytes.len()
            ));
        }
        Ok(Self {
            bytes,
            cursor: offset,
            frame_count,
            frame_index,
        })
    }

    /// Current byte cursor, pointing at the next block.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Consume one block and return its selected value.
    ///
    /// `i32 0` selects `frame_index` from a `frame_count` palette; `i32 1`
    /// selects the literal. Any other command, a block past the record, or a
    /// non-finite value is an error naming the byte offset.
    pub fn next_value(&mut self) -> Result<f32, String> {
        let command_at = self.cursor;
        let command = read_i32(self.bytes, command_at)?;
        let value = match command {
            0 => {
                let value_at = command_at
                    .checked_add(4)
                    .and_then(|value| value.checked_add(4 * self.frame_index))
                    .ok_or_else(|| {
                        format!(
                            "packed value stream: palette index offset at +0x{command_at:X} overflows"
                        )
                    })?;
                let block_end = command_at
                    .checked_add(4)
                    .and_then(|value| value.checked_add(4 * self.frame_count))
                    .ok_or_else(|| {
                        format!(
                            "packed value stream: palette block at +0x{command_at:X} overflows"
                        )
                    })?;
                if block_end > self.bytes.len() {
                    return Err(format!(
                        "packed value stream: palette block at +0x{command_at:X} needs {block_end} bytes, record has {}",
                        self.bytes.len()
                    ));
                }
                self.cursor = block_end;
                read_f32(self.bytes, value_at)?
            }
            1 => {
                let value_at = command_at.checked_add(4).ok_or_else(|| {
                    format!("packed value stream: literal offset at +0x{command_at:X} overflows")
                })?;
                let block_end = command_at.checked_add(8).ok_or_else(|| {
                    format!("packed value stream: literal block at +0x{command_at:X} overflows")
                })?;
                if block_end > self.bytes.len() {
                    return Err(format!(
                        "packed value stream: literal block at +0x{command_at:X} needs {block_end} bytes, record has {}",
                        self.bytes.len()
                    ));
                }
                self.cursor = block_end;
                read_f32(self.bytes, value_at)?
            }
            other => {
                return Err(format!(
                    "packed value stream: byte +0x{command_at:X} has command {other}; expected 0 (palette) or 1 (literal)"
                ));
            }
        };
        if !value.is_finite() {
            return Err(format!(
                "packed value stream: value at byte +0x{command_at:X} is not finite ({value})"
            ));
        }
        Ok(value)
    }
}

/// One slot's decoded binary angles and the rotation they produce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotAngles {
    /// The three angle lanes in `X`, `Y`, `Z` order (binary-angle units).
    pub angles: [f32; 3],
    /// `rotation_from_binary_angles(angles[0], angles[1], angles[2])`.
    pub rotation: Mat3,
}

/// One channel's decoded slot window.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedChannel {
    /// Palette frame index used for this decode.
    pub frame_index: usize,
    /// The plane flags byte.
    pub flags: u8,
    /// One entry per slot in the decoded window.
    pub slots: Vec<SlotAngles>,
}

/// Which slots and which value stream a channel covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelLayout {
    /// First covered slot, absolute in the bit-plane layout.
    pub first_slot: usize,
    /// Number of contiguous slots covered by this channel.
    pub count: usize,
    /// Total slots in the bit-plane layout (defines the plane stride).
    pub slot_count: usize,
    /// Index into the table's value-stream offset array.
    pub value_selector: usize,
}

/// Decode one slot's angle triple, consuming every set lane from `stream`.
///
/// The walk is ascending lane order, so scale and translation blocks are
/// consumed even though only lanes 0-2 are returned. Missing angle lanes stay
/// `0.0`, matching the note's "leaves Y and Z at zero" behaviour.
pub fn decode_slot_angles(
    planes: &BitPlanes<'_>,
    stream: &mut ValueStream<'_>,
    slot: usize,
) -> Result<[f32; 3], String> {
    let mask = planes.slot_mask(slot)?;
    let mut angles = [0.0f32; ANGLE_LANES];
    for lane in 0..LANES {
        if mask & (1 << lane) == 0 {
            continue;
        }
        let value = stream.next_value()?;
        if lane < ANGLE_LANES {
            angles[lane] = value;
        }
    }
    Ok(angles)
}

/// Decode a channel window into per-slot angles and rotations.
///
/// `layout` names the slot window and value stream. The caller supplies the
/// [`FrameIndexPolicy`] so the unvalidated wrap branch is never implicit.
pub fn decode_channel_range(
    table: &PackedFrameTable<'_>,
    layout: ChannelLayout,
    frame_time: u16,
    policy: FrameIndexPolicy,
) -> Result<DecodedChannel, String> {
    if layout.slot_count == 0 {
        return Err("packed channel: slot count 0 has no bit-plane stride".to_string());
    }
    let end = layout
        .first_slot
        .checked_add(layout.count)
        .ok_or_else(|| "packed channel: slot window overflows".to_string())?;
    if end > layout.slot_count {
        return Err(format!(
            "packed channel: slots {}..{end} exceed the {}-slot plane layout",
            layout.first_slot, layout.slot_count
        ));
    }
    let header = table.header();
    let frame_index =
        select_frame_index(frame_time, header.divisor, header.frame_count, policy)?;
    let planes = table.bit_planes(layout.slot_count)?;
    if layout.count == 0 {
        return Ok(DecodedChannel {
            frame_index,
            flags: planes.flags(),
            slots: Vec::new(),
        });
    }
    let offset = table.value_offset(layout.value_selector)?;
    let mut stream = ValueStream::new(
        table.bytes(),
        offset,
        usize::from(header.frame_count),
        frame_index,
    )?;
    let mut slots = Vec::with_capacity(layout.count);
    for slot in layout.first_slot..end {
        let angles = decode_slot_angles(&planes, &mut stream, slot)?;
        slots.push(SlotAngles {
            angles,
            rotation: rotation_from_binary_angles(angles[0], angles[1], angles[2]),
        });
    }
    Ok(DecodedChannel {
        frame_index,
        flags: planes.flags(),
        slots,
    })
}

/// Decode a whole file-form channel covering slots `0..slot_count`, stream 0.
///
/// This is the presentation-facing entry point: pure, deterministic and
/// independent of canonical frame state. It uses [`FRAME_INDEX_POLICY`]; pass a
/// [`ChannelLayout`] to [`decode_channel_range`] for a non-zero first slot, a
/// windowed count or another value selector.
pub fn decode_channel(
    bytes: &[u8],
    slot_count: usize,
    frame_time: u16,
) -> Result<DecodedChannel, String> {
    let table = PackedFrameTable::parse(bytes, TableForm::File)?;
    let layout = ChannelLayout {
        first_slot: 0,
        count: slot_count,
        slot_count,
        value_selector: 0,
    };
    decode_channel_range(&table, layout, frame_time, FRAME_INDEX_POLICY)
}

/// Decode the whole file-form channel at one explicit palette frame.
///
/// The file form concatenates every set lane's block for slots
/// `0..slot_count` in slot order, so this is the file-resident replay entry
/// point. `frame_index` is explicit; derive it from a clock with
/// [`select_frame_index`] when a frame time is what the caller holds.
pub fn decode_inline_channel(
    table: &PackedFrameTable<'_>,
    slot_count: usize,
    frame_index: usize,
) -> Result<DecodedChannel, String> {
    if slot_count == 0 {
        return Err("packed channel: slot count 0 has no bit-plane stride".to_string());
    }
    let header = table.header();
    let frame_count = usize::from(header.frame_count);
    if frame_index >= frame_count {
        return Err(format!(
            "packed channel: frame index {frame_index} is outside the {frame_count}-frame table"
        ));
    }
    let planes = table.bit_planes(slot_count)?;
    let offset = table.inline_value_stream_offset(slot_count)?;
    let mut stream = ValueStream::new(table.bytes(), offset, frame_count, frame_index)?;
    let mut slots = Vec::with_capacity(slot_count);
    for slot in 0..slot_count {
        let angles = decode_slot_angles(&planes, &mut stream, slot)?;
        slots.push(SlotAngles {
            angles,
            rotation: rotation_from_binary_angles(angles[0], angles[1], angles[2]),
        });
    }
    Ok(DecodedChannel {
        frame_index,
        flags: planes.flags(),
        slots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_COUNT: usize = 3;

    fn palette(values: &[f32]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0i32.to_le_bytes());
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    fn literal(value: f32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&1i32.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
        out
    }

    fn build_table(
        form: TableForm,
        frame_count: u16,
        divisor: i16,
        flags: u8,
        slot_count: usize,
        masks: &[u16],
        streams: &[Vec<u8>],
    ) -> Vec<u8> {
        assert!(masks.len() >= slot_count, "the mask table must cover every slot");
        let offsets_at = form.value_offsets_at();
        let width = form.pointer_width();
        let block_base = offsets_at + TABLE_COUNT * width;
        let stride_bits = ((3 * slot_count + 7) / 8) * 8;
        let mut bytes = vec![0u8; block_base + 1];
        bytes[TABLE_COUNT_AT] = TABLE_COUNT as u8;
        bytes[FRAME_COUNT_AT..FRAME_COUNT_AT + 2].copy_from_slice(&frame_count.to_le_bytes());
        bytes[DIVISOR_AT..DIVISOR_AT + 2].copy_from_slice(&divisor.to_le_bytes());
        bytes[block_base] = flags;
        let present = (flags & DEFINED_FLAGS).count_ones() as usize;
        bytes.resize(block_base + 1 + present * (stride_bits / 8), 0);
        for slot in 0..slot_count {
            let mask = masks[slot];
            let mut present_index = 0usize;
            for plane in 0..3usize {
                if flags & (1 << plane) == 0 {
                    continue;
                }
                let group = (mask >> (4 * plane)) & 0x7;
                let first_bit = block_base * 8 + 8 + 3 * slot + present_index * stride_bits;
                for lane in 0..3usize {
                    if group & (1 << lane) != 0 {
                        bytes[(first_bit + lane) / 8] |= 1 << ((first_bit + lane) % 8);
                    }
                }
                present_index += 1;
            }
        }
        let mut stream_start = (bytes.len() + 3) & !3;
        bytes.resize(stream_start, 0);
        for (selector, stream) in streams.iter().enumerate() {
            assert!(selector < TABLE_COUNT, "too many synthetic streams");
            let at = offsets_at + selector * width;
            match form {
                TableForm::File => {
                    bytes[at..at + 4].copy_from_slice(&(stream_start as u32).to_le_bytes());
                }
                TableForm::Memory => {
                    bytes[at..at + 8].copy_from_slice(&(stream_start as u64).to_le_bytes());
                }
            }
            bytes.extend_from_slice(stream);
            stream_start += stream.len();
        }
        bytes
    }

    fn matrix_close(actual: Mat3, expected: Mat3, tolerance: f32) {
        for row in 0..3 {
            for col in 0..3 {
                assert!(
                    (actual[row][col] - expected[row][col]).abs() <= tolerance,
                    "[{row}][{col}]: {} != {} (tolerance {tolerance})",
                    actual[row][col],
                    expected[row][col]
                );
            }
        }
    }

    #[test]
    fn header_parses_count_and_divisor_and_rejects_absurd_values() {
        let bytes = build_table(TableForm::File, 50, 100, 0x05, 3, &[0; 3], &[]);
        assert_eq!(
            parse_header(&bytes),
            Ok(FrameHeader {
                table_count: 3,
                frame_count: 50,
                divisor: 100,
            })
        );

        let zero_divisor = build_table(TableForm::File, 50, 0, 0x01, 1, &[0], &[]);
        assert!(parse_header(&zero_divisor).unwrap_err().contains("divisor"));

        let negative_divisor = build_table(TableForm::File, 50, -1, 0x01, 1, &[0], &[]);
        assert!(parse_header(&negative_divisor).unwrap_err().contains("divisor"));

        let no_frames = build_table(TableForm::File, 0, 100, 0x01, 1, &[0], &[]);
        assert!(parse_header(&no_frames).unwrap_err().contains("frame count"));

        let absurd = build_table(TableForm::File, MAX_FRAME_COUNT + 1, 100, 0x01, 1, &[0], &[]);
        assert!(parse_header(&absurd).unwrap_err().contains("frame count"));

        assert!(parse_header(&[]).unwrap_err().contains("table count"));
    }

    #[test]
    fn bit_plane_masks_extract_xyz_groups_for_a_three_slot_case() {
        let masks = [0x007u16, 0x700, 0x005];
        let bytes = build_table(TableForm::File, 4, 100, 0x05, 3, &masks, &[]);
        let table = PackedFrameTable::parse(&bytes, TableForm::File).expect("valid table");
        let planes = table.bit_planes(3).expect("valid planes");
        assert_eq!(planes.flags(), 0x05);
        assert!(planes.has_plane(0), "angle plane is present");
        assert!(!planes.has_plane(1), "scale plane is absent");
        assert!(planes.has_plane(2), "translation plane is present");
        for (slot, expected) in masks.iter().enumerate() {
            assert_eq!(planes.slot_mask(slot), Ok(*expected), "slot {slot}");
        }
        assert!(planes.slot_mask(3).is_err());
    }

    #[test]
    fn value_stream_consumes_palette_and_literal_blocks_in_ascending_lane_order() {
        let masks = [0x007u16, 0x100, 0x001];
        let stream: Vec<u8> = [
            palette(&[10.0, 20.0]),
            literal(30.0),
            palette(&[40.0, 50.0]),
            literal(60.0),
            literal(70.0),
        ]
        .concat();
        let bytes = build_table(TableForm::File, 2, 100, 0x05, 3, &masks, &[stream]);
        let decoded = decode_channel(&bytes, 3, 150).expect("valid channel");
        assert_eq!(decoded.frame_index, 1, "150 / 100 floors to frame 1");
        assert_eq!(decoded.slots[0].angles, [20.0, 30.0, 50.0]);
        assert_eq!(
            decoded.slots[1].angles,
            [0.0, 0.0, 0.0],
            "the translation lane consumes a block but writes no angle"
        );
        assert_eq!(decoded.slots[2].angles, [70.0, 0.0, 0.0]);
    }

    #[test]
    fn three_binary_angles_feed_rotation_from_binary_angles() {
        let masks = [0x007u16, 0x007];
        let stream: Vec<u8> = [
            literal(1024.0),
            literal(0.0),
            literal(0.0),
            literal(0.0),
            literal(0.0),
            literal(1024.0),
        ]
        .concat();
        let bytes = build_table(TableForm::File, 1, 100, 0x01, 2, &masks, &[stream]);
        let decoded = decode_channel(&bytes, 2, 0).expect("valid channel");
        assert_eq!(decoded.slots[0].angles, [1024.0, 0.0, 0.0]);
        assert_eq!(decoded.slots[1].angles, [0.0, 0.0, 1024.0]);
        assert_eq!(
            decoded.slots[0].rotation,
            rotation_from_binary_angles(1024.0, 0.0, 0.0)
        );
        matrix_close(
            decoded.slots[0].rotation,
            [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
            1e-6,
        );
        matrix_close(
            decoded.slots[1].rotation,
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            1e-6,
        );
    }

    #[test]
    fn channel_layout_decodes_only_its_slot_window() {
        let masks = [0x007u16, 0x001, 0x001];
        let stream: Vec<u8> = [literal(11.0), literal(22.0)].concat();
        let bytes = build_table(TableForm::File, 1, 100, 0x01, 3, &masks, &[stream]);
        let table = PackedFrameTable::parse(&bytes, TableForm::File).expect("valid table");
        let layout = ChannelLayout {
            first_slot: 1,
            count: 2,
            slot_count: 3,
            value_selector: 0,
        };
        let decoded =
            decode_channel_range(&table, layout, 0, FRAME_INDEX_POLICY).expect("valid window");
        assert_eq!(decoded.slots.len(), 2);
        assert_eq!(decoded.slots[0].angles, [11.0, 0.0, 0.0]);
        assert_eq!(decoded.slots[1].angles, [22.0, 0.0, 0.0]);
    }

    #[test]
    fn frame_index_policy_saturates_past_the_last_frame_and_is_a_named_choice() {
        assert_eq!(
            FRAME_INDEX_POLICY,
            FrameIndexPolicy::Saturate,
            "the default is the documented named choice for the UNKNOWN wrap"
        );
        assert_eq!(
            select_frame_index(2400, 100, 50, FrameIndexPolicy::Saturate),
            Ok(24)
        );
        assert_eq!(
            select_frame_index(4900, 100, 50, FrameIndexPolicy::Saturate),
            Ok(49)
        );
        assert_eq!(
            select_frame_index(5000, 100, 50, FrameIndexPolicy::Saturate),
            Ok(49),
            "saturate clamps to the last frame"
        );
        assert_eq!(
            select_frame_index(6000, 100, 50, FrameIndexPolicy::Wrap),
            Ok(10)
        );
        assert_eq!(
            select_frame_index(5000, 100, 50, FrameIndexPolicy::Wrap),
            Ok(0)
        );
        assert!(select_frame_index(100, 0, 50, FRAME_INDEX_POLICY)
            .unwrap_err()
            .contains("divisor"));
        assert!(select_frame_index(100, -100, 50, FRAME_INDEX_POLICY)
            .unwrap_err()
            .contains("divisor"));
        assert!(select_frame_index(100, 100, 0, FRAME_INDEX_POLICY)
            .unwrap_err()
            .contains("frame count"));
    }

    #[test]
    fn decode_channel_saturates_frame_time_past_the_last_frame() {
        let stream = palette(&[10.0, 20.0]);
        let bytes = build_table(TableForm::File, 2, 100, 0x01, 1, &[0x001], &[stream]);
        let decoded = decode_channel(&bytes, 1, 250).expect("valid channel");
        assert_eq!(decoded.frame_index, 1);
        assert_eq!(
            decoded.slots[0].angles[0], 20.0,
            "the last palette entry is used past the end"
        );
    }

    #[test]
    fn malformed_inputs_are_actionable_errors_not_panics() {
        let mut unknown = 2i32.to_le_bytes().to_vec();
        unknown.extend_from_slice(&0.0f32.to_le_bytes());
        let bytes = build_table(TableForm::File, 1, 100, 0x01, 1, &[0x001], &[unknown]);
        let err = decode_channel(&bytes, 1, 0).unwrap_err();
        assert!(err.contains("command 2"), "{err}");

        let short = palette(&[1.0]);
        let bytes = build_table(TableForm::File, 3, 100, 0x01, 1, &[0x001], &[short]);
        let err = decode_channel(&bytes, 1, 200).unwrap_err();
        assert!(err.contains("palette"), "{err}");

        let bytes = build_table(TableForm::File, 1, 100, 0x80, 1, &[0], &[]);
        let table = PackedFrameTable::parse(&bytes, TableForm::File).expect("header is fine");
        assert!(table.bit_planes(1).unwrap_err().contains("flags"));

        let bytes = build_table(TableForm::File, 1, 100, 0x01, 1, &[0x001], &[literal(0.0)]);
        let table = PackedFrameTable::parse(&bytes, TableForm::File).expect("header is fine");
        assert!(table.value_offset(3).is_err());
        assert!(table.bit_planes(0).is_err());
    }

    #[test]
    fn inline_stream_follows_the_present_planes_aligned_to_four_bytes() {
        let masks = [0x003u16, 0x001, 0x000];
        let stream: Vec<u8> = [
            palette(&[10.0, 20.0]),
            palette(&[30.0, 40.0]),
            literal(50.0),
        ]
        .concat();
        let bytes = build_table(TableForm::File, 2, 100, 0x01, 3, &masks, &[stream]);
        let table = PackedFrameTable::parse(&bytes, TableForm::File).expect("valid table");
        assert_eq!(
            table.inline_value_stream_offset(3),
            Ok(0x24),
            "flags + one 16-bit plane (2 bytes) ends at 0x23, aligned to 0x24"
        );
        let decoded = decode_inline_channel(&table, 3, 1).expect("inline frame 1");
        assert_eq!(decoded.frame_index, 1);
        assert_eq!(decoded.slots[0].angles, [20.0, 40.0, 0.0]);
        assert_eq!(decoded.slots[1].angles, [50.0, 0.0, 0.0]);
        assert_eq!(decoded.slots[2].angles, [0.0, 0.0, 0.0]);
        assert!(decode_inline_channel(&table, 3, 2).is_err());
    }

    #[test]
    fn memory_form_reads_64_bit_value_pointers() {
        let bytes = build_table(
            TableForm::Memory,
            1,
            100,
            0x01,
            1,
            &[0x001],
            &[literal(2048.0)],
        );
        let table = PackedFrameTable::parse(&bytes, TableForm::Memory).expect("valid memory table");
        assert_eq!(table.bit_plane_base(), 0x30, "memory form shifts by 0x10");
        let layout = ChannelLayout {
            first_slot: 0,
            count: 1,
            slot_count: 1,
            value_selector: 0,
        };
        let decoded = decode_channel_range(&table, layout, 0, FRAME_INDEX_POLICY)
            .expect("valid memory channel");
        assert_eq!(decoded.slots[0].angles, [2048.0, 0.0, 0.0]);
    }
}

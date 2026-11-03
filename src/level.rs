//! Untrusted SR1-remaster room containers: the `.PCM` header, name table, and
//! the room-geometry summary. Read-only: no placement, transform, or audio
//! decode.

use std::fmt;
use std::path::{Path, PathBuf};

pub const MAX_RECORDS: u32 = 1 << 20;
pub const MAX_SEGMENTS: u16 = 1 << 10;
pub const MAX_NAME_LEN: usize = 20;

#[derive(Debug)]
pub struct LevelUnit {
    pub name: String,
    pub records: u32,
    pub names: Vec<String>,
    pub links: Vec<Link>,
    pub sidecar: Sidecar,
    /// One entry per offset-table record, in table order.
    pub block_sizes: Vec<u32>,
    /// Parallel to `block_sizes`: whether every byte of that block is zero.
    pub block_all_zero: Vec<bool>,
    /// Header sanity, relocation-directory integrity, and header offset slots.
    pub structure: LevelStructure,
    /// Records decoded from the header `0x34` link table, in table order.
    pub link_table: Vec<TableLink>,
}

/// Shape class of one block. Labels describe a size (and zero-ness) only;
/// no block carries a type tag, so no semantic name is claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockClass {
    Zero,
    Small4,
    Fixed40,
    Fixed48,
    Other(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub struct BlockCensus {
    pub total: u32,
    pub zero: u32,
    pub small4: u32,
    pub fixed40: u32,
    pub fixed48: u32,
    /// `(size, count)` for every other size, ascending by size.
    pub other: Vec<(u32, u32)>,
    /// `(ordinal, size)`; ties go to the lowest ordinal.
    pub largest: (u32, u32),
}

/// Header dword slots the builder walks as base-relative offsets. The note
/// enumerates these individually and leaves `0x1B..=0x32` unenumerated, so
/// that range is deliberately not claimed here.
pub const SCHEMA_OFFSET_SLOTS: [usize; 14] =
    [0x00, 0x01, 0x03, 0x0D, 0x15, 0x17, 0x23, 0x26, 0x34, 0x35, 0x37, 0x39, 0x3B, 0x3D];

/// 36-byte source records: count at header slot `0x3A`, offset at `0x3B`.
const TABLE36_STRIDE: usize = 0x24;
const SLOT_TABLE36_COUNT: usize = 0x3A;
const SLOT_TABLE36_OFFSET: usize = 0x3B;

/// Decoded link table: header slot `0x34` holds a data-base-relative offset;
/// records are 40 bytes, `[5 x u32][12-byte name][2 x u32]`. The table ends at
/// the first record whose name is not `<target>,<digits>`.
const SLOT_LINK_TABLE: usize = 0x34;
const LINK_RECORD_STRIDE: usize = 40;
const LINK_NAME_LEN: usize = 12;

/// Directory integrity: entry `i` is the position of a dword that holds a
/// non-zero data-base-relative offset.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelocationReport {
    pub entries: u32,
    pub valid: u32,
    pub zero: u32,
    pub out_of_range: u32,
}

/// The 36-byte source table: count, stride, and one blob length or null per
/// record. Field meaning stays UNKNOWN; only the tagged-blob length applies.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Table36Report {
    pub count: u32,
    pub stride: u32,
    pub lengths: Vec<Option<u32>>,
}

/// Structural facts about one unit. `slots` holds every non-null enumerated
/// header offset as `(slot, data_base + value)`. No record type is claimed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LevelStructure {
    pub data_base: usize,
    pub data_len: usize,
    pub reloc: RelocationReport,
    pub slots: Vec<(usize, usize)>,
    pub table36: Table36Report,
}

/// Room geometry behind header slot `0x00`: counts, data-base-relative array
/// offsets, the `i16` position box, and face-index validation. The trailing
/// vertex triple is not read; no normal or UV is claimed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GeometryReport {
    pub vertices: u32,
    pub faces: u32,
    pub room_offset: u32,
    pub vertex_offset: u32,
    pub face_offset: u32,
    pub min: [i16; 3],
    pub max: [i16; 3],
    /// Face indices that do not address a vertex; zero on every valid unit.
    pub out_of_range: u32,
    pub max_index: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub index: u32,
}

/// One 40-byte link-table record. Every prelude value is reported as a raw
/// signed 32-bit integer. `target_id` repeats prelude field 4, the value that
/// stays stable per target across units; no other field is interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLink {
    pub target: String,
    pub index: u32,
    pub target_id: i32,
    pub prelude: [i32; 5],
}

#[derive(Debug, PartialEq, Eq)]
pub enum Sidecar {
    Absent,
    Present { segments: u16, rate_field: u32, pnf_entries: u32 },
}

#[derive(Debug, PartialEq, Eq)]
pub struct PmfHeader { pub id: u16, pub segments: u16, pub rate_field: u32 }

#[derive(Debug, PartialEq, Eq)]
pub struct PnfHeader { pub id: u16, pub entries: u32 }

#[derive(Debug, PartialEq, Eq)]
pub enum LevelError {
    Usage(&'static str),
    RefusedPath,
    Io,
    NoUnits,
    CountCeiling,
    TableNotAscending,
    TablePastEof,
    PaddingTruncated,
    RelocationOutOfRange,
    GeometryRecordOutOfRange,
    GeometryArrayOutOfRange,
    MissingUnitName,
    SidecarIncomplete,
    BadSidecarMagic,
    SidecarMismatch,
}

impl LevelError {
    pub fn exit_code(&self) -> u8 {
        match self {
            LevelError::Usage(_) => 1,
            LevelError::RefusedPath => 3,
            _ => 2,
        }
    }
}

impl fmt::Display for LevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            LevelError::Usage(message) => message,
            LevelError::RefusedPath => "not a .PCM file and not a directory",
            LevelError::Io => "cannot read the requested path",
            LevelError::NoUnits => "no .PCM units in the directory",
            LevelError::CountCeiling => "record count out of range",
            LevelError::TableNotAscending => "offset table must start 0, 4 and strictly ascend",
            LevelError::TablePastEof => "offset table or last record runs past the file",
            LevelError::PaddingTruncated => {
                "offset table padding is missing: file ends before the level-data base"
            }
            LevelError::RelocationOutOfRange => {
                "relocation directory entry points outside the level data"
            }
            LevelError::GeometryRecordOutOfRange => {
                "room geometry record points outside the level data"
            }
            LevelError::GeometryArrayOutOfRange => {
                "room geometry vertex or face array runs outside the level data"
            }
            LevelError::MissingUnitName => "name table does not contain the unit's own name",
            LevelError::SidecarIncomplete => "PMF and PNF must be present together",
            LevelError::BadSidecarMagic => "sidecar magic or header length is invalid",
            LevelError::SidecarMismatch => "PMF/PNF id, count, or length disagreement",
        };
        f.write_str(message)
    }
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn read_i16(bytes: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

/// Header slot `0x00` points at the room/zone record: `+0x10`/`+0x1C` give a
/// 12-byte vertex array, `+0x14`/`+0x20` a 12-byte face array whose first
/// three `u16` are vertex indices.
const SLOT_ROOM_RECORD: usize = 0x00;
const ROOM_VERTEX_COUNT: usize = 0x10;
const ROOM_FACE_COUNT: usize = 0x14;
const ROOM_VERTEX_OFFSET: usize = 0x1C;
const ROOM_FACE_OFFSET: usize = 0x20;
const VERTEX_STRIDE: usize = 12;
const FACE_STRIDE: usize = 12;

/// Data-region range of `count` records at a data-base-relative `offset`.
/// A zero count is always valid and reads nothing; a non-zero array that would
/// leave the region yields `None` so the caller can fail loud, never clamp.
fn array_range(len: usize, offset: u32, count: u32, stride: usize) -> Option<(usize, usize)> {
    if count == 0 {
        return Some((offset as usize, offset as usize));
    }
    let start = offset as usize;
    let end = start.checked_add((count as usize).checked_mul(stride)?)?;
    (end <= len).then_some((start, end))
}

/// Read the room record behind header slot `0x00`. A zero slot means no
/// geometry; a record or non-empty array outside the data region is a distinct
/// error; face indices are validated but a bad one is reported, not fatal.
fn geometry_report(data: &[u8]) -> Result<GeometryReport, LevelError> {
    let room_offset = read_u32(data, 4 * SLOT_ROOM_RECORD)
        .ok_or(LevelError::GeometryRecordOutOfRange)?;
    let mut report = GeometryReport { room_offset, ..GeometryReport::default() };
    if room_offset == 0 {
        return Ok(report);
    }
    let record = room_offset as usize;
    if record.checked_add(ROOM_FACE_OFFSET + 4).is_none_or(|end| end > data.len()) {
        return Err(LevelError::GeometryRecordOutOfRange);
    }
    let vertices = read_u32(data, record + ROOM_VERTEX_COUNT).unwrap_or(0);
    let faces = read_u32(data, record + ROOM_FACE_COUNT).unwrap_or(0);
    let vertex_offset = read_u32(data, record + ROOM_VERTEX_OFFSET).unwrap_or(0);
    let face_offset = read_u32(data, record + ROOM_FACE_OFFSET).unwrap_or(0);
    let (vertex_start, vertex_end) = array_range(data.len(), vertex_offset, vertices, VERTEX_STRIDE)
        .ok_or(LevelError::GeometryArrayOutOfRange)?;
    let (face_start, face_end) =
        array_range(data.len(), face_offset, faces, FACE_STRIDE)
            .ok_or(LevelError::GeometryArrayOutOfRange)?;
    report.vertices = vertices;
    report.faces = faces;
    report.vertex_offset = vertex_offset;
    report.face_offset = face_offset;
    if vertices > 0 {
        let mut minimum = [i16::MAX; 3];
        let mut maximum = [i16::MIN; 3];
        for at in (vertex_start..vertex_end).step_by(VERTEX_STRIDE) {
            for axis in 0..3 {
                let value =
                    read_i16(data, at + 2 * axis).ok_or(LevelError::GeometryArrayOutOfRange)?;
                minimum[axis] = minimum[axis].min(value);
                maximum[axis] = maximum[axis].max(value);
            }
        }
        report.min = minimum;
        report.max = maximum;
    }
    if faces > 0 {
        let mut max_index = 0u16;
        for at in (face_start..face_end).step_by(FACE_STRIDE) {
            for slot in 0..3 {
                let index =
                    read_u16(data, at + 2 * slot).ok_or(LevelError::GeometryArrayOutOfRange)?;
                max_index = max_index.max(index);
                if u32::from(index) >= vertices {
                    report.out_of_range += 1;
                }
            }
        }
        report.max_index = Some(max_index);
    }
    Ok(report)
}

/// Geometry rows for one file or room directory, sorted by unit name. This
/// keeps its own read pass so a malformed room record cannot break the other
/// level arms.
pub fn read_geometry_path(path: &Path) -> Result<Vec<(String, GeometryReport)>, LevelError> {
    let paths = if path.is_dir() {
        let bin = path.join("BIN");
        let directory = if bin.is_dir() { bin } else { path.to_path_buf() };
        let index = SidecarIndex::read(&directory)?;
        if index.pcm_paths.is_empty() {
            return Err(LevelError::NoUnits);
        }
        index.pcm_paths
    } else if is_pcm(path) {
        vec![path.to_path_buf()]
    } else {
        return Err(LevelError::RefusedPath);
    };
    let mut rows = Vec::with_capacity(paths.len());
    for pcm_path in paths {
        let stem = pcm_path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).ok_or(LevelError::Io)?;
        let bytes = std::fs::read(&pcm_path).map_err(|_| LevelError::Io)?;
        let unit = parse_pcm(&bytes, &stem)?;
        rows.push((unit.name, geometry_report(&bytes[unit.structure.data_base..])?));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(rows)
}

/// Word count the offset table is padded to: 512 words, i.e. a 2048-byte
/// level-data base.
pub(crate) const OFFSET_ALIGN: usize = 512;

/// Level-data base: the `4 + 4N` table padded up to a 2048-byte boundary,
/// i.e. `4 * roundup512(N)`. Equal to that form for every `N < 512`; the
/// count word is included so the base can never land inside the table when
/// `N` is an exact multiple of 512. Saturates instead of panicking.
pub(crate) fn data_base(records: u32) -> usize {
    let words = (records as usize).saturating_add(1);
    let rounded = words.saturating_add(OFFSET_ALIGN - 1) & !(OFFSET_ALIGN - 1);
    rounded.saturating_mul(4)
}

/// Classify every directory entry by the dword it points at, relative to the
/// data base: a non-zero offset inside the data region is valid.
fn relocation_report(data: &[u8], offsets: &[u32]) -> RelocationReport {
    let mut report = RelocationReport { entries: offsets.len() as u32, ..RelocationReport::default() };
    for &entry in offsets {
        match data.get(entry as usize..).and_then(|rest| read_u32(rest, 0)) {
            Some(0) => report.zero += 1,
            Some(value)
                if (value as usize).checked_add(4).is_some_and(|end| end <= data.len()) =>
            {
                report.valid += 1;
            }
            _ => report.out_of_range += 1,
        }
    }
    report
}

/// Read the enumerated offset slots: null when zero or past the header.
fn schema_slots(data: &[u8], data_base: usize) -> Vec<(usize, usize)> {
    SCHEMA_OFFSET_SLOTS
        .iter()
        .filter_map(|&slot| {
            let value = read_u32(data, 4 * slot)?;
            (value != 0).then_some((slot, data_base + value as usize))
        })
        .collect()
}

/// Length of the tagged blob at `at`, or `None` when the magic is absent, a
/// type byte is unknown, or the walk leaves the data region.
fn tagged_blob_len(data: &[u8], at: usize) -> Option<u32> {
    let blob = data.get(at..)?;
    if blob.first() != Some(&0xBE) || blob.get(1) != Some(&0xEF) {
        return None;
    }
    let count = *blob.get(2)?;
    let mut cursor = 4usize;
    for _ in 0..count {
        let kind = *blob.get(cursor)?;
        let length = usize::from(*blob.get(cursor.checked_add(1)?)?);
        let entry = match kind {
            0 => 0x12,
            1..=4 => 0x0E,
            _ => return None,
        };
        cursor = cursor.checked_add(entry + 2 * length)?;
    }
    u32::try_from(cursor).ok()
}

/// Walk the 36-byte table: one tagged-blob length or null per record. The
/// count is capped so an untrusted header cannot force unbounded work.
fn table36_report(data: &[u8]) -> Table36Report {
    let count = read_u32(data, 4 * SLOT_TABLE36_COUNT).unwrap_or(0).min(MAX_RECORDS);
    let offset = read_u32(data, 4 * SLOT_TABLE36_OFFSET).unwrap_or(0) as usize;
    let mut lengths = Vec::new();
    for index in 0..count as usize {
        let length = if offset == 0 {
            None
        } else {
            let at = offset + index * TABLE36_STRIDE;
            read_u32(data, at)
                .filter(|&relative| relative != 0)
                .and_then(|relative| tagged_blob_len(data, relative as usize))
        };
        lengths.push(length);
    }
    Table36Report { count, stride: TABLE36_STRIDE as u32, lengths }
}

/// Walk the link table from header slot `0x34`. A zero offset means no table;
/// a record that would leave the data region ends the walk without an error.
fn link_table(data: &[u8]) -> Vec<TableLink> {
    let Some(relative) = read_u32(data, 4 * SLOT_LINK_TABLE) else {
        return Vec::new();
    };
    if relative == 0 {
        return Vec::new();
    }
    let mut links = Vec::new();
    let mut at = relative as usize;
    loop {
        let Some(end) = at.checked_add(LINK_RECORD_STRIDE) else {
            break;
        };
        let Some(record) = data.get(at..end) else {
            break;
        };
        let Some((target, index)) = decode_link_name(&record[20..20 + LINK_NAME_LEN]) else {
            break;
        };
        let mut prelude = [0i32; 5];
        for (slot, value) in prelude.iter_mut().enumerate() {
            *value = read_u32(record, 4 * slot).unwrap_or(0) as i32;
        }
        links.push(TableLink { target, index, target_id: prelude[4], prelude });
        at = end;
    }
    links
}

/// `^[a-z][a-z0-9]*,[0-9]+$` inside one NUL-terminated 12-byte field.
fn decode_link_name(field: &[u8]) -> Option<(String, u32)> {
    let text = std::str::from_utf8(&field[..field.iter().position(|&byte| byte == 0)?]).ok()?;
    let (target, digits) = text.split_once(',')?;
    let (first, rest) = target.as_bytes().split_first()?;
    if !first.is_ascii_lowercase()
        || !rest.iter().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((target.to_string(), digits.parse().ok()?))
}

/// `u32 N` + `N` ascending relative offsets + zero padding up to
/// `4 * roundup512(N)`, which is the level-data base, then the level data and
/// the name table. Blocks are addressed relative to that padded base.
pub fn parse_pcm(bytes: &[u8], expected_name: &str) -> Result<LevelUnit, LevelError> {
    let records = read_u32(bytes, 0).ok_or(LevelError::TablePastEof)?;
    if !(2..=MAX_RECORDS).contains(&records) {
        return Err(LevelError::CountCeiling);
    }
    let table_end = 4usize
        .checked_add(4 * records as usize)
        .ok_or(LevelError::TablePastEof)?;
    if table_end > bytes.len() {
        return Err(LevelError::TablePastEof);
    }
    let data_base = data_base(records);
    if data_base > bytes.len() {
        return Err(LevelError::PaddingTruncated);
    }
    let mut last = 0u32;
    let mut offsets = Vec::with_capacity(records as usize);
    for index in 0..records as usize {
        let offset = read_u32(bytes, 4 + 4 * index).ok_or(LevelError::TablePastEof)?;
        let bad_prefix = index < 2 && offset != 4 * index as u32;
        if bad_prefix || (index > 0 && offset <= last) {
            return Err(LevelError::TableNotAscending);
        }
        last = offset;
        offsets.push(offset);
    }
    if data_base.saturating_add(last as usize) > bytes.len() {
        return Err(LevelError::TablePastEof);
    }
    let mut block_sizes = Vec::with_capacity(records as usize);
    let mut block_all_zero = Vec::with_capacity(records as usize);
    for (index, offset) in offsets.iter().enumerate() {
        let start = data_base + *offset as usize;
        let end = match offsets.get(index + 1) {
            Some(next) => data_base + *next as usize,
            None => bytes.len(),
        };
        let block = &bytes[start..end];
        block_sizes.push((end - start) as u32);
        block_all_zero.push(block.iter().all(|&byte| byte == 0));
    }
    let data = &bytes[data_base..];
    let table = link_table(data);
    let structure = LevelStructure {
        data_base,
        data_len: data.len(),
        reloc: relocation_report(data, &offsets),
        slots: schema_slots(data, data_base),
        table36: table36_report(data),
    };
    let names = extract_names(bytes);
    let name = names
        .iter()
        .find(|name| name.eq_ignore_ascii_case(expected_name))
        .ok_or(LevelError::MissingUnitName)?
        .clone();
    let links = table
        .iter()
        .map(|link| Link { target: link.target.clone(), index: link.index })
        .collect();
    Ok(LevelUnit {
        name,
        records,
        names,
        links,
        sidecar: Sidecar::Absent,
        block_sizes,
        block_all_zero,
        structure,
        link_table: table,
    })
}

/// Shape-only classifier: all-zero wins, then the three frequent sizes,
/// then the block's own size.
pub fn classify(size: u32, all_zero: bool) -> BlockClass {
    if all_zero {
        return BlockClass::Zero;
    }
    match size {
        4 => BlockClass::Small4,
        40 => BlockClass::Fixed40,
        48 => BlockClass::Fixed48,
        other => BlockClass::Other(other),
    }
}

/// Counts every block exactly once; the class counts sum to `records`.
pub fn census(unit: &LevelUnit) -> BlockCensus {
    debug_assert_eq!(unit.block_sizes.len(), unit.block_all_zero.len());
    let mut counts = BlockCensus {
        total: unit.records,
        zero: 0,
        small4: 0,
        fixed40: 0,
        fixed48: 0,
        other: Vec::new(),
        largest: (0, 0),
    };
    for (ordinal, &size) in unit.block_sizes.iter().enumerate() {
        match classify(size, unit.block_all_zero[ordinal]) {
            BlockClass::Zero => counts.zero += 1,
            BlockClass::Small4 => counts.small4 += 1,
            BlockClass::Fixed40 => counts.fixed40 += 1,
            BlockClass::Fixed48 => counts.fixed48 += 1,
            BlockClass::Other(size) => {
                match counts.other.iter_mut().find(|(known, _)| *known == size) {
                    Some((_, count)) => *count += 1,
                    None => counts.other.push((size, 1)),
                }
            }
        }
        if size > counts.largest.1 {
            counts.largest = (ordinal as u32, size);
        }
    }
    counts.other.sort_unstable_by_key(|(size, _)| *size);
    counts
}

/// One `blocks` header line, one line per present shape class in fixed
/// order, and one `largest` line. Counts and sizes only: no payload, file
/// offset, coordinate, or transform.
pub fn record_lines(unit: &LevelUnit) -> Vec<String> {
    let counts = census(unit);
    let mut lines = vec![format!("blocks {} total={}", unit.name, counts.total)];
    for (label, count) in [
        ("zero", counts.zero),
        ("small4", counts.small4),
        ("fixed40", counts.fixed40),
        ("fixed48", counts.fixed48),
    ] {
        if count > 0 {
            lines.push(format!("  shape {label} {count}"));
        }
    }
    lines.extend(
        counts
            .other
            .iter()
            .map(|(size, count)| format!("  shape other:{size} {count}")),
    );
    lines.push(format!("largest {} {}", counts.largest.0, counts.largest.1));
    lines
}

/// Structural report: header sanity, relocation integrity, header offset
/// slots. Fails loud when any directory entry points outside the level data.
pub fn structure_lines(unit: &LevelUnit) -> Result<Vec<String>, LevelError> {
    let structure = &unit.structure;
    if structure.reloc.out_of_range > 0 {
        return Err(LevelError::RelocationOutOfRange);
    }
    let reloc = &structure.reloc;
    let at: String = structure
        .slots
        .iter()
        .map(|(slot, position)| format!(" 0x{slot:02X}:{position}"))
        .collect();
    let list: Vec<String> = structure
        .table36
        .lengths
        .iter()
        .map(|length| length.map_or_else(|| "null".to_string(), |bytes| bytes.to_string()))
        .collect();
    let lengths = if list.is_empty() { "none".to_string() } else { list.join(",") };
    Ok(vec![
        format!(
            "level {} pcm_records={} data_base={} data_len={}",
            unit.name, unit.records, structure.data_base, structure.data_len
        ),
        format!(
            "  reloc entries={} valid={} zero={} out_of_range={}",
            reloc.entries, reloc.valid, reloc.zero, reloc.out_of_range
        ),
        format!(
            "  schema slots={} non_null={}{}",
            SCHEMA_OFFSET_SLOTS.len(),
            structure.slots.len(),
            at
        ),
        format!(
            "  table36 count={} stride={} lengths={lengths}",
            structure.table36.count, structure.table36.stride
        ),
    ])
}

/// Per-unit geometry summary: counts, offsets, the `i16` position box and its
/// centre, and face-index validation. No trailing vertex triple or face
/// attribute word is read.
pub fn geometry_lines(name: &str, g: &GeometryReport) -> Vec<String> {
    let box_line = if g.vertices > 0 {
        let [nx, ny, nz] = g.min;
        let [xx, xy, xz] = g.max;
        let center = format!(
            "{:.1},{:.1},{:.1}",
            (f64::from(nx) + f64::from(xx)) / 2.0,
            (f64::from(ny) + f64::from(xy)) / 2.0,
            (f64::from(nz) + f64::from(xz)) / 2.0
        );
        format!("  bbox min={nx},{ny},{nz} max={xx},{xy},{xz} center={center}")
    } else {
        "  bbox none".to_string()
    };
    let max_index =
        g.max_index.map_or_else(|| "none".to_string(), |index| index.to_string());
    vec![
        format!(
            "geometry {} vertices={} faces={} room={} vertex_at={} face_at={}",
            name, g.vertices, g.faces, g.room_offset, g.vertex_offset, g.face_offset
        ),
        box_line,
        format!("  indices out_of_range={} max_index={max_index}", g.out_of_range),
    ]
}

/// `FMPa` magic, 16-bit id, segment count, and the rate field at `0x28`.
pub fn parse_pmf(bytes: &[u8]) -> Result<PmfHeader, LevelError> {
    if bytes.len() < 0x2C || bytes.get(0..4) != Some(b"FMPa".as_slice()) {
        return Err(LevelError::BadSidecarMagic);
    }
    let id = read_u16(bytes, 0x06).ok_or(LevelError::BadSidecarMagic)?;
    let segments = read_u16(bytes, 0x08).ok_or(LevelError::BadSidecarMagic)?;
    if segments == 0 || segments > MAX_SEGMENTS {
        return Err(LevelError::CountCeiling);
    }
    let rate_field = read_u32(bytes, 0x28).ok_or(LevelError::BadSidecarMagic)?;
    Ok(PmfHeader { id, segments, rate_field })
}

/// `FNPa` magic, 20-byte header + one `u16` per segment: `len == 20 + 2n`.
pub fn parse_pnf(bytes: &[u8]) -> Result<PnfHeader, LevelError> {
    if bytes.len() < 20 || bytes.get(0..4) != Some(b"FNPa".as_slice()) {
        return Err(LevelError::BadSidecarMagic);
    }
    let id = read_u16(bytes, 0x06).ok_or(LevelError::BadSidecarMagic)?;
    let entries = u32::from(read_u16(bytes, 0x0A).ok_or(LevelError::BadSidecarMagic)?);
    if bytes.len() != 20 + 2 * entries as usize {
        return Err(LevelError::SidecarMismatch);
    }
    Ok(PnfHeader { id, entries })
}

pub fn read_path(path: &Path) -> Result<Vec<LevelUnit>, LevelError> {
    if path.is_dir() {
        let bin = path.join("BIN");
        let directory = if bin.is_dir() { bin } else { path.to_path_buf() };
        let index = SidecarIndex::read(&directory)?;
        if index.pcm_paths.is_empty() {
            return Err(LevelError::NoUnits);
        }
        return load_units(&index.pcm_paths, &index);
    }
    if !is_pcm(path) {
        return Err(LevelError::RefusedPath);
    }
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let index = SidecarIndex::read(directory)?;
    load_units(&[path.to_path_buf()], &index)
}

pub fn report_lines(unit: &LevelUnit) -> Vec<String> {
    let (pmf, segments, rate, pnf) = match unit.sidecar {
        Sidecar::Absent => ("absent", 0, 0, 0),
        Sidecar::Present { segments, rate_field, pnf_entries } => {
            ("present", u32::from(segments), rate_field, pnf_entries)
        }
    };
    let mut lines = vec![format!(
        "level {} pcm_records={} names={} links={} pmf={pmf} segments={segments} rate_field={rate} pnf={pnf}",
        unit.name,
        unit.records,
        unit.names.len(),
        unit.links.len()
    )];
    lines.extend(unit.links.iter().map(|link| format!("  link {} {}", link.target, link.index)));
    lines
}

/// One `links` line per unit, then one line per decoded link-table record.
/// Prelude values are raw integers and are still printed in full; `target_id`
/// repeats field 4 so the stable target identity is visible without indexing.
pub fn link_lines(unit: &LevelUnit) -> Vec<String> {
    let mut lines = vec![format!("links {} count={}", unit.name, unit.link_table.len())];
    lines.extend(unit.link_table.iter().map(|link| {
        let [a, b, c, d, e] = link.prelude;
        format!(
            "  link {} {} target_id={} prelude={a},{b},{c},{d},{e}",
            link.target, link.index, link.target_id
        )
    }));
    lines
}

fn load_units(pcm_paths: &[PathBuf], index: &SidecarIndex) -> Result<Vec<LevelUnit>, LevelError> {
    let mut units = Vec::with_capacity(pcm_paths.len());
    for pcm_path in pcm_paths {
        units.push(load_unit(pcm_path, index)?);
    }
    units.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(units)
}

fn load_unit(pcm_path: &Path, index: &SidecarIndex) -> Result<LevelUnit, LevelError> {
    let stem = pcm_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .ok_or(LevelError::Io)?;
    let bytes = std::fs::read(pcm_path).map_err(|_| LevelError::Io)?;
    let mut unit = parse_pcm(&bytes, &stem)?;
    unit.sidecar = index.sidecar(&stem)?;
    Ok(unit)
}

fn is_pcm(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pcm"))
}

struct SidecarIndex {
    pcm_paths: Vec<PathBuf>,
    files: Vec<(String, PathBuf)>,
}

impl SidecarIndex {
    fn read(directory: &Path) -> Result<Self, LevelError> {
        let mut pcm_paths = Vec::new();
        let mut files = Vec::new();
        for entry in std::fs::read_dir(directory).map_err(|_| LevelError::Io)? {
            let path = entry.map_err(|_| LevelError::Io)?.path();
            if !path.is_file() {
                continue;
            }
            if is_pcm(&path) {
                pcm_paths.push(path.clone());
            }
            if let Some(name) = path.file_name() {
                files.push((name.to_string_lossy().to_ascii_lowercase(), path));
            }
        }
        pcm_paths.sort();
        Ok(Self { pcm_paths, files })
    }

    fn find(&self, stem: &str, extension: &str) -> Option<&Path> {
        let wanted = format!("{}.{extension}", stem.to_ascii_lowercase());
        self.files.iter().find(|(name, _)| *name == wanted).map(|(_, path)| path.as_path())
    }

    fn sidecar(&self, stem: &str) -> Result<Sidecar, LevelError> {
        let (pmf_path, pnf_path) = match (self.find(stem, "pmf"), self.find(stem, "pnf")) {
            (None, None) => return Ok(Sidecar::Absent),
            (Some(pmf_path), Some(pnf_path)) => (pmf_path, pnf_path),
            _ => return Err(LevelError::SidecarIncomplete),
        };
        let pmf = parse_pmf(&std::fs::read(pmf_path).map_err(|_| LevelError::Io)?)?;
        let pnf = parse_pnf(&std::fs::read(pnf_path).map_err(|_| LevelError::Io)?)?;
        if pmf.id != pnf.id || u32::from(pmf.segments) != pnf.entries {
            return Err(LevelError::SidecarMismatch);
        }
        Ok(Sidecar::Present {
            segments: pmf.segments,
            rate_field: pmf.rate_field,
            pnf_entries: pnf.entries,
        })
    }
}

fn extract_names(bytes: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        while index < bytes.len() && (0x20..=0x7E).contains(&bytes[index]) {
            index += 1;
        }
        if let Some(name) = accepted_name(&bytes[start..index]) {
            names.push(name.to_string());
        }
        index += 1;
    }
    names
}

/// `^[a-z][a-z0-9]*(,[0-9]{1,2})?$` within 3..=20 bytes.
fn accepted_name(run: &[u8]) -> Option<&str> {
    if !(3..=MAX_NAME_LEN).contains(&run.len()) {
        return None;
    }
    let text = std::str::from_utf8(run).ok()?;
    let (target, digits) = text.split_once(',').map_or((text, None), |(t, d)| (t, Some(d)));
    let (first, rest) = target.as_bytes().split_first()?;
    if !first.is_ascii_lowercase() || !rest.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return None;
    }
    if let Some(digits) = digits {
        if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm_bytes(offsets: &[u32], body: &[u8]) -> Vec<u8> {
        let mut bytes = (offsets.len() as u32).to_le_bytes().to_vec();
        for offset in offsets {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.resize(data_base(offsets.len() as u32), 0);
        bytes.extend_from_slice(body);
        bytes
    }

    fn synthetic_pcm(name: &str, extra: &[u8]) -> Vec<u8> {
        let mut body = name.as_bytes().to_vec();
        body.push(0);
        body.extend_from_slice(extra);
        body.extend_from_slice(&[0u8; 9]);
        pcm_bytes(&[0, 4], &body)
    }

    fn pmf(id: u16, segments: u16, rate_field: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x2C];
        bytes[0..4].copy_from_slice(b"FMPa");
        bytes[0x06..0x08].copy_from_slice(&id.to_le_bytes());
        bytes[0x08..0x0A].copy_from_slice(&segments.to_le_bytes());
        bytes[0x28..0x2C].copy_from_slice(&rate_field.to_le_bytes());
        bytes
    }

    fn pnf(id: u16, entries: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 20 + 2 * usize::from(entries)];
        bytes[0..4].copy_from_slice(b"FNPa");
        bytes[0x06..0x08].copy_from_slice(&id.to_le_bytes());
        bytes[0x0A..0x0C].copy_from_slice(&entries.to_le_bytes());
        bytes
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).expect("synthetic file must be writable");
    }

    #[test]
    fn pcm_header_and_names_parse() {
        let unit = parse_pcm(&synthetic_pcm("lair5", b"lair6,8"), "LAIR5").expect("must parse");
        assert_eq!(unit.name, "lair5");
        assert_eq!(unit.records, 2);
        assert_eq!(unit.names, vec!["lair5", "lair6,8"]);
        assert_eq!(unit.links, Vec::new());
        assert_eq!(unit.sidecar, Sidecar::Absent);
    }

    #[test]
    fn a_unit_without_a_link_table_reports_no_links() {
        let unit = parse_pcm(&synthetic_pcm("lair5", b"lair6,8"), "lair5").expect("must parse");
        assert_eq!(unit.names.len(), 2, "the name scan still sees the link name");
        assert!(unit.link_table.is_empty());
        assert!(unit.links.is_empty(), "no table means no links");
        assert!(report_lines(&unit)[0].contains("links=0"));
    }

    #[test]
    fn pcm_rejects_count_outside_range() {
        let below = pcm_bytes(&[0], &[]);
        let above = (MAX_RECORDS + 1).to_le_bytes().to_vec();
        for bytes in [below, above] {
            assert_eq!(
                parse_pcm(&bytes, "lair5").err(),
                Some(LevelError::CountCeiling)
            );
        }
    }

    #[test]
    fn pcm_requires_first_offsets() {
        let body = [0u8; 24];
        assert_eq!(
            parse_pcm(&pcm_bytes(&[4, 8], &body), "lair5").err(),
            Some(LevelError::TableNotAscending)
        );
        assert_eq!(
            parse_pcm(&pcm_bytes(&[0, 8], &body), "lair5").err(),
            Some(LevelError::TableNotAscending)
        );
    }

    #[test]
    fn pcm_rejects_descending_offsets() {
        assert_eq!(
            parse_pcm(&pcm_bytes(&[0, 4, 4], &[0u8; 32]), "lair5").err(),
            Some(LevelError::TableNotAscending)
        );
    }

    #[test]
    fn pcm_rejects_tables_past_eof() {
        assert_eq!(
            parse_pcm(&2u32.to_le_bytes(), "lair5").err(),
            Some(LevelError::TablePastEof)
        );
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 3]);
        assert_eq!(
            parse_pcm(&bytes, "lair5").err(),
            Some(LevelError::PaddingTruncated)
        );
        assert_eq!(
            parse_pcm(&pcm_bytes(&[0, 4, 5000], &[0u8; 8]), "lair5").err(),
            Some(LevelError::TablePastEof)
        );
    }

    #[test]
    fn data_base_rounds_the_record_count_up_to_512() {
        for (records, expected) in [(2, 2048), (62, 2048), (74, 2048), (512, 4096), (513, 4096)] {
            assert_eq!(data_base(records), expected, "records={records}");
            assert_eq!(data_base(records) % 2048, 0);
        }
        assert!(data_base(MAX_RECORDS) > 4 + 4 * MAX_RECORDS as usize);
    }

    #[test]
    fn pcm_reads_blocks_from_the_aligned_data_base() {
        let blocks = vec![block_of(4, 0x01), block_of(40, 0x00), named_block(48, 0x03)];
        let (bytes, body_len) = blocks_pcm(&blocks);
        assert_eq!(data_base(3), 2048);
        assert_eq!(bytes.len(), 2048 + body_len);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(unit.block_sizes, vec![4, 40, 48]);
        assert_eq!(unit.block_all_zero, vec![false, true, false]);
    }

    #[test]
    fn pcm_rejects_a_file_truncated_inside_the_offset_padding() {
        let (bytes, _) = blocks_pcm(&[block_of(4, 0x01), block_of(40, 0x02)]);
        assert_eq!(data_base(2), 2048);
        assert_eq!(
            parse_pcm(&bytes[..data_base(2) - 1], "lair5").err(),
            Some(LevelError::PaddingTruncated)
        );
        assert_eq!(
            parse_pcm(&bytes[..11], "lair5").err(),
            Some(LevelError::TablePastEof)
        );
    }

    #[test]
    fn pcm_requires_own_name() {
        assert_eq!(
            parse_pcm(&synthetic_pcm("kain", b"lair6,8"), "lair5").err(),
            Some(LevelError::MissingUnitName)
        );
        assert!(parse_pcm(&synthetic_pcm("lair5", b""), "lair5").is_ok());
    }

    #[test]
    fn name_rule_bounds_and_noise() {
        let unit = parse_pcm(
            &synthetic_pcm("lair5", b"LAIR\0abcdefghijklmnopqrstu\0ab\0lair6,123"),
            "lair5",
        )
        .expect("must parse");
        assert_eq!(unit.names, vec!["lair5"]);
    }

    #[test]
    fn links_come_from_the_link_table_not_the_name_scan() {
        let mut bytes = link_pcm(
            &[
                ([1, 67, 18, 67, -4579], "lair29,66"),
                ([1, 52, 18, 52, -4605], "lair3,51"),
            ],
            2 * LINK_RECORD_STRIDE,
        );
        let base = data_base(2);
        bytes[base + 16..base + 24].copy_from_slice(b"lair9,9\0");
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert!(
            unit.names.iter().any(|name| name == "lair9,9"),
            "the scan sees a link-like name the table does not contain"
        );
        assert_eq!(unit.link_table.len(), 2);
        assert_eq!(
            unit.links,
            vec![
                Link { target: "lair29".into(), index: 66 },
                Link { target: "lair3".into(), index: 51 },
            ]
        );
        assert!(report_lines(&unit)[0].contains("links=2"));
    }

    #[test]
    fn pmf_header_parses_and_refuses() {
        assert_eq!(
            parse_pmf(&pmf(7, 3, 22_050)),
            Ok(PmfHeader {
                id: 7,
                segments: 3,
                rate_field: 22_050
            })
        );
        let mut bad_magic = pmf(7, 3, 22_050);
        bad_magic[0] = b'X';
        assert_eq!(parse_pmf(&bad_magic).err(), Some(LevelError::BadSidecarMagic));
        assert_eq!(parse_pmf(&vec![0u8; 0x2C]).err(), Some(LevelError::BadSidecarMagic));
        assert_eq!(parse_pmf(&pmf(7, 0, 22_050)).err(), Some(LevelError::CountCeiling));
        assert_eq!(
            parse_pmf(&pmf(7, MAX_SEGMENTS + 1, 22_050)).err(),
            Some(LevelError::CountCeiling)
        );
    }

    #[test]
    fn pnf_header_parses_and_refuses() {
        assert_eq!(parse_pnf(&pnf(7, 3)), Ok(PnfHeader { id: 7, entries: 3 }));
        let mut contradiction = pnf(7, 3);
        contradiction.push(0);
        assert_eq!(
            parse_pnf(&contradiction).err(),
            Some(LevelError::SidecarMismatch)
        );
        let mut bad_magic = pnf(7, 0);
        bad_magic[0] = b'X';
        assert_eq!(
            parse_pnf(&bad_magic).err(),
            Some(LevelError::BadSidecarMagic)
        );
        assert_eq!(parse_pnf(&[0u8; 19]).err(), Some(LevelError::BadSidecarMagic));
    }

    #[test]
    fn read_path_single_file_with_sidecar() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "lair5.PCM", &synthetic_pcm("lair5", b"lair6,8"));
        write(tmp.path(), "lair5.pmf", &pmf(7, 3, 22_050));
        write(tmp.path(), "lair5.pnf", &pnf(7, 3));
        let units = read_path(&tmp.path().join("lair5.PCM")).expect("must read");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].links.len(), 0);
        assert_eq!(
            units[0].sidecar,
            Sidecar::Present {
                segments: 3,
                rate_field: 22_050,
                pnf_entries: 3
            }
        );
    }

    #[test]
    fn read_path_directory_prefers_bin_and_sorts() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let bin = tmp.path().join("BIN");
        std::fs::create_dir(&bin).expect("BIN must be creatable");
        write(&bin, "ZETA.PCM", &synthetic_pcm("zeta", b""));
        write(&bin, "alpha.PCM", &synthetic_pcm("alpha", b""));
        write(&bin, "notes.txt", b"noise");
        let units = read_path(tmp.path()).expect("must read");
        let names: Vec<&str> = units.iter().map(|unit| unit.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn read_path_directory_without_bin() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "alpha.PCM", &synthetic_pcm("alpha", b""));
        let units = read_path(tmp.path()).expect("must read");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "alpha");
    }

    #[test]
    fn read_path_refusals() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "lair5.PMF", &pmf(7, 1, 22_050));
        let pmf_path = tmp.path().join("lair5.PMF");
        assert_eq!(read_path(&pmf_path).err(), Some(LevelError::RefusedPath));
        let empty = tmp.path().join("EMPTY");
        std::fs::create_dir(&empty).expect("EMPTY must be creatable");
        assert_eq!(read_path(&empty).err(), Some(LevelError::NoUnits));
        assert_eq!(
            read_path(&tmp.path().join("missing.PCM")).err(),
            Some(LevelError::Io)
        );
    }

    #[test]
    fn sidecar_pair_checks() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "lair5.PCM", &synthetic_pcm("lair5", b""));
        let pcm_path = tmp.path().join("lair5.PCM");
        write(tmp.path(), "lair5.PMF", &pmf(7, 3, 11_025));
        assert_eq!(
            read_path(&pcm_path).err(),
            Some(LevelError::SidecarIncomplete)
        );
        write(tmp.path(), "lair5.PNF", &pnf(8, 3));
        assert_eq!(
            read_path(&pcm_path).err(),
            Some(LevelError::SidecarMismatch)
        );
        write(tmp.path(), "lair5.PNF", &pnf(7, 2));
        assert_eq!(
            read_path(&pcm_path).err(),
            Some(LevelError::SidecarMismatch)
        );
        write(tmp.path(), "lair5.PNF", &pnf(7, 3));
        assert!(read_path(&pcm_path).is_ok());
    }

    #[test]
    fn report_lines_shape() {
        let bytes = link_pcm(&[([1, 2, 3, 4, 5], "lair6,8")], LINK_RECORD_STRIDE);
        let mut unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(unit.names, vec!["lair6,8", "lair5"]);
        assert_eq!(
            report_lines(&unit),
            vec![
                "level lair5 pcm_records=2 names=2 links=1 pmf=absent segments=0 rate_field=0 pnf=0",
                "  link lair6 8",
            ]
        );
        unit.sidecar = Sidecar::Present {
            segments: 3,
            rate_field: 22_050,
            pnf_entries: 3,
        };
        assert_eq!(
            report_lines(&unit)[0],
            "level lair5 pcm_records=2 names=2 links=1 pmf=present segments=3 rate_field=22050 pnf=3"
        );
    }

    /// Room record at 16; its fields and both arrays sit after it, and the
    /// unit name stays in the last six bytes.
    fn geometry_pcm(vertices: &[[i16; 3]], faces: &[[u16; 3]]) -> Vec<u8> {
        let room = 16usize;
        let face_at = room + 0x24;
        let vertex_at = face_at + faces.len() * FACE_STRIDE;
        let len = vertex_at + vertices.len() * VERTEX_STRIDE + 6;
        let mut bytes = structure_pcm(&[0, 4], &[(SLOT_ROOM_RECORD, room as u32)], len);
        let base = data_base(2);
        let record = base + room;
        for (field, value) in [
            (ROOM_VERTEX_COUNT, vertices.len() as u32),
            (ROOM_FACE_COUNT, faces.len() as u32),
            (ROOM_VERTEX_OFFSET, vertex_at as u32),
            (ROOM_FACE_OFFSET, face_at as u32),
        ] {
            bytes[record + field..record + field + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (index, vertex) in vertices.iter().enumerate() {
            let at = base + vertex_at + index * VERTEX_STRIDE;
            for (axis, value) in vertex.iter().enumerate() {
                bytes[at + 2 * axis..at + 2 * axis + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        for (index, face) in faces.iter().enumerate() {
            let at = base + face_at + index * FACE_STRIDE;
            for (slot, value) in face.iter().enumerate() {
                bytes[at + 2 * slot..at + 2 * slot + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }

    fn geometry_of(bytes: &[u8]) -> Result<GeometryReport, LevelError> {
        let unit = parse_pcm(bytes, "lair5")?;
        geometry_report(&bytes[unit.structure.data_base..])
    }

    #[test]
    fn geometry_reports_counts_offsets_box_and_index_validation() {
        let vertices = [[-10, 4, 0], [30, -6, 12], [2, 8, -20]];
        let faces = [[0u16, 1, 2], [2, 2, 1]];
        let g = geometry_of(&geometry_pcm(&vertices, &faces)).expect("must read");
        assert_eq!((g.vertices, g.faces, g.room_offset), (3, 2, 16));
        assert_eq!((g.min, g.max), ([-10, -6, -20], [30, 8, 12]));
        assert_eq!((g.out_of_range, g.max_index), (0, Some(2)));
        assert!(geometry_lines("lair5", &g)[1].ends_with("center=10.0,1.0,-4.0"));
    }

    #[test]
    fn geometry_counts_out_of_range_indices_without_failing() {
        let g = geometry_of(&geometry_pcm(&[[0, 0, 0]], &[[0, 1, 9]])).expect("must read");
        assert_eq!((g.out_of_range, g.max_index), (2, Some(9)));
    }

    #[test]
    fn geometry_zero_counts_report_zero_and_no_box() {
        let g = geometry_of(&geometry_pcm(&[], &[])).expect("must read");
        assert_eq!((g.vertices, g.faces, g.max_index), (0, 0, None));
        assert_eq!(geometry_lines("lair5", &g)[1], "  bbox none");
    }

    #[test]
    fn geometry_record_or_array_out_of_range_fails_loud() {
        let mut record = geometry_pcm(&[], &[]);
        record[data_base(2)..data_base(2) + 4].copy_from_slice(&2000u32.to_le_bytes());
        assert_eq!(geometry_of(&record).err(), Some(LevelError::GeometryRecordOutOfRange));
        let mut array = geometry_pcm(&[[0, 0, 0]], &[]);
        let count = data_base(2) + 16 + ROOM_VERTEX_COUNT;
        array[count..count + 4].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(geometry_of(&array).err(), Some(LevelError::GeometryArrayOutOfRange));
    }

    fn block_of(size: usize, byte: u8) -> Vec<u8> {
        vec![byte; size]
    }

    fn named_block(size: usize, byte: u8) -> Vec<u8> {
        let mut block = vec![byte; size];
        block[..5].copy_from_slice(b"lair5");
        block
    }

    /// `N` blocks whose ascending table starts at 0, 4, so block 0 must be
    /// 4 bytes. Returns the file bytes and the data-region length.
    fn blocks_pcm(blocks: &[Vec<u8>]) -> (Vec<u8>, usize) {
        let mut offsets = Vec::with_capacity(blocks.len());
        let mut body = Vec::new();
        for block in blocks {
            offsets.push(body.len() as u32);
            body.extend_from_slice(block);
        }
        let bytes = pcm_bytes(&offsets, &body);
        let data_len = body.len();
        (bytes, data_len)
    }

    /// Body with chosen header dwords and the unit name in its last 6 bytes.
    fn structure_pcm(offsets: &[u32], slots: &[(usize, u32)], len: usize) -> Vec<u8> {
        let mut body = vec![0u8; len];
        for &(slot, value) in slots {
            body[4 * slot..4 * slot + 4].copy_from_slice(&value.to_le_bytes());
        }
        body[len - 6..].copy_from_slice(b"lair5\0");
        pcm_bytes(offsets, &body)
    }

    #[test]
    fn structure_reports_relocation_and_schema_slots() {
        let bytes = structure_pcm(&[0, 4, 12], &[(0x00, 16), (0x01, 0), (0x03, 20)], 40);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!((unit.structure.data_base, unit.structure.data_len), (2048, 40));
        let reloc = &unit.structure.reloc;
        assert_eq!(
            (reloc.entries, reloc.valid, reloc.zero, reloc.out_of_range),
            (3, 2, 1, 0)
        );
        assert_eq!(unit.structure.slots, vec![(0x00, 2064), (0x03, 2068)]);
        assert_eq!(
            structure_lines(&unit).expect("in range"),
            vec![
                "level lair5 pcm_records=3 data_base=2048 data_len=40",
                "  reloc entries=3 valid=2 zero=1 out_of_range=0",
                "  schema slots=14 non_null=2 0x00:2064 0x03:2068",
                "  table36 count=0 stride=36 lengths=none",
            ]
        );
    }

    #[test]
    fn table36_reports_blob_lengths_and_nulls() {
        let mut bytes = structure_pcm(
            &[0, 4],
            &[(0x00, 16), (0x01, 20), (0x3A, 6), (0x3B, 256)],
            800,
        );
        let base = data_base(2);
        for (index, first) in [500u32, 0, 600, 640, 700, 660].iter().enumerate() {
            let at = base + 256 + index * TABLE36_STRIDE;
            bytes[at..at + 4].copy_from_slice(&first.to_le_bytes());
        }
        let mut typed = vec![0u8; 20];
        typed[..6].copy_from_slice(&[0xBE, 0xEF, 0x02, 0x00, 0x01, 0x00]);
        for (at, blob) in [
            (500, &b"\xBE\xEF\x01\x00\x01\x03"[..]),
            (640, &b"\xBE\xEF\x00"[..]),
            (660, &typed[..]),
        ] {
            bytes[base + at..base + at + blob.len()].copy_from_slice(blob);
        }
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let table = &unit.structure.table36;
        assert_eq!((table.count, table.stride), (6, 36));
        assert_eq!(
            table.lengths,
            vec![Some(24), None, None, Some(4), None, Some(36)]
        );
        assert!(structure_lines(&unit).expect("in range")[3].ends_with(
            "table36 count=6 stride=36 lengths=24,null,null,4,null,36"
        ));
    }

    #[test]
    fn structure_fails_loud_on_an_out_of_range_directory_entry() {
        let bytes = structure_pcm(&[0, 4], &[(0x01, 90)], 40);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!((unit.structure.reloc.zero, unit.structure.reloc.out_of_range), (1, 1));
        assert_eq!(
            structure_lines(&unit).err(),
            Some(LevelError::RelocationOutOfRange)
        );
        let past = structure_pcm(&[0, 4, 40], &[], 40);
        let unit = parse_pcm(&past, "lair5").expect("must parse");
        assert_eq!(unit.structure.reloc.out_of_range, 1);
        assert!(structure_lines(&unit).is_err());
    }

    /// Header slot `0x34` points 256 bytes into the data region; each record
    /// fills its 12-byte name field and leaves its tail as written.
    fn link_pcm(records: &[([i32; 5], &str)], tail: usize) -> Vec<u8> {
        let offset = 256usize;
        let mut bytes = structure_pcm(&[0, 4], &[(SLOT_LINK_TABLE, offset as u32)], offset + tail);
        let base = data_base(2);
        for (index, (prelude, name)) in records.iter().enumerate() {
            let at = base + offset + index * LINK_RECORD_STRIDE;
            for (slot, value) in prelude.iter().enumerate() {
                bytes[at + 4 * slot..at + 4 * slot + 4].copy_from_slice(&value.to_le_bytes());
            }
            bytes[at + 20..at + 20 + name.len()].copy_from_slice(name.as_bytes());
        }
        bytes
    }

    fn links_of(bytes: &[u8]) -> Vec<TableLink> {
        parse_pcm(bytes, "lair5").expect("must parse").link_table
    }

    #[test]
    fn link_table_decodes_records_and_stops_at_the_terminator() {
        let bytes = link_pcm(
            &[
                ([1, 67, 18, 67, -4579], "lair29,66"),
                ([1, 52, 18, 52, -4605], "lair3,51"),
            ],
            2 * LINK_RECORD_STRIDE,
        );
        let links = links_of(&bytes);
        assert_eq!(links.len(), 2);
        assert_eq!(
            links[0],
            TableLink {
                target: "lair29".into(),
                index: 66,
                target_id: -4579,
                prelude: [1, 67, 18, 67, -4579]
            }
        );
        assert_eq!((links[1].target.as_str(), links[1].index), ("lair3", 51));
        assert_eq!(links[1].target_id, -4605);
    }

    #[test]
    fn target_id_exposes_the_fourth_prelude_field() {
        let bytes = link_pcm(
            &[
                ([1, 67, 18, 67, -4579], "lair29,66"),
                ([1, 52, 18, 52, -4605], "lair3,51"),
            ],
            2 * LINK_RECORD_STRIDE,
        );
        let links = links_of(&bytes);
        for link in &links {
            assert_eq!(link.target_id, link.prelude[4]);
        }
        assert_ne!(links[0].target_id, links[1].target_id);
    }

    #[test]
    fn a_zero_slot_or_a_non_link_first_record_means_no_table() {
        let no_slot = structure_pcm(&[0, 4], &[], 300);
        assert!(links_of(&no_slot).is_empty());
        let non_link = link_pcm(&[([9, 8, 7, 6, 5], "junk")], LINK_RECORD_STRIDE);
        assert!(links_of(&non_link).is_empty());
    }

    #[test]
    fn a_record_that_leaves_the_data_region_ends_the_walk() {
        let bytes = link_pcm(&[([1, 2, 3, 4, 5], "lair7,3")], LINK_RECORD_STRIDE + 20);
        let links = links_of(&bytes);
        assert_eq!(links.len(), 1);
        assert_eq!((links[0].target.as_str(), links[0].index), ("lair7", 3));
    }

    #[test]
    fn link_lines_report_the_count_targets_target_ids_and_raw_preludes() {
        let bytes = link_pcm(&[([1, 67, 18, 67, -4579], "lair29,66")], LINK_RECORD_STRIDE);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(
            link_lines(&unit),
            vec![
                "links lair5 count=1",
                "  link lair29 66 target_id=-4579 prelude=1,67,18,67,-4579",
            ]
        );
    }

    #[test]
    fn classify_is_zero_first_then_size() {
        for size in [0, 4, 40, 48, 72] {
            assert_eq!(classify(size, true), BlockClass::Zero);
        }
        assert_eq!(classify(4, false), BlockClass::Small4);
        assert_eq!(classify(40, false), BlockClass::Fixed40);
        assert_eq!(classify(48, false), BlockClass::Fixed48);
        assert_eq!(classify(72, false), BlockClass::Other(72));
    }

    #[test]
    fn census_counts_shape_classes_and_the_largest_block() {
        let blocks = vec![
            block_of(4, 0),
            block_of(4, 0x01),
            block_of(40, 0),
            block_of(40, 0x02),
            block_of(48, 0x03),
            named_block(72, 0x04),
        ];
        let (bytes, data_len) = blocks_pcm(&blocks);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let counts = census(&unit);
        assert_eq!(counts.total, 6);
        assert_eq!(
            (counts.zero, counts.small4, counts.fixed40, counts.fixed48),
            (2, 1, 1, 1)
        );
        assert_eq!(counts.other, vec![(72, 1)]);
        assert_eq!(counts.largest, (5, 72));
        let classified: u32 = counts.zero
            + counts.small4
            + counts.fixed40
            + counts.fixed48
            + counts.other.iter().map(|(_, count)| count).sum::<u32>();
        assert_eq!(classified, unit.records);
        assert_eq!(unit.block_sizes.iter().sum::<u32>() as usize, data_len);
    }

    #[test]
    fn largest_tie_keeps_the_lowest_ordinal_and_its_class() {
        let blocks = vec![block_of(4, 0), named_block(40, 0x02), block_of(40, 0x02)];
        let (bytes, _) = blocks_pcm(&blocks);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let counts = census(&unit);
        assert_eq!(counts.fixed40, 2, "the largest keeps its shape class");
        assert_eq!(counts.largest, (1, 40), "ties go to the lowest ordinal");
    }

    #[test]
    fn other_rows_ascend_by_size_and_lines_keep_fixed_order() {
        let blocks = vec![block_of(4, 0), block_of(72, 0x02), named_block(28, 0x04)];
        let (bytes, _) = blocks_pcm(&blocks);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let counts = census(&unit);
        assert_eq!(counts.other, vec![(28, 1), (72, 1)]);
        assert_eq!(counts.largest, (1, 72));
        assert_eq!(
            record_lines(&unit),
            vec![
                "blocks lair5 total=3",
                "  shape zero 1",
                "  shape other:28 1",
                "  shape other:72 1",
                "largest 1 72",
            ]
        );
    }

    #[test]
    fn exit_codes_follow_spec() {
        assert_eq!(LevelError::Usage("usage").exit_code(), 1);
        assert_eq!(LevelError::RefusedPath.exit_code(), 3);
        for error in [
            LevelError::Io,
            LevelError::NoUnits,
            LevelError::CountCeiling,
            LevelError::TableNotAscending,
            LevelError::TablePastEof,
            LevelError::PaddingTruncated,
            LevelError::RelocationOutOfRange,
            LevelError::GeometryRecordOutOfRange,
            LevelError::GeometryArrayOutOfRange,
            LevelError::MissingUnitName,
            LevelError::SidecarIncomplete,
            LevelError::BadSidecarMagic,
            LevelError::SidecarMismatch,
        ] {
            assert_eq!(error.exit_code(), 2);
        }
    }
}

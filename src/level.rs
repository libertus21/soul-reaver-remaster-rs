//! Untrusted SR1-remaster room containers: the `.PCM` header and name table.
//! Read-only inventory: no geometry, placement, transform, or audio decode.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub index: u16,
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
    let names = extract_names(bytes);
    let name = names
        .iter()
        .find(|name| name.eq_ignore_ascii_case(expected_name))
        .ok_or(LevelError::MissingUnitName)?
        .clone();
    let links = names.iter().filter_map(|entry| split_link(entry)).collect();
    Ok(LevelUnit {
        name,
        records,
        names,
        links,
        sidecar: Sidecar::Absent,
        block_sizes,
        block_all_zero,
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

fn split_link(name: &str) -> Option<Link> {
    let (target, index) = name.split_once(',')?;
    Some(Link { target: target.to_string(), index: index.parse().ok()? })
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
        assert_eq!(unit.links, vec![Link { target: "lair6".into(), index: 8 }]);
        assert_eq!(unit.sidecar, Sidecar::Absent);
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
    fn links_split_and_keep_order_and_duplicates() {
        let unit = parse_pcm(
            &synthetic_pcm("lair5", b"lair6,8\0lair5\0lair6,8"),
            "lair5",
        )
        .expect("must parse");
        assert_eq!(unit.names.len(), 4);
        let expected = Link { target: "lair6".into(), index: 8 };
        assert_eq!(unit.links, vec![expected.clone(), expected]);
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
        assert_eq!(units[0].links.len(), 1);
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
        let mut unit =
            parse_pcm(&synthetic_pcm("lair5", b"lair6,8"), "lair5").expect("must parse");
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
            LevelError::MissingUnitName,
            LevelError::SidecarIncomplete,
            LevelError::BadSidecarMagic,
            LevelError::SidecarMismatch,
        ] {
            assert_eq!(error.exit_code(), 2);
        }
    }
}

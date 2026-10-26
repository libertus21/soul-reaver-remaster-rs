//! Remaster `.BIG` fixed-slot container (`big-slots` + `big-decompressor`):
//! verified 512-byte header (`0..2` size must be 512, `2..4` count must be
//! `1..=32768`) plus fixed 131072-byte slots (`file_len == 4096 + count *
//! 131072`, slot `i` at `4096 + i * 131072`). Geometry mismatches are loud,
//! never clamped. Slots are classified over their WHOLE length: all-`0xFF` =
//! empty, all-zero = zero, else data. No sampling. A `data` payload is raw
//! 256 x 256 little-endian A1R5G5B5 (`slot_texture`), NOT compressed, so
//! `--tshow` decodes one slot. This is NOT the SR1PC layout — `bigfile.rs`
//! must never parse it. Only classes/verdicts leave, never content.
//! Synthetic bytes in tests only.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::slot_texture;

/// Sanity bound from the bigfile spec source (`MAX_ENTRIES = 32768`).
/// Used here as a guard on the parsed entry count — not a format claim.
pub const MAX_ENTRIES: u32 = 32768;

/// Verified `.BIG` header length: the size word must equal this value.
pub const HEADER_SIZE: u16 = 512;

/// Verified fixed slot size for the four SR1 remaster containers.
pub const SLOT_SIZE: u64 = 131072;

/// First slot starts here: the 4 KiB prefix after the 512-byte header.
pub const SLOT_DATA_START: u64 = 4096;

/// Exact file length implied by the header entry count.
pub fn expected_len(count: u16) -> u64 {
    SLOT_DATA_START + u64::from(count) * SLOT_SIZE
}

/// Byte offset of slot `index` (pure arithmetic, no table).
pub fn slot_offset(index: u16) -> u64 {
    SLOT_DATA_START + u64::from(index) * SLOT_SIZE
}

/// The only parsed header fields: a `u16` size word and a `u16` entry
/// count. Bytes `4..16` are observed zero in all four real containers
/// but are NOT validated or interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigHeader {
    pub header_size: u16,
    pub entry_count: u16,
}

/// Parse the header from the first four bytes plus the known file
/// length. Nothing past byte 4 participates.
pub fn parse_header(prefix: [u8; 4], file_len: u64) -> Result<BigHeader, TexturesError> {
    if file_len < u64::from(HEADER_SIZE) {
        return Err(TexturesError::TruncatedHeader { len: file_len });
    }
    let header_size = u16::from_le_bytes([prefix[0], prefix[1]]);
    if header_size != HEADER_SIZE {
        return Err(TexturesError::WrongHeaderSize { found: header_size });
    }
    let entry_count = u16::from_le_bytes([prefix[2], prefix[3]]);
    if entry_count == 0 || u32::from(entry_count) > MAX_ENTRIES {
        return Err(TexturesError::ImplausibleCount { count: entry_count });
    }
    Ok(BigHeader { header_size, entry_count })
}

/// Shape of one probed window (classified, never dumped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionClass {
    ZeroFill,
    FfFill,
    DdsMagic,
    Data,
}

impl fmt::Display for RegionClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegionClass::ZeroFill => write!(f, "zero-fill"),
            RegionClass::FfFill => write!(f, "0xFF-fill"),
            RegionClass::DdsMagic => write!(f, "DDS-magic"),
            RegionClass::Data => write!(f, "data"),
        }
    }
}

/// Classify one window (pure, no I/O): all-zeros, all-`0xFF`, `DDS `
/// prefix, else data. Empty windows report `ZeroFill`.
pub fn classify_window(bytes: &[u8]) -> RegionClass {
    if bytes.len() >= 4 && bytes[0..4] == *b"DDS " {
        return RegionClass::DdsMagic;
    }
    if bytes.iter().all(|b| *b == 0) {
        return RegionClass::ZeroFill;
    }
    if bytes.iter().all(|b| *b == 0xFF) {
        return RegionClass::FfFill;
    }
    RegionClass::Data
}

/// Whole-slot class (contract names exactly three classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotClass {
    Empty,
    Zero,
    Data,
}

impl fmt::Display for SlotClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotClass::Empty => write!(f, "empty"),
            SlotClass::Zero => write!(f, "zero"),
            SlotClass::Data => write!(f, "data"),
        }
    }
}

/// Classify a WHOLE slot slice: all-`0xFF` -> `Empty`, all-zero ->
/// `Zero`, anything else -> `Data`. Empty slice is `Zero`.
pub fn classify_slot(bytes: &[u8]) -> SlotClass {
    if bytes.is_empty() {
        return SlotClass::Zero;
    }
    if bytes.iter().all(|b| *b == 0xFF) {
        return SlotClass::Empty;
    }
    if bytes.iter().all(|b| *b == 0) {
        return SlotClass::Zero;
    }
    SlotClass::Data
}

/// Reject any file length that is not `4096 + count * 131072` exactly.
/// Never clamps: a mismatch is loud, not silently repaired.
pub fn validate_geometry(file_len: u64, header: BigHeader) -> Result<(), TexturesError> {
    let expected = expected_len(header.entry_count);
    if file_len == expected {
        Ok(())
    } else {
        Err(TexturesError::SlotGeometryMismatch {
            file_len,
            count: header.entry_count,
            expected,
        })
    }
}

/// One class per slot, in index order. Built by a single sequential pass
/// over whole slots with one reusable 128 KiB buffer (constant memory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotScan {
    pub classes: Vec<SlotClass>,
}

impl SlotScan {
    /// How many slots carry `class`.
    pub fn count(&self, class: SlotClass) -> usize {
        self.classes.iter().filter(|c| **c == class).count()
    }

    /// `--tlist` slot section: addressing line, one line per slot, and
    /// the per-class footer. Classes only; never payload bytes.
    pub fn report(&self) -> String {
        let total = self.classes.len();
        let mut out = format!(
            "addressing: fixed {SLOT_SIZE}-byte slots (slot i at {SLOT_DATA_START} + i * {SLOT_SIZE})\nslots (whole-slot scan; no sampling):"
        );
        for (i, class) in self.classes.iter().enumerate() {
            out.push_str(&format!("\nslot {i} offset {} {class}", slot_offset(i as u16)));
        }
        out.push_str(&format!(
            "\nslots: {total} total, {} empty, {} zero, {} data",
            self.count(SlotClass::Empty),
            self.count(SlotClass::Zero),
            self.count(SlotClass::Data),
        ));
        out
    }
}

/// Read every slot whole and classify it. Short read is a loud `Io`
/// error (geometry validation already guaranteed the length).
pub fn scan_slots(path: &Path, header: &BigHeader) -> Result<SlotScan, TexturesError> {
    let mut file = File::open(path).map_err(TexturesError::Io)?;
    file.seek(SeekFrom::Start(SLOT_DATA_START))
        .map_err(TexturesError::Io)?;
    let mut buf = vec![0u8; SLOT_SIZE as usize];
    let mut classes = Vec::with_capacity(usize::from(header.entry_count));
    for _ in 0..header.entry_count {
        file.read_exact(&mut buf).map_err(TexturesError::Io)?;
        classes.push(classify_slot(&buf));
    }
    Ok(SlotScan { classes })
}

/// Read exactly one slot and classify it. Returns `(offset, class)`.
pub fn slot_class_at(
    path: &Path,
    header: &BigHeader,
    index: u16,
) -> Result<(u64, SlotClass), TexturesError> {
    debug_assert!(
        index < header.entry_count,
        "slot index must be range-checked by the caller"
    );
    let offset = slot_offset(index);
    let mut file = File::open(path).map_err(TexturesError::Io)?;
    file.seek(SeekFrom::Start(offset))
        .map_err(TexturesError::Io)?;
    let mut buf = vec![0u8; SLOT_SIZE as usize];
    file.read_exact(&mut buf).map_err(TexturesError::Io)?;
    Ok((offset, classify_slot(&buf)))
}

/// Read exactly one slot, classify it, and decode it when it is `data`.
/// `empty`/`zero` slots carry no payload and are loud; a payload whose
/// length is not the slot size is loud too. A short read stays `Io`.
#[allow(dead_code)] // Transitional: the `--tshow` arm wires this in a later commit.
pub fn decode_slot_at(
    path: &Path,
    header: &BigHeader,
    index: u16,
) -> Result<(u64, Vec<u8>), TexturesError> {
    debug_assert!(
        index < header.entry_count,
        "slot index must be range-checked by the caller"
    );
    let offset = slot_offset(index);
    let mut file = File::open(path).map_err(TexturesError::Io)?;
    file.seek(SeekFrom::Start(offset))
        .map_err(TexturesError::Io)?;
    let mut buf = vec![0u8; SLOT_SIZE as usize];
    file.read_exact(&mut buf).map_err(TexturesError::Io)?;
    match classify_slot(&buf) {
        SlotClass::Data => match slot_texture::decode_a1r5g5b5(&buf) {
            Ok(rgba) => Ok((offset, rgba)),
            Err(slot_texture::SlotDecodeError::WrongLength { found, expected }) => {
                Err(TexturesError::SlotPayloadLength {
                    index,
                    found,
                    expected,
                })
            }
        },
        class => Err(TexturesError::SlotNoPayload {
            index,
            offset,
            class,
        }),
    }
}


/// Every textures failure maps to stderr + nonzero exit in `main`.
/// `BadEntry`/`NotDds` await a future table parser (no-go: unset).
#[allow(dead_code)]
#[derive(Debug)]
pub enum TexturesError {
    Io(std::io::Error),
    NotTextures(String),
    BadEntry(String),
    BadSelector(String),
    NotDds(String),
    TruncatedHeader { len: u64 },
    WrongHeaderSize { found: u16 },
    ImplausibleCount { count: u16 },
    SlotGeometryMismatch {
        file_len: u64,
        count: u16,
        expected: u64,
    },
    SlotSelectorInvalid {
        selector: String,
        count: u16,
    },
    SlotIndexOutOfRange {
        index: usize,
        count: u16,
    },
    SlotNoPayload {
        index: u16,
        offset: u64,
        class: SlotClass,
    },
    SlotPayloadLength {
        index: u16,
        found: usize,
        expected: usize,
    },
}

impl fmt::Display for TexturesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TexturesError::Io(e) => write!(f, "cannot read textures file: {e}"),
            TexturesError::NotTextures(msg) => write!(f, "not a TEXTURES container: {msg}"),
            TexturesError::BadEntry(msg) => write!(f, "bad textures entry: {msg}"),
            TexturesError::BadSelector(sel) => write!(
                f,
                "no entry table identified for selector '{sel}': index location UNKNOWN (no stride inferred)"
            ),
            TexturesError::NotDds(msg) => write!(f, "not a DDS entry: {msg}"),
            TexturesError::TruncatedHeader { len } => write!(
                f,
                "truncated .BIG header: file holds {len} bytes, fewer than the {HEADER_SIZE}-byte header"
            ),
            TexturesError::WrongHeaderSize { found } => write!(
                f,
                "not a .BIG container: header size word {found}, expected {HEADER_SIZE}"
            ),
            TexturesError::ImplausibleCount { count } => write!(
                f,
                "implausible .BIG entry count: {count} (sanity range 1..={MAX_ENTRIES})"
            ),
            TexturesError::SlotGeometryMismatch {
                file_len,
                count,
                expected,
            } => write!(
                f,
                "slot geometry mismatch: file holds {file_len} bytes, header count {count} implies {expected} (4096 + {count} * {SLOT_SIZE}); refusing to clamp"
            ),
            TexturesError::SlotSelectorInvalid { selector, count } => write!(
                f,
                "invalid slot selector '{selector}': expected a slot index in 0..{count}"
            ),
            TexturesError::SlotIndexOutOfRange { index, count } => write!(
                f,
                "slot index {index} is out of range: container holds {count} slots (valid 0..{count})"
            ),
            TexturesError::SlotNoPayload {
                index,
                offset,
                class,
            } => match class {
                SlotClass::Empty => write!(
                    f,
                    "slot {index} (offset {offset}) is empty (0xFF marker): no payload to decode"
                ),
                SlotClass::Zero => write!(
                    f,
                    "slot {index} (offset {offset}) is zero (0x00 fill): no payload to decode"
                ),
                SlotClass::Data => write!(
                    f,
                    "slot {index} (offset {offset}) is data: no payload to decode"
                ),
            },
            TexturesError::SlotPayloadLength {
                index,
                found,
                expected,
            } => write!(
                f,
                "slot {index} payload length {found} does not match {}x{} A1R5G5B5 ({expected} bytes)",
                slot_texture::WIDTH,
                slot_texture::HEIGHT,
            ),
        }
    }
}

impl std::error::Error for TexturesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TexturesError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Observed layout shapes (never content). `table_found` is wired by the
/// Task-2 hunt — an evidence field, never a parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexturesLayout {
    pub file_len: u64,
    pub header: BigHeader,
    pub header_class: RegionClass,
    pub fill_class: RegionClass,
    pub data_class: RegionClass,
    pub table_found: bool,
}

impl TexturesLayout {
    /// Coarse bucket for reports (the exact length never leaves in output).
    pub fn size_class(&self) -> String {
        const MIB: u64 = 1024 * 1024;
        if self.file_len < MIB {
            "sub-MiB class".to_string()
        } else {
            format!("{}-MiB class", self.file_len / MIB)
        }
    }

    /// `--tlist` report (stdout): parsed header, size class, region
    /// classes, and the explicit UNKNOWN entry-table verdict. No content,
    /// no raw offsets, no game paths, no exact byte lengths.
    pub fn summary(&self) -> String {
        format!(
            "BIG header: {HEADER_SIZE}-byte header, {} entries\nsize: {}\nheader window: {}\nfill window: {}\ndata window: {}\nentry table: UNKNOWN (location not established; no stride inferred)",
            self.header.entry_count,
            self.size_class(),
            self.header_class,
            self.fill_class,
            self.data_class,
        )
    }
}

// Probe roles (positions mirror the planning ledger in design.md).
const HEADER_PROBE_OFF: u64 = 0;
const FILL_PROBE_OFF: u64 = 4096;
const DATA_PROBE_OFF: u64 = 1024 * 1024;
const PROBE_LEN: usize = 64;

/// Data-probe offset clamped so a geometry-valid short container (as few
/// as one slot) never fails with a misleading probed-EOF error. Real
/// containers are all larger than 1 MiB, so their offset is unchanged.
fn data_probe_offset(file_len: u64) -> u64 {
    DATA_PROBE_OFF.min(file_len.saturating_sub(PROBE_LEN as u64))
}

/// Read exactly `PROBE_LEN` bytes at `offset` (bounded, read-only).
fn probe_window(file: &mut File, offset: u64, file_len: u64) -> Result<[u8; PROBE_LEN], TexturesError> {
    if offset + PROBE_LEN as u64 > file_len {
        return Err(TexturesError::NotTextures(format!(
            "probe window past EOF (file holds {file_len} bytes)"
        )));
    }
    let mut buf = [0u8; PROBE_LEN];
    file.seek(SeekFrom::Start(offset)).map_err(TexturesError::Io)?;
    file.read_exact(&mut buf).map_err(|_| {
        TexturesError::NotTextures("truncated probe window (short read past EOF)".to_string())
    })?;
    Ok(buf)
}

/// Bounded table hunt (read-only): tail 1 KB (index-at-end hypothesis)
/// plus the data window past the fill region. Returns whether a table
/// was identified — always `false` here: tail count words fail sanity,
/// and a sane word alone is not a table (no observed fields, so no
/// parser is invented). The documented no-go, not a failure.
pub fn hunt_table(path: &Path, file_len: u64) -> Result<bool, TexturesError> {
    const TAIL_LEN: u64 = 1024;
    let mut file = File::open(path).map_err(TexturesError::Io)?;
    let tail_off = file_len.saturating_sub(TAIL_LEN);
    let mut tail = vec![0u8; (file_len - tail_off) as usize];
    file.seek(SeekFrom::Start(tail_off)).map_err(TexturesError::Io)?;
    file.read_exact(&mut tail).map_err(|_| {
        TexturesError::NotTextures("truncated tail window (short read past EOF)".to_string())
    })?;
    let sane_at = |off: usize| {
        tail.get(off..off + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) <= MAX_ENTRIES)
            .unwrap_or(false)
    };
    // Observed, then deliberately unused: even a sane tail word cannot
    // anchor a parser without observed fields — refusing to guess.
    let _tail_signal = sane_at(0) || sane_at(tail.len().saturating_sub(4));
    let _data_class = classify_window(&probe_window(&mut file, data_probe_offset(file_len), file_len)?);
    Ok(false)
}

/// Describe `path` with bounded read-only probes. Truncated input fails
/// loud — never a partial layout as success.
pub fn describe(path: &Path) -> Result<TexturesLayout, TexturesError> {
    let file_len = std::fs::metadata(path).map_err(TexturesError::Io)?.len();
    if file_len < u64::from(HEADER_SIZE) {
        return Err(TexturesError::TruncatedHeader { len: file_len });
    }
    let mut file = File::open(path).map_err(TexturesError::Io)?;
    let mut prefix = [0u8; 4];
    file.read_exact(&mut prefix)
        .map_err(|_| TexturesError::TruncatedHeader { len: file_len })?;
    let header = parse_header(prefix, file_len)?;
    validate_geometry(file_len, header)?;
    let header_class = classify_window(&probe_window(&mut file, HEADER_PROBE_OFF, file_len)?);
    let fill_class = classify_window(&probe_window(&mut file, FILL_PROBE_OFF, file_len)?);
    let data_class = classify_window(&probe_window(&mut file, data_probe_offset(file_len), file_len)?);
    drop(file);
    let table_found = hunt_table(path, file_len)?;
    Ok(TexturesLayout {
        file_len,
        header,
        header_class,
        fill_class,
        data_class,
        table_found,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Four header bytes for the given words (everything else invented).
    fn prefix(header_size: u16, count: u16) -> [u8; 4] {
        let mut p = [0u8; 4];
        p[0..2].copy_from_slice(&header_size.to_le_bytes());
        p[2..4].copy_from_slice(&count.to_le_bytes());
        p
    }

    /// Synthetic container with EXACT geometry: header words, `0xFF` fill
    /// from 4K, nonzero bytes in the last probe window. All values
    /// invented — never game bytes.
    fn synthetic_textures(header_size: u16, count: u16) -> Vec<u8> {
        let len = expected_len(count) as usize;
        let mut b = vec![0u8; len];
        b[0..4].copy_from_slice(&prefix(header_size, count));
        for byte in b[SLOT_DATA_START as usize..].iter_mut() {
            *byte = 0xFF;
        }
        let probe = data_probe_offset(len as u64) as usize;
        for (i, byte) in b[probe..].iter_mut().enumerate() {
            *byte = (i % 251) as u8 + 1;
        }
        b
    }

    /// Synthetic container whose slots are exactly the requested classes.
    /// Header prefix is zero; slot payloads are invented.
    fn synthetic_slots(classes: &[SlotClass]) -> Vec<u8> {
        let count = classes.len() as u16;
        let mut b = vec![0u8; expected_len(count) as usize];
        b[0..4].copy_from_slice(&prefix(512, count));
        for (i, class) in classes.iter().enumerate() {
            let start = slot_offset(i as u16) as usize;
            let slot = &mut b[start..start + SLOT_SIZE as usize];
            match class {
                SlotClass::Empty => slot.fill(0xFF),
                SlotClass::Zero => slot.fill(0x00),
                SlotClass::Data => {
                    for (j, byte) in slot.iter_mut().enumerate() {
                        *byte = (j % 251) as u8 + 1;
                    }
                }
            }
        }
        b
    }

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("textures-test-{name}-{}", std::process::id()));
        let mut f = std::fs::File::create(&p).expect("temp file must be writable");
        f.write_all(bytes).expect("temp file must be writable");
        p
    }

    #[test]
    fn classifies_each_region_shape() {
        assert_eq!(classify_window(&[0u8; 64]), RegionClass::ZeroFill);
        assert_eq!(classify_window(&[0xFFu8; 64]), RegionClass::FfFill);
        let mut magic = vec![0u8; 64];
        magic[0..4].copy_from_slice(b"DDS ");
        assert_eq!(classify_window(&magic), RegionClass::DdsMagic);
        assert_eq!(classify_window(&[0u8, 1, 0, 0]), RegionClass::Data);
    }

    #[test]
    fn parses_the_two_header_words_only() {
        let header = parse_header(prefix(512, 3), 4096).expect("valid header must parse");
        assert_eq!(
            header,
            BigHeader {
                header_size: 512,
                entry_count: 3
            }
        );
    }

    #[test]
    fn rejects_wrong_header_size_with_distinct_message() {
        let err = parse_header(prefix(256, 3), 4096).expect_err("256 must be rejected");
        assert!(matches!(
            err,
            TexturesError::WrongHeaderSize { found: 256 }
        ));
        let msg = err.to_string();
        assert!(msg.contains("256") && msg.contains("512"), "got: {msg}");
    }

    #[test]
    fn rejects_truncated_files_with_distinct_message() {
        let err = parse_header(prefix(512, 3), 100).expect_err("100 bytes must be rejected");
        assert!(matches!(err, TexturesError::TruncatedHeader { len: 100 }));
        assert!(err.to_string().contains("100"), "got: {err}");
        let err = parse_header(prefix(512, 3), 2).expect_err("2 bytes must be rejected");
        assert!(matches!(err, TexturesError::TruncatedHeader { len: 2 }));
    }

    #[test]
    fn rejects_implausible_counts_with_distinct_message() {
        let err = parse_header(prefix(512, 0), 4096).expect_err("zero must be rejected");
        assert!(matches!(err, TexturesError::ImplausibleCount { count: 0 }));
        assert!(err.to_string().contains("1..=32768"), "got: {err}");
        let err = parse_header(prefix(512, 32769), 4096).expect_err("32769 must be rejected");
        assert!(matches!(
            err,
            TexturesError::ImplausibleCount { count: 32769 }
        ));
        assert!(err.to_string().contains("32769"), "got: {err}");
        assert!(parse_header(prefix(512, 32768), 4096).is_ok());
    }

    #[test]
    fn slot_arithmetic_matches_verified_layout() {
        assert_eq!(expected_len(1), 135168);
        assert_eq!(expected_len(1287), 4096 + 1287 * 131072);
        assert_eq!(slot_offset(0), 4096);
        assert_eq!(slot_offset(1), 135168);
        for i in 0..16u16 {
            assert_eq!(slot_offset(i), SLOT_DATA_START + u64::from(i) * SLOT_SIZE);
        }
    }

    #[test]
    fn classify_slot_uses_the_whole_slice() {
        assert_eq!(classify_slot(&[0xFFu8; 131072]), SlotClass::Empty);
        assert_eq!(classify_slot(&[0u8; 131072]), SlotClass::Zero);
        assert_eq!(classify_slot(&[]), SlotClass::Zero);
        let mut mixed = vec![0xFFu8; 131072];
        mixed[131071] = 0x00;
        assert_eq!(classify_slot(&mixed), SlotClass::Data);
        assert_eq!(classify_slot(&[0u8, 1]), SlotClass::Data);
        assert_eq!(SlotClass::Empty.to_string(), "empty");
        assert_eq!(SlotClass::Zero.to_string(), "zero");
        assert_eq!(SlotClass::Data.to_string(), "data");
    }

    #[test]
    fn validate_geometry_accepts_exact_length_only() {
        let header = BigHeader {
            header_size: 512,
            entry_count: 3,
        };
        let exact = expected_len(3);
        validate_geometry(exact, header).expect("exact geometry must pass");
        let short = validate_geometry(exact - 1, header).expect_err("one byte short must fail");
        assert!(matches!(
            short,
            TexturesError::SlotGeometryMismatch { file_len, count: 3, expected }
                if file_len == exact - 1 && expected == exact
        ));
        let long = validate_geometry(exact + 1, header).expect_err("one byte long must fail");
        let msg = long.to_string();
        assert!(
            msg.contains(&(exact + 1).to_string())
                && msg.contains(&exact.to_string())
                && msg.contains("refusing to clamp"),
            "got: {msg}"
        );
    }

    #[test]
    fn short_geometry_valid_container_passes_probes() {
        let path = write_temp("one-slot", &synthetic_textures(512, 1));
        let layout = describe(&path).expect("one-slot container must describe");
        assert_eq!(layout.header.entry_count, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn describe_rejects_geometry_mismatch() {
        let mut bytes = synthetic_textures(512, 2);
        bytes.push(0);
        let path = write_temp("one-byte-long", &bytes);
        assert!(matches!(
            describe(&path),
            Err(TexturesError::SlotGeometryMismatch { .. })
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn scan_slots_reports_every_slot_and_footer() {
        let classes = [
            SlotClass::Empty,
            SlotClass::Zero,
            SlotClass::Data,
            SlotClass::Data,
            SlotClass::Data,
            SlotClass::Data,
            SlotClass::Data,
            SlotClass::Data,
        ];
        let path = write_temp("eight-slots", &synthetic_slots(&classes));
        let header = BigHeader {
            header_size: 512,
            entry_count: 8,
        };
        let scan = scan_slots(&path, &header).expect("scan must succeed");
        assert_eq!(scan.classes, classes);
        assert_eq!(scan.count(SlotClass::Empty), 1);
        assert_eq!(scan.count(SlotClass::Zero), 1);
        assert_eq!(scan.count(SlotClass::Data), 6);
        let report = scan.report();
        assert!(
            report.starts_with("addressing: fixed 131072-byte slots (slot i at 4096 + i * 131072)"),
            "got: {report}"
        );
        assert!(report.contains("slots (whole-slot scan; no sampling):"));
        assert!(report.contains("slot 0 offset 4096 empty"));
        assert!(report.contains("slot 1 offset 135168 zero"));
        assert!(report.contains("slot 7 offset 921600 data"));
        assert!(report.contains("slots: 8 total, 1 empty, 1 zero, 6 data"));
        assert_eq!(report.lines().count(), 11, "got: {report}");
        let (offset, class) = slot_class_at(&path, &header, 1).expect("one slot must read");
        assert_eq!(offset, slot_offset(1));
        assert_eq!(class, SlotClass::Zero);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn no_sampling_last_byte_decides() {
        // Slot 1 is 0xFF in every byte except its last one -> data.
        let mut bytes = synthetic_slots(&[SlotClass::Empty, SlotClass::Data]);
        let start = slot_offset(1) as usize;
        bytes[start..start + SLOT_SIZE as usize].fill(0xFF);
        bytes[start + SLOT_SIZE as usize - 1] = 0x00;
        let path = write_temp("no-sampling", &bytes);
        let header = BigHeader {
            header_size: 512,
            entry_count: 2,
        };
        let scan = scan_slots(&path, &header).expect("scan must succeed");
        assert_eq!(scan.classes[1], SlotClass::Data);
        let (_, class) = slot_class_at(&path, &header, 1).expect("one slot must read");
        assert_eq!(class, SlotClass::Data);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn slot_errors_name_class_length_selector_and_range() {
        let empty = TexturesError::SlotNoPayload {
            index: 0,
            offset: 4096,
            class: SlotClass::Empty,
        }
        .to_string();
        assert!(
            empty.contains("slot 0 (offset 4096)")
                && empty.contains("is empty")
                && empty.contains("0xFF marker")
                && empty.contains("no payload to decode"),
            "got: {empty}"
        );
        let zero = TexturesError::SlotNoPayload {
            index: 1,
            offset: 135168,
            class: SlotClass::Zero,
        }
        .to_string();
        assert!(
            zero.contains("is zero") && zero.contains("0x00 fill") && zero.contains("no payload"),
            "got: {zero}"
        );
        for msg in [&empty, &zero] {
            assert!(
                !msg.contains("compressed") && !msg.contains("UNKNOWN"),
                "stale compression wording: {msg}"
            );
        }
        let length = TexturesError::SlotPayloadLength {
            index: 2,
            found: 131071,
            expected: 131072,
        }
        .to_string();
        assert!(
            length.contains("slot 2")
                && length.contains("131071")
                && length.contains("256x256")
                && length.contains("131072"),
            "got: {length}"
        );
        let selector = TexturesError::SlotSelectorInvalid {
            selector: "abc".to_string(),
            count: 8,
        }
        .to_string();
        assert!(
            selector.contains("'abc'") && selector.contains("0..8"),
            "got: {selector}"
        );
        let range = TexturesError::SlotIndexOutOfRange { index: 8, count: 8 }.to_string();
        assert!(
            range.contains("slot index 8") && range.contains("valid 0..8"),
            "got: {range}"
        );
    }

    #[test]
    fn decode_slot_at_reads_one_data_slot() {
        assert_eq!(slot_texture::SLOT_BYTES as u64, SLOT_SIZE);
        let path = write_temp(
            "decode-data",
            &synthetic_slots(&[SlotClass::Zero, SlotClass::Data]),
        );
        let header = BigHeader {
            header_size: 512,
            entry_count: 2,
        };
        let (offset, rgba) = decode_slot_at(&path, &header, 1).expect("data slot must decode");
        assert_eq!(offset, slot_offset(1));
        assert_eq!(rgba.len(), 262144);
        assert_eq!(
            rgba.len(),
            slot_texture::WIDTH * slot_texture::HEIGHT * 4
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn decode_slot_at_rejects_empty_and_zero_slots() {
        let path = write_temp(
            "decode-classes",
            &synthetic_slots(&[SlotClass::Empty, SlotClass::Zero, SlotClass::Data]),
        );
        let header = BigHeader {
            header_size: 512,
            entry_count: 3,
        };
        let empty = decode_slot_at(&path, &header, 0).expect_err("empty must not decode");
        assert!(
            matches!(
                empty,
                TexturesError::SlotNoPayload {
                    index: 0,
                    offset: 4096,
                    class: SlotClass::Empty
                }
            ),
            "got: {empty:?}"
        );
        let zero = decode_slot_at(&path, &header, 1).expect_err("zero must not decode");
        assert!(
            matches!(
                zero,
                TexturesError::SlotNoPayload {
                    index: 1,
                    class: SlotClass::Zero,
                    ..
                }
            ),
            "got: {zero:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn describe_parses_header_and_keeps_table_unknown() {
        let path = write_temp("valid-header", &synthetic_textures(512, 3));
        let layout = describe(&path).expect("valid header must describe");
        assert_eq!(layout.header.entry_count, 3);
        assert!(!layout.table_found);
        assert_eq!(layout.fill_class, RegionClass::FfFill);
        assert_eq!(layout.data_class, RegionClass::Data);
        let s = layout.summary();
        assert!(s.contains("BIG header: 512-byte header, 3 entries"), "got: {s}");
        assert!(
            s.contains("entry table: UNKNOWN (location not established; no stride inferred)"),
            "got: {s}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn describe_fails_loud_on_each_invalid_header_class() {
        let short = write_temp("short-100", &[0u8; 100]);
        assert!(matches!(
            describe(&short),
            Err(TexturesError::TruncatedHeader { len: 100 })
        ));
        let _ = std::fs::remove_file(&short);
        let wrong = write_temp("wrong-size", &synthetic_textures(256, 3));
        assert!(matches!(
            describe(&wrong),
            Err(TexturesError::WrongHeaderSize { found: 256 })
        ));
        let _ = std::fs::remove_file(&wrong);
        let zero = write_temp("zero-count", &synthetic_textures(512, 0));
        assert!(matches!(
            describe(&zero),
            Err(TexturesError::ImplausibleCount { count: 0 })
        ));
        let _ = std::fs::remove_file(&zero);
    }

    #[test]
    fn summary_carries_shapes_not_content() {
        let layout = TexturesLayout {
            file_len: 160 * 1024 * 1024,
            header: BigHeader {
                header_size: 512,
                entry_count: 1287,
            },
            header_class: RegionClass::Data,
            fill_class: RegionClass::FfFill,
            data_class: RegionClass::Data,
            table_found: false,
        };
        let s = layout.summary();
        assert!(s.contains("1287 entries") && s.contains("0xFF-fill"));
        assert!(s.contains("entry table: UNKNOWN"));
        assert!(!s.contains("167772160"), "exact length must not leak: {s}");
    }

    #[test]
    fn truncated_and_missing_files_fail_loud() {
        let path = write_temp("truncated", &[0x01, 0x02]);
        assert!(matches!(
            describe(&path),
            Err(TexturesError::TruncatedHeader { len: 2 })
        ));
        let _ = std::fs::remove_file(&path);
        let mut missing = std::env::temp_dir();
        missing.push(format!("textures-test-no-such-file-{}", std::process::id()));
        let _ = std::fs::remove_file(&missing);
        assert!(matches!(describe(&missing), Err(TexturesError::Io(_))));
    }

    #[test]
    fn table_hunt_reports_no_table_even_for_sane_tail() {
        // Sane tail word must NOT become a table: without observed fields
        // there is nothing to parse (refusing to guess, tested here).
        let mut tricky = synthetic_textures(512, 16);
        let tail = tricky.len() - 4;
        tricky[tail..].copy_from_slice(&16u32.to_le_bytes());
        let path = write_temp("sane-tail", &tricky);
        let len = tricky.len() as u64;
        assert_eq!(hunt_table(&path, len).expect("hunt must run"), false);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn error_displays_carry_details() {
        let msg = TexturesError::BadSelector("3".to_string()).to_string();
        assert!(msg.contains('3') && msg.contains("UNKNOWN"), "got: {msg}");
        assert!(TexturesError::NotDds("entry 0".into()).to_string().contains("entry 0"));
    }
}

//! Remaster `.BIG` header parser (`big-header`): observed CUSTOM layout
//! (NOT SR1PC — `bigfile.rs` must never parse it). Parses exactly bytes
//! `0..2` (`u16` header size, must be 512) and `2..4` (`u16` entry
//! count, must be `1..=32768`), then keeps bounded read-only probes.
//! Only classes/verdicts leave, never content. The entry index stays
//! UNKNOWN: no stride, no offset, no entry lines. `--tlist` prints the
//! header report and exits 0; `--tshow` stays loud (exit 1).
//! Synthetic bytes in tests only.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Sanity bound from the bigfile spec source (`MAX_ENTRIES = 32768`).
/// Used here as a guard on the parsed entry count — not a format claim.
pub const MAX_ENTRIES: u32 = 32768;

/// Verified `.BIG` header length: the size word must equal this value.
pub const HEADER_SIZE: u16 = 512;

/// Next candidate named by loud errors when no table is identified
/// (relative name only — never an install path).
pub const NEXT_CANDIDATE: &str = "2\\BIGFILE.DAT (2236-entry sibling, layout unverified)";

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
}

impl fmt::Display for TexturesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TexturesError::Io(e) => write!(f, "cannot read textures file: {e}"),
            TexturesError::NotTextures(msg) => write!(f, "not a TEXTURES container: {msg}"),
            TexturesError::BadEntry(msg) => write!(f, "bad textures entry: {msg}"),
            TexturesError::BadSelector(sel) => write!(
                f,
                "no textures entry matches selector '{sel}' (no entry table identified; {NEXT_CANDIDATE})"
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
    let _data_class = classify_window(&probe_window(&mut file, DATA_PROBE_OFF, file_len)?);
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
    let header_class = classify_window(&probe_window(&mut file, HEADER_PROBE_OFF, file_len)?);
    let fill_class = classify_window(&probe_window(&mut file, FILL_PROBE_OFF, file_len)?);
    let data_class = classify_window(&probe_window(&mut file, DATA_PROBE_OFF, file_len)?);
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

    /// Synthetic container: header words + `0xFF` fill from 4K + nonzero
    /// data from 1 MiB. All values invented — never game bytes.
    fn synthetic_textures(header_size: u16, count: u16) -> Vec<u8> {
        let mut b = vec![0u8; DATA_PROBE_OFF as usize + PROBE_LEN];
        b[0..4].copy_from_slice(&prefix(header_size, count));
        for byte in b[4096..].iter_mut() {
            *byte = 0xFF;
        }
        for (i, byte) in b[DATA_PROBE_OFF as usize..].iter_mut().enumerate() {
            *byte = (i % 251) as u8 + 1;
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
        assert!(msg.contains('3') && msg.contains("entry table"), "got: {msg}");
        assert!(TexturesError::NotDds("entry 0".into()).to_string().contains("entry 0"));
    }
}

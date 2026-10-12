//! Remaster `TEXTURES.BIG` probe (`textures-big`): observed CUSTOM layout
//! (NOT SR1PC — `bigfile.rs` must never parse it). Bounded read-only
//! probes; only classes/verdicts leave, never content. No table is
//! identified, so `--tlist` prints the summary and exits 1 loud.
//! Synthetic bytes in tests only.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Sanity bound from the bigfile spec source (`MAX_ENTRIES = 32768`).
/// Above it, a count word is not an SR1PC index — for this container
/// that rejection is the expected signal, not a failure.
pub const MAX_ENTRIES: u32 = 32768;

/// Next candidate named by loud errors when no table is identified
/// (relative name only — never an install path).
pub const NEXT_CANDIDATE: &str = "2\\BIGFILE.DAT (2236-entry sibling, layout unverified)";

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
/// `BadEntry`/`BadSelector`/`NotDds` are for the Task-3 `--tshow` arm.
#[allow(dead_code)]
#[derive(Debug)]
pub enum TexturesError {
    Io(std::io::Error),
    NotTextures(String),
    BadEntry(String),
    BadSelector(String),
    NotDds(String),
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
/// Task-2 hunt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexturesLayout {
    pub file_len: u64,
    pub count_word_sane: bool,
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

    /// `--tlist` no-table summary (stdout): verdict, size class, region
    /// classes, table status. No content, no raw offsets, no game paths.
    pub fn summary(&self) -> String {
        let count_verdict = if self.count_word_sane {
            "count-at-0 within sanity limit"
        } else {
            "count-at-0 exceeds sanity limit (not SR1PC)"
        };
        let table_verdict = if self.table_found {
            "table: identified"
        } else {
            "table: none identified"
        };
        format!(
            "textures layout: CUSTOM ({count_verdict})\nsize: {}\nheader window: {}\nfill window: {}\ndata window: {}\n{table_verdict}",
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
    let mut file = File::open(path).map_err(TexturesError::Io)?;
    let mut word = [0u8; 4];
    file.read_exact(&mut word).map_err(|_| {
        TexturesError::NotTextures(format!(
            "file holds {file_len} bytes, fewer than the 4-byte count word"
        ))
    })?;
    let count_word_sane = u32::from_le_bytes(word) <= MAX_ENTRIES;
    let header_class = classify_window(&probe_window(&mut file, HEADER_PROBE_OFF, file_len)?);
    let fill_class = classify_window(&probe_window(&mut file, FILL_PROBE_OFF, file_len)?);
    let data_class = classify_window(&probe_window(&mut file, DATA_PROBE_OFF, file_len)?);
    drop(file);
    let table_found = hunt_table(path, file_len)?;
    Ok(TexturesLayout {
        file_len,
        count_word_sane,
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

    /// Synthetic container: count word + `0xFF` fill from 4K + nonzero
    /// data from 1 MiB. All values invented — never game bytes.
    fn synthetic_textures(count: u32) -> Vec<u8> {
        let mut b = vec![0u8; DATA_PROBE_OFF as usize + PROBE_LEN];
        b[0..4].copy_from_slice(&count.to_le_bytes());
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
    fn insane_count_word_marks_custom_layout_not_failure() {
        let path = write_temp("insane-count", &synthetic_textures(u32::MAX));
        let layout = describe(&path).expect("insane count must still describe");
        assert!(!layout.count_word_sane);
        assert!(!layout.table_found);
        assert_eq!(layout.fill_class, RegionClass::FfFill);
        assert_eq!(layout.data_class, RegionClass::Data);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn summary_carries_shapes_not_content() {
        let layout = TexturesLayout {
            file_len: 160 * 1024 * 1024,
            count_word_sane: false,
            header_class: RegionClass::Data,
            fill_class: RegionClass::FfFill,
            data_class: RegionClass::Data,
            table_found: false,
        };
        let s = layout.summary();
        assert!(s.contains("CUSTOM") && s.contains("none identified") && s.contains("0xFF-fill"));
        assert!(!s.contains("167772160"), "exact length must not leak: {s}");
    }

    #[test]
    fn truncated_and_missing_files_fail_loud() {
        let path = write_temp("truncated", &[0x01, 0x02]);
        assert!(matches!(describe(&path), Err(TexturesError::NotTextures(_))));
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
        let mut tricky = synthetic_textures(u32::MAX);
        let tail = tricky.len() - 4;
        tricky[tail..].copy_from_slice(&16u32.to_le_bytes());
        let path = write_temp("sane-tail", &tricky);
        let len = tricky.len() as u64;
        assert_eq!(hunt_table(&path, len).expect("hunt must run"), false);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn error_displays_carry_details_and_candidate() {
        let msg = TexturesError::BadSelector("3".to_string()).to_string();
        assert!(msg.contains('3') && msg.contains("BIGFILE.DAT"), "got: {msg}");
        assert!(TexturesError::NotDds("entry 0".into()).to_string().contains("entry 0"));
    }
}

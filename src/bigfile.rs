//! SR1PC `BIGFILE.DAT` index parser + range-checked entry reader.
//!
//! One job: parse the archive index at byte 0 (`u32` LE entry count,
//! then 16-byte LE entries `[nameHash, length, offset, unknown]`) and
//! read one entry's bytes into process memory. Extracted bytes are
//! NEVER written to disk or repo — decode happens in memory at runtime.
//!
//! Layout facts (clean-room reimplementation of observed field layout;
//! see `docs/changes/bigfile-index/design.md` for the spec source +
//! license note): no global magic; raw byte offsets (`OffsetMultiplier
//! = 1`); little-endian; names are hashes only (the hash algorithm is
//! UNKNOWN — stored hashes are matched, never computed); the 4th index
//! dword is opaque and carried through uninterpreted.
//!
//! Failure policy (LOUD, never guess): bad count, truncated index, or
//! an entry range outside the file each yield [`BigfileError`] and the
//! caller exits nonzero. Tests use synthetic bytes only — never game
//! files.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Upper bound on the entry count, quoted from the spec source
/// (`Index.cs`: `MAX_ENTRIES = 32768`). A count above this is not a
/// bigfile index — loud error, no guessing.
pub const MAX_ENTRIES: u32 = 32768;

/// Byte size of one index entry: 4 x `u32` LE.
pub const ENTRY_BYTES: usize = 16;

/// One archive entry: the stored name hash plus the byte range
/// `[offset, offset + length)` of its data in the container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigfileEntry {
    pub name_hash: u32,
    pub offset: u64,
    pub length: u64,
}

/// Every way reading a bigfile index or entry can fail. Each variant
/// maps to a clear stderr message + nonzero exit in `main`.
#[derive(Debug)]
pub enum BigfileError {
    /// The archive could not be OPENED (missing path, permissions,
    /// ...); carries the path plus the OS detail. Pre-open stage only:
    /// mirrors the original loader's open-stage message family
    /// (`docs/notes/s1-bigfile-loader-behavior.md`, O1 CONFIRMED).
    OpenFailed { path: String, detail: String },
    /// File IO AFTER a successful open (seek, entry-byte reads);
    /// carries the OS error. Post-open routing in the original is
    /// UNKNOWN, so this keeps its own text (excluded per spec R3).
    Io(std::io::Error),
    /// Bytes at offset 0 are not a sane SR1PC index (bad count,
    /// truncated index table); carries a detail message.
    NotBigfile(String),
    /// An entry's `[offset, offset + length)` range lies outside the
    /// file; carries a detail message naming the entry.
    BadEntry(String),
    /// A `--show` selector matched no entry; carries the selector.
    BadSelector(String),
    /// An entry's bytes are not a DDS texture; carries a detail
    /// message naming the entry.
    NotDds(String),
}

impl fmt::Display for BigfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BigfileError::OpenFailed { path, detail } => {
                write!(f, "Can't load big file named \"{path}\" ({detail})")
            }
            BigfileError::Io(e) => write!(f, "cannot read bigfile: {e}"),
            BigfileError::NotBigfile(msg) => write!(f, "not a BIGFILE index: {msg}"),
            BigfileError::BadEntry(msg) => write!(f, "bad bigfile entry: {msg}"),
            BigfileError::BadSelector(sel) => write!(
                f,
                "no bigfile entry matches selector '{sel}' (use a decimal list index or an 8-hex-digit name hash from --list)"
            ),
            BigfileError::NotDds(msg) => write!(f, "not a DDS entry: {msg}"),
        }
    }
}

impl std::error::Error for BigfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BigfileError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Build the pre-open failure for `path`: the original loader's
/// open-stage message family with our OS detail kept as the suffix.
fn open_failed(path: &Path, e: &std::io::Error) -> BigfileError {
    BigfileError::OpenFailed {
        path: path.display().to_string(),
        detail: e.to_string(),
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Parse the SR1PC index from the start of `bytes` (`file_len` is the
/// real container size, used to validate every range).
///
/// Layout: `u32` LE entry count at offset 0 (rejected when
/// `> MAX_ENTRIES` or when the table would overrun `file_len`), then
/// `count` 16-byte LE entries `[nameHash, length, offset, unknown]`.
/// The 4th dword is read and ignored (opaque per the design UNKNOWN
/// rule). Every entry's `offset + length` is checked against
/// `file_len` in `u64` arithmetic (overflow-safe: `u32` values cannot
/// overflow `u64`, and any sum past EOF is rejected).
pub fn parse_index(bytes: &[u8], file_len: u64) -> Result<Vec<BigfileEntry>, BigfileError> {
    let count = u32_at(bytes, 0).ok_or_else(|| {
        BigfileError::NotBigfile(format!(
            "file has {file_len} bytes, fewer than the 4-byte entry count"
        ))
    })?;
    if count > MAX_ENTRIES {
        return Err(BigfileError::NotBigfile(format!(
            "entry count {count} exceeds the {MAX_ENTRIES} sanity limit"
        )));
    }
    let table_len = 4u64 + u64::from(count) * ENTRY_BYTES as u64;
    if table_len > file_len {
        return Err(BigfileError::NotBigfile(format!(
            "index table needs {table_len} bytes for {count} entries, file has {file_len}"
        )));
    }
    if (bytes.len() as u64) < table_len {
        return Err(BigfileError::NotBigfile(format!(
            "truncated index: need {table_len} bytes for {count} entries, buffer has {}",
            bytes.len()
        )));
    }
    let mut entries = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let base = 4 + i * ENTRY_BYTES;
        // `u32_at` cannot fail here: `table_len` was verified above.
        let name_hash = u32_at(bytes, base).unwrap_or(0);
        let length = u32_at(bytes, base + 4).unwrap_or(0);
        let offset = u32_at(bytes, base + 8).unwrap_or(0);
        let end = u64::from(offset) + u64::from(length);
        if end > file_len {
            return Err(BigfileError::BadEntry(format!(
                "entry {i} (hash {name_hash:08X}) range [{offset}, {end}) lies past EOF ({file_len} bytes)"
            )));
        }
        entries.push(BigfileEntry {
            name_hash,
            offset: u64::from(offset),
            length: u64::from(length),
        });
    }
    Ok(entries)
}

/// Parse the index of the container at `path`: read the count word,
/// sanity-check it, then buffer exactly the index table and hand it
/// to [`parse_index`]. Only index bytes are read — entry data stays
/// on disk until [`read_entry`] is asked for one entry.
pub fn load_index(path: &Path) -> Result<Vec<BigfileEntry>, BigfileError> {
    let file_len = std::fs::metadata(path)
        .map_err(|e| open_failed(path, &e))?
        .len();
    let mut file = File::open(path).map_err(|e| open_failed(path, &e))?;
    let mut count_buf = [0u8; 4];
    file.read_exact(&mut count_buf).map_err(|_| {
        BigfileError::NotBigfile(format!(
            "file '{}' has {file_len} bytes, fewer than the 4-byte entry count",
            path.display()
        ))
    })?;
    let count = u32::from_le_bytes(count_buf);
    if count > MAX_ENTRIES {
        return Err(BigfileError::NotBigfile(format!(
            "entry count {count} exceeds the {MAX_ENTRIES} sanity limit"
        )));
    }
    let table_len = 4u64 + u64::from(count) * ENTRY_BYTES as u64;
    if table_len > file_len {
        return Err(BigfileError::NotBigfile(format!(
            "index table needs {table_len} bytes for {count} entries, file has {file_len}"
        )));
    }
    let mut table = vec![0u8; table_len as usize];
    file.rewind()
        .map_err(|e| BigfileError::NotBigfile(format!("cannot rewind index: {e}")))?;
    file.read_exact(&mut table)
        .map_err(|_| BigfileError::NotBigfile("truncated index table".to_string()))?;
    parse_index(&table, file_len)
}

/// Read exactly `[entry.offset, entry.offset + entry.length)` into
/// process memory. The range is pre-validated against the real file
/// size (an out-of-range entry is [`BigfileError::BadEntry`], never a
/// panic or a short read presented as success). Nothing is written.
pub fn read_entry(path: &Path, entry: &BigfileEntry) -> Result<Vec<u8>, BigfileError> {
    let file_len = std::fs::metadata(path)
        .map_err(|e| open_failed(path, &e))?
        .len();
    let end = entry.offset + entry.length;
    if end > file_len {
        return Err(BigfileError::BadEntry(format!(
            "entry (hash {:08X}) range [{}, {end}) lies past EOF ({file_len} bytes)",
            entry.name_hash, entry.offset
        )));
    }
    let length = usize::try_from(entry.length).map_err(|_| {
        BigfileError::BadEntry(format!("entry length {} does not fit memory", entry.length))
    })?;
    let mut file = File::open(path).map_err(|e| open_failed(path, &e))?;
    file.seek(SeekFrom::Start(entry.offset))
        .map_err(BigfileError::Io)?;
    let mut buf = vec![0u8; length];
    file.read_exact(&mut buf).map_err(BigfileError::Io)?;
    Ok(buf)
}

/// Resolve a `--show` selector against `entries`: a decimal list index
/// (what `--list` prints leftmost, copy-paste friendly) or an
/// 8-hex-digit name hash (case-insensitive, optional `0x` prefix,
/// stable across listings).
///
/// Disambiguation: an all-digit selector that is a valid in-range
/// index wins (so `5` means entry 5, not hash `0x00000005`); anything
/// else is tried as hex and matched against stored hashes — hashes are
/// NEVER computed, only compared. Unknown selectors, out-of-range
/// indexes, and unmatched hashes are [`BigfileError::BadSelector`].
pub fn resolve_selector(entries: &[BigfileEntry], selector: &str) -> Result<usize, BigfileError> {
    let trimmed = selector.trim();
    let (hex_forced, hex_digits) = match trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        Some(rest) => (true, rest),
        None => (false, trimmed),
    };
    if !hex_forced
        && !hex_digits.is_empty()
        && hex_digits.bytes().all(|b| b.is_ascii_digit())
        && let Ok(index) = hex_digits.parse::<usize>()
        && index < entries.len()
    {
        return Ok(index);
    }
    if let Ok(hash) = u32::from_str_radix(hex_digits, 16)
        && let Some(index) = entries.iter().position(|e| e.name_hash == hash)
    {
        return Ok(index);
    }
    Err(BigfileError::BadSelector(selector.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal synthetic container: `u32` LE count + `entries` of
    /// `[hash, length, offset, unknown]`, followed by `total_len`
    /// bytes of filler so `file_len` validation has room. All values
    /// invented here — never game bytes.
    fn synthetic_container(entries: &[(u32, u32, u32)], total_len: usize) -> Vec<u8> {
        let mut b = vec![0u8; total_len.max(4 + entries.len() * ENTRY_BYTES)];
        b[0..4].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        for (i, (hash, length, offset)) in entries.iter().enumerate() {
            let base = 4 + i * ENTRY_BYTES;
            b[base..base + 4].copy_from_slice(&hash.to_le_bytes());
            b[base + 4..base + 8].copy_from_slice(&length.to_le_bytes());
            b[base + 8..base + 12].copy_from_slice(&offset.to_le_bytes());
            b[base + 12..base + 16].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        }
        b
    }

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("bigfile-test-{name}-{}", std::process::id()));
        std::fs::write(&p, bytes).expect("test temp file must be writable");
        p
    }

    #[test]
    fn parses_single_entry_with_exact_fields() {
        let bytes = synthetic_container(&[(0x1234_ABCD, 100, 500)], 1024);
        let entries = parse_index(&bytes, 1024).expect("synthetic index must parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            BigfileEntry {
                name_hash: 0x1234_ABCD,
                offset: 500,
                length: 100,
            }
        );
    }

    #[test]
    fn parses_multi_entry_table_in_order() {
        let bytes = synthetic_container(
            &[(0xAAAA_AAAA, 10, 100), (0xBBBB_BBBB, 20, 200), (0xCCCC_CCCC, 0, 300)],
            512,
        );
        let entries = parse_index(&bytes, 512).expect("3-entry index must parse");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1].name_hash, 0xBBBB_BBBB);
        assert_eq!(entries[1].offset, 200);
        assert_eq!(entries[1].length, 20);
        // Zero-length entries are legal (empty range at a valid offset).
        assert_eq!(entries[2].length, 0);
    }

    #[test]
    fn empty_archive_parses_to_zero_entries() {
        let bytes = synthetic_container(&[], 64);
        let entries = parse_index(&bytes, 64).expect("count 0 must parse");
        assert!(entries.is_empty());
    }

    #[test]
    fn count_above_sanity_limit_is_rejected() {
        let mut bytes = vec![0u8; 8];
        bytes[0..4].copy_from_slice(&(MAX_ENTRIES + 1).to_le_bytes());
        let err = parse_index(&bytes, 8).expect_err("insane count must fail loud");
        assert!(
            err.to_string().starts_with("not a BIGFILE index:"),
            "S2 HELD row (no count-word reader edge): own text stays, got: {err}"
        );
    }

    #[test]
    fn truncated_count_word_is_rejected() {
        for short in [vec![0x01, 0x02], vec![]] {
            let len = short.len() as u64;
            let err = parse_index(&short, len).expect_err("short count must fail loud");
            assert!(
                err.to_string().starts_with("not a BIGFILE index:"),
                "S2 HELD row (no count-word reader edge): own text stays, got: {err}"
            );
        }
    }

    #[test]
    fn truncated_index_table_is_rejected() {
        // Declares 2 entries but the buffer holds only 1.
        let full = synthetic_container(&[(1, 10, 100), (2, 10, 200)], 512);
        let cut = &full[..4 + ENTRY_BYTES];
        let err = parse_index(cut, 512).expect_err("cut table must fail loud");
        assert!(
            err.to_string().starts_with("not a BIGFILE index:"),
            "UNKNOWN-mapping hold: own text stays (spec R3), got: {err}"
        );
        // Table would overrun the real file size: loud, no guessing.
        assert!(matches!(
            parse_index(&full, 10),
            Err(BigfileError::NotBigfile(_))
        ));
    }

    #[test]
    fn entry_range_past_eof_is_rejected() {
        let bytes = synthetic_container(&[(0x1111_1111, 100, 900)], 1024);
        match parse_index(&bytes, 950) {
            Err(BigfileError::BadEntry(msg)) => {
                assert!(msg.contains('0'), "message must name the entry, got: {msg}");
                assert!(
                    BigfileError::BadEntry(msg)
                        .to_string()
                        .starts_with("bad bigfile entry:"),
                    "UNKNOWN-mapping hold: own text stays (spec R3)"
                );
            }
            other => panic!("expected BadEntry, got: {other:?}"),
        }
    }

    #[test]
    fn offset_plus_length_u32_overflow_is_rejected() {
        // offset u32::MAX + length 1 fits u64 but cannot fit the file.
        let bytes = synthetic_container(&[(0x2222_2222, 1, u32::MAX)], 4096);
        assert!(matches!(
            parse_index(&bytes, 4096),
            Err(BigfileError::BadEntry(_))
        ));
    }

    #[test]
    fn read_entry_returns_exact_range_and_nothing_more() {
        let mut bytes = synthetic_container(&[(0x3333_3333, 8, 64)], 128);
        bytes[64..72].copy_from_slice(b"ABCDEFGH");
        let path = write_temp("read-exact", &bytes);
        let entries = load_index(&path).expect("synthetic file must load");
        assert_eq!(entries.len(), 1);
        let data = read_entry(&path, &entries[0]).expect("in-range read must succeed");
        assert_eq!(data, b"ABCDEFGH");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_entry_past_eof_is_loud_not_a_short_read() {
        let bytes = synthetic_container(&[(0x4444_4444, 8, 64)], 128);
        let path = write_temp("read-past-eof", &bytes);
        // Shrink the file AFTER parsing so the entry range overruns EOF.
        std::fs::write(&path, &bytes[..70]).expect("truncate must succeed");
        let bad = BigfileEntry {
            name_hash: 0x4444_4444,
            offset: 64,
            length: 8,
        };
        let err = read_entry(&path, &bad).expect_err("past-EOF read must fail loud");
        assert!(
            err.to_string().starts_with("bad bigfile entry:"),
            "S2 HELD row (no bounds-check edge): own text stays, got: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_index_table_stage_messages_are_held() {
        // Declares 2 entries but the file is physically shorter than the
        // table: the on-disk table stage fails loud with our own prefix.
        let full = synthetic_container(&[(1, 10, 100), (2, 10, 200)], 512);
        let path = write_temp("index-table-short", &full[..10]);
        let err = load_index(&path).expect_err("short table file must fail loud");
        assert!(
            err.to_string().starts_with("not a BIGFILE index:"),
            "S2 HELD row (no table-reader edge): own text stays, got: {err}"
        );
        let _ = std::fs::remove_file(&path);
        // Fewer than 4 bytes: not even the count word is present.
        let tiny = write_temp("index-table-tiny", &[0x01, 0x02]);
        let err = load_index(&tiny).expect_err("tiny file must fail loud");
        assert!(
            err.to_string().starts_with("not a BIGFILE index:"),
            "S2 HELD row (no count-word reader edge): own text stays, got: {err}"
        );
        let _ = std::fs::remove_file(&tiny);
    }

    #[test]
    fn short_read_maps_to_held_io_prefix() {
        // A real OS short read (past EOF) surfaces through the held
        // post-open IO variant — never as a silent short buffer.
        let path = write_temp("short-read", b"ABCDEFGH");
        let mut file = File::open(&path).expect("synthetic file must open");
        file.seek(SeekFrom::Start(6)).expect("seek must succeed");
        let mut buf = [0u8; 8];
        let io_err = file.read_exact(&mut buf).expect_err("read past EOF must fail");
        let held = BigfileError::Io(io_err);
        assert!(
            held.to_string().starts_with("cannot read bigfile:"),
            "S2 HELD rows (no seek/read-cluster edge): own text stays, got: {held}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_index_on_missing_file_is_open_failed() {
        let mut p = std::env::temp_dir();
        p.push(format!("bigfile-test-no-such-file-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        assert!(matches!(load_index(&p), Err(BigfileError::OpenFailed { .. })));
    }

    #[test]
    fn open_failed_display_uses_load_prefix_naming_path() {
        // Parity pin for the ONE confirmed row (spec O1): fails if the
        // Task 3 prefix is reverted. Synthetic absence only — no game file.
        let mut p = std::env::temp_dir();
        p.push(format!("bigfile-test-no-such-file-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let err = load_index(&p).expect_err("missing file must fail loud");
        let msg = err.to_string();
        assert!(
            msg.starts_with("Can't load big file named \""),
            "open failure must use the load-failure prefix, got: {msg}"
        );
        assert!(
            msg.contains(&p.display().to_string()),
            "open failure must name the path, got: {msg}"
        );
    }

    #[test]
    fn read_entry_on_missing_file_is_open_failed() {
        let bad = BigfileEntry {
            name_hash: 0x6666_6666,
            offset: 64,
            length: 8,
        };
        let mut p = std::env::temp_dir();
        p.push(format!("bigfile-test-no-such-read-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        assert!(matches!(read_entry(&p, &bad), Err(BigfileError::OpenFailed { .. })));
    }

    #[test]
    fn selector_decimal_index_wins_when_in_range() {
        let bytes = synthetic_container(&[(0xAAAA_AAAA, 10, 100), (0xBBBB_BBBB, 20, 200)], 512);
        let entries = parse_index(&bytes, 512).unwrap();
        assert_eq!(resolve_selector(&entries, "0").unwrap(), 0);
        assert_eq!(resolve_selector(&entries, "1").unwrap(), 1);
        assert_eq!(resolve_selector(&entries, " 1 ").unwrap(), 1);
    }

    #[test]
    fn selector_hash_forms_match_stored_hashes() {
        let bytes = synthetic_container(&[(0x1234_ABCD, 10, 100), (0xBBBB_BBBB, 20, 200)], 512);
        let entries = parse_index(&bytes, 512).unwrap();
        assert_eq!(resolve_selector(&entries, "1234ABCD").unwrap(), 0);
        assert_eq!(resolve_selector(&entries, "1234abcd").unwrap(), 0);
        assert_eq!(resolve_selector(&entries, "0x1234ABCD").unwrap(), 0);
        assert_eq!(resolve_selector(&entries, "0XBBBBBBBB").unwrap(), 1);
    }

    #[test]
    fn selector_out_of_range_index_unknown_hash_and_garbage_fail_loud() {
        let bytes = synthetic_container(&[(0x1234_ABCD, 10, 100)], 512);
        let entries = parse_index(&bytes, 512).unwrap();
        for bad in ["7", "DEADBEEF", "0x12345678", "not-a-hash!", "", "0x", "-1"] {
            assert!(
                matches!(resolve_selector(&entries, bad), Err(BigfileError::BadSelector(_))),
                "selector '{bad}' must fail loud"
            );
        }
        let msg = BigfileError::BadSelector("xyz".to_string()).to_string();
        assert!(msg.contains("xyz"), "Display must echo the selector, got: {msg}");
    }

    #[test]
    fn all_digit_hash_outside_index_range_falls_through_to_hash_match() {
        // "12345678" is all digits but far past the index range: it must
        // be tried as a hash, not rejected as an index.
        let bytes = synthetic_container(&[(0x1234_5678, 10, 100)], 64);
        let entries = parse_index(&bytes, 0x2000_0000).unwrap();
        assert_eq!(resolve_selector(&entries, "12345678").unwrap(), 0);
    }

    #[test]
    fn error_displays_carry_details() {
        assert!(
            BigfileError::NotBigfile("count 99999".to_string())
                .to_string()
                .contains("99999")
        );
        assert!(
            BigfileError::BadEntry("entry 3".to_string())
                .to_string()
                .contains("entry 3")
        );
        assert!(
            BigfileError::NotDds("entry 0".to_string())
                .to_string()
                .contains("entry 0")
        );
        // UNKNOWN-mapping hold: post-open seek/read routing in the
        // original is unconfirmed, so post-open IO keeps our own text.
        let io = BigfileError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "failed to fill whole buffer",
        ));
        assert!(
            io.to_string().starts_with("cannot read bigfile:"),
            "hold prefix must survive, got: {io}"
        );
    }
}

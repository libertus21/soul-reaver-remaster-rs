//! `TEX_HD.MAP` slot-name aliases (`slot-name`): the remaster's fixed
//! 384,000-byte matrix — 6,000 rows × 64 bytes, one row per container slot,
//! 16 little-endian `u32` alias codes per row. `0xFFFFFFFF` means "no
//! reference"; every other code decodes to the engine's numeric asset name
//! (`<id>_<sub>`, `M\<id>_<sub>`, `B\<id>_<sub>`). Block bases are
//! `TEXTURES` 0, `MOVIES` 2,000, `BONUS` 4,000; `E3` has no block.
//!
//! Pure module: it takes bytes and returns values, never touching the
//! filesystem. The exact-size guard turns a different layout into a loud
//! no-go instead of a silent misparse. Synthetic bytes in tests only; no
//! game data lives here.

use std::fmt;

/// Exact map length: 6,000 rows × 64 bytes.
pub const MAP_BYTES: usize = 384_000;

/// Rows in the alias matrix.
pub const MAP_ROWS: usize = 6_000;

/// One row per slot: 16 × `u32`.
pub const ROW_BYTES: usize = 64;

/// Alias codes per row.
pub const ROW_CODES: usize = 16;

/// Slots per container block.
pub const GROUP_ROWS: usize = 2_000;

/// "No reference" for an alias code.
pub const SENTINEL: u32 = u32::MAX;

/// Container block that owns rows `base_row() .. base_row() + GROUP_ROWS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Textures,
    Movies,
    Bonus,
}

impl Group {
    /// Case-insensitive file-stem match: `TEXTURES` / `MOVIES` / `BONUS`.
    pub fn from_container_stem(stem: &str) -> Option<Group> {
        match stem.to_ascii_uppercase().as_str() {
            "TEXTURES" => Some(Group::Textures),
            "MOVIES" => Some(Group::Movies),
            "BONUS" => Some(Group::Bonus),
            _ => None,
        }
    }

    /// First row of this block in the matrix.
    pub fn base_row(self) -> usize {
        match self {
            Group::Textures => 0,
            Group::Movies => 2_000,
            Group::Bonus => 4_000,
        }
    }

    /// Report label (the container stem).
    pub fn label(self) -> &'static str {
        match self {
            Group::Textures => "TEXTURES",
            Group::Movies => "MOVIES",
            Group::Bonus => "BONUS",
        }
    }
}

/// Name bank encoded by a code's high range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    Root,
    M,
    B,
}

/// One decoded asset name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureName {
    pub bank: Bank,
    pub id: u32,
    pub sub: u32,
}

impl TextureName {
    /// Decode one alias code: sentinel → `None`; `< 32,000` root;
    /// `32,000..64,000` `M\`; `>= 64,000` `B\`. Integer division and
    /// remainder. `0` is a valid code (`0_0`), not a terminator.
    pub fn from_code(code: u32) -> Option<TextureName> {
        let (bank, offset) = match code {
            SENTINEL => return None,
            0..=31_999 => (Bank::Root, code),
            32_000..=63_999 => (Bank::M, code - 32_000),
            _ => (Bank::B, code - 64_000),
        };
        Some(TextureName {
            bank,
            id: offset / 16,
            sub: offset % 16,
        })
    }

    /// Display form: `<id>_<sub>`, `M\<id>_<sub>`, or `B\<id>_<sub>`.
    pub fn display(&self) -> String {
        match self.bank {
            Bank::Root => format!("{}_{}", self.id, self.sub),
            Bank::M => format!("M\\{}_{}", self.id, self.sub),
            Bank::B => format!("B\\{}_{}", self.id, self.sub),
        }
    }

    /// Parse the spec grammar: optional case-insensitive `B\` / `M\`,
    /// then ASCII digits `_` ASCII digits, nothing else.
    pub fn parse(s: &str) -> Option<TextureName> {
        let (bank, rest) = match s.get(0..2) {
            Some(prefix) if prefix.eq_ignore_ascii_case("B\\") => (Bank::B, &s[2..]),
            Some(prefix) if prefix.eq_ignore_ascii_case("M\\") => (Bank::M, &s[2..]),
            _ => (Bank::Root, s),
        };
        let (id_text, sub_text) = rest.split_once('_')?;
        Some(TextureName {
            bank,
            id: parse_digits(id_text)?,
            sub: parse_digits(sub_text)?,
        })
    }
}

/// Strict decimal digits: rejects `+`, signs, whitespace, and overflow.
fn parse_digits(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Loud map failures: a wrong layout or a row outside the block.
#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    WrongLength { found: usize, expected: usize },
    SlotOutOfCapacity { slot: u16, group: Group },
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::WrongLength { found, expected } => write!(
                f,
                "TEX_HD.MAP is {found} bytes, expected {expected} ({MAP_ROWS} rows x {ROW_BYTES} bytes)"
            ),
            MapError::SlotOutOfCapacity { slot, group } => write!(
                f,
                "slot {slot} has no name row in the {} block (capacity {GROUP_ROWS})",
                group.label()
            ),
        }
    }
}

impl std::error::Error for MapError {}

/// Borrowed, length-validated map. Row access is bounds-checked; the
/// caller owns the bytes so all I/O stays in `main.rs`.
#[derive(Debug, Clone, Copy)]
pub struct TextureMap<'a> {
    bytes: &'a [u8],
}

impl<'a> TextureMap<'a> {
    /// Accept exactly [`MAP_BYTES`]; nothing is padded, truncated, or
    /// partially read.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, MapError> {
        if bytes.len() == MAP_BYTES {
            Ok(TextureMap { bytes })
        } else {
            Err(MapError::WrongLength {
                found: bytes.len(),
                expected: MAP_BYTES,
            })
        }
    }

    /// The 16 raw codes of `slot` in `group`.
    pub fn codes(&self, group: Group, slot: u16) -> Result<[u32; ROW_CODES], MapError> {
        if usize::from(slot) >= GROUP_ROWS {
            return Err(MapError::SlotOutOfCapacity { slot, group });
        }
        let start = (group.base_row() + usize::from(slot)) * ROW_BYTES;
        let mut codes = [0u32; ROW_CODES];
        for (i, chunk) in self.bytes[start..start + ROW_BYTES]
            .chunks_exact(4)
            .enumerate()
        {
            codes[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        Ok(codes)
    }

    /// Decoded names of `slot`, deduplicated in first-seen order. An
    /// all-sentinel row yields an empty vector.
    pub fn names(&self, group: Group, slot: u16) -> Result<Vec<TextureName>, MapError> {
        let mut names: Vec<TextureName> = Vec::new();
        for code in self.codes(group, slot)? {
            if let Some(name) = TextureName::from_code(code) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Ok(names)
    }

    /// Slots in `0..slot_count` (capped at the block capacity) that own
    /// `name`, in ascending order. Never guesses: every match is returned.
    pub fn slots_for_name(
        &self,
        group: Group,
        slot_count: u16,
        name: &TextureName,
    ) -> Vec<u16> {
        let limit = usize::from(slot_count).min(GROUP_ROWS);
        (0..limit)
            .filter(|slot| {
                self.names(group, *slot as u16)
                    .map(|names| names.iter().any(|candidate| candidate == name))
                    .unwrap_or(false)
            })
            .map(|slot| slot as u16)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All-sentinel map: every row is "no reference" until overridden.
    fn synthetic_map() -> Vec<u8> {
        vec![0xFF; MAP_BYTES]
    }

    fn put_row(map: &mut [u8], row: usize, codes: &[u32]) {
        let start = row * ROW_BYTES;
        for i in 0..ROW_CODES {
            let code = codes.get(i).copied().unwrap_or(SENTINEL);
            let at = start + i * 4;
            map[at..at + 4].copy_from_slice(&code.to_le_bytes());
        }
    }

    fn names_of(codes: &[u32]) -> Vec<String> {
        codes
            .iter()
            .filter_map(|code| TextureName::from_code(*code))
            .map(|name| name.display())
            .collect()
    }

    #[test]
    fn codec_boundaries_follow_the_contract() {
        let cases = [
            (0u32, "0_0"),
            (31_999, "1999_15"),
            (32_000, "M\\0_0"),
            (63_999, "M\\1999_15"),
            (64_000, "B\\0_0"),
        ];
        for (code, expected) in cases {
            let name = TextureName::from_code(code).expect("non-sentinel code must decode");
            assert_eq!(name.display(), expected, "code {code}");
        }
        assert_eq!(TextureName::from_code(SENTINEL), None);
    }

    #[test]
    fn parse_and_display_round_trip() {
        for text in ["100_10", "4_0", "M\\9_2", "B\\262_1", "B\\2_1"] {
            let name = TextureName::parse(text).expect("valid name must parse");
            assert_eq!(name.display(), text, "input {text}");
        }
        let lower = TextureName::parse("m\\9_2").expect("bank is case-insensitive");
        assert_eq!(lower.bank, Bank::M);
        assert_eq!(lower.display(), "M\\9_2");
        let lower_b = TextureName::parse("b\\262_1").expect("bank is case-insensitive");
        assert_eq!(lower_b.bank, Bank::B);
    }

    #[test]
    fn parse_rejects_every_non_name_shape() {
        for text in [
            "abc", "100", "_10", "100_", "B/1_2", "B\\1_2_x", "x\\1_2", "", "+1_2", " 1_2",
            "1_2 ", "B\\", "M\\_1", "1__2",
        ] {
            assert_eq!(TextureName::parse(text), None, "must reject {text:?}");
        }
    }

    #[test]
    fn group_selection_matches_stems_case_insensitively() {
        assert_eq!(
            Group::from_container_stem("TEXTURES"),
            Some(Group::Textures)
        );
        assert_eq!(
            Group::from_container_stem("textures"),
            Some(Group::Textures)
        );
        assert_eq!(Group::from_container_stem("Movies"), Some(Group::Movies));
        assert_eq!(Group::from_container_stem("BONUS"), Some(Group::Bonus));
        assert_eq!(Group::from_container_stem("E3"), None);
        assert_eq!(Group::Textures.base_row(), 0);
        assert_eq!(Group::Movies.base_row(), 2_000);
        assert_eq!(Group::Bonus.base_row(), 4_000);
        assert_eq!(Group::Textures.label(), "TEXTURES");
        assert_eq!(Group::Movies.label(), "MOVIES");
        assert_eq!(Group::Bonus.label(), "BONUS");
    }

    #[test]
    fn wrong_lengths_are_loud_and_exact() {
        for len in [MAP_BYTES - 1, MAP_BYTES + 1] {
            let err = TextureMap::parse(&vec![0u8; len]).expect_err("wrong length must fail");
            assert_eq!(
                err,
                MapError::WrongLength {
                    found: len,
                    expected: MAP_BYTES
                }
            );
            let msg = err.to_string();
            assert!(
                msg.contains(&len.to_string())
                    && msg.contains("384000")
                    && msg.contains("6000 rows x 64 bytes"),
                "got: {msg}"
            );
        }
        assert_eq!(TextureMap::parse(&vec![0u8; MAP_BYTES]).is_ok(), true);
    }

    #[test]
    fn a_row_decodes_in_order_with_dedup() {
        let mut map = synthetic_map();
        put_row(&mut map, 0, &[0, 32_000, 64_000, SENTINEL, 0, 0]);
        let map = TextureMap::parse(&map).expect("exact length must parse");
        let names = map.names(Group::Textures, 0).expect("row must read");
        let shown: Vec<String> = names.iter().map(TextureName::display).collect();
        assert_eq!(shown, ["0_0", "M\\0_0", "B\\0_0"]);
    }

    #[test]
    fn all_sentinel_row_has_no_names() {
        let map_bytes = synthetic_map();
        let map = TextureMap::parse(&map_bytes).expect("exact length must parse");
        assert!(map.names(Group::Textures, 0).expect("row must read").is_empty());
    }

    #[test]
    fn group_bases_hit_different_rows() {
        let mut map = synthetic_map();
        put_row(&mut map, 0, &[16]);
        put_row(&mut map, 2_000, &[32]);
        put_row(&mut map, 4_000, &[48]);
        let map = TextureMap::parse(&map).expect("exact length must parse");
        assert_eq!(names_of(&map.codes(Group::Textures, 0).unwrap()), ["1_0"]);
        assert_eq!(names_of(&map.codes(Group::Movies, 0).unwrap()), ["2_0"]);
        assert_eq!(names_of(&map.codes(Group::Bonus, 0).unwrap()), ["3_0"]);
        // Slot 2,000 of TEXTURES lands on the MOVIES block's first row.
        assert_eq!(
            map.codes(Group::Textures, 2_000),
            Err(MapError::SlotOutOfCapacity {
                slot: 2_000,
                group: Group::Textures
            })
        );
    }

    #[test]
    fn out_of_capacity_rows_are_loud_never_adjacent_reads() {
        let map_bytes = synthetic_map();
        let map = TextureMap::parse(&map_bytes).expect("exact length must parse");
        for group in [Group::Textures, Group::Movies, Group::Bonus] {
            for slot in [2_000u16, 5_999, 6_000] {
                let expected = MapError::SlotOutOfCapacity { slot, group };
                assert_eq!(map.codes(group, slot), Err(expected));
            }
        }
        let msg = MapError::SlotOutOfCapacity {
            slot: 2_000,
            group: Group::Movies,
        }
        .to_string();
        assert!(
            msg.contains("slot 2000")
                && msg.contains("MOVIES")
                && msg.contains("capacity 2000"),
            "got: {msg}"
        );
    }

    #[test]
    fn slots_for_name_returns_every_match_ascending() {
        let mut map = synthetic_map();
        put_row(&mut map, 3, &[5, 5]);
        put_row(&mut map, 1, &[5]);
        put_row(&mut map, 7, &[6]);
        let map = TextureMap::parse(&map).expect("exact length must parse");
        let name = TextureName::parse("0_5").expect("valid name");
        assert_eq!(map.slots_for_name(Group::Textures, 8, &name), [1, 3]);
        let other = TextureName::parse("0_6").expect("valid name");
        assert_eq!(map.slots_for_name(Group::Textures, 8, &other), [7]);
        let absent = TextureName::parse("9_9").expect("valid name");
        assert!(map.slots_for_name(Group::Textures, 8, &absent).is_empty());
        // The scan is capped at the block capacity, never beyond it.
        assert_eq!(
            map.slots_for_name(Group::Textures, u16::MAX, &name),
            [1, 3]
        );
    }

    #[test]
    fn movies_and_bonus_banks_resolve_in_their_own_blocks() {
        let mut map = synthetic_map();
        put_row(&mut map, 2_000, &[64_000]);
        put_row(&mut map, 4_000, &[32_000]);
        let map = TextureMap::parse(&map).expect("exact length must parse");
        let bonus_name = TextureName::parse("B\\0_0").expect("valid name");
        assert_eq!(
            map.slots_for_name(Group::Movies, 29, &bonus_name),
            [0]
        );
        assert!(map
            .slots_for_name(Group::Textures, 29, &bonus_name)
            .is_empty());
        let m_name = TextureName::parse("M\\0_0").expect("valid name");
        assert_eq!(map.slots_for_name(Group::Bonus, 268, &m_name), [0]);
    }
}

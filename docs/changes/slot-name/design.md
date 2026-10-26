# Design: slot-name

Expose the names the engine already derives from `TEX_HD.MAP`. The map is a
fixed 6,000-row × 64-byte matrix; row = container slot, 16 `u32` alias codes,
each code → `<id>_<sub>` with optional `M\` / `B\` bank. The work is one pure
decoder module plus a small CLI extension (`--map`), with name-to-slot
resolution that refuses to guess when a name is shared.

## Technical approach

`src/tex_hd_map.rs` owns the map format only: constants, exact-length guard,
container→group selection, row lookup, code→name decoding, name parsing, and a
scan to find the slots that own a name. It takes bytes and returns values; no
file I/O.

`src/main.rs` owns I/O and surfaces:
- parse the optional trailing `--map <path>` pair;
- `--tlist --map`: append `; names ...` to each existing slot line;
- `--tshow <name> --map`: resolve the name to a unique slot, then reuse the
  existing selector/decode/show path;
- `--tshow <index> --map`: unchanged decode/show, line gains `; names ...`.

Without `--map`, no existing line or message changes.

## Exact map shape (established)

| Property | Value |
|---|---|
| Size | 384,000 bytes exactly |
| Rows | 6,000 × 64 bytes |
| Row | 16 × `u32`, little-endian |
| Header | none |
| Sentinel | `0xFFFFFFFF` = no reference |
| `0` | valid code → `0_0`, not a terminator |
| Strings | none |

| Block | Base row | Capacity | Used rows |
|---|---|---|---|
| `TEXTURES.BIG` | 0 | 2,000 | 1,287 |
| `MOVIES.BIG` | 2,000 | 2,000 | 29 |
| `BONUS.BIG` | 4,000 | 2,000 | 268 (2 trailing rows all-sentinel) |
| `E3.BIG` | — | — | no block |

Unused rows are 16 × `0xFFFFFFFF`. The first 8,192 bytes are the first 128 rows,
not a separate `u16` table: every code is < 65,536, so every high half reads as
zero. See `docs/notes/s8-big-slot-names.md`.

## Name encoding

| Code `v` | Bank | Display |
|---|---|---|
| `0xFFFFFFFF` | — | none |
| `v < 32,000` | Root (`TEX\`) | `<v/16>_<v%16>` |
| `32,000 ≤ v < 64,000` | M (`TEX\M\`) | `M\<(v-32000)/16>_<(v-32000)%16>` |
| `v ≥ 64,000` | B (`TEX\B\`) | `B\<(v-64000)/16>_<(v-64000)%16>` |

Integer division and remainder. The engine formats the same string with
`%s%d_%d` and the prefixes `""`, `M\`, `B\`, then looks it up in its texture
cache.

## Evidence ledger

| # | Hypothesis | Verdict | Evidence that decided it |
|---|---|---|---|
| 1 | Container stores names | RULED OUT | No ASCII in a slot payload; raw pixels only |
| 2 | First 8,192 bytes are a 4,096-entry `u16` table | RULED OUT | They are rows 0–127 of the 64-byte matrix; codes < 65,536 make every odd `u16` zero |
| 3 | A separate `u32` ID list starts at 8,192 | RULED OUT | Offset 8,192 is row 128 (slot 128's aliases) |
| 4 | Tail values are PS1 texture-page words | RULED OUT | Decoding as names resolves to real loose files at 99.6%; page words have no such match |
| 5 | One 64-byte row per slot, 16 codes | SUPPORTED | Consumer strides by 64 and loops 16 times; block row counts = 1,287 / 29 / 268 |
| 6 | Group bases 0 / 2,000 / 4,000 | SUPPORTED | Consumer adds 2,000 for movies and 4,000 for bonus; 6,000 rows fill the blob |
| 7 | Name = bank + `<id>_<sub>` | SUPPORTED | Consumer formats `%s%d_%d` with `""` / `M\` / `B\`; 99.6% of names exist as files; pixel cross-check agrees for slot 2 |
| 8 | Descriptive labels exist in the data | RULED OUT | No ASCII in the map; container has no name table; loose names are numeric |
| 9 | `FX_TEX_MAP.TXT` is a name source | RULED OUT | It is a text UV/tpage remap, not a name list |
| 10 | Public docs cover the map | NONE FOUND | Two search rounds; see the findings note |
| 11 | E3.BIG is covered | UNKNOWN | No block; no E3 offset in the observed consumer path |

Observed coverage: 1,287/1,287 TEXTURES rows have names; 20,516/20,592 references
(99.6%) resolve to an existing `.DDS`; 76 distinct references point to loose
files missing from the observed install. Names are shared: 2,070/5,876 distinct
TEXTURES names occur in more than one slot.

## Falsifiability

The model makes three independent predictions, all checked:

1. Block row counts must equal container counts — 1,287 / 29 / 268 — and they do.
2. Decoded names must be real loose files — 99.6% are.
3. A slot's pixels must resemble at least one of its decoded names — slot 2's
   best loose match (`100_10`) is in slot 2's row.

If an apply-time probe contradicts (1) or (2), the no-go path in `proposal.md`
applies.

## Module shape

```rust
// src/tex_hd_map.rs
pub const MAP_BYTES: usize = 384_000;      // 6_000 * 64
pub const ROW_BYTES: usize = 64;           // 16 * 4
pub const ROW_CODES: usize = 16;
pub const GROUP_ROWS: usize = 2_000;
pub const SENTINEL: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group { Textures, Movies, Bonus }

impl Group {
    /// Case-insensitive file-stem match: TEXTURES / MOVIES / BONUS.
    pub fn from_container_stem(stem: &str) -> Option<Group>;
    pub fn base_row(self) -> usize;        // 0 / 2_000 / 4_000
    pub fn label(self) -> &'static str;    // "TEXTURES" / "MOVIES" / "BONUS"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank { Root, M, B }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureName { pub bank: Bank, pub id: u32, pub sub: u32 }

impl TextureName {
    pub fn display(&self) -> String;                    // `<id>_<sub>` / `M\...` / `B\...`
    pub fn parse(s: &str) -> Option<TextureName>;       // grammar from spec.md
}

#[derive(Debug)]
pub enum MapError {
    WrongLength { found: usize, expected: usize },
    SlotOutOfCapacity { slot: u16, group: Group },
}

/// Pure: validates length once; row access is bounds-checked.
pub struct TextureMap<'a> { bytes: &'a [u8] }

impl<'a> TextureMap<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, MapError>;
    pub fn codes(&self, group: Group, slot: u16) -> Result<[u32; ROW_CODES], MapError>;
    /// Deduplicated, first-seen order; empty for an all-sentinel row.
    pub fn names(&self, group: Group, slot: u16) -> Result<Vec<TextureName>, MapError>;
    /// Slots in `0..slot_count` that own `name` (exact display match).
    pub fn slots_for_name(&self, group: Group, slot_count: u16, name: &TextureName) -> Vec<u16>;
}

pub fn decode_code(code: u32) -> Option<TextureName>; // SENTINEL -> None
```

`TextureMap` borrows the caller's bytes, so CLI I/O stays in `main.rs`.
`decode_code` may also be `TextureName::from_code`; either way it is pure and
table-tested.

## Data flow

```text
--tlist --map:
  read map -> TextureMap::parse (length guard)
           -> Group from container stem (loud if unknown)
           -> for each slot: codes -> names -> existing summary/addressing/class
              line + "; names ..." -> per-class footer -> exit 0

--tshow <name> --map:
  metadata -> parse_header -> validate_geometry -> parse map -> group
           -> selector is not an integer -> TextureName::parse
           -> slots_for_name (0: loud; >1: loud + candidates; 1: chosen slot)
           -> existing payload/decode/show path -> exit 0 on close

--tshow <index> --map:
  ... existing path ... stdout line + "; names ..." -> window -> exit 0
```

## Contract preservation boundary

| Surface | Status |
|---|---|
| `--tlist` without `--map` (summary, addressing, slot lines, footer) | Unchanged, byte-for-byte |
| `--tshow <index>` without `--map` (line, decode, window, errors) | Unchanged |
| Header parsing, geometry validation and messages | Unchanged |
| Integer selector and range errors | Unchanged |
| Empty/zero payload errors | Unchanged |
| Other modes (single DDS, gallery, `--bigfile`, mesh, launcher) | Unchanged |
| `--tlist --map` | New suffix on slot lines |
| `--tshow <name> --map` | New selector grammar and resolution errors |
| `--tshow <index> --map` | Same behavior; line gains `; names ...` |

## Decisions

| Decision | Choice | Alternatives | Rationale |
|---|---|---|---|
| Name source | Engine alias codes | Heuristic labels from pixels | Only real names in the data; heuristics would lie |
| Map selection | Explicit `--map <path>` | Auto-find `../AREA/TEX_HD.MAP` | No magic paths; the user passes their own install path, same as other modes |
| Group selection | Container file stem | `--group` flag | Matches the engine's own file-based selection; one fewer flag. A renamed file fails loud |
| Ambiguity | Loud error with candidates | First match | Shared names are common (2,070); guessing would show the wrong slot |
| Display | Deduped names on the existing line | New verb `--tnames` | Keeps one browsing surface; `--tlist` stays recognizable |
| Duplicates | Dedupe for display; keep all 16 codes in parsing | Collapse at parse | The map is parsed faithfully; display is ergonomic |
| `--tlist` with `--map` | Appends a suffix | Separate table | Minimal diff, still one line per slot |
| E3 | Refuse loud when `--map` is used | Map E3 to another block | No block exists; guessing would mislabel |
| Tests | Pure unit + CLI on synthetic maps | Real map in tests | Legal rule: no game bytes in the repo |
| Empty slot 0 | Show `0_0` (map says so) | Hide names for class `empty` | Faithful to the map; class is still printed |

## Out of scope

- Descriptive/semantic labels ("stone", "face"). No such vocabulary exists in
  the data.
- E3.BIG names (no block).
- Mapping alias families to original PS1 page/room catalogues.
- `FX_TEX_MAP.TXT` (separate text UV remap).
- Writing, exporting, or extracting textures; mip chains; atlases.
- SR2, protection, DRM.
- Auto-locating the map from the install tree.

## Testing strategy

| Layer | What | Approach |
|---|---|---|
| Unit (codec) | Boundaries 31,999/32,000/63,999/64,000; `0`; sentinel; round-trip parse/display; invalid name grammar | Synthetic codes, no files |
| Unit (map) | Exact-length guard; base rows; row offsets; capacity guard; dedupe and order; all-sentinel row; `slots_for_name` zero/one/many | Synthetic 384,000-byte buffers |
| CLI | `--tlist --map` suffix; `--tshow <name>` unique/unknown/ambiguous; name without map; bad map length; unknown stem; byte-identical index mode | `run_textures` on synthetic containers/maps in temp dirs |
| Manual | Real install: names resolve to real loose files; several unique names show recognizable slots | Playtest note (container, slot, name, what was seen) |
| Regression | Header/geometry/listing/other modes | Existing suite stays green; no game files |

## Threat matrix

N/A — no routing, shell, subprocess, VCS/PR automation, executable
classification, or process-integration boundary. Inputs are paths to files the
user already owns; malformed maps and containers fail loud.

## Migration / rollout

No data migration, no flag persistence, no state. The only behavioral change is
the new `--map` mode. Reverting the commits restores index-only operation.

## Open questions

- [ ] Does every retail build ship the same 6,000-row layout? One install
      observed; the exact-size guard turns a mismatch into a loud no-go.
- [ ] Are the 76 unresolved references edition/build gaps? The tool reports the
      name from the map regardless; file existence is not required.
- [ ] What do the 16 aliases mean semantically? Not needed for name lookup.
- [ ] E3 naming — separate slice if evidence appears.

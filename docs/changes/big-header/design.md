# Design: big-header

Turn the existing `.BIG` probe from a wrong-field verdict into a real header parser. The change stays inside `src/textures.rs` plus its CLI arm: read four bytes, validate three invariants, report the header, and keep everything past byte 4 explicitly UNKNOWN. No new module, no new crate, no entry parsing.

## Technical approach

`main.rs` `--textures` arm keeps its shape. `textures::describe` stops reading a `u32` count word, reads the two `u16` header fields instead, validates them, and keeps the existing bounded probe windows + table-hunt evidence. `TexturesLayout` carries a `BigHeader`; `summary()` prints it. `--tlist` exits 0 when the header validates; failures return the new distinct errors (exit 1). `--tshow` stays a loud exit 1 because the entry table is still UNKNOWN.

## Parsed vs UNKNOWN bytes

| Offset | Width | Field | Parsed | Validated |
|---|---|---|---|---|
| `0..2` | `u16` LE | header size | Yes | MUST equal 512 |
| `2..4` | `u16` LE | entry count | Yes | MUST be `1..=32768` |
| `4..512` | 508 B | header tail | No | Nothing. Observed zero in 4/4 files; NOT promoted to a rule |
| `512..EOF` | rest | index + payload | No | Nothing — location and stride UNKNOWN |

Read budget for the header: 4 bytes. The pre-existing bounded windows (64 B at 0, 4 KiB, 1 MiB, 1 KiB tail) are unchanged.

## Architecture decisions

| Decision | Choice | Alternatives considered | Rationale |
|---|---|---|---|
| Module placement | Extend `src/textures.rs` | New `src/big_header.rs` | The CLI arm and tests already live here; a new module for ~60 lines adds wiring and still sits behind `--textures`. Revisit if a second consumer appears |
| Field decode | Two `u16` LE fields | Keep `u32` and shift/mask | `u16` matches the confirmed layout and makes the validation read exactly as the format does |
| Truncation rule | `file_len < 512` fails | Accept any file with 4 readable bytes | The header size field says 512; a shorter file cannot contain the header it claims |
| Count bound | `1..=32768` | Accept any `u16`; new arbitrary bound | Reuses the existing documented sanity constant; zero entries is implausible for these containers. Bound is a guard, not a format field |
| Exit contract | `--tlist` valid header → 0 | Keep 1 because nothing is listed | Exit 0 now means "header verified", which is the slice's deliverable; invalid files stay 1, so the two classes are distinguishable by code as well as message |
| Probe + hunt code | Keep | Delete as obsolete | They carry the documented no-go evidence and their tests; deleting them expands the diff without serving the header slice |
| Error shape | Three new variants (`TruncatedHeader`, `WrongHeaderSize`, `ImplausibleCount`) | One `NotTextures(String)` with different text | Distinct variants make the message contract testable with `matches!` instead of string scraping |
| Header tail | Do not validate zero bytes | Require `4..512` zero | Four samples are not a format rule; enforcing it would invent a constraint and risk rejecting valid files |

## Interface

```rust
pub const HEADER_SIZE: u16 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BigHeader {
    pub header_size: u16,
    pub entry_count: u16,
}

pub fn parse_header(prefix: [u8; 4], file_len: u64) -> Result<BigHeader, TexturesError>;

pub struct TexturesLayout {
    pub file_len: u64,
    pub header: BigHeader,
    pub header_class: RegionClass,
    pub fill_class: RegionClass,
    pub data_class: RegionClass,
    pub table_found: bool, // always false for now; evidence field, not a parser
}
```

`summary()` emits, in order: `BIG header: 512-byte header, <count> entries`, `size: <size class>`, the three window classes, and `entry table: UNKNOWN (location not established; no stride inferred)`.

## Data flow

```
path ──> metadata (len)
     ──> read 4 bytes ──> parse_header ──> BigHeader ──┐
     ──> 3 bounded windows + tail hunt (unchanged) ─────┤
                                                        ▼
                                            TexturesLayout ──> summary() ──> stdout
                                                        └────> Result ──> stderr (exit 1)
```

## File changes

| File | Action | Description |
|---|---|---|
| `src/textures.rs` | Modify | Add `BigHeader`, `HEADER_SIZE`, `parse_header`, three error variants; swap `count_word_sane` for `header`; update `describe`, `summary`, and tests |
| `src/main.rs` | Modify | `--tlist` prints the summary and returns success; `--tshow` stays loud; remove the duplicated no-table stderr block and `NEXT_CANDIDATE`; add CLI exit-code tests |
| `README.md` | Modify | Correct the `textures-big` bullet and the "What's missing" entry to the parsed-header behavior |
| `docs/changes/big-header/*.md` | Create | This planning bundle |

## Testing strategy

| Layer | What | Approach |
|---|---|---|
| Unit (pure) | Valid header, wrong size, truncated length, count `0` / `32769` | `parse_header` on synthetic `[u8; 4]` + synthetic lengths |
| Unit (I/O) | `describe` + `summary` on synthetic 512+ byte temp files | `tempfile`; assert report fields, UNKNOWN line, no exact length |
| CLI | `--tlist` exit 0 on valid, 1 on each invalid class; `--tshow` exit 1 | `run_textures` on temp files in `main.rs` tests |
| Regression | Existing modes untouched | Keep existing suite green; no game files |

## Threat matrix

N/A — no routing, shell, subprocess, VCS/PR automation, executable-file classification, or process-integration boundary.

## Migration / rollout

No data migration. One intentional contract change: `--tlist` on a valid header exits 0 instead of 1. README and spec state it; no feature flag, no persisted state.

## Open questions

- [ ] Does the count include a sentinel or terminator entry? UNKNOWN, does not affect this slice.
- [ ] Do bytes `4..512` carry data in any container? UNKNOWN; not validated.
- [ ] Where the per-file index lives and what its stride is. UNKNOWN; deliberately out of scope.

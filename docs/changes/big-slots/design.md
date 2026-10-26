# Design: big-slots

Extend `src/textures.rs` with the confirmed fixed-slot arithmetic and wire it into the existing `--textures` arm. Header parsing and its messages stay byte-for-byte; geometry validation, whole-slot classification, listing, and the loud non-decodable verdict are additive. No new module, no crate, no decompressor.

## Technical approach

`parse_header` is untouched, so its three rejection messages and tests keep passing. After it succeeds, `describe` calls `validate_geometry(file_len, header)`; a mismatch returns a distinct error. `--tlist` runs `scan_slots` and prints the preserved `summary()` plus a slot section. `--tshow` validates the selector against `0..count`, reads exactly that slot, classifies it, and returns a loud error carrying class and offset.

## Classification strategy (honest by construction)

| Item | Choice |
|---|---|
| Read scope | Every byte of every slot: 131072 bytes per slot, no sampling, no head-only heuristic |
| Definition | `empty` = all `0xFF`; `zero` = all `0x00`; anything else = `data`. Order matters only for documentation (`0xFF` is not `0x00`) |
| Buffer | One reusable `vec![0u8; 131072]` (`128 KiB`) for the whole scan; memory is constant, not proportional to slot count |
| Cost | One sequential pass. The largest container is 168 MB, read once; disk-bound, seconds at most on local storage, acceptable for a CLI. `--tshow` reads only the one selected slot |
| Rejected | Sampling the head (would require "sampled" wording and could misclassify a mostly-fill slot); reading the whole file into memory (168 MB peak, no benefit) |
| Honesty test | A slot that is `0xFF` except for its final byte must classify `data`; this test fails if anyone reintroduces sampling |

## Contract preservation boundary

| Surface | Status |
|---|---|
| `parse_header`, `HEADER_SIZE`, `MAX_ENTRIES` | Unchanged |
| `TruncatedHeader`, `WrongHeaderSize`, `ImplausibleCount` messages and exits | Unchanged |
| `TexturesLayout::summary()` text (including `entry table: UNKNOWN ...`) | Unchanged |
| `--textures` usage lines and arg shapes | Unchanged |
| `--tlist` report | Extended: existing summary, then the new slot section |
| `--tshow` | Extended: still exit 1, but now distinguishes selector, range, and class |

The `entry table: UNKNOWN` line is retained deliberately: no index table exists, and the slice does not rewrite the header report. The appended `addressing: fixed 131072-byte slots ...` line is what states the new model.

## Decisions

| Decision | Choice | Alternatives | Rationale |
|---|---|---|---|
| Where geometry is checked | New `validate_geometry` called from `describe` | Inside `parse_header` | Keeps the header contract and tests intact; geometry is a file-level invariant, not a header field |
| Probe windows | Keep all three; clamp only the data probe to `min(1 MiB, file_len - 64)` | Keep fixed 1 MiB | Header counts allow as few as 1 slot; a fixed 1 MiB probe would fail a geometry-valid short container with a misleading verdict. Real containers are all > 1 MiB, so their output is unchanged |
| Classification type | New `SlotClass` (`Empty`, `Zero`, `Data`) | Reuse `RegionClass` | The contract names exactly three classes; reuse would leak `DdsMagic` and change the names |
| Classifier | `classify_slot(&[u8])` with `all()` checks | Byte histogram; head scan | `all()` is exact, allocation-free, and short-circuits on the first differing byte |
| Scan shape | `scan_slots` -> `SlotScan { classes }` + `report()` | Slots inside `describe` | `--tshow` must not pay a 168 MB scan |
| Single slot | `slot_class_at` seeks and reads 131072 bytes | Full scan and pick | A full pass for one slot is wasteful; the seek arithmetic is verified |
| Errors | Four new variants | One `String` error | Message contracts stay testable with `matches!`, matching existing style |
| Decode verdict | Valid index always returns an error | Placeholder success | Compression is UNKNOWN; success would claim a decode that never happened |

## Interface

```rust
pub const SLOT_SIZE: u64 = 131072;
pub const SLOT_DATA_START: u64 = 4096;

pub fn expected_len(count: u16) -> u64;              // 4096 + count * 131072
pub fn slot_offset(index: u16) -> u64;               // 4096 + index * 131072
pub fn validate_geometry(file_len: u64, header: BigHeader) -> Result<(), TexturesError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotClass { Empty, Zero, Data }             // Display: "empty" | "zero" | "data"
pub fn classify_slot(bytes: &[u8]) -> SlotClass;     // whole slice; empty -> Zero

pub struct SlotScan { pub classes: Vec<SlotClass> }
impl SlotScan {
    pub fn count(&self, class: SlotClass) -> usize;
    pub fn report(&self) -> String;                  // addressing + slot lines + footer
}
pub fn scan_slots(path: &Path, header: &BigHeader) -> Result<SlotScan, TexturesError>;
pub fn slot_class_at(path: &Path, header: &BigHeader, index: u16)
    -> Result<(u64, SlotClass), TexturesError>;      // (offset, class), one slot read
```

`report()` emits, in order: `addressing: fixed 131072-byte slots (slot i at 4096 + i * 131072)`, `slots (whole-slot scan; no sampling):`, `slot <i> offset <off> <class>` per index, and `slots: <total> total, <empty> empty, <zero> zero, <data> data`.

## Data flow

```
--tlist:  path -> metadata -> 4-byte prefix -> parse_header -> validate_geometry
                -> summary() (unchanged) + scan_slots (whole slots) -> report() -> stdout, exit 0

--tshow:  path -> ... -> validate_geometry -> parse selector -> range check
                -> slot_class_at (one slot) -> SlotNotDecodable -> stderr, exit 1
```

## File changes

| File | Action | Description |
|---|---|---|
| `src/textures.rs` | Modify | Slot constants, `SlotClass`/`classify_slot`, geometry validation, `scan_slots`/`report`, `slot_class_at`, four error variants, probe clamp, tests |
| `src/main.rs` | Modify | `--tlist` prints slot report; `--tshow` selector parse/range/class; update synthetic fixtures to exact geometry; CLI tests |
| `README.md` | Modify | `.BIG` bullet: slot listing, classes, loud non-decodable `--tshow` |
| `docs/changes/big-slots/*.md` | Create | Planning bundle |

## Testing strategy

| Layer | What | Approach |
|---|---|---|
| Unit (pure) | `expected_len`/`slot_offset` arithmetic; `classify_slot` for all-fill, all-zero, mixed, last-byte-differs | In-memory synthetic slices |
| Unit (geometry) | Exact pass; one byte short/long; no clamping | `validate_geometry` on synthetic lengths |
| Unit (I/O) | `scan_slots` report lines, offsets, footer totals; `slot_class_at` on one slot | Synthetic containers in temp dirs (`4096 + count * 131072` bytes) |
| CLI | `--tlist` exit 0; geometry mismatch exit 1; selector/range/class exit 1 | `run_textures` on synthetic temp files |
| Regression | Header messages, summary text, all other modes | Keep the existing suite green; no game files |

## Threat matrix

N/A - no routing, shell, subprocess, VCS/PR automation, executable-file classification, or process-integration boundary.

## Migration / rollout

No data migration, no flag, no persisted state. Additive output; `--tshow` keeps exit 1 while changing only the reason. Reverting the commits restores header-only behavior.

## Open questions

- [ ] Does every container always use 131072-byte slots beyond these four samples? Four exact fits; the geometry check makes any exception loud rather than silent.
- [ ] Does the count include a sentinel slot? Does not affect listing or classification; every slot in `0..count` is listed as found.
- [ ] Decompression algorithm for `data` slots. UNKNOWN; out of scope and never guessed.

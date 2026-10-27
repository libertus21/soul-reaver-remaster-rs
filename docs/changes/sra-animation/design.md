# Design: sra-animation

## Technical approach

Decode the container only, and make the census falsifiable in code. A new
pure module `src/sra.rs` owns the header model, the two structural probes
(hierarchy run, curve runs) and the sparkline renderer. `main.rs` gains one
`--sra-info` arm that prints the summary and exits. Nothing touches
`render.rs`, `pose.rs` or `srm.rs`.

The parser is deliberately shallow: it proves what is *absent* (no parent
array, no clip table) as loudly as what is present (count, table, curves).
It never interprets a curve as a bone rotation.

## Probe ledger (read-only, 2026-10-27)

| # | Probe | Result | Evidence class |
|---|---|---|---|
| P1 | Census of `.SRA` in the SR1 remaster tree | 227 files, all under `OBJECT`, alongside 349 `.SRM`, 60 `.SRL`, 29 `.PCM`. No `.SRA` elsewhere | Observed |
| P2 | Size distribution | total 3,828,348 B; min 2,084; p25 2,308; median 3,236; p75 12,432; p90 41,176; max 533,444 | Observed |
| P3 | Duplicate content | 134 distinct SHA-256 digests; 93 files are duplicates; clusters of 36 and 30 | Observed |
| P4 | Text magic | none. First word is a small count (2..1054); second word is `0` in 227/227; third is `4` (144), `28` (79), or one of 36/40/68/140 | Observed |
| P5 | Name pairing | 1/227 basenames match an `.SRM`; 13/227 match any other file (12 `.PCM`, 1 `.SRM`) | Observed |
| P6 | Header floor | the first non-zero byte after the table sits at `max(2048, 8 + 4*(N+1))` in 225/227; the two exceptions are the largest table (pushed to the next 2 KiB boundary) and a one-byte skew | Derived (strong) |
| P7 | Offset table | ascending; `0` terminator in 227/227; in the largest table 822 of 1,053 gaps are exactly 4 bytes | Observed |
| P8 | Payload content | mixed packed-integer records (a small count, an `0xFFFFFFFF` marker, a repeating 16-bit pair carrying a `100` scale) and smooth float runs | Observed shape / Inferred meaning |
| P9 | Curve runs | strict increasing, `0..360`, step `<30`, length `>=8`: 0 runs in 71/227, median 1, max 323 | Derived (strong) |
| P10 | Quaternion test | 3.2% of aligned 4-float windows have norm in `0.9..=1.1` (near chance) → not a quaternion array | Derived |
| P11 | Parent/index array | longest `u16` run in `1..=127` is 8; longest `u32` run in `1..=255` is 15 | Observed |
| P12 | Name table | 107 files show printable runs, but qualitative checks show float-bit noise; no name table | Observed / Derived |
| P13 | Sibling `.SRL` | ASCII magic `SRL.`; a count word; size ≈ count × 1200 (1,418 → 1,701,620). No name overlap with `.SRA`/`.SRM` | Observed |

## Evidence classes

- **Observed** — a byte-level fact repeatable on the census (counts, sizes,
  table shape, terminator, run maxima, magic presence).
- **Derived (strong)** — a model that fits the census with no exception
  (2 KiB header floor; header length rule; curve-run shape).
- **Inferred** — semantic reading that the bytes permit but do not prove
  (curves are rotation angles in degrees; the `100`-scaled pair is a
  quantisation scale).
- **UNKNOWN** — not established and not guessed.

## What stays UNKNOWN

1. **Channel-to-joint binding.** No bone index, name, or ordering evidence
   links a curve to a `.SRM` joint slot. One-bone posing still has no
   authoritative input.
2. **Time base and frame rate.** No duration, tick, or fps field survives a
   cross-file check. `N` is not proven to be frames, clips, or blocks.
3. **Clip boundaries.** No per-clip record, name, or count was found.
4. **Packed header fields.** The small count, `0xFFFFFFFF`, and the
   `0x0064xxxx` triple are observed but unread.
5. **`N` semantics** and why the table is capped by the 2 KiB floor in the
   largest file.
6. **`.SRL`** — magic and ~1200-byte records are observed; the record
   meaning is not decoded and is out of scope.

## Architecture decisions

| Decision | Options considered | Choice | Rationale |
|---|---|---|---|
| Where the parser lives | extend `srm.rs`; new `sra.rs` | new `src/sra.rs` | `.SRA` shares no byte structure with `.SRM`; a separate module keeps the proven mesh path untouched and makes the no-go independent |
| Dependency policy | add a parser/codec crate; hand-rolled | hand-rolled | the container is 4 words plus two linear scans; no crate earns its weight |
| Classification language | claim "animation"; claim nothing | heuristic + explicit `hierarchy=`/`clip_table=` fields | the probe supports shape, not semantics; the output must be as honest as the notes |
| Curve rendering | none; 2D window; text sparkline | text sparkline | "visible" without a GPU path, and it exercises the classification on real bytes |
| Exit-code split | one code; three codes | 1 usage / 2 malformed / 3 refused | tests and scripts can distinguish invocation errors from format errors |

## Data flow

    path ──> extension gate ──> read file ──> header model ──> probes ──> Formatter ──> stdout
                    │                  │             │            │
                    └── exit 3         └── exit 2    └── exit 2   └── counts + sparkline

## File changes

| File | Action | Description |
|---|---|---|
| `src/sra.rs` | Create | `SraInfo`, `parse`, header/table validation, hierarchy run, curve runs, sparkline |
| `src/main.rs` | Modify | one `--sra-info` arm, usage text, exit-code mapping |
| `README.md` | Modify | "What works / What's missing": `.SRA` scoped as diagnostic, animation still missing |
| `docs/notes/sra-animation.md` | Create | this probe's findings note (already written at plan time) |
| `docs/changes/sra-animation/*` | Add | proposal, spec, design, tasks |
| `src/render.rs`, `src/pose.rs`, `src/srm.rs` | None | untouched by contract |

## Interfaces

```rust
pub const HEADER_FLOOR: usize = 2048;
pub const MAX_DECLARATIONS: usize = 1 << 20;

pub struct SraInfo {
    pub declarations: usize,
    pub header_len: usize,
    pub payload_len: usize,
    pub hierarchy: HierarchyProbe,   // None | Candidate
    pub curve_segments: usize,
    pub longest_curve: Vec<f32>,
}

pub enum SraError { TooShort, HeaderPastEof, CountCeiling, TableNotAscending, MissingTerminator }

pub fn parse(bytes: &[u8]) -> Result<SraInfo, SraError>;
pub fn sparkline(values: &[f32]) -> String;
```

## Testing strategy

| Layer | What to test | Approach |
|---|---|---|
| Unit | header length rule, 2 KiB floor, count ceiling, ascending check, terminator, truncation at every boundary | synthetic `Vec<u8>` builders |
| Unit | hierarchy probe both arms; curve detector both arms; sparkline mapping | synthetic payloads, hand-computed expectations |
| Unit | `SraError` → exit-code mapping | plain match tests |
| Integration | `--sra-info` arg shapes and refusals, no window, stdout contract | process-level CLI tests in the existing style |
| Manual | three real files (character / prop / stub) | user acceptance, relative names only |

## Threat matrix

N/A — no routing, shell, subprocess, VCS/PR automation, executable-file
classification, or process-integration boundary. The change reads one
user-supplied data file and writes stdout.

## Migration / rollout

No migration. Additive CLI arm; all existing modes keep their behavior.

## Open questions

- [ ] None block this slice. The unresolved questions are all semantic
      (binding, time base) and are recorded above as UNKNOWN rather than
      guessed.

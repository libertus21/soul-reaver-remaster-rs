# Design: room-geometry

## Technical approach

Keep the parser shallow. `parse_pcm` already walks the offset table; it now also
records each block's size. A pure classifier maps a size (plus an all-zero test)
to one shape class, and a formatter prints counts. `main.rs` gains one
`--level-records` arm beside `--level-info`. No payload is interpreted, no mesh
is built, and no new module or crate is introduced.

## Probe ledger (read-only, 2026-10-27)

Box: 3 passes, ~120 minutes, SR1 only. Scratch stayed outside the repo. No game
bytes, addresses, coordinates, or absolute install paths were recorded.

| # | Probe | Result | Evidence class |
|---|---|---|---|
| P1 | Block census, 89 units | 46,124 blocks; median 281 per unit; 3,778 all-zero blocks (8.2%) | Observed |
| P2 | Records 0/1 | 89/89: record 0 is 4 zero bytes; record 1 is all zero (8 B x21, 80 B x67, 120 B x1) | Observed |
| P3 | Size cliques | 4 B 16,595 (36.0%), 48 B 13,316 (28.9%), 40 B 12,810 (27.8%); next is 28 B at 424. Three sizes carry 92.7% | Observed |
| P4 | Largest block | exactly one dominant block per unit, 3,616-143,872 B; it is the last block in only 1/89 | Observed |
| P5 | 16-bit stride | in the largest block, u16 lag-6 equality is 0.14-0.43 (median 0.26) against ~0.03 at lags 1/2/3/4/8 | Observed statistic / Derived stride |
| P6 | Value range | 62-78% of i16 values in the largest blocks fall in +/-8192; 72% for a sampled 48-byte set | Derived |
| P7 | 48-byte layout | 24 x i16, 20.3 of 24 values distinct per block; no clean 3-field split | Observed shape |
| P8 | 40-byte layout | 20 x i16; mirrored halves hold in only 8.4% of pairs | Observed shape |
| P9 | 4 -> 40 adjacency | 12,642 adjacent pairs; the 4-byte block repeats the next block's head in only 12.6% | Observed |
| P10 | Community sources | `fmil95/soul-re` (MIT) is a PS1 decompilation. `Gh0stBlade/KAIN2` has **no license file** at the probed path. PS1 structs there use `_SVector` = 8 B (3 x i16 + pad) and `_Position` = 6 B (3 x i16); `_BSPLeaf` carries faces and boxes | Observed docs |

P2 **refutes** the earlier "records 0/1 look like a directory" hint: they are
constant zero padding in all 89 units. The offset table is the only directory
and no block carries a type tag.

## Evidence classes

- **Observed** - repeatable count or shape on the 89-unit census.
- **Derived** - a model that fits with no census exception (the 12-byte stride
  signature; three-size domination).
- **Inferred** - a reading the bytes permit (the largest block is the geometry
  buffer) but do not prove.
- **UNKNOWN** - not established and not guessed.

## Architecture decisions

| Decision | Options | Choice | Rationale |
|---|---|---|---|
| Classification key | payload heuristics; size clique | size clique | no tags exist; three sizes hold 92.7% of 46,124 blocks, so the clique is stable and cheap |
| Where the census lives | new module; `level.rs` | `level.rs` | reuses the parsed offset table; no new dependency, no payload copy |
| Block storage | re-read bytes in `main`; sizes on `LevelUnit` | `Vec<u32>` sizes on `LevelUnit` | one pass, constant per block, and unit tests can call the formatter directly |
| Largest block | a class; a separate field | separate field | classes stay mutually exclusive and sum to the total |
| Geometry | extract vertices; stop at the census | stop at the census | the layout is not proven; a guess is worse than a documented no-go |
| Output | bytes/offsets; counts | counts + ordinal + size | legal constraint and no address-style claim |

## Data flow

```text
path -> read_path -> LevelUnit { block_sizes, .. }
                         |
                         v
                  classify(size, all_zero)
                         |
                         v
                 census (counts, largest) -> record_lines -> outln
```

`parse_pcm` computes sizes while it already validates the ascending table, so
there is no second read and no extra failure mode.

## Interfaces

```rust
pub enum BlockClass { Zero, Small4, Fixed40, Fixed48, Other(u32) }

pub struct BlockCensus {
    pub total: u32,
    pub zero: u32,
    pub small4: u32,
    pub fixed40: u32,
    pub fixed48: u32,
    pub other: Vec<(u32, u32)>,   // (size, count), ascending by size
    pub largest: (u32, u32),      // (ordinal, size)
}

pub fn classify(block: &[u8]) -> BlockClass;
pub fn census(unit: &LevelUnit) -> BlockCensus;
pub fn record_lines(unit: &LevelUnit) -> Vec<String>;
```

`LevelUnit` gains `pub block_sizes: Vec<u32>`. `records` stays for the existing
`--level-info` line.

## Testing strategy

| Layer | What to test | Approach |
|---|---|---|
| Unit | class precedence: zero beats 4/40/48; `other:<size>`; largest tie | synthetic block vectors |
| Unit | sum invariant: class counts equal `records` | synthetic `parse_pcm` fixture |
| Unit | line shape and fixed class order | call `record_lines` directly |
| Integration | arg shapes, `.PMF` refusal, empty dir, file and directory happy paths, exit 0/1/2/3, no partial output | existing CLI test style |
| Regression | `--level-info` output unchanged | same fixture before/after |
| Manual | one small, one medium, one large unit | relative names only |

## Threat matrix

N/A - no routing, shell, subprocess, VCS/PR automation, executable-file
classification, or process integration. The change reads user-supplied data and
writes stdout through the existing helper.

## Migration / rollout

No migration. Additive CLI arm; all existing modes keep their behavior.

## What stays UNKNOWN

1. **Type tags.** None were found; class labels are shape-only.
2. **Largest-block layout.** The 12-byte stride signature is statistical, not a
   proven record; field roles, counts, and terminator are unknown.
3. **40- and 48-byte block semantics.** Numeric shape is measured; meaning is
   not.
4. **Faces, indices, normals.** Not located; no topology claim.
5. **Placement and transforms.** No instance/transform table was located.
6. **Compression.** Whether the largest block is compressed is not established;
   entropy is consistent with structured 16-bit data, not proof either way.
7. **PS1 lineage.** `.PCM` may or may not be a repacked `.drm`.
8. **Zero blocks.** 3,778 blocks are all zero: padding, empty slots, or
   placeholders - not decided.
9. **Why three sizes dominate.** Structural convention, not a proven schema.

## Open questions

- [ ] None block this slice. Every unresolved item is recorded above as UNKNOWN
      rather than guessed.

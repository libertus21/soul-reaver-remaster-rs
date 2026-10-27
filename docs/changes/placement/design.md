# Design: placement

## Technical approach

Keep the parser shallow and the output mechanical. `parse_pcm` already exposes
per-block sizes. The new work is a pure census over the **4-byte word grid**:
count words, partition each 40/48-byte block into whole 12-byte groups plus tail
words, count the `0x34` marker by word phase, and count a bounded f32-range
predicate by phase. `main.rs` gains one `--level-groups` arm beside
`--level-records`. No payload is interpreted, no mesh is built, and no semantic
type is named.

The probe settled the question this change exists to ask. Its answer is mostly
negative, so the shipped artifact is a **refutation instrument**, not a decoder.

## Probe ledger (read-only, 2026-10-27)

Box: 3 passes, ~120 minutes, SR1 only. Scratch stayed outside the repo and
outside the game tree. No game bytes, addresses, coordinates, or absolute
install paths are recorded. All figures are corpus ratios over 89 units and
46,124 blocks.

| # | Probe | Result | Evidence class |
|---|---|---|---|
| P1 | Corpus shape | 89 units, 46,124 blocks; 3,778 all-zero (8.2%); 42,346 non-zero | Observed |
| P2 | Word alignment | **0 / 42,346** non-zero blocks violate `size % 4 == 0` | Observed |
| P3 | Dominant word counts | 40 B = 10 words (12,736 non-zero), 48 B = 12 words (13,285 non-zero); 4 B = 1 word | Observed |
| P4 | Stride by entropy | byte-position entropy has a 4-byte sawtooth; the low-entropy byte is the MSB of each word | Observed / Derived |
| P5 | 12-byte phase roles | mod-3 phases are indistinguishable: cardinality ~60k each, `-1` share 0.002 each, small-int share 0.074 each, f32-plausible 0.020/0.029/0.025 | Observed |
| P6 | Marker `0x34` | 1,110 / 286,780 words = **0.39%**; present in 40/89 units; by phase 0.52% / 0.39% / 0.23%; not constant | Observed |
| P7 | Marker low bytes | 261 distinct low-24 values; top value 9.4%; printable 31.5%; all-uppercase 0.11%; dominated by repeated-byte patterns | Observed |
| P8 | Marker index spread | marker words occur at every word index 0-11, not only 0/3/6/9 | Observed |
| P9 | f32 corpus-wide | 93.7% finite; of non-zero readings, 75.6% are subnormal (`<1e-3`) and 18.3% are `>1e9` | Observed |
| P10 | f32 local signal | one unit (LAIR14) has marker-and-value phase lock: lead-marker 0.48, middle-value 0.47, other phases 0.12-0.14 | Observed |
| P11 | Local joint pattern | even in LAIR14 the full `[marker][value][small flags]` triple is **5/102 = 4.9%**; the highest-marker unit (LAIR5, lead-marker 0.76) has **0/74** | Observed |
| P12 | Unit spread | lead-marker `>0.5`: 1/89 (LAIR5); `0.1-0.5`: 3/89; `<=0.1`: 85/89 | Observed |
| P13 | Name correlation | 2,106 names; median `names/(fixed40+fixed48)` = 0.089; Pearson 0.652 / 0.709 / 0.686; ordinal equality 0/89 | Observed |
| P14 | 40 B tail | the trailing word is a `0x34` marker in 5/14, 4/10, 4/19 blocks in three sampled units - near chance | Observed |

### What the ledger decides

1. **Stride.** The settled unit is **4 bytes**. Every non-zero block is a word
   array, and 40/48 are not special semantics: they are 10- and 12-word arrays.
   12 bytes still divides 48 exactly, but it leaves a 4-byte tail on 40 - the
   arithmetic tell that 12 is an imposed stride, not the storage unit.
2. **f32 role.** Corpus-wide the "value" word is not float-like: 75.6% of
   non-zero f32 readings are subnormal, which is the signature of small
   integers misread as floats. The float phase lock exists in **one unit** and
   does not survive the joint test.
3. **id-ish u32.** The `0x34` high byte is a **rare marker**, not a constant
   tag. Its low bytes are mostly repeated-byte filler, not a name prefix.
4. **Names.** There is no ordinal correspondence; the correlation is scale
   only (bigger units have more names and more blocks).
5. **Placement/transforms.** **NO-GO.** No corpus-wide layout, no proven
   coordinate triple, no shared-id triple.

P10 and P11 together explain the earlier observation: it was drawn from a small
number of groups in one unit where two coincidental phase locks align. Across
the corpus those locks disappear.

## Evidence classes

- **Observed** - a repeatable count on the 89-unit corpus.
- **Derived** - a model that fits with no corpus exception (the 4-byte word
  grid; the 12-byte arithmetic partition).
- **Inferred** - a reading the bytes permit but do not prove (a unit-local
  marker/value pattern).
- **UNKNOWN** - not established and not guessed.

## Architecture decisions

| Decision | Options | Choice | Rationale |
|---|---|---|---|
| Settled unit | 12-byte group; 16/20/24 stride; 4-byte word | **4-byte word** | 0/42,346 alignment violations; 40/48 are 10/12 words |
| What to ship | decode; census; nothing | **counts-only census** | the group layout is refuted; a decode would be a guess |
| Marker handling | semantic tag; diagnostic constant | **diagnostic constant** | 0.39% of words and absent from 49 units |
| f32 predicate | decoded type; bounded plausibility | **bounded plausibility** | deterministic and falsifiable; no semantic claim |
| Scope of census | all sizes; 40/48 only | **40/48 only** | those are the placement candidates; other sizes stay in `--level-records` |
| Where the census lives | new module; `level.rs` | **`level.rs`** | reuses parsed sizes; no new dependency |

## Data flow

```text
path -> level::read_path -> LevelUnit { block_sizes, .. }
                              |
                              v
                     group_census(unit)  (pure)
                              |
                              v
                    group_lines(unit) -> outln
```

`parse_pcm` already stores `block_sizes`, so the census needs no second read and
adds no failure mode. Non-zero detection uses the existing `block_all_zero`.

## Interfaces

```rust
#[derive(Debug, PartialEq, Eq)]
pub struct GroupCensus {
    pub total: u32,
    pub words: u32,
    pub whole12: u32,
    pub tail_words: u32,
    pub fixed40_blocks: u32,
    pub fixed48_blocks: u32,
    pub marker34: u32,
    pub marker_phase: [u32; 3],
    pub value_f32_range: u32,
    pub value_phase: [u32; 3],
    pub unaligned: u32,
}

pub fn group_census(unit: &LevelUnit) -> GroupCensus;
pub fn group_lines(unit: &LevelUnit) -> Vec<String>;
```

`GroupCensus` is a sum of pure counts. `group_lines` is the only formatter and
prints the exact shape in the spec.

## Testing strategy

| Layer | What to test | Approach |
|---|---|---|
| Unit | 40 B -> 3 groups + 1 tail; 48 B -> 4 groups + 0 tail | synthetic block vectors |
| Unit | marker count and phase split | synthetic `0x34` at chosen word indices |
| Unit | value predicate in/out of range, NaN/Inf, subnormal | synthetic `f32` bit patterns |
| Unit | unaligned non-zero block is counted and contributes no words | synthetic 6-byte block |
| Unit | zero blocks are excluded from every count | synthetic fixture |
| Unit | line shape and ordering | call `group_lines` directly |
| Integration | arg shapes, `.PMF` refusal, empty dir, file/dir happy paths, exit 0/1/2/3, no partial output | existing CLI test style |
| Regression | `--level-info` and `--level-records` unchanged | same fixtures before/after |
| Manual | one group-like unit (LAIR14) and one non-group unit (LAIR5) | relative names only |

## Threat matrix

N/A - no routing, shell, subprocess, VCS/PR automation, executable-file
classification, or process integration. The change reads user-supplied data and
writes stdout through the existing helper.

## Migration / rollout

No migration. Additive CLI arm; all existing modes keep their behavior.

## What stays UNKNOWN

1. **What the 40/48-byte word arrays mean.** The word grid is settled; the
   field roles are not.
2. **The unit-local marker/value pattern.** It exists in LAIR14 and fails the
   joint test; its meaning is unknown.
3. **Why some units carry `0x34` markers and most do not.** Content type,
   toolchain path, or coincidence - not decided.
4. **The 4-byte tail on 40-byte blocks.** Arithmetic is known; meaning is not.
5. **The 0x34 low-byte patterns.** Repeated-byte filler dominates; their origin
   is unknown.
6. **Placement and transforms.** No instance table, no transform table, no
   coordinate triple is proven.
7. **Faces, indices, normals, topology.** Not located.
8. **Compression.** Not established for any block.
9. **Name-to-block mapping.** No ordinal correspondence was found.

## Explicit non-goals

- No coordinate, angle, scale, or transform is printed or claimed.
- No block or word receives a semantic type name.
- No name-to-block ordinal is claimed.
- No mesh, face, index, normal, or render output.
- No address, file offset, or payload dump.

## Open questions

- [ ] None block this slice. Every unresolved item is recorded above as UNKNOWN
      rather than guessed.

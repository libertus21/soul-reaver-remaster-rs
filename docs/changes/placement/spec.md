# Level Group Census Specification

## Purpose

Define a read-only diagnostic over the non-zero 40- and 48-byte blocks. A block
is treated as a **4-byte word grid**; the 12-byte partition is an arithmetic
diagnostic, **not** a record type. The command prints counts only. It does not
decode placement, transforms, coordinates, or semantic types.

## Requirements

### Requirement: The word grid is the settled unit

The system MUST treat every non-zero block as a sequence of 4-byte
little-endian words. It MUST report an `unaligned` count for non-zero blocks
whose size is not a multiple of 4. On the observed corpus this count is zero,
but the command MUST NOT crash or guess when it is not.

#### Scenario: aligned block

- GIVEN a synthetic non-zero 40-byte block
- WHEN the census runs
- THEN it counts 10 words
- AND `unaligned` stays 0

#### Scenario: unaligned block

- GIVEN a synthetic non-zero 6-byte block
- WHEN the census runs
- THEN it is counted in `unaligned`
- AND it contributes no words to the marker or value totals

### Requirement: The 12-byte partition is arithmetic only

For each non-zero 40/48-byte block the system MUST compute:

- `whole12 = size / 12` (integer division)
- `tail_words = (size % 12) / 4`

It MUST NOT label these as records, objects, groups, or transforms. The output
label is `groups12` and `tail_words`.

#### Scenario: 40-byte block

- GIVEN a synthetic non-zero 40-byte block
- WHEN the partition is computed
- THEN `whole12 = 3` and `tail_words = 1`

#### Scenario: 48-byte block

- GIVEN a synthetic non-zero 48-byte block
- WHEN the partition is computed
- THEN `whole12 = 4` and `tail_words = 0`

### Requirement: The marker is counted by word phase

The system MUST count words whose most-significant byte equals `0x34`
(`word >> 24 == 0x34`) and MUST report that count split by `word_index % 3`
(`phase0`, `phase1`, `phase2`). The marker is a diagnostic constant, not a
type tag.

#### Scenario: marker placement

- GIVEN a synthetic 48-byte block with a `0x34` high byte on word 0 only
- WHEN the census runs
- THEN `marker34 = 1`, `phase0 = 1`, `phase1 = 0`, `phase2 = 0`

### Requirement: The value predicate is bounded and deterministic

The system MUST count words that satisfy all of:

- the 4 bytes parse as a finite IEEE-754 `f32`
- `1e-3 <= |value| <= 1e6`

It MUST report that count split by word phase. The field name is
`value_f32_range`. It is a **plausibility predicate**, not a decoded type, and
MUST NOT be described as a coordinate, angle, scale, or transform.

#### Scenario: in-range and out-of-range

- GIVEN a synthetic block with one word that parses to `1.5f32` and one that
  parses to `1e-40f32`
- WHEN the census runs
- THEN only the `1.5` word increments `value_f32_range`

### Requirement: Census output

The system MUST print, per unit, exactly one header line and one line per
figure, in this shape (numbers are illustrative):

```
groups LAIR14 blocks=29 words=320 whole12=102 tail_words=14
  shape fixed40 blocks=14 words=140
  shape fixed48 blocks=15 words=180
  marker34 78 phase0=54 phase1=12 phase2=12
  value_f32_range 76 phase0=16 phase1=48 phase2=12
  unaligned 0
```

Lines MUST appear in that order. The `shape` lines MUST appear for `fixed40`
and `fixed48` only when their count is non-zero, in that order. The command
MUST NOT print payload bytes, file offsets, coordinates, transforms, or
semantic type names.

#### Scenario: mixed synthetic unit

- GIVEN a synthetic unit with one 40-byte and one 48-byte non-zero block
- WHEN `--level-groups` runs
- THEN `blocks=2`, `words=22`, `whole12=7`, `tail_words=1`
- AND the shape lines report `fixed40 1` and `fixed48 1`

### Requirement: Path and exit behavior

The system MUST accept the same path forms as `--level-info` and
`--level-records` (a single `.PCM` file or a room directory), reuse exit codes
0/1/2/3, and keep the no-partial-output rule.

#### Scenario: wrong argument count

- GIVEN no path, or two paths
- WHEN invoked
- THEN exit 1
- AND no `groups` line is printed

#### Scenario: non-PCM path

- GIVEN a `.PMF` path
- WHEN invoked
- THEN exit 3

### Requirement: Existing commands are unchanged

The system MUST NOT change `--level-info` or `--level-records` output,
ordering, or exit codes.

#### Scenario: regression

- GIVEN the existing synthetic fixtures
- WHEN `--level-info` and `--level-records` run
- THEN their output is unchanged

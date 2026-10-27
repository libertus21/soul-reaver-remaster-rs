# Level Block Census Specification

## Purpose

Define a read-only view of the blocks inside a `.PCM` unit. A **block** is one
entry of the existing relative-offset table. This spec does not define payload
semantics, geometry, or placement.

## Requirements

### Requirement: Block boundaries come from the offset table

The system MUST derive every block from `u32 N`, the `N` relative offsets, and
the data base `4+4N`. It MUST NOT infer boundaries from payload content.

#### Scenario: block count matches the header

- GIVEN a `.PCM` whose offset table passes the existing header checks
- WHEN the census runs
- THEN it reports exactly `N` blocks
- AND the sum of block sizes equals the data-region length

### Requirement: Deterministic shape classes

The system MUST assign each block exactly one mutually exclusive class:

| Class | Rule |
|-------|------|
| `zero` | every byte is zero |
| `small4` | 4 bytes, not all zero |
| `fixed40` | 40 bytes, not all zero |
| `fixed48` | 48 bytes, not all zero |
| `other:<size>` | any other size |

Labels describe shape ONLY. The system MUST NOT print a semantic type name.

#### Scenario: zero wins over size

- GIVEN a 40-byte block whose bytes are all zero
- WHEN classified
- THEN it is `zero`, not `fixed40`

#### Scenario: other size

- GIVEN a 72-byte block that is not all zero
- WHEN classified
- THEN it is `other:72`

### Requirement: Largest block is reported separately

The system MUST report the single largest block's ordinal and size. On a tie,
the lowest ordinal wins. The largest block MUST NOT replace its shape class.

#### Scenario: largest tie

- GIVEN two blocks share the maximum size
- WHEN the census runs
- THEN the lower ordinal is reported as largest
- AND both keep their shape classes

### Requirement: Census output

The system MUST print one `blocks <unit>` line with the total block count, one
line per class present in a fixed order (`zero`, `small4`, `fixed40`, `fixed48`,
`other`), and one `largest <ordinal> <size>` line. It MUST NOT print payload
bytes, file offsets, coordinates, or transforms.

#### Scenario: mixed synthetic unit

- GIVEN a synthetic unit with a known mix of zero, 4, 40, 48, and 72-byte blocks
- WHEN `--level-records` runs
- THEN the class lines are exactly the expected counts
- AND the class counts sum to the header's block count

### Requirement: Path and exit behavior

The system MUST accept the same path forms as `--level-info` (a single `.PCM`
file or a room directory) and reuse exit codes 0/1/2/3 and the no-partial-output
rule.

#### Scenario: wrong argument count

- GIVEN no path, or two paths
- WHEN invoked
- THEN exit 1
- AND no `blocks` line is printed

#### Scenario: non-PCM path

- GIVEN a `.PMF` path
- WHEN invoked
- THEN exit 3

### Requirement: `--level-info` is unchanged

The system MUST NOT change `--level-info` output, ordering, or exit codes.

#### Scenario: regression

- GIVEN the existing fixture
- WHEN `--level-info` runs
- THEN its output is unchanged

# `.BIG` container geometry — fixed 128 KB slots

Follow-up probe after the header slice. Bounded, read-only: header reads, slot-head
reads, arithmetic checks. No extraction, no game content committed.

## Result (CONFIRMED)

The four `.BIG` containers are **arrays of fixed 131072-byte (128 KB) slots**, not
variable-length archives with an index table. That is why no index was ever found:
there is nothing to find.

For every file, `file_len == 4096 + count * 131072` **exactly**:

| File | count | file_len | 4096 + count·131072 |
|---|---|---|---|
| TEXTURES.BIG | 1287 | 168,693,760 | 168,693,760 |
| MOVIES.BIG | 29 | 3,805,184 | 3,805,184 |
| BONUS.BIG | 268 | 35,131,392 | 35,131,392 |
| E3.BIG | 178 | 23,334,912 | 23,334,912 |

So entry `i` lives at offset `4096 + i * 131072`, with no table lookup.

## Layout

| Range | Content |
|---|---|
| `0..2` | `u16` header size = 512 |
| `2..4` | `u16` entry count |
| `4..512` | zero |
| `512..4096` | zero (padding to the 4096 boundary) |
| `4096 + i·131072 ..` | slot `i` |

Read as one `u32`, the first word is `(count << 16) | 512`, which is why it looked
like an implausible count when read as a scalar.

## Slot 0 is an empty-slot marker (CONFIRMED)

In both TEXTURES.BIG and MOVIES.BIG, slot 0 is **131072 bytes of `0xFF`** — the
entire slot, exactly. Slot 1 in both files is zero-filled. Real payload starts
around slot 2.

## Slot payloads are compressed (CONFIRMED for the heads; algorithm UNKNOWN)

- No slot begins with the `DDS ` four-character code; TEXTURES.BIG has 676 distinct
  4-byte slot-head patterns across 1287 slots, dominated by repeats such as
  `20 A4 20 A4`, `00 84 00 84`, `06 A1 06 A1`.
- MOVIES.BIG has 28 distinct heads across its 28 non-empty slots.
- Repeating 16-bit symbols at the head are characteristic of a dictionary/LZ stream,
  not of raw surface data.

So a slot must be decompressed before it can be interpreted. The decompression
algorithm is **UNKNOWN** and is the only thing standing between the container and
actual pixels.

## Side tables (measured)

- `AREA\TEX_HD.MAP` — exactly **384,000 bytes**, matching the size the engine reads
  as a fixed blob. The first 8,192 bytes are 4,096 `u16` values (2,032 non-zero);
  from offset 8,192 the payload begins with a run of ascending `u32` IDs
  (`2032, 2033, 2034, 2051, 2036, 2037, 15018, 15018, …`).
- `AREA\FX_TEX_MAP.TXT` — 4,500 bytes of plain text (already known: `#` comments,
  numeric fields, builds a 6144-entry table).

## Consequences for the rewrite

- `--tlist` can list slots without any index: index, offset, and a class derived
  from the slot head (empty / zero / data). That is honest and verifiable now.
- `--tshow` cannot render a texture until the decompressor is identified; it must
  stay loud instead of guessing.
- The `.BIG` geometry is now settled, so the container work reduces to two known
  problems: slot addressing (solved) and payload decompression (open).

## Open questions

- Decompression algorithm for slot payloads (primary blocker).
- Whether `TEX_HD.MAP` maps texture IDs to slot indices, and what the `u16` prefix
  table means.
- Whether every container uses the same slot size (all four fit 131072 exactly, so
  yes for these four — but that is four samples, not a format guarantee).

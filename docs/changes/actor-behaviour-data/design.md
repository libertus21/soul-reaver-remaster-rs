# Design: actor-behaviour-data (actor census)

## Summary

Add a small, read-only actor view on top of the existing `.PCM` reader. The
mesh parser stays the single source of truth for the container and model
header. The new code reads the object-header block pointer at `+0x1C`, detects
the actor/player signature, extracts the 24-byte state trios and the
0x20-byte behaviour-entry array, and formats counts and small ids only.

No new dependencies. No rendering, textures, bones or world transforms. No game
file is opened by a test.

## Data model

```rust
pub struct ActorBlock {
    pub kind: ActorKind,          // Monster | Player
    pub offset: usize,            // data-base-relative block start
    pub length: usize,            // bytes after offset in the data region
    pub trios: [[u8; 3]; 8],      // block + 0x18, stored order
    pub entries: Vec<BehaviourEntry>,
    pub entry_array: EntryArrayKind, // Clean | Variant | None
}

pub struct BehaviourEntry {
    pub id: u8,                   // entry + 0x1B
    pub raw_head: [u8; 0x1B],     // retained, not printed
}

pub enum ActorKind { Monster, Player }
pub enum EntryArrayKind { Clean, Variant, None }
```

`ObjectModel` keeps its current shape. The actor block is produced by a separate
function so existing callers and `info_lines` are untouched:

```rust
pub fn parse_actor_block(bytes: &[u8]) -> Result<Option<ActorBlock>, ObjectError>
```

It re-walks only the bytes it needs:

1. `records = u32 at 0`, `base = level::data_base(records)`, `data = &bytes[base..]`.
2. Require `data.len() >= 0x64`; read the object-header names and the
   `+0x1C` pointer; reject `0` or `out-of-range` as `Ok(None)`.
3. Read the four-byte signature. `MONSTER_SIGNATURE` and `PLAYER_SIGNATURE` are
   constants. Any other value is `Ok(None)`.
4. Read the 24 bytes at `block + 0x18` into `[[u8; 3]; 8]`.
5. Walk the pointer list at `block + 0x30` (max 32, stop on a non-block value
   or a repeated/decreasing value).
6. Run the entry-array detector (spec R4) and retain the best candidate.

`parse_actor_block` returns `Ok(None)` for a non-actor; it returns `Err` only
when the container itself is malformed (bad data base, header outside the
slice, block pointer inside the slice but the block header truncated).

## Entry-array detector

```text
candidates = sorted(set(pointers))
for s in candidates:
    e = smallest candidate > s
    if e is None: continue
    if (e - s) % 0x20 != 0: continue
    n = (e - s) / 0x20
    if n < 1 or n > 24: continue
    if data[s..s+2] == [0x00, 0x10]: continue
    good = count of records r in 0..n where
             1 <= data[s+r*0x20+0x1B] <= 0x7F
             and (data[s+r*0x20+0x1C..+0x1E] == [0xFF,0xFF]
                  or data[s+r*0x20+0x1C..+0x20] == [0xFF;4])
    if good >= max(1, n-1): accept (n, s, ids)
choose the accepted candidate with the largest n; tie -> lower s
```

If no candidate passes, scan for a contiguous 0x20 run with the same per-record
id/sentinel test. If that also fails, set `EntryArrayKind::Variant` when the
block is non-degenerate and contains at least one record-shaped candidate at
`+0x1B`, else `None`. `Variant` and `None` both produce `entries=0` and `ids=-`.

Rationale for the `0x00,0x10` rejection: the block's animation/animation-param
sub-table also starts with that word and has a 0x10 stride; without the check
it can be mistaken for a behaviour-entry array.

## CLI wiring

`src/main.rs`:

- `--actor-census` -> `run_actor_census(&args[1..])`.
  - Parse exactly `--bigfile <path>` (PR B) and optionally
    `--objects-dir <path>` (PR C).
  - `bigfile::load_index`, then for each entry `bigfile::read_entry` +
    `object::parse_object` + `object::parse_actor_block`. Reuse the container
    parser so the census never invents a second container reader.
  - Collect `CensusRow { name, index, block }`, sort by index, print the lines
    from the spec, then the summary.
  - For `--objects-dir`, walk `*.pcm` recursively in a deterministic sorted
    order; each file is parsed in memory. No writes.
- `--object-info` -> `run_object_info`. After the existing `info_lines`, if
  `--actors` is present, call `parse_actor_block` on the same entry bytes and
  print the actor lines. Without `--actors`, print exactly what it prints now.

Argument parsing follows the existing hand-written pattern in `main.rs`
(`"--bigfile" => ...`), not a new clap/structopt dependency.

## Error handling

- Reuse `ObjectError`; add no public variant unless a message truly differs.
  A truncated actor block maps to `HeaderTruncated` or
  `ModelHeaderOutOfRange` only when the block pointer itself is in range; a bad
  pointer is `Ok(None)` so non-actors never fail the census.
- The census prints no `actor` line for a failed entry and exits 2 at the end
  with the first failure's message, matching the existing loud-failure policy.
- `--objects-dir` reports an unreadable directory as exit 3; an unparsable
  `.PCM` is exit 2.

## Tests (synthetic only)

In `src/object.rs` tests, extend the existing synthetic container builder:

1. `synthetic_actor_block(ids, trios)`: writes the actor signature, the 24-byte
   trio table, a pointer list at `+0x30` with one pointer to an entry array,
   and `n` 0x20 records with the supplied ids and `0xFFFFFFFF` sentinels.
2. Assert `parse_actor_block` reports `Monster`, the same trios, `n` entries and
   the same ids.
3. A synthetic non-actor signature -> `Ok(None)`.
4. A block with a 0x10 stride sub-table -> detector rejects it and reports
   `None`/`Variant`, not a false entry array.
5. A block whose pointer list points outside the payload -> exit-2 error.
6. `--actor-census` on a synthetic two-entry archive -> exact line shape and
   summary counts.
7. `--object-info` without `--actors` -> byte-for-byte prior output (assert the
   existing `info_lines` result).

## Legal and data handling

- Counts, offsets, short names and small ids only; no record dumps.
- Tests never read a game file; all fixtures are synthetic byte buffers.
- No absolute install paths in code, tests or docs.
- The census opens the archive read-only and writes nothing.

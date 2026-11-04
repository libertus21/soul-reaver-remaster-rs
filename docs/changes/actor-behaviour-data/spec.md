# Spec: actor-behaviour-data (actor census + object-info actor lines, proposed)

`--actor-census --bigfile <BIGFILE.DAT>` walks the archive index, parses every
object container through the existing `.PCM` reader, detects the actor block at
object data `+0x1C`, and prints one line per actor container: name, index,
behaviour-entry count, distinct ids and the eight state trios.
`--object-info --actors` appends the same actor summary for one entry.

This spec is for the follow-up slice; no code lands in the docs pass.

## Output contract (`--actor-census`)

One line per actor container, then one summary line. No coordinates, no raw
records, no game bytes:

```text
actor <name> index=<I> block=<B> entries=<E> ids=<id,id,...> trios=<r0>/<r1>/.../<r7>
...
actors=<A> entries=<T> distinct_ids=<D>
```

- `<name>` = the first 12-byte object header name, `_` padding stripped.
- `<I>` = decimal archive entry index.
- `<B>` = `clean` when a contiguous 0x20-stride array was found, `variant` when
  entry-like records exist at a different shape, `none` when no entry array was
  found. Only `clean` contributes to `entries` and `ids`.
- `<E>` = number of 0x20-byte entries (0 for `variant`/`none`).
- `<ids>` = comma-separated lowercase two-digit hex ids in stored order;
  `-` when empty.
- `trios` = eight `/`-separated rows, each a comma-separated decimal triple,
  read from block `+0x18`.
- `<A>` = actor containers reported; `<T>` = sum of `entries`; `<D>` = size of
  the union of ids across `clean` actors.

Order: ascending archive entry index, so output is stable.

## Output contract (`--object-info --actors`)

Without `--actors`, `--object-info` output is byte-for-byte unchanged. With
`--actors`, the existing count lines stay first, then either:

```text
  actor block: none
```

or:

```text
  actor block: offset=<B> length=<L> entries=<E> ids=<id,id,...>
  actor trios: row0=<a,b,c> row1=<a,b,c> ... row7=<a,b,c>
  actor entry array: clean|variant|none
```

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shapes: `--actor-census` takes exactly `--bigfile <path>` (PR B) and optionally `--objects-dir <path>` (PR C). `--object-info` keeps `--bigfile <path> --entry <n>` and gains an optional `--actors` flag. Wrong shape is a usage error + exit 1. Existing modes unchanged |
| R2 | Container parsing reuses the existing `.PCM` reader (`N`, data base, object header, model header, both boundary identities). The census never reads a model header as an actor block |
| R3 | Actor detection: read the u32 at object data `+0x1C`; it MUST be inside `[1, data_len)`; read four signature bytes there. The actor-family signature and the player-family signature are distinct constants. A zero/out-of-range pointer is `none`, not an error |
| R4 | Entry-array discovery: read the dword pointer list at block `+0x30` while each value lies inside the block and the list stays under 32 pointers. For each pointer target `s`, let `e` be the smallest target greater than `s`. Accept `s` when `(e-s) % 0x20 == 0`, `1 <= (e-s)/0x20 <= 24`, the first two bytes at `s` are not `0x1000`, and at least `n-1` records have an id byte at `+0x1B` in `1..=0x7F` with `0xFF` at `+0x1C..+0x1D` or a full `0xFFFFFFFF` sentinel at `+0x1C`. Choose the accepted candidate with the most entries; ties break on the lower offset |
| R5 | Trios: read exactly 24 bytes at block `+0x18` and print them as eight rows of three decimal bytes, in stored order |
| R6 | Ids: print `+0x1B` of each accepted record, in stored order, lowercase two-digit hex. Do not deduplicate in the per-actor column; the summary's `distinct_ids` is the deduplicated union |
| R7 | Bounds: every read is range-checked against the entry slice. A short/truncated container, a bad data base, a bad table, a bad model header or an out-of-range block pointer is a loud error + exit 2; no panic, no partial line |
| R8 | Tests use SYNTHETIC byte buffers only. No test reads a game file; no game bytes, coordinates, absolute paths or dumps are committed. Actor ids and trio rows are printed as numbers, never as a record dump |
| R9 | No regression: `--object-info` without `--actors` keeps its exact prior output; every existing CLI arm keeps its messages and exit codes; `cargo test` stays green |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Census printed / object parsed |
| 1 | Usage error: wrong argument shape, unknown flag |
| 2 | Malformed container, bad bounds, bad signature-adjacent structure, refused multi-part container |
| 3 | Unreadable input / missing archive or directory |

## Scenarios (real corpus)

- `--actor-census` reports 21 actor containers from `BIGFILE.DAT` (19 archive +
  the 2 loose files are PR C; PR B reports 19).
- `skinner` -> `entries=9 ids=25,26,27,28,25,26,27,28,18`.
- `morlock` -> `entries=8 ids=2b,2c,2e,2d,2b,2c,2e,2d`.
- `aluka` -> `entries=10`.
- `soul` -> `entries=0 ids=-` with `block=none` (no 0x20 array).
- `raziel` -> player-family block; not counted as a monster actor.
- A synthetic non-actor signature -> skipped by the census, `none` in
  `--object-info --actors`.
- A synthetic truncated pointer list -> exit 2, no `actor` line.

## Evidence basis

Every field used by this reader is CONFIRMED or INFERRED in
`docs/notes/actor-behaviour-data.md`:

- object header `+0x1C` block pointer: CONFIRMED;
- actor/player signature split: CONFIRMED;
- 0x20 entry stride, `+0x1B` id, `+0x1C` sentinel: CONFIRMED;
- 24-byte 8x3 trio table at `+0x18`: CONFIRMED;
- trio semantics as the state-to-clip-set map: INFERRED, so the reader prints
  rows, never state names.

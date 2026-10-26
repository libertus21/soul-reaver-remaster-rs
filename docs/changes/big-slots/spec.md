# Spec: big-slots

`cargo run -- --textures <file> --tlist` validates the fixed-slot geometry, prints the existing header report plus one line per slot (index, byte offset, class), then a per-class footer, exit 0. `--tshow <i>` validates the index and fails loud with the slot class and the compression verdict (exit 1). Any geometry mismatch is a distinct loud error, never clamped. Header parsing, its messages, and every other CLI mode keep their prior behavior. No decompressor is implemented.

## Quick path

1. Run `--tlist` on any of the four containers: exit 0, one line per slot, footer counts that sum to the header count.
2. Truncate or pad a synthetic container by one byte: distinct geometry error, exit 1, no listing.
3. `--tshow 0`: names `empty`, states no payload / decompressor UNKNOWN, exit 1, no window.
4. `--tshow <count>` and `--tshow abc`: distinct loud index errors, exit 1.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shapes SHALL stay unchanged: `--textures <path> --tlist` (2 args) and `--textures <path> --tshow <sel>` (3 args). Missing path, unknown verb, or wrong arg count SHALL stay a loud usage error with exit 1 |
| R2 | The header SHALL be parsed and validated exactly as before: `u16` size at `0..2` must be 512, `u16` count at `2..4` must be `1..=32768`, file shorter than 512 SHALL fail. The three existing messages SHALL NOT change |
| R3 | Geometry SHALL be validated as `file_len == 4096 + count * 131072`. Any other length SHALL be a DISTINCT loud error naming the observed length, the count, and the expected length, with exit 1. The tool SHALL NOT clamp, truncate, or ignore trailing bytes |
| R4 | For a geometry-valid file, `--tlist` SHALL print the existing header summary unchanged, then `addressing: fixed 131072-byte slots (slot i at 4096 + i * 131072)`, then one line per slot in index order: `slot <i> offset <off> <class>`, where `off == 4096 + i * 131072` |
| R5 | Slot classes SHALL be computed over the WHOLE slot (all 131072 bytes): `empty` if every byte is `0xFF`, `zero` if every byte is `0x00`, otherwise `data`. Sampling SHALL NOT be used. A slot that is `0xFF` except for its last byte SHALL be `data` |
| R6 | `--tlist` SHALL end with `slots: <total> total, <empty> empty, <zero> zero, <data> data`, where total equals the header count and the three class counts sum to it. Exit SHALL be 0 |
| R7 | `--tshow <sel>` SHALL reject a non-integer selector and an index outside `0..count` with DISTINCT loud errors, exit 1, naming the selector or index and the valid range. Header and geometry errors SHALL take precedence over selector errors |
| R8 | `--tshow <i>` with a valid index SHALL read that one slot, classify it over its whole length, and fail loud with exit 1: class named, and for `data` the payload is compressed with decompressor UNKNOWN; for `empty`/`zero`, no payload and nothing to decode. No window SHALL open and no payload bytes SHALL be printed |
| R9 | No decompression, texture decoding, `TEX_HD.MAP` name mapping, or other-container support SHALL be added. No new dependency SHALL be added. Single-file, gallery, `--bigfile`, and mesh modes SHALL keep their exact prior behavior and exit codes |
| R10 | Tests SHALL use synthetic bytes only, in temp dirs. No game bytes, install paths, or content dumps SHALL appear in code, tests, output, or artifacts |

## Message contracts

| Condition | Stream | Message (shape) | Exit |
|---|---|---|---|
| Geometry mismatch | stderr | `slot geometry mismatch: file holds <len> bytes, header count <count> implies <expected> (4096 + <count> * 131072); refusing to clamp` | 1 |
| `--tlist` success | stdout | existing summary + `addressing: fixed 131072-byte slots (slot i at 4096 + i * 131072)` + `slot <i> offset <off> <class>` lines + `slots: <total> total, <empty> empty, <zero> zero, <data> data` | 0 |
| Selector not an integer | stderr | `invalid slot selector '<sel>': expected a slot index in 0..<count>` | 1 |
| Index out of range | stderr | `slot index <index> is out of range: container holds <count> slots (valid 0..<count>)` | 1 |
| Valid index, `data` | stderr | `slot <i> (offset <off>) is data: payload is compressed, decompressor UNKNOWN (cannot decode)` | 1 |
| Valid index, `empty` | stderr | `slot <i> (offset <off>) is empty (0xFF marker): no payload; decompressor UNKNOWN (nothing to decode)` | 1 |
| Valid index, `zero` | stderr | `slot <i> (offset <off>) is zero (0x00 fill): no payload; decompressor UNKNOWN (nothing to decode)` | 1 |
| Header/geometry error on `--tshow` | stderr | the corresponding message above; no selector error | 1 |

## Acceptance scenarios

- [ ] **Geometry exact:** a synthetic container of exactly `4096 + count * 131072` bytes lists all slots and exits 0.
- [ ] **Geometry mismatch:** the same container one byte shorter and one byte longer each exit 1 with the geometry message and print no listing or footer.
- [ ] **Classes:** an 8-slot synthetic container with one all-`0xFF` slot, one all-`0x00` slot, and the rest invented data reports the exact class per index; the footer counts match the index lines.
- [ ] **No sampling:** a slot that is `0xFF` in every byte except the last one is reported `data`.
- [ ] **Offsets arithmetic:** every listed offset equals `4096 + i * 131072`; no table lookup is performed.
- [ ] **Index range:** `--tshow <count>` exits 1 with the out-of-range message; `--tshow abc` exits 1 with the selector message; neither opens a window.
- [ ] **Loud decode verdict:** `--tshow` on a `data` slot exits 1 naming `data`, `compressed`, and `UNKNOWN`; on `empty`/`zero` it exits 1 naming the class and no payload.
- [ ] **Preserved contracts:** truncated header, wrong header size, and implausible count still produce their existing messages and exit 1; existing header summary content is unchanged.
- [ ] **Regressions:** single-file DDS, directory gallery, `--bigfile --list/--show`, and mesh modes behave as before.
- [ ] **Unit/CLI:** `cargo test` covers geometry pass/fail, all three classes, no-sampling, footer totals, selector/index errors, and CLI exit codes on synthetic bytes only.
- [ ] **Legal:** `git status --short` shows only intended files; no game bytes or install paths in artifacts.

## Next step

See `design.md` for the classification strategy and its cost, the preserved contract boundary, and rejected alternatives; `tasks.md` for work units and the review forecast.

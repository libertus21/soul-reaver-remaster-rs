# Spec: big-decompressor

`cargo run -- --textures <file> --tshow <i>` reads ONE slot and, when it holds
data, decodes it as a raw 256 × 256 little-endian A1R5G5B5 image into RGBA8,
prints one verifying line, and shows it through the existing window. Empty and
all-zero slots stay loud with exit 1 and no window. Header parsing, geometry
validation, `--tlist`, selector/range errors, and every other CLI mode keep
their prior contracts. No decompressor is implemented because the payload is not
compressed.

## Quick path

1. `--tshow 4` on a real TEXTURES.BIG: exit 0 after the window closes, image
   recognizable, one stdout line.
2. `--tshow 0` (all-`0xFF`) and an all-zero synthetic slot: loud, exit 1, no
   window.
3. `--tshow abc` and `--tshow <count>`: the exact prior selector/range errors.
4. A slot that cannot be read as exactly 131072 bytes: loud length error, exit 1.
5. `cargo test`: decoder unit tests and all CLI error paths on synthetic bytes.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shapes SHALL stay unchanged: `--textures <path> --tlist` (2 args) and `--textures <path> --tshow <sel>` (3 args). Missing path, unknown verb, or wrong arg count SHALL stay a loud usage error with exit 1 |
| R2 | Header parsing and geometry validation SHALL stay exactly as before, including all messages, bounds, and exit codes. Header/geometry errors SHALL take precedence over selector errors |
| R3 | `--tshow <i>` with a valid index whose slot class is `data` SHALL read that one slot (131072 bytes, no other slot), decode it as 256 × 256 A1R5G5B5 to RGBA8, print exactly one stdout line, open the existing window, and exit 0 when the window closes |
| R4 | The decoder SHALL read little-endian `u16` pixels in row-major, top-down order with no stride padding; bit 15 is alpha, bits 14–10 red, 9–5 green, 4–0 blue; each 5-bit channel SHALL expand by bit replication `(v << 3) \| (v >> 2)`; the alpha bit SHALL map to 0 or 255 |
| R5 | The decoder SHALL reject any input whose length is not exactly 131072 with a distinct loud error naming the observed length and the expected geometry. It SHALL NOT pad, truncate, or guess |
| R6 | `--tshow` on a slot classified `empty` (all `0xFF`) or `zero` (all `0x00`) SHALL stay loud with exit 1 and no window, naming the index, offset, and class, and stating that there is no payload to decode |
| R7 | Selector handling SHALL be unchanged: non-integer and out-of-range selectors keep their exact messages and exit 1, with no window |
| R8 | `--tlist` output SHALL be unchanged (summary, addressing line, per-slot class lines, footer). Slot classification, offsets, and geometry rules SHALL NOT change |
| R9 | No new dependency SHALL be added. Single-file, gallery, `--bigfile`, mesh, and launcher modes SHALL keep their exact prior behavior and exit codes |
| R10 | Tests SHALL use synthetic bytes only, in temp dirs. No game bytes, install paths, or content dumps SHALL appear in code, tests, output, or artifacts. The visible render SHALL be covered by a manual playtest note, not by a headless test |

## Decode contract

| Input condition | Result |
|---|---|
| Length exactly 131072 | 256 × 256 × 4 RGBA8 bytes, 262144 bytes total |
| Pixel `u16` with bit 15 set | alpha 255 |
| Pixel `u16` with bit 15 clear | alpha 0 |
| 5-bit channel value 0 | 8-bit 0 |
| 5-bit channel value 31 | 8-bit 255 |
| Any other length | `SlotPayloadLength` error, exit 1, no window |

## Message contracts

| Condition | Stream | Message (shape) | Exit |
|---|---|---|---|
| `--tshow`, `data` slot, decode ok | stdout | `slot <i> (offset <off>) decoded: 256x256 A1R5G5B5 -> RGBA8; showing image` | 0 |
| `--tshow`, `data` slot, window closed | — | window closes normally, no extra line | 0 |
| `--tshow`, `empty` slot | stderr | `slot <i> (offset <off>) is empty (0xFF marker): no payload to decode` | 1 |
| `--tshow`, `zero` slot | stderr | `slot <i> (offset <off>) is zero (0x00 fill): no payload to decode` | 1 |
| Payload length wrong | stderr | `slot <i> payload length <n> does not match 256x256 A1R5G5B5 (131072 bytes)` | 1 |
| Selector not an integer | stderr | unchanged: `invalid slot selector '<sel>': expected a slot index in 0..<count>` | 1 |
| Index out of range | stderr | unchanged: `slot index <index> is out of range: container holds <count> slots (valid 0..<count>)` | 1 |
| Header/geometry error | stderr | unchanged; no selector or payload message | 1 |
| Window/render failure | stderr | the existing render error text, prefixed `error: ` | 1 |

## Acceptance scenarios

### Identified-format path (active)

- [ ] **One slot decoded:** `--tshow <i>` on a `data` slot reads only that slot
  (the file is not scanned) and shows a 256 × 256 image; exit 0 on close.
- [ ] **Verifiable line:** stdout carries exactly one line with index, offset,
  `256x256`, `A1R5G5B5`, and `RGBA8`.
- [ ] **Colour conversion:** a synthetic slot built from known `u16` values
  (all-zero, all-ones, pure red, pure green, pure blue, alpha-set and
  alpha-clear) converts to the expected RGBA8 bytes.
- [ ] **Orientation:** row 0 of the slot is the top row of the image and the
  window shows the sampled atlas upright (manual playtest, legible signage).
- [ ] **No payload:** `--tshow` on all-`0xFF` and all-`0x00` slots errors with
  the class-specific message, exit 1, no window.
- [ ] **Length guard:** a 131071- or 131073-byte payload slice is rejected with
  `SlotPayloadLength`; nothing is padded or clamped.
- [ ] **Preserved selectors:** `--tshow abc` and `--tshow <count>` keep their
  exact prior messages and exit 1.
- [ ] **Preserved listing:** `--tlist` output is byte-for-byte unchanged and
  still exits 0 on a geometry-valid container.
- [ ] **Cross-container:** `--tshow` works on MOVIES.BIG, BONUS.BIG, and E3.BIG
  because the geometry and pixel format are container-independent at this
  layer.
- [ ] **Regressions:** single-file DDS, directory gallery, `--bigfile
  --list/--show`, mesh, and launcher modes behave as before.
- [ ] **Legal:** `git status --short` shows only intended files; no game bytes
  or install paths in artifacts.

### Documented no-go path (specified, not triggered)

- [ ] If the raw-format read is contradicted at apply time, `--tshow` SHALL
  return to a loud `exit 1` verdict that names the slot class and states the
  payload format is UNKNOWN, and the next candidate (bounded Ghidra call-path
  probe for the slot consumer) SHALL be named in the change's verification
  report. No alternative format SHALL be guessed in code.
- [ ] If a protection or anti-tamper layer appears on this path, the work SHALL
  stop and be reported; nothing SHALL attempt to defeat it.

## Next step

See `design.md` for the evidence ledger, the converter interface, and what stays
out of scope; `tasks.md` for work units and the review forecast.

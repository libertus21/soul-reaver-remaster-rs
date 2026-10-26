# Design: big-decompressor

Add a pure A1R5G5B5 → RGBA8 converter and wire it into `--tshow`, which today
always errors. The evidence says the slot payload is raw 256 × 256 16-bit
pixels, so there is no stream to decompress and no dictionary, size table, or
transform to reverse-engineer. The work is one new module, a one-slot decode
helper, and a CLI arm that shows the result through `render::show_rgba8`.

## Technical approach

`src/textures.rs` keeps owning the container: header, geometry, slot classes,
offsets. A new `src/slot_texture.rs` owns the pixel format only: a pure function
from a 131072-byte slice to 262144 RGBA8 bytes, plus its constants and one
length error. `--tshow` keeps its selector/range checks, reads exactly one slot
through a new `textures::decode_slot_at`, prints one verifying line, and calls
`render::show_rgba8(256, 256, &rgba)`. Empty and zero slots never reach the
converter.

## Evidence ledger

The verdict is "no compression, raw A1R5G5B5". Every hypothesis below was
tested against sampled real slots; the full method and measurements are in
`docs/notes/s7-big-slot-payload-format.md`.

| # | Hypothesis | Verdict | Evidence that decided it |
|---|---|---|---|
| 1 | zlib / deflate | Ruled out | No zlib header pair at slot start; bytes are already spatially coherent as pixels; no block boundaries |
| 2 | LZ4 / LZSS / LZO | Ruled out | No frame magic; lag-2 byte difference ≈ 0 while lag-1 is large, i.e. 2-byte pixel units, not tokens; flat regions repeat one 16-bit value per pixel |
| 3 | RLE | Ruled out | No (count, value) structure; a solid area costs one full pixel per pixel |
| 4 | BC1 / BC3 (DXT) | Ruled out | BC1 decode of the same bytes is noise on slots that are plainly flat as 16-bit; 131072 would imply a 512 × 512 BC1 image with no visible content |
| 5 | 8-bit paletted | Ruled out | 8-bit render shows a forced 2-pixel vertical stripe; the byte pair is the pixel |
| 6 | RGB565 | Ruled out | Implausible magenta/olive colouring on natural surfaces |
| 7 | RGBA4444 | Ruled out | Same implausible colouring; channel weights do not match |
| 8 | Raw A1R5G5B5, 256 × 256, LE | **Supported** | Natural imagery with correct colours on sampled slots in all four containers; exact 256·256·2 fit; bit 15 set in 98.9% of sampled pixels |
| 9 | Per-slot header or index | Ruled out | 131072 is exactly 256·256·2, so no header fits; image content starts at slot byte 0 |
| 10 | Encrypted / obfuscated | Ruled out | Raw read renders a coherent image with no transform |
| 11 | Swizzled / tiled layout | Ruled out | Linear row-major read is coherent and legible at 256 pixels wide |

Unknowns that do not block the slice: whether every one of the 1286 TEXTURES
data slots is a valid image (sampled, not exhaustive), whether alpha is
semantic everywhere, and whether channel order could be B/G/R (visual evidence
says R/G/B; a playtest catches a swap). Slot-to-name mapping is a separate
problem.

## Falsifiability

This is not an unfalsifiable "looks like an image" claim. Three independent
predictions follow from the raw-format model and all hold:

1. The exact byte count must be 256·256·2 with no remainder — it is.
2. The 2-byte pair must behave as one pixel under correlation — it does.
3. A linear decode must produce legible structure at 256 pixels wide — sampled
   atlases contain readable signage lettering at that width.

If a playtest shows noise, sheared rows, or wrong colours, the model is wrong
and the no-go path in `proposal.md` applies.

## Decoder module shape

```rust
// src/slot_texture.rs
pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 256;
pub const PIXEL_BYTES: usize = 2;
pub const SLOT_BYTES: usize = WIDTH * HEIGHT * PIXEL_BYTES; // 131072

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotDecodeError {
    WrongLength { found: usize, expected: usize },
}

/// Pure: little-endian A1R5G5B5, row-major, top-down, no padding.
/// Returns WIDTH * HEIGHT * 4 RGBA8 bytes.
pub fn decode_a1r5g5b5(bytes: &[u8]) -> Result<Vec<u8>, SlotDecodeError>;

/// 5-bit -> 8-bit by bit replication: (v << 3) | (v >> 2).
pub fn expand5(v: u8) -> u8;
```

Per pixel: `v = u16::from_le_bytes([lo, hi])`; `r = expand5(((v >> 10) & 0x1F) as u8)`,
`g = expand5(((v >> 5) & 0x1F) as u8)`, `b = expand5((v & 0x1F) as u8)`,
`a = if v & 0x8000 != 0 { 255 } else { 0 }`. Bytes are written in row-major
order; no flip, no alpha premultiplication.

Container-side helper (in `textures.rs`, keeps all file I/O there):

```rust
/// Read exactly one slot, classify it, and decode it. Errors on empty/zero
/// classes and on a payload whose length is not the slot size.
pub fn decode_slot_at(
    path: &Path,
    header: &BigHeader,
    index: u16,
) -> Result<(u64, Vec<u8>), TexturesError>;
```

## Data flow

```
--tshow: path -> metadata -> parse_header -> validate_geometry
         -> parse selector -> range check
         -> decode_slot_at (one seek, one 131072-byte read, classify, convert)
         -> stdout verifying line -> render::show_rgba8(256, 256, rgba) -> exit 0
         empty/zero: loud error, exit 1, no window
```

## Contract preservation boundary

| Surface | Status |
|---|---|
| `parse_header`, `HEADER_SIZE`, `MAX_ENTRIES`, geometry messages | Unchanged |
| `expected_len`, `slot_offset`, `validate_geometry` | Unchanged |
| `SlotClass`, `classify_slot`, `scan_slots`, `SlotScan::report` | Unchanged |
| `--tlist` output (summary, addressing, slot lines, footer) | Unchanged |
| Selector and range errors | Unchanged |
| `--tshow` on `empty` / `zero` | Still exit 1; message drops the "decompressor UNKNOWN" clause |
| `--tshow` on `data` | Changed: decodes and shows, exit 0 |
| `TexturesError::SlotNotDecodable` | Replaced by `SlotNoPayload { index, offset, class }` for empty/zero only |

The struck wording matters: the old message asserted a compression that does not
exist. Keeping it would make the code lie.

## Decisions

| Decision | Choice | Alternatives | Rationale |
|---|---|---|---|
| Module split | New `slot_texture.rs` | Grow `textures.rs` | Container and pixel format are separate concerns; `textures.rs` is already ~800 lines |
| Input type | `&[u8]` with a length check | `&[u8; 131072]` | A fixed array forces an awkward copy and hides the error path; the length error is testable |
| 5→8 expansion | Bit replication | `v * 255 / 31` | Replication is branch-free, exact at 0 and 31, and standard; a wrong ramp is visible as banding |
| Alpha | Bit 15 → 0/255 | Ignore alpha | The bit is set in ~99% of sampled pixels; dropping it would break any masked texture |
| Orientation | Top-down, no flip | Bottom-up | Sampled signage is legible at 256 wide with top-down rows |
| Empty/zero handling | Loud error before decode | Decode as a blank image | A window of black would claim a decode that carries no payload |
| Success output | One stdout line plus window | Window only | The line makes the decode checkable in a terminal and in a playtest log |
| CLI test strategy | Pure-unit tests + error-path CLI tests | Attempt a window in `cargo test` | The existing suite never opens a window; keep that property |
| Uncompressed verdict | Encode it in docs and messages | Keep "UNKNOWN" wording | The wording must match verified behavior, not the original premise |

## Out of scope

- Decompressors for any format (none is needed).
- Slot-to-texture-name mapping via `TEX_HD.MAP` or `FX_TEX_MAP.TXT`.
- Atlas slicing, mip extraction, batch export, PNG writing to disk.
- Other dimensions than 256 × 256, other pixel formats, other containers'
  non-texture semantics.
- SR2, DRM, or any protection work.
- Ghidra. It is not needed for this slice and stays deferred.

## Testing strategy

| Layer | What | Approach |
|---|---|---|
| Unit (pure) | `expand5` endpoints and midpoints; length guard; known pixel patterns to RGBA8; alpha bit; row order | In-memory synthetic slices |
| Unit (I/O) | `decode_slot_at` reads one slot; empty/zero errors; wrong-length payload error | Synthetic containers of exact geometry in temp dirs |
| CLI | Selector/range/empty/zero remain exit 1 with their messages; `--tlist` unchanged | `run_textures` on synthetic temp files |
| Manual | `--tshow` on several real slots shows recognizable images | Playtest note in the PR/commit body (file + what was seen) |
| Regression | Header, geometry, listing, all other modes | Existing suite stays green; no game files |

## Threat matrix

N/A — no routing, shell, subprocess, VCS/PR automation, executable
classification, or process-integration boundary. Inputs are paths to files the
user already owns; malformed input fails loud.

## Migration / rollout

No data migration, no flag, no persisted state. The only behavioral change is
`--tshow` on a `data` slot. Reverting the commits restores the loud verdict.

## Open questions

- [ ] Is every TEXTURES data slot a valid image? Sampled, not exhaustive; the
  slice does not need the answer, a future gallery slice does.
- [ ] Is the alpha bit semantic everywhere? Masked textures will show it; if a
  slot looks wrong only in alpha, revisit.
- [ ] Is the channel order R/G/B? A playtest on a known texture answers it.

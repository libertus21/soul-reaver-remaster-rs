# `.BIG` slot payload format — raw 16-bit texture data, not compressed

Follow-up probe after `s6-big-slot-geometry.md`. Bounded, read-only: whole-slot
reads of sampled slots, byte-statistics probes, and offline renders of
unknown-format bytes to images. No extraction, no game content committed, no
dump of payload bytes anywhere in this repo.

## Investigation box

| Item | Planned | Actual |
|---|---|---|
| Pass 1 — signature + statistics | ≤30 min | ~35 min, DECISIVE |
| Pass 2 — community docs | ≤30 min | not run (stopped at first strong lead) |
| Pass 3 — Ghidra call path | ≤30 min | not run (stopped at first strong lead) |
| Ghidra project | outside repo | not created |

The dispatch says to stop as soon as one pass yields a strong lead. Pass 1 did:
the payload renders as a coherent image with no transform at all. Passes 2 and 3
stay available as the fallback if a playtest ever contradicts this note.

## Verdict (CONFIRMED for the four containers sampled)

`data` slots are **NOT compressed**. Each one is a raw, headerless
**256 × 256 image of little-endian 16-bit A1R5G5B5 pixels**:

```
131072 bytes = 256 * 256 * 2 bytes/pixel
```

There is no per-slot header, no size field, no dictionary, and no transform.
The "repeating 16-bit symbols" that suggested a dictionary/LZ stream are simply
**flat-coloured image regions**: a fill area is the same 16-bit pixel value
repeated, which is exactly what a raw surface looks like.

Consequence for the rewrite: `--tshow` does not need a decompressor. It needs a
pixel-format converter (A1R5G5B5 → RGBA8) and the existing window path.

## Evidence ledger

| Hypothesis | Verdict | Evidence (measured) |
|---|---|---|
| zlib / deflate stream | RULED OUT | No zlib CMF/FLG pair at slot start; payload bytes are spatially coherent as pixels; no block boundaries |
| LZ4 / LZSS / LZO | RULED OUT | No frame magic; byte autocorrelation at lag 2 is near zero while lag 1 is large (2-byte pixel unit, not a token stream); flat regions repeat a 16-bit value per pixel, not offsets/lengths |
| RLE | RULED OUT | No run marker or (count, value) pair structure; a solid area occupies one 16-bit value per pixel, i.e. uncompressed |
| BC1/BC3 (DXT) | RULED OUT | BC1 decoding of the same bytes is unstructured noise on slots that are plainly flat as 16-bit pixels; 131072 bytes would be a 512×512 BC1 image, and that render shows no image |
| 8-bit paletted indices | RULED OUT | 8-bit rendering shows a forced 2-pixel vertical stripe everywhere; lag-2 ≈ 0 means the two bytes belong to one pixel, not two pixels |
| RGB565 | RULED OUT | Renders implausible magenta/olive colouring on natural surfaces |
| RGBA4444 | RULED OUT | Same implausible colouring; channel bit weights do not match |
| **Raw A1R5G5B5, 256 × 256** | **SUPPORTED** | Natural imagery (stone, faces, metal, legible signage) with correct colours; exact size fit; bit 15 set in 98.9% of pixels over 40 random TEXTURES slots (consistent with an alpha mask that is mostly opaque) |
| Per-slot header / index | RULED OUT | 131072 is exactly 256·256·2, leaving no room; image content starts at byte 0 of the slot |

## Method (what actually produced the evidence)

1. **Geometry fit.** 131072 bytes divides exactly by 2 bytes/pixel into
   65536 pixels, i.e. 256 × 256. No remainder for a header.
2. **Autocorrelation.** In an 8-slot detailed sample, mean absolute byte
   difference at lag 1 is large (74–100) while at lag 2 it is small (0.2–20).
   Odd lags crossing pairs stay large; even lags stay small. The same 2-byte
   unit signature holds in 28 of 40 randomly sampled slots; the other 12 are
   smooth images where lag 1 is already small. A compressed token stream does
   not behave this way.
3. **Alpha-bit census.** Bit 15 of each little-endian `u16` is set in 98.9% of
   pixels across 40 random TEXTURES slots; per-slot rates elsewhere range
   71.7%–100% (BONUS/MOVIES/E3 samples). That is an alpha mask that is mostly
   opaque, not noise.
4. **Render matrix.** Each sampled slot was rendered offline four ways
   (RGB565, A1R5G5B5, RGBA4444, 8-bit indices). Only **A1R5G5B5** produces
   natural images; the others produce implausible colours or the 2-pixel
   stripe artifact.
5. **Cross-container sample.** TEXTURES.BIG (slots 2–7, 10, 50, 125, 200, 400,
   700, 900, 1286), MOVIES.BIG (2, 3, 10, 28), BONUS.BIG (2, 3, 4, 100) and
   E3.BIG (2, 3, 50) all render as coherent images under the same decode.
6. **Contact sheet.** 64 random TEXTURES slots and 32 random BONUS slots,
   thumbnailed into one grid each: every cell is a plausible texture or a
   flat-colour fill. No cell looks like a failed decode.
7. **Orientation.** One sampled atlas contains legible signage lettering that
   reads upright at 256 pixels wide, so the rows are top-down with no vertical
   flip and the width really is 256 (`u16` per pixel, 512 bytes per row).

## Format statement

| Property | Value | Confidence |
|---|---|---|
| Dimensions | 256 × 256 | High (exact fit + readable text at that width) |
| Pixel size | 2 bytes, little-endian | High |
| Layout | A1R5G5B5: bit 15 alpha, bits 14–10 red, 9–5 green, 4–0 blue | High (visual) |
| Row order | top-down, left-to-right, no padding | High (visual) |
| 5→8 bit expansion | bit replication: `(v << 3) \| (v >> 2)` | Design choice, matches common practice; verify visually |
| Row stride | 512 bytes (256 × 2) | High |
| Per-slot header | none | High (no room, content at offset 0) |

## Container-wide observations

| Container | Slots | Classification | Sampled slots |
|---|---|---|---|
| TEXTURES.BIG | 1287 | full scan: 1 all-`0xFF` (slot 0), 0 all-zero, 1286 data | 15 |
| MOVIES.BIG | 29 | sampled only (slot 0 all-`0xFF`, slot 1 near-black) | 4 |
| BONUS.BIG | 268 | sampled only | 4 |
| E3.BIG | 178 | sampled only | 3 |

- Slot 0 is byte-identical in TEXTURES.BIG and MOVIES.BIG (all `0xFF`).
- Slot 1 is also byte-identical between those two containers, but it is **not**
  all-zero: about 93% zero bytes plus a small non-zero patch, i.e. a near-black
  image. Under the whole-slot rule it classifies `data`.
- TEXTURES.BIG has 412 distinct 4-byte tails across its 1287 slots, so there is
  no fixed footer or trailer signature.
- Many slots are partly or wholly flat colour. Those flat fills are the reason
  the head bytes repeat; they are not compression markers.

## Corrections to `s6-big-slot-geometry.md`

| Prior statement | Correction |
|---|---|
| "Slot 1 in both files is zero-filled" | Near-black, not all-zero (about 93% `0x00` plus a small patch). It classifies `data`, not `zero`. |
| "`data` slots are COMPRESSED … algorithm UNKNOWN" | Not compressed. Raw A1R5G5B5, 256 × 256, no header. The "dictionary-like" heads are flat-colour pixel runs. |
| "`--tshow` cannot render a texture until the decompressor is identified" | `--tshow` can render now; no decompressor exists to identify. |

## Remaining unknowns

- Whether every one of the 1286 TEXTURES data slots is a valid image (sampled,
  not exhaustive; the contact sheet covered 64).
- Whether the alpha bit is meaningful transparency or a constant flag in some
  containers (98.9% set on average).
- Whether channel order is definitely R/G/B and not B/G/R. Visual evidence says
  R/G/B; a DDS cross-check was not run.
- What the blank slot 1 is for (shared null texture is the plain reading).
- How slot indices map to texture names (`TEX_HD.MAP` is the likely table) —
  out of scope for this slice.

## Reproducibility and hygiene

- Scratch analysis code and rendered images lived under
  `%TEMP%\opencode\bigprobe\` only; they are not part of the repo.
- No Ghidra project was created; no game binary was opened in this probe.
- `git status --short` after this note shows only the new `*.md` files.

## Legal

Format facts and measured statistics only. No payload dumps, no code listings,
no binary addresses, no install paths. All renders and scratch outputs stayed
outside the repo. Game data was opened read-only.

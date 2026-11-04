# Archive: room-textures — DONE (room UVs sample TEXTURES.BIG pages 379..1286)

**Stage banner: DONE.** Room geometry is textured. Each room has a 12-byte
record array bounded by room `+0x34` (start) and `+0x38` (end), both
data-base-relative; face `+0x0A` is a byte offset into that array.

## Delivered and verified

- Record shape: `TextureFT3` minus the trailing colour word — `+0 u8 u0`,
  `+1 u8 v0`, `+2 u16 textureWord`, `+4 u8 u1`, `+5 u8 v1`,
  `+6 u16 attributesA`, `+8 u8 u2`, `+9 u8 v2`, `+10 u16 attributes`.
- `page = textureWord & 0x07FF`; rooms use **pages 379..1286** while objects use
  2..378, so the two families do not overlap in the 1,287-page bank.
- Coverage: **372 rooms / 574,458 faces / 86,222 records**. `B` is inside the
  array and 12-aligned for **99.76%** of faces. The 214 `0xFFFF` tails and 1,160
  non-aligned tails are counted as unresolved and drawn flat; they are never
  dropped or clamped.
- Allocation is ordered: 371/371 rooms use one contiguous page range, with zero
  inversions in 370 archive-order comparisons.
- Face `+0x08` (`A`) is not a page or texture id; it is a signed-looking
  per-face attribute and its role stays UNKNOWN.
- Implementation: `src/level.rs` `room_texture_faces` / `textured_room_mesh`,
  the shared `PageBank`, and `--level-render ... --textures`.

## Limits and merge

The room record omits the colour word, so room blend/colour behaviour is
UNKNOWN; the meanings of the flag bits and `A` also remain UNKNOWN. PR #76
(docs) and #77 (room parse, scene and CLI) merged into `main` at `577c24b`. No
game bytes, decoded imagery, payload dumps or absolute paths were committed.

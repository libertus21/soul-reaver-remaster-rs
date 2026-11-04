# Archive: object-textures — DONE (TextureFT3 faces sample TEXTURES.BIG)

**Stage banner: DONE.** Archived object textures are not inside the object
container. Each textured face carries one 16-byte PC `TextureFT3` record, and
the record's low 11 bits index the legacy `TEXTURES.BIG` bank.

## Delivered and verified

- Record: `+0 u8 u0`, `+1 u8 v0`, `+2 u16 textureWord`, `+4 u8 u1`,
  `+5 u8 v1`, `+6 u16 attributesA`, `+8 u8 u2`, `+9 u8 v2`,
  `+10 u16 attributes`, `+12 u32 colour`.
- `textureWord & 0x07FF` is the page; the upper five bits are flags and never
  change the page. UV bytes top out at 254; orientation is direct (`v` top-down).
- The bank is `TEXTURES.BIG`: a 4,096-byte prefix, then 256x256 little-endian
  A1R5G5B5 pages of 131,072 bytes, no palette and no compression. Page `p`
  starts at `4096 + p * 131072`.
- Objects use pages **2..378**. Across 394 single-model containers and 32,370
  textured faces the sequence is dense, non-decreasing in archive order, with
  zero inversions and only three gaps (7, 112, 154) from untextured models.
- Capacity rules out embedded pixels: `morlock` needs two pages (262,144 bytes)
  but its whole payload is 56,684 bytes. The auxiliary tail is not texture data.
- Implementation: `src/object.rs` `TextureRef` / `textured_mesh`,
  `src/object_texture.rs` `PageBank`, and `--object-mesh ... --textures`.
## Limits and merge

Loose `TEX\*.DDS` files are re-authored remaster assets and do not match these
legacy pages; the bank is the only source. Flag-bit, attribute-word and colour
alpha meanings remain UNKNOWN. PR #74 (docs) and #75 (parser, page bank,
textured render) merged into `main` at `7d4add0`. No game bytes, decoded imagery,
payload dumps or absolute paths were committed.

# Design: object-textures (record view, bank reader, textured render)

## Decisions

| Topic | Decision | Why |
|---|---|---|
| Record type | `TextureRef { uvs: [(u8,u8);3], page: u16, attributes_a: u16, attributes: u16, colour: u32 }` stored per polygon | The record is per face, not shared; two identical faces still own two records |
| Page mask | `page = word & 0x07FF`; keep the raw word for diagnostics | Community reader uses the same mask; the high bits are UNKNOWN flags and must not leak into the index |
| Bank access | New pure module reading `TEXTURES.BIG` by page index; no new dependency | Same file shape as `textures.rs`/`slot_texture.rs`, but the object path addresses pages directly and must not require the `TEX_HD.MAP` |
| Decode | Reuse the existing A1R5G5B5 converter from `slot_texture.rs` | One implementation, one set of tests |
| Orientation | Texel row = `v`; no flip | Direct orientation produced fewer transparent hits on both sampled models; `s7` established top-down rows |
| Sampling | Nearest texel at the integer UV; no filtering in the sampler arm | Keeps the sample arm deterministic and comparable to the note's numbers |
| Render | One RGBA8 page texture per model, one draw per face group | A legacy model uses 1-3 pages; no atlas repacking in this slice |
| Failure | Loud on page out of range, empty/zero slot, or out-of-bounds texel | Never clamp, never substitute a loose DDS |
| Existing contracts | Without `--textures`, `--object-info` adds only `pages=` (documented breaking-by-addition) and the mesh arm is unchanged | The note is the only new truth; existing tests pin the old text where it matters |

## Data flow

```text
BIGFILE.DAT entry
  -> object container (existing parse_object)
  -> per polygon: materialOffset -> TextureFT3 record
  -> page = word & 0x07FF, UVs (u,v) bytes
  -> TEXTURES.BIG page = 4096 + page*131072, 256x256 A1R5G5B5
  -> texel = 2*(u + 256*v); expand to RGBA8
  -> sample arm prints numbers / mesh arm uploads one page + UVs
```

## Module plan

| Area | Impact | Description |
|---|---|---|
| `src/object.rs` | Modified | Add `TextureRef`, store it beside each polygon, expose `pages()`; keep validation unchanged |
| `src/object_texture.rs` | New | Pure `TextureFT3` decode, page mask, texel address and A1R5G5B5 sample; no I/O |
| `src/textures.rs` | Modified | Expose page-count + page-slice read for a caller-supplied index (reuse `BigHeader`) |
| `src/main.rs` | Modified | `--object-info pages=`; new `--object-sample`; `--object-mesh --textures` |
| `README.md` | Modified | One bullet: archived objects are textured from `TEXTURES.BIG` page = record word & 0x7FF |
| `docs/notes/object-textures.md` | Modified | Reader section only after the code lands |

## Test strategy

- Synthetic 16-byte records: field split, mask, high-bit flags, UV bounds.
- Synthetic bank: 3 pages, empty/zero/data classes, page out of range, last
  texel of the last page.
- Contract tests for the three CLI arms: usage, malformed record, bad page.
- No test reads a game file; no fixture contains game bytes.

## Legal

Synthetic fixtures only. The note and code describe format facts; no game
bytes, decoded imagery, payload dumps or absolute install paths.

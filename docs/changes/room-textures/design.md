# Design: room-textures (record view, page summary, textured room)

## Decisions

| Topic | Decision | Why |
|---|---|---|
| Address model | Room record `+0x34` = start offset, `+0x38` = end offset; a face selects a 12-byte record with `start + B` | Corpus: span is a multiple of 12 in 372/372 rooms; `B` is 12-aligned and inside the span for 99.76% of faces |
| Record type | `RoomFaceTexture { uvs: [(u8,u8);3], page: u16, attributes_a: u16, attributes: u16 }` | Same field order as object `TextureFT3` minus the colour word; no colour field is invented |
| Face tail | Typed accessors `face_a` (`+0x08`) and `face_texture_offset` (`+0x0A`); `A` is carried but never read as a page | `A` is signed, symmetric and agrees with the page at or below chance |
| Tail faces | `B == 0xFFFF` or `B % 12 != 0` is a counted `tail` face; the sample arm prints `status=tail`, the render arm skips and reports `tail=<n>` | 1,374 faces (0.24%), 1,157 in the last 10% of a room's list; never a silent drop |
| Page mask | `page = word & 0x07FF`; keep the raw word | Same mask as objects; high bits are flags and must not leak into the index |
| Bank access | Reuse the existing `TEXTURES.BIG` page reader and A1R5G5B5 converter | One implementation, one set of tests; no new module for the same bank |
| Orientation | Texel row = `v`; no flip | `city2`: 1.1% transparent direct vs 13.9% flipped; matches object-textures T9 |
| Sampling | Nearest texel at the integer UV; no filtering in the sample arm | Deterministic and comparable to the note's numbers |
| Render | One RGBA8 page per referenced page, reuse the existing textured room path; tail faces reported | A room uses 1-7 pages; no atlas repacking in this slice |
| Failure | Loud on `end <= start`, `(end-start) % 12 != 0`, `B >= end-start`, `B % 12 != 0` (sample), bad page, empty/zero slot, out-of-bounds texel | Never clamp, never substitute a loose DDS |
| Existing contracts | Without `--textures`, `--level-geometry` adds only `pages=`/`tail=` and the render arm is unchanged | The note is the only new truth; existing tests pin the old text where it matters |

## Data flow

```text
BIGFILE.DAT entry (level .PCM)
  -> header slot 0x00 -> room record
  -> +0x14 count / +0x20 offset -> 12-byte face array
  -> face +0x0A -> B -> room +0x34 + B -> 12-byte UV/texture record
  -> page = record +0x02 & 0x07FF, UVs at +0/+1, +4/+5, +8/+9
  -> TEXTURES.BIG page = 4096 + page*131072, 256x256 A1R5G5B5
  -> texel = 2*(u + 256*v); expand to RGBA8
  -> sample arm prints numbers / render arm uploads one page + UVs
```

## Module plan

| Area | Impact | Description |
|---|---|---|
| `src/level.rs` | Modified | Add the room texture table (`+0x34/+0x38`), the 12-byte record view, the `B` lookup and tail classification; expose `pages()` and `tail_count()`; keep the existing geometry/object readers unchanged |
| `src/level_texture.rs` | New | Pure room record decode, page mask, texel address and A1R5G5B5 sample; no I/O, no game bytes |
| `src/textures.rs` / `src/slot_texture.rs` | Maybe modified | Expose/reuse the page-count and page-slice reader for a caller-supplied index |
| `src/main.rs` | Modified | `--level-geometry pages=/tail=`; new `--level-sample`; `--level-render --textures` |
| `README.md` | Modified | One bullet: rooms are textured from `TEXTURES.BIG` pages 379..1286 |
| `docs/notes/room-textures.md` | Modified | Reader section only after the code lands |

## Test strategy

- Synthetic room records: `+0x34/+0x38` bounds, span not a multiple of 12,
  `B` out of range, `B` non-aligned (tail), `B = 0xFFFF`.
- Synthetic 12-byte records: field split, page mask, high-bit flags, UV bytes
  0 and 255.
- Synthetic bank: 3 pages, empty/zero/data classes, page out of range, last
  texel of the last page.
- Contract tests for the three CLI arms: usage, malformed record, bad page.
- One model-free synthetic unit proves the room path does not touch objects.
- No test reads a game file; no fixture contains game bytes.

## Legal

Synthetic fixtures only. The note and code describe format facts; no game
bytes, decoded imagery, payload dumps or absolute install paths.

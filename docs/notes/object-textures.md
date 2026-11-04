# object-textures - the 16-byte face record and where archived object textures live

Bounded, read-only byte probe of the SR1 remaster archive, plus a bounded
community-source check. Behaviour/format facts only: field offsets, counts,
index arithmetic and a handful of sampled texel values. No game bytes, no
decoded imagery, no payload dumps, no absolute install paths. Scratch lived in
`%TEMP%\opencode\objtex`; game files were opened read-only.

Date: 2026-11-04. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable test), **INFERRED** (one side only), **UNKNOWN**.

## Verdict

- **The per-face 16-byte record is the PC `TextureFT3` layout, and it is
  CONFIRMED byte-for-byte.** It carries three UV pairs, one 16-bit texture word,
  two attribute words and one ARGB colour word. The texture word is **not** a
  pointer and **not** a palette index.
- **Archived object textures do NOT live inside the object container.** The
  container tail is the auxiliary record table, not pixels. The capacity test is
  decisive: `morlock` uses two texture pages but its whole payload is 56,684
  bytes, less than one 131,072-byte page.
- **They live in `1\TEXTURES.BIG`, at slot = low 11 bits of the texture word.**
  That container is the legacy PC monolithic texture bank: 4,096-byte prefix,
  then 256x256 little-endian A1R5G5B5 pages, 131,072 bytes each, no per-slot
  header. The texture word's low 11 bits are the bank page index; the upper 5
  bits are flags.
- **No palette and no index buffer exist.** Every page is direct 15-bit colour
  plus 1-bit alpha. The "palette + indexed pixels" hypothesis is ruled out.
- **The loose `TEX\*.DDS` files are NOT the source for these models.** They are
  the remaster's re-authored textures: pixel correlation against the legacy
  pages is at or below noise (max 0.46 over the sampled slots, and 0.07 for
  `raziel`'s page against 16 aliases).
- **The index mapping is ordered, dense and falsifiable.** Across 394
  single-model containers and 32,370 textured faces, page indices run 2..378
  with **zero inversions** against archive entry order and only three gaps
  (7, 112, 154), each explained by a model whose faces are untextured. This is
  a sequential bank allocation, not a per-model texture list.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| T1 | Record stride is 16 bytes and there is exactly one record per textured face: 574/574 distinct records for `raziel`, 315/315 for `morlock`, contiguous offsets. | CONFIRMED |
| T2 | Field split: `+0 u8 u0`, `+1 u8 v0`, `+2 u16 textureWord`, `+4 u8 u1`, `+5 u8 v1`, `+6 u16 attributesA`, `+8 u8 u2`, `+9 u8 v2`, `+10 u16 attributes`, `+12 u32 colour`. | CONFIRMED |
| T3 | Object platform is PC, not PSX: the object header `+0x44..+0x4B` is `FFFFFFFFFFFFFFFF` in both sampled containers, which selects the PC `TextureFT3` branch. | CONFIRMED |
| T4 | UVs are bytes with a corpus maximum of 254; all three pairs sit at `+0/+1`, `+4/+5`, `+8/+9`. | CONFIRMED |
| T5 | `textureWord & 0x07FF` is a bank page index. Indices across 394 models are dense 2..378, unique per model, non-decreasing in archive order (0 inversions / 365 comparisons). | CONFIRMED |
| T6 | The texture bank is `1\TEXTURES.BIG`: 512-byte header (`u16 512`, `u16 1287`), data at 4,096, stride 131,072, 256x256 A1R5G5B5. | CONFIRMED |
| T7 | Textures cannot be inside the object container: `morlock` needs 2 pages (262,144 bytes) but its entire payload is 56,684 bytes; `raziel`'s post-material tail is 180,524 bytes and is fully accounted for by 230 auxiliary records, with no 65,536-byte or 131,072-byte block. | CONFIRMED |
| T8 | Loose `.DDS` are not the legacy pages: best correlation for page 4 against its 16 `TEX_HD.MAP` aliases is 0.07; for page 52 it is 0.09. | CONFIRMED (negative result) |
| T9 | UV orientation is direct (texel row = v), not vertically flipped: sampling flipped raises transparent-texel hits from 4.4% to 5.4% (`raziel`) and from 1.6% to 12.1% (`morlock`). | CONFIRMED (measurement) / INFERRED (convention) |
| T10 | The colour word is grey with a variable alpha byte (`0x34808080`, `0x24808080`, ...); low 24 bits are a tint, not a palette index. | CONFIRMED (values) / UNKNOWN (blend use) |
| T11 | Upper 5 bits of the texture word are flags. Observed high-bit groups `0x0800`, `0x1000`, `0x2000`, `0x4000`, `0x6000`, `0x8000`, `0xA000`; they never change the low page index. | CONFIRMED (masking) / UNKNOWN (meaning) |
| T12 | Slots 0 and 1 are null pages (slot 0 all `0xFF`, slot 1 near-black) and no model uses them; the first used page is 2. | CONFIRMED (data) |

## 1. The 16-byte per-face record

One textured face points at exactly one 16-byte record (`materialOffset`
increments by 16). The record is the PC-port `TextureFT3`:

| Offset | Size | Field | Evidence |
|---:|---:|---|---|
| `+0` | 1 | `u0` first UV U | T4 |
| `+1` | 1 | `v0` first UV V | T4 |
| `+2` | 2 | texture word: low 11 bits = page, high 5 bits = flags | T5, T11 |
| `+4` | 1 | `u1` second UV U | T4 |
| `+5` | 1 | `v1` second UV V | T4 |
| `+6` | 2 | `attributesA` (bit `0x0020` selects blend mode in the community reader) | T2, ledger T11 note |
| `+8` | 1 | `u2` third UV U | T4 |
| `+9` | 1 | `v2` third UV V | T4 |
| `+10` | 2 | second attributes word; unread by the PC path | T2 |
| `+12` | 4 | ARGB colour word | T10 |

### Why this split and not another

Three splits were tested against the corpus, not chosen by eye:

1. **Interleaved order matters.** The first guess "UVs at `+0/+2/+4`" is
   impossible: the u16 at `+2` would then be a UV value, but it is the only
   field that forms a dense, strictly ordered 2..378 sequence across archive
   entries. UVs never exceed 254 (T4), while this field reaches 378.
2. **The u16 at `+6` cannot be the texture id.** Its corpus values are only
   seven distinct values (264, 265, 266, 296, 267, 360, 328). A texture index
   would not be constant across unrelated models and would not reproduce the
   archive-order allocation.
3. **The u16 at `+10` cannot be a UV or a page.** It has 25 distinct values
   including `0x3100`, `0xFB00` and `0xFF00`; it is a flag word.
4. **The colour word only makes sense at `+12`.** It is a little-endian ARGB
   value whose low 24 bits are a neutral grey tint (`0x808080`) in 100% of the
   32,370 textured faces sampled; the top byte varies (0x24, 0x26, 0x34, 0x36).

The community reader uses the same split and masks the texture word with
`0x07FF`; see section 5.

### The texture word

- Bits 0..10: page index into the texture bank (0..2047).
- Bits 11..15: flags. They are not part of the index. Masking with `0x07FF`
  makes 460 distinct raw words collapse to 374 distinct pages without losing a
  single allocation.
- Highest observed raw word: 41,301 (`0xA155`), which masks to page 341.

## 2. Where the texture data lives

### 2.1 Not in the object container

The object payload after the material array is the auxiliary table. For
`raziel` the material array ends at 22,116 and the table covers the payload up
to 202,640; for `morlock` the material array ends at 11,952 and the next table
entry begins there. Even if one ignored that, capacity rules it out:

| Model | Pages | Bytes needed at 131,072/page | Whole payload |
|---|---:|---:|---:|
| `raziel` | 1 | 131,072 | 202,640 |
| `morlock` | 2 | 262,144 | **56,684** |

`morlock` alone proves that the pixels are external. There is also no palette
block and no index plane anywhere in the tails.

### 2.2 The `TEXTURES.BIG` monolithic bank

`1\TEXTURES.BIG` is the legacy PC texture bank:

```text
[ 4,096-byte prefix ][ page 0 ][ page 1 ] ... [ page N-1 ]
                        131,072 bytes per page
```

- Header: `u16 512` (header size), `u16 1287` (page count); the rest of the
  4,096-byte prefix is not part of a page.
- Page `p` starts at `4096 + p * 131072` and is exactly 131,072 bytes.
- Page content: 256 x 256 little-endian A1R5G5B5, 512 bytes per row, top-down,
  no header, no palette, no compression.
- Page index = `textureWord & 0x07FF` (T5).
- Slot 4 (`raziel`) starts at 528,384. Slot 52 (`morlock` page A) starts at
  6,819,840.

This matches the community's "Soul Reaver (PC) Uncompressed ARGB 1555
Monolithic Texture File" format: 4,096-byte header plus 131,072 bytes per
texture, and the file extension it recognises is `.BIG`. That is the same file
shape observed here, independently of any ModelEx code.

### 2.3 Why the slot order is trusted

The mapping is not "assume slot = texture id because the ranges overlap". It
is the observed allocation order of the archive itself:

- 394 single-model containers, 32,370 textured faces, 374 distinct pages.
- Page indices are **non-decreasing in archive entry order: 0 inversions in
  365 adjacent comparisons**.
- Seven multi-page models use consecutive pages (`morlock` 52/53, `walboss`
  110/111, `kain` 122/123, `babar` 232/233, `roninbss` 307/308, `skinbos`
  322/323/324, `astat` 326/327).
- Pages 2..378 are dense apart from three gaps (7, 112, 154); each gap sits at
  an untextured model (e.g. `force` between `soul` page 6 and `particle` page 8),
  which is what sequential allocation of a page per model produces.
- The first archive objects match the first pages: `sphere` -> 2, `box` -> 3,
  `raziel` -> 4, `glphicon` -> 5, `soul` -> 6, `particle` -> 8.

For the index order to be this dense and ordered by chance while pointing at an
unrelated bank would be implausible; the container format and the index order
agree.

### 2.4 The loose `.DDS` tree is not the source

Two independent checks, both negative:

1. `raziel`'s page 4 against the 16 `TEX_HD.MAP` aliases for row 4
   (`4_0.DDS` .. `4_14.DDS`, `100_10.DDS`): best correlation 0.072.
2. `raziel`'s page 4 against every `RAZIEL_*_D.DDS` remaster texture: best
   correlation 0.161, with a random-slot baseline of 0.007 mean / 0.145 max.

So the loose DDS files are a parallel, re-authored asset family. They are on
disk and named, but they are not what a legacy face samples. The archived
model must be textured from `TEXTURES.BIG` itself.

## 3. Proof: one `raziel` face

`raziel` is archive entry 66; its first textured face is polygon 0:

| Property | Value |
|---|---|
| Vertex indices | 124, 123, 159 |
| Polygon normal / flags | 93 / `0x07` (texture-used bit set) |
| Material record offset | 12,932 (16-byte aligned, first record) |
| Texture word | `0x0004`; page = 4 |
| UVs | `(40,134)`, `(22,134)`, `(41,128)` |
| `attributesA` | `0x0108` (no blend bit) |
| Colour word | `0x34808080` (grey tint, alpha 0x34) |

Sampling page 4 at texel `(u, v)` = byte offset `2 * (u + 256*v)` from
528,384 (`v` top-down):

| Vertex | UV | Raw A1R5G5B5 | RGB8 | Alpha bit |
|---|---|---:|---|---:|
| v0 | (40,134) | `0xBD68` | (123, 90, 66) | 1 |
| v1 | (22,134) | `0xACE4` | (90, 57, 33) | 1 |
| v2 | (41,128) | `0x9062` | (33, 24, 16) | 1 |
| centroid | (34,132) | `0xB947` | (115, 82, 57) | 1 |

These are measured texel values, not an image. The face lies on opaque,
warm-brown texels; the U span is 19 texels and the V span 6. The page is
present on disk and resolves to `TEXTURES.BIG` slot 4 (offset 528,384), and
`TEX_HD.MAP` row 4 names 16 on-disk loose aliases for that slot, but those
aliases do not contain the same pixels (section 2.4).

Repeating the sample with `v` flipped (`255-v`) gives (16,16,8), (57,41,24),
(8,0,0) for the same three vertices - darker and less coherent. Across all
`raziel` faces the flipped orientation raises transparent-texel hits from 4.4%
to 5.4%; for `morlock` from 1.6% to 12.1%. Direct `v` is the working
convention.

## 4. Palette / indexed-pixel question

There is no palette and no index plane. `TEXTURES.BIG` pages are direct
A1R5G5B5:

- 2 bytes per pixel, little-endian;
- bit 15 alpha, bits 14..10 red, 9..5 green, 4..0 blue;
- 256 x 256, row stride 512, top-down;
- 5-bit channels expand by bit replication `(v << 3) | (v >> 2)`.

The negative findings from `s7-big-slot-payload-format.md` (no zlib, no LZ,
no RLE, no BC1, no 8-bit indices) apply unchanged. The only thing this note
adds is that the object path uses the same bank, indexed by the face record.

## 5. Community research

Web search had returned HTTP 403 in the earlier note, so GitHub raw and the
GitHub API were used directly this pass (HTTP 200). No code or data was copied;
only field order and file-shape facts were read.

| Tool | URL | License | Relevance |
|---|---|---|---|
| ModelEx | https://github.com/TheSerioliOfNosgoth/ModelEx | LICENSE present (third-party notices); GitHub reports `NOASSERTION` (no SPDX id) | `CDC/Models/SR1Model.cs` documents the PC `TextureFT3` field order: `u0, v0, u16 textureWord, u1, v1, u16 attributesA, u2, v2`, then optional `u16 attributes`; the texture id is `textureWord & 0x07FF`. `CDC/Models/SR1ObjectModel.cs` skips 2 bytes and reads the colour word, and selects PC vs PSX from object `+0x44`. `CDC/Files/SR1File.cs` confirms the platform test. |
| Soul-Reaver-PC-Texture-Utility-Official (mirror of Ben Lincoln's tool, vendored in ModelEx) | https://github.com/TheSerioliOfNosgoth/Soul-Reaver-PC-Texture-Utility-Official | GitHub reports `NOASSERTION`; README mirrors Ben Lincoln's tool "used with permission" | `CDC/Textures/SoulReaverPCTextureFile.cs` documents the PC monolithic texture file: 4,096-byte header, 131,072 bytes per 256x256 ARGB1555 texture. Its `TextureFile.GetFileType` maps `.big` to that format. |
| io_scene_xrm (remaster Blender addon) | https://github.com/TheDodylectableX/io_scene_xrm | No LICENSE file | Remaster `.SRM` only; no archived `.PCM` reader and no `TextureFT3` handling. It explains why the named `.DDS` family exists but does not provide a mapping for legacy objects. |

The community reader names legacy textures `<object>_<textureID:X4>` (for
example `raziel_0004`). No such file exists in this install; the equivalent
data is the bank page. That naming rule is therefore a description of the
original PC install, not a path to use here.

## 6. UNKNOWN (explicit)

1. Meaning of texture-word bits 11..15. They are flags (T11), but which one is
   depth, semi-transparency, double-sided or scroll is not established.
2. Meaning of `attributesA` beyond bit `0x0020` (blend) used by the community
   reader. Corpus values are only `0x0108`, `0x0109`, `0x010A`, `0x0128`,
   `0x010B`, `0x0168`, `0x0148`.
3. Meaning of the `+10` attributes word (25 distinct values, including
   `0x3100`, `0xFB00`, `0xFF00`). The PC reader skips it.
4. Exact blend use of the colour word's alpha byte (0x24/0x26/0x34/0x36
   observed); the low 24 bits are a neutral grey tint in every sampled face.
5. Whether this remaster build repacked the legacy pages or merely preserved
   them at their original indices. The index order is preserved and the bank
   format is the legacy one; pixel identity against the original PC release was
   not tested.
6. The 6,308-byte stretch between `raziel`'s last material record and its first
   auxiliary record. It is not a texture (too small) and its role is unknown.
7. Ownership of the three untextured page gaps (7, 112, 154). The allocation
   order and the adjacent untextured models make them model pages, but that is
   inferred, not directly observed.
8. Whether the 1-bit alpha is transparency or a flag in every page. Overall it
   is set in 98.9% of pixels (`s7`), and the sampled `raziel` face is fully
   opaque.
9. What a future build does if it repacks `TEXTURES.BIG`; the reader should
   refuse loud when the page index is outside the header entry count.

## 7. Reproduction and hygiene

- Scratch scripts lived under `%TEMP%\opencode\objtex` only; images and dumps
  were never written into the repository.
- The game tree was opened read-only. `git status --short` after this note
  shows only the new `*.md` files.
- Corpus counts quoted here come from parsing all 394 single-model containers
  in `1\BIGFILE.DAT` and sampling `1\TEXTURES.BIG` by index.

## 8. Legal

Format and behaviour facts only: field offsets, strides, counts, index
arithmetic and a few sampled numeric texel values. No game bytes, no decoded
imagery, no payload dumps, no code listings, no absolute install paths. Game
data was opened read-only and never copied into the repository.

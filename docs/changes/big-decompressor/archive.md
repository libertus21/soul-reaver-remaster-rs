# Archive: big-decompressor — DONE (raw A1R5G5B5 slots render; no decompressor needed)

The slice was planned to build a decompressor and ended by proving none is
needed: `data` slots are uncompressed 256 × 256 little-endian A1R5G5B5 images.
`--tshow` now converts one slot to RGBA8 and renders it in the existing window;
`empty` and `zero` slots stay loud. Real textures from all four SR1 `.BIG`
containers render and were accepted in playtest. PR #30 is **MERGED** into
`main`; nothing outside `src/`, `README.md`, and the change docs was touched.

## Quick path

1. `git checkout main && git pull` — HEAD is `afe336e` (merge of PR #30).
2. `cargo run -- --textures <one .BIG from your own install> --tshow 4` — prints
   one decode line, shows a 256 × 256 image, exits 0 on close.
3. `cargo test` — 131/131 green on synthetic bytes only.

## Outcome

| Question | Answer |
|---|---|
| Was a decompressor built? | No. The name is kept for continuity; the payload has no stream to decompress. |
| What is a `data` slot? | 131072 bytes = 256 × 256 × 2, raw A1R5G5B5, no per-slot header, content at byte 0. |
| What changed in behavior? | `--tshow` on a `data` slot decodes and shows (exit 0). Everything else keeps its prior contract. |
| What is still open? | Slot → texture-name mapping (`TEX_HD.MAP` is the likely table) and exhaustive validity of all 1287 slots (sampled only). |

## Playtest evidence (accepted, recorded verbatim)

| Item | Evidence |
|---|---|
| Real textures render | Real textures render from `TEXTURES.BIG` / `BONUS.BIG` / `E3.BIG` / `MOVIES.BIG` slots: stone, faces, legible runes. |
| Atlas fill is padding, not a decode error | Large flat violet/brown regions are ATLAS FILL/PADDING, proven numerically: slot 50 has one value (`0xF8A6`) covering 57344 of 65536 pixels (87.5%) and its top-6 values cover ~99% → a mostly-blank page; slot 4's top-6 cover only 41% → real content. |
| Format discriminator | Under RGBA5551 the brown value `0xA420` decodes to pink (165, 33, 132), contradicting the observed brown; A1R5G5B5 gives (74, 8, 0) dark brown. A1R5G5B5 alpha is ~0.94–1.0 (plausible opaque atlas); RGBA5551 alpha would be ~0.02 (nonsense). |
| Gates | 131/131 tests, zero warnings on a cold isolated build, legal clean, repository private. |
| Converter review | `expand5 = (v << 3) | (v >> 2)`, bit 15 = alpha, row-major, no swizzle, `SLOT_BYTES = 256 * 256 * 2` enforced. |

The flat-colour regions that originally suggested a dictionary/LZ stream are
atlas fill and padding. That reading is now measured, not assumed, which is why
the "compressed payload" premise in the prior note was retracted rather than
worked around.

## Details

| Topic | Outcome |
|---|---|
| PR | [#30 — feat(textures): decode raw A1R5G5B5 slots in --tshow](https://github.com/libertus21/soul-reaver-remaster-rs/pull/30), state MERGED (`mergedAt` 2026-10-26T04:07:49Z), base `main`, head `feature/big-decompressor`, 9 files, +1023/−67. |
| Commits on `main` | `d6f0cc6` plan bundle (proposal/spec/design/tasks + s7 payload note) → `ac732af` pure A1R5G5B5 → RGBA8 converter → `064c13e` one-slot decode + no-payload errors → `6f0c018` `--tshow` shows one decoded slot → `afe336e` PR #30 merge. |
| Delivered in code | `src/slot_texture.rs`: `WIDTH`/`HEIGHT`/`PIXEL_BYTES`/`SLOT_BYTES`, `expand5`, `decode_a1r5g5b5`, `SlotDecodeError::WrongLength`. `src/textures.rs`: `decode_slot_at`, `SlotNoPayload { index, offset, class }` replacing `SlotNotDecodable`, updated module doc. `src/main.rs`: `--tshow` decode arm, one stdout line, render through `render::show_rgba8(256, 256, &rgba)`, updated usage text. |
| Verified contract | Decoder reads little-endian `u16` pixels row-major, top-down, no stride padding; bit 15 → alpha 0/255; bits 14–10 red, 9–5 green, 4–0 blue; 5→8 expansion by bit replication. Any length other than 131072 fails loud with `SlotPayloadLength`; no pad, no truncate, no guess. |
| Preserved contract | Header parsing, geometry validation, `--tlist` output, selector/range errors, and all other modes (single file, gallery, `--bigfile`, mesh, launcher) are unchanged. No new dependency. |
| Deliberately not built | Any decompressor, slot → name mapping, atlas slicing, mip extraction, batch export, other dimensions or pixel formats, SR2 or protection work. |
| Warnings (recorded) | (1) The slice name still says "decompressor"; it is kept as the change identifier, and the docs state plainly that no decompressor exists. (2) Exhaustive validity of the 1287 TEXTURES slots remains unproven; playtest plus contact sheets sampled them. |
| Branch cleanup | Deleted only branches verified fully contained in `main` (see the report in the archive PR). Uncontained branches were left in place. |
| Follow-ups | Slot → texture-name mapping via `TEX_HD.MAP` is the remaining texture gap. Exhaustive 1287-slot validity is sampled, not proven. Alpha semantics per container and channel order are visually settled but not cross-checked against a reference DDS. |
| Legal | Format and behavior facts only. No game bytes, no decoded imagery, no absolute game paths, no content dumps in any artifact or commit. Tests use synthetic bytes in temp dirs; the real containers are read-only playtest inputs. |

## Checklist

- [x] PR #30 MERGED and `main` contains all slice commits (HEAD `afe336e`)
- [x] Playtest evidence recorded verbatim (renders in four containers, atlas-fill proof, format discriminator, gates)
- [x] `docs/notes/s6-big-slot-geometry.md` annotated where it was wrong (payload compression, slot 1 all zero) and linked to `s7`
- [x] README "What works" carries the render capability; "What's missing" names slot → name mapping as the remaining texture gap
- [x] Branches deleted only after containment checks; kept branches reported with the reason
- [x] No game bytes, absolute game paths, offsets, or hashes added by this archive

## Next step

Scope the slot → texture-name mapping slice: establish how `TEX_HD.MAP` (and
possibly `FX_TEX_MAP.TXT`) associates names or IDs with slot indices, then let
`--tlist` print the name beside the index. Keep the same slice discipline:
proposal → spec → tasks → code → playtest, read-only probes, synthetic tests.

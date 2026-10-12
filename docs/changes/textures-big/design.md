# Design: textures-big

Same binary, same decode, same upload — but a NEW layout reader: `TEXTURES.BIG` is a custom container (no index at byte 0), so apply hunts for its entry table with bounded read-only probes and builds the smallest parser that the observed bytes support. When no table or no DDS entry is found, the slice lands as layout documentation plus loud CLI behavior plus synthetic-only tests, and points at the next candidate. Nothing is ever extracted to disk.

## Quick path

`main.rs` (NEW `--textures <path> --tlist | --tshow <entry>` arm; file/dir/`--bigfile` arms untouched) -> NEW `src/textures.rs` (`describe` region classifier over bounded reads -> optional `parse_table` + `read_entry`, both pure + loud errors) -> `dds.rs` (EXISTING `decode_memory` + `has_dds_magic`, zero changes) -> `render.rs` (EXISTING `show_rgba8` via the `show_texture` tail, zero changes) -> pixels.

## Layout verdict (planning probe, read-only, 2026-10-12)

`src/bigfile.rs` MUST NOT parse this file: the SR1PC count-at-0 test fails (count word reads in the tens of millions, far above `MAX_ENTRIES` 32768 — `parse_index` would correctly answer `NotBigfile`). The SR2 hash-table header reading fails on the same bytes (nonzero pad where the layout requires zero). Observed instead: a near-zero 4K header with a short tag-like prefix, `0xFF` fill extending at least 64K, then high-entropy data with no `DDS ` magic at any sampled window. The probe results below are the primary source for this container — no community reference documents the remaster layout (see spec-source table).

## Spec source (credit + license)

| Item | Value |
|---|---|
| Remaster TEXTURES.BIG layout | NO community documentation found. Web search (2026-10-12) surfaced only: the original-1999-PC `textures.big` (XentaxWiki `Legacy_Of_Kain:_Soul_Reaver_BIG` snippet: per-image 4096-byte header + 256x256 ARGB1555 pixels — fetch FAILED: https-mirror transport error, legacy host 404; license unverified), Lost Worlds Gex-engine notes (PC textures live in a separate `textures.big`; `https://www.thelostworlds.net/TechDocs/Soul_Reavers_Gex_Engine.html`), Soul Spiral (unpacks `bigfile.dat` ONLY, never `textures.big`; `https://www.thelostworlds.net/Software/Soul_Spiral.html`), Only Skin Deep + Soul Reaver PC Texture Utility v2.4 incl. source (open/export the ORIGINAL `textures.big` to PNG; `https://www.thelostworlds.net/Software/Soul_Reaver_PC_Texture_Utility.html`), Recombobulator (`https://github.com/TheSerioliOfNosgoth/Recombobulator`, packs original `bigfile.dat` + `textures.big`), cdcengine.re bigfile docs (SR2+ hash-table layout context only; `https://cdcengine.re/docs/files/bigfile`) |
| Remaster modding (loose files, not the container) | Tomb Raider Forums SRM modding thread (`https://www.tombraiderforums.com/showthread.php?t=230457`, 2025: remaster textures ship loose as `TEX/*_{D,N,S,E}.DDS`, materials reference them by prefix) + `io_scene_xrm` Blender plugin (`https://github.com/TheDodylectableX/io_scene_xrm`). Neither documents the `TEXTURES.BIG` container — cited so a later slice does not re-search them |
| Soul Spiral license (carried forward) | GNU GPL v3 per source-file headers (mirror root carries NO license file — verified during `bigfile-index`). CONSEQUENCE unchanged: facts only, clean-room, NO Soul Spiral code copied; reusing its code or hash-lookup data later MUST add a `THIRD-PARTY-NOTICES.md` entry first |
| Original-PC texture tools | Licenses vary per Lost Worlds release page; we copy NO code from them — listed as apply-time background reading only (the original layout does not match the remaster bytes observed below) |

## Real-file grounding (read-only probes, 2026-10-12)

Total read from `1\TEXTURES.BIG`: ~4.6 KB across 11 bounded windows; plus 5 count-word windows (`[0..64]`) on sibling containers and 3 more (`[0..4]`) on SR2-side files; plus file-size/dir-count metadata (no reads). No writes, nothing copied out of the install. Counts/codes/field-shapes only — no hashes, offsets, or dumps recorded.

| Probe | Offsets + lengths | Observed (shape only) |
|---|---|---|
| P1 count-word sanity, 5 files × `[0..64]` | `1\TEXTURES.BIG`, `1\BIGFILE.DAT`, `1\E3.BIG`, `1\BONUS.BIG`, `1\MOVIES.BIG` | `BIGFILE.DAT` count parses sane (authoritative N=1325, archived); the other four count words read in the millions-to-tens-of-millions — SR1PC-at-0 REJECTED for all of them |
| P2 header census `[0..4096]` | `1\TEXTURES.BIG` full first 4K | Near-zero fill (single-digit nonzero bytes, all inside the first word, tag-like); rest zeros. Refines the cached "zeros through 4K" note: no global magic at 0, confirmed with the prefix exception recorded |
| P3 fill check `[4096..4160]` 64 B | `1\TEXTURES.BIG` | All `0xFF` (single distinct byte value) |
| P4 sparse 64 B windows ×9 | 8192, 65536, 1048576, 16777216, 33554432, 67108864, 100663296, 134217728, `[size-64..size]` | 8192 + 65536 windows all `0xFF` (fill extends at least 64K, exact extent UNKNOWN); all 6 data windows fully nonzero, high-entropy, none starting with `DDS ` (9/9 negative incl. EOF window); the same 4-byte code recurs at 3 far-apart data windows (block-structure hint, carried opaque, never interpreted) |
| P5 SR2-side count words `[0..4]` | `2\BIGFILE.DAT`, `2\INFO.STR`, `2\sr2.dll` + sizes | `2\BIGFILE.DAT` (~789 MiB class) count parses as 2236 with SR1PC table-size sanity holding; the u16 header reading gives the same value with zero pad — ambiguous between SR1PC and SR2 layouts, layout UNKNOWN beyond the count word. This is the 2236-entry sibling noted during `bigfile-index` verify — now located, counts only. Other two files fail sanity (not containers) |
| P6 dir metadata (no content reads) | `2\` listing, `1\TEX\` listing | SR2 side mirrors the SR1 layout (AREA/AREA_E3/OBJECT/PIX/TEX/TEXT/TRACKS + own `BIGFILE.DAT`); `1\TEX\` holds ~8K loose `*.DDS` (matches the gallery playtest range + modding-thread `_D/_N/_S/_E` naming) |

## UNKNOWNs + byte-level fallback plan

| Gap | Status | Fallback (read-only, ≤1 KB across ≤5 new windows, documented in the apply PR) |
|---|---|---|
| Entry-table location/shape | UNKNOWN (no index at 0, no table in any probed window) | Table-hunt at apply: file tail 1 KB (index-at-end is the common alternative) + first data window past the fill region. Found → parse per observed fields (Task 2); not found → documented no-go, loud stubs stay (spec R2/R3 no-table arms) |
| Data-region encoding (compressed/encrypted/chunked?) | UNKNOWN, never guessed | Carried opaque; `--tshow` sniffs `DDS ` magic on in-memory bytes only. No decompression or decryption is attempted in this slice |
| Recurring 4-byte data code | UNKNOWN, opaque | Recorded as recurrence count only; never interpreted, never used as a magic |
| Fill-region exact extent | UNKNOWN (somewhere past 64K, before the 1M window) | Not needed for the slice; apply must not binary-search it beyond the table-hunt budget |
| `2\BIGFILE.DAT` layout (2236) | UNKNOWN beyond the count word | Next slice's scoping probe; this slice names it in loud errors and docs, touches nothing |
| `E3.BIG` / `BONUS.BIG` / `MOVIES.BIG` | UNKNOWN, out of scope | Same SR1PC-at-0 rejection observed (P1); each needs its own probe before any claim |

## Details

| Topic | Decision |
|---|---|
| New module (`src/textures.rs`) | `pub struct TexturesLayout { file_len: u64, header_class: RegionClass, fill_extent_known: bool, table: Option<TableRef> }` (shapes only, never content) + `pub fn describe(path) -> Result<TexturesLayout, TexturesError>` over bounded reads + optional `parse_table`/`read_entry` IF the table-hunt succeeds (fields per observed layout, offsets carried as opaque `u64` with `offset + length <= file_len` checked overflow-safe, mirroring `bigfile.rs` range discipline without reusing its parser) |
| `RegionClass` | `ZeroFill` / `FfFill` / `Data` / `DdsMagic` — classified per window, never dumping bytes; `--tlist` no-table summary prints region classes + counts only |
| Error type | NEW `TexturesError`: `NotTextures` (no table identified / truncated header), `BadEntry` (range outside file), `BadSelector`, `NotDds`, `Io`. Every variant → stderr + `ExitCode(1)`; no-table messages name the next candidate (`2\BIGFILE.DAT`, 2236-entry, layout unverified) |
| `--tlist` format (table found) | `index identifier offset length` per line to stdout, fields per the identified layout (exact labels fixed at apply; retail values stay on stdout, never in docs). No-table: layout summary to stdout + loud stderr + exit 1 (spec R2) |
| `--tshow` selector | Decimal list index OR the stable identifier printed by `--tlist` (exact second form fixed at apply to match the identified layout); matched against parsed entries only — identifiers are NEVER computed, only compared |
| Render reuse | `--tshow` calls the EXISTING `show_texture` tail (`LoadedTexture` → CPU BC7 or RGBA8 → `show_rgba8`); window/exit contract identical to single-file and `--bigfile --show` modes |
| Tests | Unit tests on SYNTHETIC bytes only (hand-built zero/`0xFF`/data/magic windows + fake table buffers in temp dirs via `tempfile`, already a dev-dep): region classification, summary output, count-word rejection, range validation, selector parsing, DDS-magic gate. No GPU needed; pixel correctness stays a user playtest |
| Dependencies | ZERO new shipped crates; `winit`/`wgpu`/`image`/`pollster`/`bcdec_rs` unchanged |

## Deliberately NOT built

Full format reverse-engineering (decompression/decryption/chunking), extraction-to-disk (`Export`-equivalent — legally out of the question), hash-name reverse lookup, browser UI, non-DDS decoding, `E3.BIG`/`BONUS.BIG`/`MOVIES.BIG`, SR2-side containers, `bevy_ecs`, audio, models, Ghidra.

## Checklist

- [ ] Reviewer can trace file-to-screen in one paragraph (table above) and name the reused decode/render entry points (`decode_memory`, `show_texture` tail).
- [ ] Reviewer can state every probed offset+length (P1–P6 table) and the total read budget (~5 KB, read-only).
- [ ] Reviewer can state the layout verdict and why `src/bigfile.rs` is not reused (count-at-0 fails; SR2 pad rule fails).
- [ ] Reviewer can state every UNKNOWN and its fallback probe (table hunt budget, opaque data, next candidate named not touched).
- [ ] Entry ranges (if a table is found) are validated overflow-safe before any read; extracted bytes never touch disk.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

# `.BIG` slot names — the engine's alias matrix in `TEX_HD.MAP`

Follow-up to `s6-big-slot-geometry.md` and `s7-big-slot-payload-format.md`.
Bounded, read-only: whole-file reads of `AREA\TEX_HD.MAP`, statistics and
name-existence checks against the loose `.DDS` tree, two web searches, and a
bounded Ghidra pass over the engine DLL. No extraction, no game content
committed, no payload bytes in this repo.

## Investigation box

| Pass | Planned | Actual |
|---|---|---|
| Data correlation (map shape + loose-file cross-check) | ≤30 min | ~30 min, DECISIVE |
| Community documentation | ≤30 min | ~10 min, nothing on this map |
| Ghidra consumer (bounded, read-only) | ≤30 min | ~25 min, CONFIRMED |
| Ghidra project | outside repo | `%TEMP%\opencode\slotname\ghidra` |

## Verdict (CONFIRMED)

`AREA\TEX_HD.MAP` is a **fixed 384,000-byte alias matrix**:

```
384,000 bytes = 6,000 rows × 64 bytes
one row per container slot, 16 little-endian u32 alias codes per row
0xFFFFFFFF = "no reference"
```

Each code decodes to one loose-texture **name**; the name is exactly the
basename the engine uses when it loads a loose `.DDS`:

| Code `v` | Folder | Name |
|---|---|---|
| `v < 32000` | `TEX\` | `<v/16>_<v%16>` |
| `32000 ≤ v < 64000` | `TEX\M\` | `M\<(v-32000)/16>_<(v-32000)%16>` |
| `v ≥ 64000` | `TEX\B\` | `B\<(v-64000)/16>_<(v-64000)%16>` |
| `0xFFFFFFFF` | — | none |

`/` is integer division and `%` is the remainder. `0` is a valid code
(`0_0`); only `0xFFFFFFFF` means "no reference".

The names are the game's own numeric asset names, not descriptive words
(`100_10`, `M\9_2`, `B\262_1`). No descriptive vocabulary exists in the
container, the map, or the loose tree.

## Exact file shape (established, not guessed)

| Property | Value | Evidence |
|---|---|---|
| Size | 384,000 bytes exactly | file metadata; engine reads a 384,000-byte blob |
| Rows | 6,000 × 64 bytes | 384,000 / 64; row counts of the three blocks match container counts |
| Row | 16 × u32, little-endian | consumer indexes `blob + slot × 64` and iterates 16 entries |
| Header | none | row 0 starts at byte 0 |
| Strings | none | no ASCII run of 4+ characters anywhere |
| Sentinel | `0xFFFFFFFF` | unused rows are 16 sentinels; consumer skips exactly this value |
| Blocks | TEXTURES base 0 (capacity 2,000, used 1,287); MOVIES base 2,000 (capacity 2,000, used 29); BONUS base 4,000 (capacity 2,000, used 268) | consumer adds 2,000 for the movies handle and 4,000 for the bonus handle; 6,000 rows fill the blob |
| E3 | no block | no E3 offset in the consumer path; left UNKNOWN |

The two trailing BONUS rows (266 and 267 within that block) are all-sentinel,
so a BONUS slot count of 268 is consistent with the file.

## Observed coverage

| Measurement | Result |
|---|---|
| TEXTURES rows that decode to ≥1 name | 1,287 / 1,287 |
| TEXTURES references decoded | 20,592 (1,287 × 16) |
| References whose loose `.DDS` exists in the observed install | 20,516 (99.6%) |
| Missing references | 76, all distinct, concentrated in a few families (missing loose variants) |
| MOVIES rows with names | 29 / 29 |
| BONUS rows with names | 266 / 268 (two all-sentinel rows) |
| Distinct names per slot (after dedupe) | 1..16; 520 TEXTURES slots have all 16 distinct |
| Shared names | 2,070 of 5,876 distinct TEXTURES names occur in more than one slot; one common effect name occurs in 519 slots |

Content cross-check, independent of the map: slot 2's row contains `100_10`,
and an image-signature comparison of slot 2 against the loose tree ranked
`100_10.DDS` as the closest match. The map and the pixels agree.

## Evidence ledger

| # | Hypothesis | Verdict | Evidence that decided it |
|---|---|---|---|
| 1 | First 8,192 bytes are a separate 4,096-entry `u16` table | RULED OUT | 8,192 bytes are the first 128 rows of the 64-byte matrix. Every stored code is < 65,536, so every high `u16` half is zero; the "2,032 non-zero `u16`" are the low halves of 2,048 `u32` codes. |
| 2 | A separate `u32` ID run starts at offset 8,192 | RULED OUT | Offset 8,192 is row 128; its 16 codes are slot 128's aliases. |
| 3 | Tail values are PS1 texture-page words | RULED OUT | Decoding them as names resolves to real loose files at 99.6%; a page-word interpretation has no such correspondence. |
| 4 | One 64-byte row per container slot, 16 codes | SUPPORTED | Consumer strides by 64 bytes and loops 16 times; block row counts equal 1,287 / 29 / 268. |
| 5 | Group bases 0 / 2,000 / 4,000 | SUPPORTED | Consumer offsets the slot by 2,000 for the movies container and 4,000 for the bonus container; 6,000 rows exactly fill the blob. |
| 6 | Name = bank prefix + `<id>_<sub>` | SUPPORTED | Consumer formats `%s%d_%d` with three prefixes (empty, `M\`, `B\`); 99.6% of decoded names are existing files; image cross-check agrees. |
| 7 | Descriptive names exist somewhere in the data | RULED OUT | No ASCII in the map; no name table in the container; loose names are numeric. |
| 8 | `FX_TEX_MAP.TXT` supplies names | RULED OUT | It is a text source-region → destination-tpage UV remap, not a name list. |
| 9 | Public docs describe this mapping | NONE FOUND | See Community research. |
| 10 | E3.BIG has names | UNKNOWN | No E3 block; no E3 offset in the observed consumer path. Out of scope. |

## Method (what produced the evidence)

1. **Shape fit.** 384,000 / 64 = 6,000 rows. Sentinel runs start and end exactly
   at rows 1,287, 2,000, 2,029, 4,000 and 4,266, so the active rows are three
   contiguous blocks of 1,287, 29 and 266 rows.
2. **Decode and existence check.** Every code in the three blocks was decoded
   with the encoding above and every resulting name was checked against the
   loose `.DDS` tree: 20,516 / 20,592 TEXTURES references resolve to a file.
3. **Independent pixel check.** A downsampled mean-colour signature compared
   sampled slots against the loose tree without using the map; the best match
   for slot 2 is one of the names in slot 2's row.
4. **Consumer trace (bounded, read-only).** The engine reads the 384,000-byte
   blob once, then for a requested slot reads that slot's payload and builds up
   to 16 names from the row, formatting each as prefix + id + `_` + sub and
   looking it up in a fixed-size name cache. Behavior only; no code listing and
   no addresses recorded.

## Community research

Searches run: "Soul Reaver Remastered TEX_HD.MAP texture IDs mapping",
"\"TEX_HD.MAP\" Soul Reaver remaster", GitHub variants, and loose-name variants.
No page documents `TEX_HD.MAP`, the 6,000-row layout, or the `<id>_<sub>`
naming.

Pages checked (context only; **no code or data reused**):

| URL | What it covers | License status |
|---|---|---|
| https://www.thelostworlds.net/Software/Only_Skin_Deep.html | Original/PC/Dreamcast texture extraction | Not verified here; no reuse |
| https://github.com/TheSerioliOfNosgoth/Soul-Reaver-PC-Texture-Utility-Official | Mirror of Ben Lincoln's PC texture utility | GPL-3.0-only noted for the Soul Spiral family in this repo's README; no reuse |
| https://github.com/fmil95/soul-re | PS1 reverse-engineering project | MIT; no reuse |
| https://www.sr1hdremaster.com/ | Dreamcast fan texture pack | Non-commercial fan project; no reuse |

Next candidate if descriptive names are ever required: map the numeric alias
families to original PS1 page/room catalogues (Soul Spiral / Only Skin Deep
domain), not the remaster container.

## Corrections to earlier notes

| Prior statement | Correction |
|---|---|
| `s6`: "first 8,192 bytes = 4,096 `u16` values (2,032 non-zero)" | Those bytes are the first 128 rows of a 6,000-row matrix; read as `u16` they look like a prefix only because every code is < 65,536. |
| `s6`: "from offset 8,192 the payload begins with ascending `u32` IDs" | Offset 8,192 is row 128; the "ascending IDs" are that slot's alias codes. |
| `s7`: "How slot indices map to texture names (`TEX_HD.MAP` is the likely table) — out of scope" | Resolved here: the table is per-slot, 16 aliases, encoded as `<id>_<sub>` with `M\` / `B\` banks. |

## Remaining unknowns

- E3.BIG names (no block in this map; separate path or no names).
- Whether every retail build ships the same 6,000-row layout (one install
  observed).
- Whether the 76 unresolved references are edition/build differences.
- What the 16 aliases mean semantically (shared-source set vs atlas); this does
  not affect name lookup.

## Reproducibility and hygiene

- Scratch scripts, decoded signatures and the Ghidra project lived under
  `%TEMP%\opencode\slotname\` only; nothing is part of the repo.
- The game tree was opened read-only. No extracted files were written.
- `git status --short` after this note shows only the new `*.md` files.

## Legal

Format and behavior facts only. No payload dumps, no code listings, no
addresses, no absolute install paths. All scratch outputs and the Ghidra
project stayed outside the repo. Game data was opened read-only.

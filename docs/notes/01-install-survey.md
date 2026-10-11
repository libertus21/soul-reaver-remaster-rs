# Install survey (read-only, 2026-10-11)

Game: Legacy of Kain: Soul Reaver 1&2 Remastered (2024, Aspyr), GOG build.
Base: `D:\Juego\planescape\Soul Reaver I-II` — READ-ONLY, never modified.
Game 1 data: `...\1\`. Game 2 data: `...\2\` (same layout, SR2 files).
Only headers/magic bytes and directory stats were read. No bytes were
copied into this repo.

## Top-level layout (`1\`)

| Entry | Size |
|---|---|
| `AREA/` | 5 files + subdirs per level (ALUKA, CATHY, CITY, FORGE, HUBA, LAIR, MOVIE, MRLOCK, NIGHTB, RETREAT, TREAT, UNDRCT) + `FX_TEX_MAP.TXT`, `MODEL_BRIGHTNESS.TXT`, `NOISE.RAW` (2 MiB), `TEX_HD.MAP`, `TILED.TXT` |
| `AREA_E3/`, `EXTRA/`, `FMV/`, `MUSIC/`, `TEXT/` | present (MUSIC holds per-track dirs) |
| `OBJECT/` | 636 entries; subdirs per object (e.g. `BLADE/`, `BOX/`, `BRING/`…); loose `.SRM`/`.SRA`/`.SRL` files |
| `OBJECT_E3/` | present |
| `PIX/` | 61 entries; `GALLERY/` holds `*_HD.DDS` (2 MiB each) + `CHRONICLE/` |
| `TEX/` | 8066 files, mostly `*.DDS` split into `B/` and `M/` subdirs |
| `TRACKS/` | `DE EN ES FR IT` locale dirs with `.OGG` voice files |
| `BIGFILE.DAT` | 85,923,840 bytes |
| `TEXTURES.BIG` | 168,693,760 bytes |
| `BONUS.BIG` | 35,131,392 bytes |
| `E3.BIG` | 23,334,912 bytes |
| `MOVIES.BIG` | 3,805,184 bytes |
| `SFX.WAS` | 154,066 bytes |
| `sr1.dll` | 2,416,008 bytes |

Per-extension stats (`1\`, recursive): `.DDS` 9842 (4.6 GiB), `.OGG` 3949
(449 MiB), `.TXT` 3478, `.SRM` 349, `.SRA` 227, `.PCM` 214, `.SRL` 60,
`.SND` 31 + `.SMP` 31 (paired, under `MUSIC/<track>/`), `.OGV` 24
(1.9 GiB), `.PMF`/`.PNF` 23 each, `.BIG` 4, `.WAS` 1, `.DAT` 1,
`.MAP` 1, `.RAW` 1, `.dll` 1.

Game `2\`: same folder set; `BIGFILE.DAT` 827,439,104 bytes,
`INFO.STR`, `sr2.dll`; `TEX/` holds `*_D/_N/_S.DDS` triples
(diffuse/normal/specular naming, 349,588/349,568 bytes).

## Finding: textures as .DDS — CONFIRMED

- `1\TEX\` holds loose `.DDS` files (e.g. `0_0.DDS`, `1000_0.DDS`,
  all ~349,588 bytes) plus `B/` and `M/` subdirs.
- `1\PIX\GALLERY\` holds large `*_HD.DDS` menu/gallery art.
- First slice can therefore load one real `.DDS` straight from the
  install at runtime. No extraction step needed.

## Magic bytes (first bytes only, nothing copied)

- `1\TEX\0_0.DDS` offset 0: `44 44 53 20` ("DDS "), standard DirectDraw
  Surface header. Header reports DX10 extended format (`DX10` fourcc at
  offset 84); pixel data follows at offset 128+20. Plausible format:
  standard DDS with DX10 header (BC-compressed). Exact `dxgiFormat`
  value: UNKNOWN until the viewer slice parses it (first 256 bytes
  re-read at implementation time).
- `1\BIGFILE.DAT` offset 0: `2D 05 00 00 ...` followed by `DING`/`INGF`/
  `INGG`/`INGI`/`INGS` chunk tags and `FONT`/`TDBG` tags. Plausible
  format: Crystal Dynamics "Gex engine" bigfile archive (matches Soul
  Spiral's documented family). Internal layout: UNKNOWN — do not parse
  by guessing; use Soul Spiral / community specs when the slice needs it.
- `1\TEXTURES.BIG` offset 0..4095: all `00` bytes; offset 4096: all
  `FF` bytes. No magic at 0. Plausible format: UNKNOWN — either a raw
  blob with an index elsewhere (e.g. `TEX_HD.MAP`/`AREA/TEX_HD.MAP`) or
  an archive with a trailing/offset table. Marked UNKNOWN; the DDS-first
  slice deliberately avoids it by reading loose `TEX\*.DDS` files.

## Struct/offset discipline

Anything above marked UNKNOWN stays UNKNOWN until a slice reads the real
file with a community tool/spec. No invented structs live in this repo.

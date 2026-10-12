# Design: bigfile-index

Same binary, same decode, same upload: a bigfile is just an index of `(hash, offset, length)` triples parsed from byte 0, plus a bounded in-memory read of one entry's bytes fed through the slice-3/4 pipeline. The only new logic is the SR1PC index parser, the entry-range reader, and the `--bigfile` CLI arm.

## Quick path

`main.rs` (NEW `--bigfile <path> --list | --show <entry>` arm; file/dir arms untouched) -> NEW `src/bigfile.rs` (`parse_index` at offset 0 -> `read_entry` bounded slice, both pure + loud errors) -> `dds.rs` (NEW `decode_memory`, same validation as `load_texture`) -> `render.rs` (EXISTING `show_rgba8`-family) -> pixels.

## Spec source (credit + license)

| Item | Value |
|---|---|
| Primary source | Soul Spiral mirror `https://github.com/TheSerioliOfNosgoth/SoulSpiral-Official` (Ben Lincoln's tool, mirrored with permission; upstream `https://www.thelostworlds.net/Software/Soul_Spiral.html`) |
| Exact files read | `BenLincoln.TheLostWorlds.CDBigFile/Index.cs` (u32 LE count at `Offset`, `FirstEntryOffset = 4`, `MAX_ENTRIES = 32768`, little-endian), `BenLincoln.TheLostWorlds.CDBigFile/FileIndex.cs` + `File.cs` (`offset = raw[offsetPos] * OffsetMultiplier`, `length = raw[lengthPos]`, offset+length sanity vs file size), `BenLincoln.TheLostWorlds.CDBigFile/BigFileTypeSoulReaverPC.cs` (`MasterIndexType = SR1PC`), `Index.cs/CreateIndex` SR1PC arm (`EntryLength = 4` uints, `NameHashPosition = 0`, `LengthPosition = 1`, `OffsetPosition = 2`) |
| License | GNU GPL v3 per every source-file header ("version 3 of the GNU General Public License", headers reference a `LICENSE.txt`; the mirror root carries NO license file — verified 2026-10-12). CONSEQUENCE: this milestone reimplements observed field-layout FACTS clean-room and copies NO Soul Spiral code. If future work reuses Soul Spiral code or its `Hashes-SR1.txt` lookup data, it MUST add a `THIRD-PARTY-NOTICES.md` entry first (GPL compliance) |
| Corroboration | `https://cdcengine.re/docs/files/bigfile` documents the LATER (SR2+) hash-table layout (`numFiles` + `hashes[]` + `{fileLen, filePos, compressedLen}`) and the hash evolution (custom hash pre-Defiance, CRC-32 poly `0x4C11DB7` since) — useful context, NOT the SR1 layout; never conflated |
| Fields documented | No global magic (detection is index sanity); table at offset 0; u32 LE count; 4×u32 LE entries `[nameHash, length, offset, unknown]`; raw byte offsets (`OffsetMultiplier = 1`); little-endian; names stored as hashes only |

## Real-file grounding (read-only probes, 2026-10-12)

5 regions × ≤64 B each of the real `1\BIGFILE.DAT` (size ~82 MiB): count word parses as a sane value (≤ `MAX_ENTRIES`; planning probe observed 1325 — apply MUST re-record the authoritative N); the first entries parse with `offset + length` inside the file size (Soul Spiral's own sanity rule); `DING`/`INGx` 4-byte tags present at data level (matches the init survey; they are entry CONTENT tags, not the index — the index's 4th dword is opaque). No struct or offset was invented: everything above is either spec-quoted or probe-observed.

## UNKNOWNs + byte-level fallback plan

| Gap | Status | Fallback (read-only, ≤1 KB from ≤5 new regions, documented in the apply PR) |
|---|---|---|
| 4th index dword (position 3, Soul Spiral marks `//??Position = 3`) | UNKNOWN, opaque | Carried through and printed as hex only; never interpreted. Probe: check constancy across entries |
| Name-hash algorithm (custom pre-Defiance hash; lookup via external `Hashes-SR1.txt` we do not ship) | UNKNOWN to us, NOT needed | We never compute hashes — `--list` prints stored hashes, `--show` matches the stored value. No reverse lookup; hash-to-name table is a follow-up slice |
| Which entries are DDS (SR1 FileType list has TIM/DRM/CRM, no DDS; the remaster archive may still embed DDS) | UNKNOWN until `--show` | Sniff `DDS ` magic on in-memory bytes; non-DDS fails loud per spec R5. Probe: scan entry first-4-bytes during apply playtest and record the DDS-entry count |
| `TEXTURES.BIG` magic/layout | UNKNOWN, out of scope | Untouched this slice; recorded here so a later slice does not assume this design covers it |
| `E3.BIG` / `BONUS.BIG` / `MOVIES.BIG` | UNKNOWN, out of scope | Same layout PROBABLE (same engine family) but unverified — each file needs its own `--list` sanity run before any claim |

## Details

| Topic | Decision |
|---|---|
| New module (`src/bigfile.rs`) | `pub struct BigfileEntry { name_hash: u32, offset: u64, length: u64 }` + `pub fn parse_index(bytes: &[u8], file_len: u64) -> Result<Vec<BigfileEntry>, BigfileError>`: u32 LE count (reject `> 32768` and index overrunning `file_len`), then 16-byte entries `[hash, length, offset]` LE with `offset + length <= file_len` each (checked in u64, overflow-safe). 4th dword read and ignored (UNKNOWN rule above). `pub fn read_entry(file: &Path, entry) -> Result<Vec<u8>, BigfileError>`: single bounded read, range pre-validated; returns bytes, never writes |
| Error type | NEW `BigfileError` (or extend `ViewerError` — apply picks the smaller diff): `NotBigfile` (bad count/truncated index), `BadEntry` (range outside file), `NotDds` (entry lacks `DDS ` magic), `Io`. Every variant → stderr + `ExitCode(1)`; messages name the file/entry/selector |
| In-memory decode (`dds.rs`) | NEW `pub fn decode_memory(bytes: &[u8]) -> Result<LoadedTexture, ViewerError>` sharing `load_texture`'s validation (magic + DX10/`dxgiFormat` + BC7-shape checks); `load_texture` becomes read-file-then-`decode_memory` (or vice versa — apply picks the smaller diff). Zero new decoders; `dxgiFormat` 99 stays loud |
| `--show` selector | Decimal list index OR 8-hex-digit hash (case-insensitive, optional `0x`); matched against `parse_index` output. Rationale: index is what `--list` prints leftmost (copy-paste), hash is stable across listings; both are ~15 lines |
| `--list` format | `index hash(8 hex) offset length` per line to stdout (e.g. `0 1234ABCD 4096 8192` — synthetic example, never real-file values: retail hashes/offsets are never committed per AGENTS.md). Stdout stays machine-quotable for the playtest record |
| Render reuse | `--show` calls the EXISTING single-file tail (`LoadedTexture` → CPU BC7 or RGBA8 → `show_rgba8`); window/exit contract identical to single-file mode |
| Tests | Unit tests on SYNTHETIC bytes only (hand-built minimal fake bigfiles in temp dirs via `tempfile`, already a dev-dep): 1-entry + N-entry parse, count-too-large rejected, truncated index/entry rejected, offset+length overflow rejected, DDS-magic gate (DDS vs non-DDS bytes). No GPU needed; pixel correctness stays a user playtest |
| Dependencies | ZERO new shipped crates; `winit`/`wgpu`/`image`/`pollster`/`bcdec_rs` unchanged |

## Deliberately NOT built

Browser UI over the index, extraction-to-disk (`Export`-equivalent — legally out of the question for this project), hash-name reverse lookup, non-DDS decoding (TIM/DRM/CRM/PMF), `TEXTURES.BIG`, other `.BIG` files, compression handling (SR1PC has none), `bevy_ecs`, audio, models, Ghidra.

## Checklist

- [ ] Reviewer can trace file-to-screen in one paragraph (table above) and name the single shared decode entry point (`decode_memory`).
- [ ] Reviewer can state the spec source + license + why no THIRD-PARTY-NOTICES.md entry is needed yet (facts only, no code/data copied).
- [ ] Reviewer can state every UNKNOWN and its fallback probe (4th dword, hash algorithm, DDS-entry identity, TEXTURES.BIG, other BIGs).
- [ ] `offset + length` is validated in u64 before any read; extracted bytes never touch disk.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

# soul-reaver-remaster-rs

Unofficial fan-project Rust rewrite of Legacy of Kain: Soul Reaver 1&2
Remastered (2024, Aspyr). Not affiliated with or endorsed by Aspyr or
Crystal Dynamics. Written with AI coding-agent assistance.

The engine reads data from the player's OWN installed copy of the game
at runtime. This repo contains only our code — never game assets,
extracted files, decompiled code, or database files.

## What works

Every row is backed by the exact command shown. Replace `<...>` with a path from
your own install; nothing is bundled.

| Capability | Command | What you get |
|---|---|---|
| Loose DDS textures (DXT1 + BC7) | `cargo run -- <FILE.DDS>` | Shows one texture. Legacy DXT1/DX10 files the `image` crate decodes; BC7 (`dxgiFormat` 98) is CPU-decoded. Bad input fails loud. |
| DDS gallery | `cargo run -- <DIR>` | Browses sorted direct-child `*.DDS` one at a time (`←`/`→`, Esc). Undecodable entries are skipped loud. |
| `.BIG` container textures | `cargo run -- --textures <TEXTURES.BIG> --tlist` / `--tshow <SEL>` | Validates the 512-byte header and fixed 131072-byte slots; `--tshow` decodes one 256×256 A1R5G5B5 slot. Works on `TEXTURES.BIG`, `MOVIES.BIG`, `BONUS.BIG`, `E3.BIG`. |
| `.BIG` texture names | `cargo run -- --textures <TEXTURES.BIG> --tlist --map <TEX_HD.MAP>` / `--tshow <NAME> --map <TEX_HD.MAP>` | Decodes the engine alias matrix (1287/1287 `TEXTURES.BIG` slots named). `--tshow` accepts `<id>_<sub>`, `M\<id>_<sub>`, `B\<id>_<sub>`; shared names are refused. |
| `.SRM` meshes | `cargo run -- --mesh <FILE.SRM>` | Parses one loose mesh (positions + triangle indices) and draws a flat-shaded auto-rotating solid. Left-drag orbit, wheel zoom, right/middle-drag pan, `R` reset, `F` fly. |
| One-bone pose | `cargo run -- --mesh <FILE.SRM> --pose <BONE>:<DEG>` | Rotates exactly one decoded weighted bone slot (`0..=127`, `|deg| <= 360`) and shows the deformed mesh. A local cluster rotation, not a full pose. |
| Textured meshes + mesh gallery | `cargo run -- --mesh-textured <FILE.SRM> --tex-dir <DIR>` / `cargo run -- --mesh-all <OBJECT_DIR> --tex-dir <DIR>` | Loads UVs, material indices and the three DDS textures per material; renders lit. `--mesh-all` browses a directory (`←`/`→`, Esc). |
| `.SND`/`.SMP` audio → WAV | `cargo run -- --audio <FILE.SND> --alist` / `cargo run -- --audio <FILE.SND> --wave <N> --out <FILE.wav>` | Lists each wave's offset, bytes, frames, duration and loop span; decodes one wave as Sony 4-bit ADPCM to a canonical PCM16 mono WAV (44100 Hz chosen). The writer refuses paths inside a git work tree. |
| `BIGFILE.DAT` index | `cargo run -- --bigfile <BIGFILE.DAT> --list` / `--show <SEL>` | Prints `index hash offset length` per entry; `--show` reads one entry in memory and shows it if it is a DDS. |
| Archive-entry and group level access | `cargo run -- --level-geometry --bigfile <BIGFILE.DAT> --entry 115` / `cargo run -- --level-geometry --bigfile <BIGFILE.DAT> --group under` | Reads one entry slice in memory as a `.PCM` unit (entry 115 is `under1`, 1,468 V / 2,922 F), or resolves every matching entry in a group. `--level-objects`, `--level-links` and `--level-render` accept both forms. No extraction, no temp file. |
| Level structure | `cargo run -- --level-structure <PATH>` | Reports header sanity, relocation-directory integrity (`entries`/`valid`/`zero`/`out_of_range`), non-null header slots and the 36-byte table. |
| Level links | `cargo run -- --level-links <PATH>` | Prints the decoded link table (`target`, `index`, `target_id`, five raw prelude words). The corrected parser accepts mixed-case targets and gates the prologue on the observed prelude; dumps keep stored case and graph work folds it. |
| Level geometry | `cargo run -- --level-geometry <PATH>` | Prints vertex/face counts, both offsets, the position axis box and centre, and face-index validation. |
| Level objects | `cargo run -- --level-objects <PATH>` / `cargo run -- --level-objects --bigfile <BIGFILE.DAT> --entry <INDEX>` / `cargo run -- --level-objects --bigfile <BIGFILE.DAT> --group under` | Prints each placed object's name, world position, type and flags. |
| Archived object model info | `cargo run -- --object-info --bigfile <BIGFILE.DAT> --entry 66` | Reads one object container in memory and prints vertex/polygon/material counts, both array-boundary identities and the out-of-range index count. Entry 66 is `raziel` (299 V / 640 P). No extraction. |
| Archived object model render | `cargo run -- --object-mesh --bigfile <BIGFILE.DAT> --entry 66` / `cargo run -- --object-mesh --bigfile <BIGFILE.DAT> --entry 66 --textures <TEXTURES.BIG>` | Parses the same container, applies the 24-byte bone rest pose and draws its triangles through the flat-shaded viewer. `--textures` draws each face with its `TextureFT3` page. Louder refusals happen before any window opens. |
| Actor behaviour census | `cargo run -- --actor-census --bigfile <BIGFILE.DAT> [--objects-dir <DIR>]` / `cargo run -- --object-info --bigfile <BIGFILE.DAT> --entry 129 --actors` | Reads the object-header `+0x1C` actor block, prints each actor's 0x20-byte entry count, entry ids (hex) and the eight 8x3 state trios, then a corpus summary. Partial actors report `variant`/`none` and never contribute to the totals. Real corpus: 21 actors / 75 entries / 37 distinct ids (19 archive + `morboss`/`priests` loose). |
| `--level-render` (orbit + fly) | `cargo run -- --level-render <PATH>` / `cargo run -- --level-render --bigfile <BIGFILE.DAT> --entry <INDEX>` / `cargo run -- --level-render --bigfile <BIGFILE.DAT> --group under [--objects] [--textures <TEXTURES.BIG>]` | Renders room geometry, flat or textured from `TEXTURES.BIG`. Orbit, wheel zoom, pan, `R` reset; `F` enters a first-person fly camera. `--objects` adds each placed object's real archived model at its world position; `--textures` textures both rooms and models. |
| Level reachability | `cargo run -- --level-reach --bigfile <BIGFILE.DAT> [--start <UNIT>] [--names]` | BFS from `under1` over the decoded link graph, then groups reachable, in-degree islands and inbound-but-unreachable units. `--names` lists each island unit. Corrected reader: 372 discovered / 334 reachable / 38 unreachable / 1 island (`push1`). |
| Unified CLI | `cargo run -- --help` / `cargo run -- menu` / `cargo run -- texture list <BIG>` | `--help` lists the 32 capabilities in five groups; grouped commands rewrite to the exact legacy argv; `menu` is a numbered terminal picker (`q` quits). |

`<PATH>` accepts a `.PCM` file or a room directory. The level arms also accept
`--bigfile <DAT> --entry <INDEX>` or `--bigfile <DAT> --group <PREFIX>`.

## Opening level quick-start

The new-game opening unit is `under1`, reached through the archive (entry 115),
not the loose `UNDRCT1.PCM`. Render the whole tutorial family with these commands:

```text
cargo run -- --level-objects --bigfile <BIGFILE.DAT> --group under
cargo run -- --level-render  --bigfile <BIGFILE.DAT> --group under
cargo run -- --level-render  --bigfile <BIGFILE.DAT> --group under --objects
cargo run -- --level-render  --bigfile <BIGFILE.DAT> --group under --textures <TEXTURES.BIG>
cargo run -- --level-render  --bigfile <BIGFILE.DAT> --group under --objects --textures <TEXTURES.BIG>
```

The first prints the four rooms' object counts and placements. The next opens
one window with the merged `under` geometry, flat or textured from
`TEXTURES.BIG`. Adding `--objects` places each object's real archived model at
its world position, also flat or textured; only `campath`, `marker` and `splob`
keep a flat cube marker because they have no mesh.

Fly camera keys: `F` toggles orbit/fly; `W`/`A`/`S`/`D` move relative to the
view; the mouse looks around; `Space`/`E` rise; `C`/`Q` sink; `Shift` flies
faster; `R` resets the camera; `Esc` exits.

## What's missing / known limits

- **No animation.** `.SRA` is not parsed by this build (the loader callback is a
  `RET 0` stub) and `.SRM` has no hierarchy, so `--pose` is a local cluster
  rotation only and the mesh tears at cluster seams.
- **No game logic.** This is a viewer/analysis tool, not a playable engine.
- **No sound sequences.** `.SND` waves export to WAV and loops are reported, but
  they are decoded one-shot; there is no `QSMa`/sequence playback, mixing or ADSR.
- **Objects are real archived models, not markers.** `--level-render --objects`
  resolves 375/378 placed names to their `.PCM` OBJECT containers; only
  `campath`, `marker` and `splob` keep a flat marker. Object `type` is authoring
  metadata: the engine keys runtime behaviour on the object name, not on the
  type or the `+0x26` code (see `docs/notes/object-behaviour.md`). The
  `TextureFT3` attribute and colour-blend meanings remain UNKNOWN.
- **Object records carry no orientation.** There is no rotation field; the
  `+0x18` triple is a cross-level offset, not angles.
- **Room textures are implemented, with a small unresolved tail.** 99.76% of
  faces address their 12-byte UV record; the 214 `0xFFFF` and 1,160 non-aligned
  tails are counted and drawn flat, never dropped. Face attribute `+0x08` stays
  UNKNOWN.
- **`--group` scans the whole archive per run** (no cached index).
- **Level geometry gaps.** Array C, the face attribute words, the room-local
  group records and the dominant 143,872-byte block stay UNKNOWN; no normal,
  BSP or culling claim is made.
- **Cut content: 30 `Test` levels.** The engine's debug/media load list names
  `Test 1` .. `Test 30`, but no shipped file or archive entry backs them. That
  is the concrete target list for restoration work; absence from the archive
  alone is not proof, because many levels ship loose under `AREA\`.
- **BC7 sRGB (`dxgiFormat` 99) fails loud.** sRGB correctness, resize handling
  and a full mip-chain UI are deferred.
- **Binary startup beyond `run` stays UNKNOWN**, and the loader's post-open error
  rows remain HELD with their existing prefixes pinned by regression tests.
- **`.SRL` (60 files) and the `.PMF` codec are undecoded**; the SR2 side is
  unscoped.
- **Menu prompts show only the operand token** (no path hint), so a menu-style
  number typed as a path fails downstream by design.

## Repository rules

- `.gitignore` is a whitelist: only `*.rs`, `Cargo.toml`, `Cargo.lock`, `*.md`,
  `.gitignore` and `.gitattributes` can be committed; `/target/` stays ignored.
- No game bytes, extracted files, decompiled listings, Ghidra databases, hashes
  or absolute install paths. Game data is read-only at runtime from the user's
  own single-player/offline copy.
- Evidence lives in `docs/notes/` as behavior paraphrases, field offsets and
  counts; tests use synthetic bytes only.
- Community work is credited in `THIRD-PARTY-NOTICES.md`; no community code is
  vendored without a notice.
- The repository is private.
- Process: small chained slices (≤400 changed lines each), proposal → spec →
  tasks → code → playtest report.

## Details

| Topic | Decision |
|---|---|
| Stack | Rust 2024 (cargo 1.99.0): winit 0.30, wgpu 30, image 0.25, bytemuck, pollster; pinned `bcdec_rs` 0.2.0 (MIT) for CPU BC7. No game-logic ECS. |
| Game data | Read-only at runtime from the user's install; paths via CLI args/env only, never committed. |
| Legal guardrails | Whitelist `.gitignore` verified every commit; single-player/offline owned copy only; community work credited in `THIRD-PARTY-NOTICES.md` once reuse starts. |
| Process | Small chained slices (≤400 changed lines each); proposal → spec → tasks → code → playtest report. |
| Tests | `cargo test` — 321 tests on synthetic bytes only; no game files in the suite. |

## Further reading

- `docs/notes/` — evidence notes: formats, field offsets, counts and UNKNOWN lists.
- `docs/changes/` — per-slice proposal/spec/design/tasks and archives.
- `AGENTS.md` — repository rules and the session log.

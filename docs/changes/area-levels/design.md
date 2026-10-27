# Design: area-levels

## Technical approach

Decode the container and the object-name table, and nothing else. A new pure
module `src/level.rs` owns the PCM header model, the lexical name-table
extraction, the link split, and the PMF/PNF sidecar headers. `main.rs` gains
one `--level-info` arm that resolves a file or a room directory, prints one
block per unit, and exits. Nothing touches `mesh_entry.rs`, `srm.rs`,
`render.rs`, `tex_hd_map.rs`, or `bigfile.rs`.

The parser is deliberately shallow. It proves the record table and the name
table, and it refuses to interpret a record payload as geometry. The output
is counts, unit names, link targets, and small ordinals.

## Probe ledger (read-only, 2026-10-27)

| # | Probe | Result | Evidence class |
|---|---|---|---|
| P1 | Census under `1\AREA` | 12 room directories; 89 `.PCM`, 23 `.PMF`, 23 `.PNF`; 5 loose files; `.PCM` range 9,136–274,084 B; `.PMF` range 80–289,688 B; `.PNF` 22–30 B | Observed |
| P2 | PCM header model | `u32 N`; `N` `u32` relative offsets; `offset[0]=0`, `offset[1]=4`; strictly ascending; data base `4+4N`; last record start inside the file. Holds 89/89 | Observed |
| P3 | PCM own name | every file's lexical name table contains its own stem, 89/89 | Observed |
| P4 | PCM link names | 368 names of the form `<target>,<index>`; 346 (94.0%) targets resolve to a real `.PCM` unit; 11 files have none; special targets (`warpgate`, …) do not resolve | Observed / Derived |
| P5 | PCM record payload | records carry mixed packed data, including the name strings; vertex, face, collision and placement layouts were not decoded | Observed shape / UNKNOWN |
| P6 | PMF file header | magic, version, 16-bit id, 16-bit segment count; 23/23 parse | Observed |
| P7 | PMF segment header | 24-byte header with a size word and a rate word; first segment at the file header boundary; further headers are reachable by walking the size | Observed |
| P8 | PMF same-size identity | four size groups (145,184 ×3; 63,852 ×2; 5,416 ×5; 80 ×4) are byte-identical except the id word | Observed |
| P9 | PMF payload character | smooth 16-bit LE runs; across six sampled files, lag-1 autocorrelation 0.81–0.998 and zero-crossing 0.015–0.173; a decaying tail in the largest sampled files; the rate word is 22,050 in 18/23 and 11,025 in 5/23 | Derived (strong): audio-like sample bank |
| P10 | PNF sidecar | magic `FNPa`; 20-byte header + one `u16` per segment; id and count match the PMF in 23/23; `len = 20 + 2*count` | Observed |
| P11 | Community sources | `fmil95/soul-re` (MIT) is a PS1 decompilation with per-unit area configs. `Gh0stBlade/KAIN2` (Unlicense) documents the PS1 stream-unit engine: `STREAM_FillOutFileNames` emits per-unit `.drm`/`.crm`/`.snf`, `StreamUnitPortal.tolevelname[16]`, `Level.objectNameList`. Neither documents the remaster `.PCM/.PMF/.PNF` container | Observed docs |
| P12 | Search limits | web search returned results; `grep.app` returned HTTP 429 on the magic query; no remaster-format tool/doc was found | Observed |

## Evidence classes

- **Observed** — a byte-level fact repeatable on the census (counts, sizes,
  header shape, id/count equality, own-name presence).
- **Derived (strong)** — a model that fits the census with no exception (PCM
  offset model; PNF length rule; PMF audio-like reading).
- **Inferred** — a semantic reading the bytes permit but do not prove (links
  are portals/doors; PNF entries bind emitters).
- **UNKNOWN** — not established and not guessed.

## What stays UNKNOWN

1. **PCM geometry and collision.** The record payloads are present but their
   vertex, face, and collision layouts are not decoded. No render, no
   collision, no bounds.
2. **Object placement.** No instance/transform table was located. No
   positions, orientations, or counts are claimed.
3. **PMF codec.** The 16-bit LE reading is derived, not decoded. Loop points,
   the exact segment table, and the meaning of the remaining header words are
   unknown. Playback is out of scope.
4. **Why same-size PMFs are identical.** Shared ambience, placeholder data, or
   a packaging quirk: not established.
5. **Id and PNF entry meaning.** The 16-bit id is stable per unit and matches
   across PMF/PNF, but its semantic (resource id, unit id, hash) is unknown.
   PNF `u16` values are not interpreted.
6. **PCM record 0/1.** The first two records are tiny (4, 8, or 80 bytes in
   the samples) and look like a directory; their meaning is not proven.
7. **Trailing PCM blob.** The last record ends before EOF in the sample; the
   remainder is not classified.
8. **PS1 lineage.** Whether `.PCM` is a repacked PS1 `.drm` is not proven; the
   name semantics agree, the container does not.

## Architecture decisions

| Decision | Options considered | Choice | Rationale |
|---|---|---|---|
| Where the parser lives | extend `srm.rs`; new `level.rs` | new `src/level.rs` | the container shares no proven structure with `.SRM`; a separate module keeps the mesh path untouched and makes the no-go independent |
| Path model | file only; directory only; both | both | a room directory is the natural unit; a single `.PCM` keeps tests and ad-hoc probes cheap |
| Name extraction | offset table only; lexical scan + own-name guard | lexical scan + own-name guard | the name table is interleaved with records; the own-name check is a strong, falsifiable guard (89/89) |
| Link validation | require every target to exist; syntax only | syntax only | 6% of targets are special or unresolved; requiring existence would reject real data |
| Sidecar policy | best-effort; strict pair | strict pair | the census has 23/23 pairs; a lone or mismatched sidecar is a format contradiction and must fail loud |
| Output | include ids/offsets; counts + names | counts + names + ordinals | proves the model without dumping game data or inviting an address-style claim |
| Geometry | best-effort decode; stop at the table | stop at the table | the probe did not establish geometry; an invented layout is worse than a documented no-go |

## Data flow

```text
path ──> kind gate ──> { .pcm file | directory scan }
             │                    │
             └── exit 3           ├──> PCM header (exit 2 on violation)
                                  ├──> name table + own-name guard (exit 2)
                                  ├──> PMF/PNF sidecar (exit 2 on violation)
                                  └──> sort by unit ──> outln ──> stdout
```

All units are parsed before the first line is printed, so a failure produces
no partial output.

## File changes

| File | Action | Description |
|---|---|---|
| `src/level.rs` | Create | `LevelUnit`, `Link`, `Sidecar`, `PmfHeader`, `PnfHeader`, `LevelError`, `parse_pcm`, `parse_pmf`, `parse_pnf`, `read_path` |
| `src/main.rs` | Modify | one `--level-info` arm, usage text, error-to-exit mapping |
| `README.md` | Modify | "What works / What's missing": level inventory, geometry still missing |
| `docs/notes/area-levels.md` | Create | findings note (already written at plan time) |
| `docs/changes/area-levels/*` | Add | proposal, spec, design, tasks |
| `src/render.rs`, `src/srm.rs`, `src/mesh_entry.rs` | None | untouched by contract |

## Interfaces

```rust
pub const MAX_RECORDS: u32 = 1 << 20;
pub const MAX_SEGMENTS: u16 = 1 << 10;
pub const MAX_NAME_LEN: usize = 20;

pub struct LevelUnit {
    pub name: String,
    pub records: u32,
    pub names: Vec<String>,
    pub links: Vec<Link>,
    pub sidecar: Sidecar,
}

pub struct Link { pub target: String, pub index: u16 }

pub enum Sidecar {
    Absent,
    Present { segments: u16, rate_field: u32, pnf_entries: u32 },
}

pub struct PmfHeader { pub id: u16, pub segments: u16, pub rate_field: u32 }
pub struct PnfHeader { pub id: u16, pub entries: u32 }

pub enum LevelError {
    Usage(&'static str),        // exit 1
    RefusedPath,                // exit 3
    Io,                         // exit 2
    NoUnits,                    // exit 2
    CountCeiling,               // exit 2
    TableNotAscending,          // exit 2
    TablePastEof,               // exit 2
    MissingUnitName,            // exit 2
    SidecarIncomplete,          // exit 2
    BadSidecarMagic,            // exit 2
    SidecarMismatch,            // exit 2
}

pub fn read_path(path: &Path) -> Result<Vec<LevelUnit>, LevelError>;
pub fn parse_pcm(bytes: &[u8], expected_name: &str) -> Result<LevelUnit, LevelError>;
pub fn parse_pmf(bytes: &[u8]) -> Result<PmfHeader, LevelError>;
pub fn parse_pnf(bytes: &[u8]) -> Result<PnfHeader, LevelError>;
```

`Usage` is only produced by the CLI arm; `read_path` owns the path and format
errors so tests can call it without a process.

## Testing strategy

| Layer | What to test | Approach |
|---|---|---|
| Unit | PCM count/offset model both arms, truncation at every boundary, first-two offsets, last record past EOF | synthetic `Vec<u8>` builders |
| Unit | name extraction: own name, link split, duplicate counting, noise rejection, name-length bounds | synthetic buffers with planted runs |
| Unit | PMF/PNF headers: magic, count ranges, length rule, id/count match, incomplete pair | synthetic sidecars |
| Unit | sort order and output line shape | call the formatter directly |
| Integration | arg shapes, path kinds, exit codes 0/1/2/3, no partial output on failure | process-level CLI tests in the existing style |
| Manual | one linked room, one linkless room, one sidecar-present room, one sidecar-absent room | user acceptance, relative names only |

## Threat matrix

N/A — no routing, shell, subprocess, VCS/PR automation, executable-file
classification, or process integration. The change reads user-supplied data
files and writes stdout through the existing broken-pipe-safe helper.

## Migration / rollout

No migration. Additive CLI arm; all existing modes keep their behavior.

## Open questions

- [ ] None block this slice. Geometry, placement, and PMF codec questions are
      recorded above as UNKNOWN rather than guessed.

# area-levels — SR1 remaster room data probe

Read-only probe of the SR1 remaster `<install>/1/AREA` tree. No game bytes,
absolute install paths, coordinates, or decompiled code are recorded here.
Date: 2026-10-27. Box: 3 passes, ~90 minutes.

## Scope and method

The probe asked four questions: what is `.PNF`, what is `.PMF`, what is
`.PCM`, and what is the smallest visible win. Method: file census (counts and
sizes only), header reads, cross-file identity checks, lexical string
extraction, and structural statistics. All scratch work stayed outside the
repo. No file under the game tree was modified.

## Census (aggregate)

- 12 room directories under `AREA`, plus 5 loose files (3 `.TXT`, 1 `.MAP`,
  1 `.RAW`).
- 89 `.PCM` (9,136–274,084 B), 23 `.PMF` (80–289,688 B), 23 `.PNF` (22–30 B).
- 23 basenames have the full triple; 66 `.PCM` units have no PMF/PNF sibling.
- The asymmetry is real: `.PCM` is per unit, `.PMF`/`.PNF` are per *some*
  units.

## What each container is

### `.PCM` — the unit container (observed)

- Header: `u32 N`, then `N` `u32` **relative** offsets. `offset[0] = 0`,
  `offset[1] = 4`, and the list is strictly ascending. The data base is
  `4 + 4N`; record `i` starts at `base + offset[i]`.
- The model holds on **89/89** files.
- The first two records are small (4 and 80 bytes in the samples) and look
  like a directory; their meaning is not proven.
- A lexical scan of printable runs finds an object-name table interleaved
  with the records. Every file contains its own unit name (**89/89**).
- 368 names have the form `<target>,<index>`; 346 (94.0%) resolve to a real
  `.PCM` unit. The rest are special targets (for example a warp-gate marker)
  or names of units not present in this install. 11 files have no link names.
- Names also include entity classes such as player start, ambient sound,
  camera path, light, and monster types. One illustrative pair: unit `lair5`
  lists `lair6,8`.
- The record payloads contain geometry/collision-like data, but vertex, face,
  and collision layouts were **not** decoded.

Reading: `.PCM` is the per-unit level container. The name table is a real
object/portal name list, not float-bit noise. The `<target>,<index>` names are
**inferred** to be portal/door links (94% target resolution is the evidence;
the semantics are not proven).

### `.PMF` — a segment bank, audio-like (derived, strong)

- Fixed file header: 4-byte magic, version, 16-bit id, 16-bit segment count.
- 23/23 parse. Segment counts observed: 1, 2, 3, 4, 5.
- Each segment starts with a 24-byte header carrying a size word and a rate
  word. The first segment starts at the file header boundary; later headers
  are reachable by walking the size.
- The rate word is **22,050 in 18/23** files and **11,025 in 5/23**.
- The payload is smooth 16-bit little-endian data: across six sampled files,
  lag-1 autocorrelation 0.81–0.998, zero-crossing rate 0.015–0.173, and a
  decaying tail in the largest sampled files.
- Four size groups are **byte-identical except the id word**: 145,184 ×3,
  63,852 ×2, 5,416 ×5, 80 ×4. The 80-byte files are an all-zero segment.

Reading: audio-like sample bank. This is **derived**, not decoded: no playback,
no codec proof, no loop semantics. The identical same-size files are recorded
as a discovery, not explained.

### `.PNF` — the PMF sidecar (observed)

- Magic `FNPa`; 20-byte header + one `u16` per segment.
- `len = 20 + 2 * count` in 23/23.
- The id and count match the PMF of the same basename in **23/23**.
- The `u16` values are not interpreted; they are likely per-segment ids, but
  that is **inferred**.

Reading: a tiny index sidecar for the PMF, not a level file.

## Probe ledger

| # | Probe | Result | Class |
|---|---|---|---|
| P1 | Census | 89 / 23 / 23; 23 triples; 66 PCM-only units | Observed |
| P2 | PCM header | count + ascending relative offsets; 89/89 | Observed |
| P3 | PCM own name | 89/89 | Observed |
| P4 | PCM links | 368; 346 resolve (94.0%); 11 files with none | Observed / Derived |
| P5 | PCM payload | mixed records and geometry-like data; not decoded | Observed shape / UNKNOWN |
| P6 | PMF header | magic, id, segment count; 23/23 | Observed |
| P7 | PMF segment | 24-byte header with size + rate; walkable | Observed |
| P8 | PMF identity | 4 same-size groups, identical except id | Observed |
| P9 | PMF payload | smooth 16-bit LE; 22,050/11,025 rate word | Derived (strong) |
| P10 | PNF sidecar | 20-byte header + `u16` per segment; 23/23 id/count match | Observed |
| P11 | Community docs | PS1 engine only, not the remaster container | Observed docs |
| P12 | Search | web search worked; `grep.app` 429 | Observed |

## Evidence classes

- **Observed** — repeatable byte-level fact on the census.
- **Derived (strong)** — model with no census exception (PCM offsets, PNF
  length rule, PMF audio-like reading).
- **Inferred** — semantic reading permitted but not proven (links are portals;
  PNF entries bind emitters).
- **UNKNOWN** — not established, not guessed.

## What stays UNKNOWN

1. PCM geometry and collision layouts; no vertex/face decode.
2. Object placement and transforms; no positions or counts claimed.
3. PMF codec, loop points, exact segment table, and the remaining header
   words; playback out of scope.
4. Why same-size PMFs are identical (shared ambience vs placeholder vs
   packaging artifact).
5. The semantic of the 16-bit id and of the PNF `u16` values.
6. The meaning of PCM records 0 and 1.
7. The trailing bytes after the last PCM record.
8. Whether `.PCM` is a repacked PS1 `.drm`; the name semantics agree, the
   container does not.

## Community research

| Source | License | What it establishes | Limit |
|---|---|---|---|
| https://github.com/fmil95/soul-re | MIT | PS1 decompilation; per-unit area configs (`aluka`, `kain`, …) for the PS1 build | does not document the remaster `.PCM/.PMF/.PNF` container |
| https://github.com/Gh0stBlade/KAIN2 | Unlicense | PS1-era engine structures: `_StreamUnit.baseAreaName[16]`, `StreamUnitPortal.tolevelname[16]`, `Level.objectNameList`; `STREAM_FillOutFileNames` emits per-unit `.drm`/`.crm`/`.snf` | same limit: PS1 engine, not the remaster container |

No code was copied. Only format/behavior facts were read and restated. The
PS1 sources support the *semantics* (per-unit names, portal target names,
object-name lists), not the remaster byte layout.

Search status: the web search tool returned results (no 403 this session).
`grep.app` returned HTTP 429 on the magic queries. No remaster-format tool or
documentation was found. That absence is recorded as a limit, not as proof
that none exists.

## Smallest visible win

Candidates were: (a) header-only `--level-info`; (b) object placement
positions and a count; (c) room geometry. (b) and (c) are **NO-GO**: the
placement table was not located and the geometry layout was not decoded. (a)
is **GO** and is more than a census: the PCM name table yields the unit's
portal/door graph, and the PMF/PNF pair yields the sidecar shape.

Chosen slice: `--level-info <PATH>` prints one `level` line per unit (record
count, name count, link count, PMF segment/rate/PNF counts) and one `link`
line per link. Header and name table only. No render, no playback, no
geometry.

## Next probes (future changes)

1. Parse PCM records 0/1 as a directory and map record kinds (name table,
   terrain, collision, objects).
2. Search for an instance/transform table by looking for bounded 16-bit
   triples or float triples; validate against a known unit before claiming
   placement.
3. Compare the PCM record counts with PS1 `Level` counts (BSP leaves, faces)
   to test the repacked-`.drm` hypothesis.
4. Decode one PMF segment to 16-bit PCM outside the repo, check the spectrum,
   and find the loop fields; do not ship audio.
5. Compare same-size PMFs across the whole install to see whether the
   identity is a global duplication or a local one.
6. If the original PS1 data is available, compare unit names and counts.

# Proposal: object-models (outcome: archived object meshes located; docs-only + reader proposed)

## Intent

Placed objects currently render as uniform cube markers because the viewer has
no way to turn an object name (`raziel`, `undhost`, `warpg`, …) into a mesh.
This pass asked where the archived object models live, what their payload shape
is, and how a level object name resolves to one. Probe-first; a documented
no-go was acceptable.

## Outcome

**The models are CONFIRMED in archive `.PCM` object containers** (loader type 1),
not in `.SRM` files and not in the level mesh. The outer container is the same
relocation-directory `.PCM` shape as a level: `u32 N`, `N` relocation positions,
padding, then `data_base = 4*roundup512(N)`. The "first offset is 12" anomaly is
explained by the object header having no self-pointer at offset 0.

**The mesh is CONFIRMED.** A single-model object header points at an 8-byte
vertex array (`i16 x, y, z`, `u16 normalID`) and a 12-byte polygon array
(`u16 v1, v2, v3`, `u8 normal`, `u8 flags`, `u32 materialOffset`). Materials are
16-byte records at model `+0x30`.

**The falsifiable test passes at corpus scale.** 394 single-model containers
hold 18,740 vertices and 32,557 polygons with **0 out-of-range indices**. All
394 satisfy `vertexStart + vertexCount*8 == polygonStart` and
`polygonStart + polygonCount*12 == endOfPolygons`. 32,370/32,370 textured
polygons land on a valid 16-byte material record.

**Name -> model is CONFIRMED.** Across 364 level units, 375 of 378 distinct
placed-object names resolve to an object container. Only `campath`, `marker`
and `splob` have no mesh. The fourth bigfile index dword is a four-character
name suffix (last four characters, uppercased), not a loader type.

Full evidence: `docs/notes/object-models.md` (ledger O1-O14, corpus numbers,
UNKNOWN list).

## What changed versus prior assumptions

- Entry 66 is the standard `.PCM` container, not a different shape.
- `N = 1813` is a relocation count, not a record count.
- The fourth index dword is a name suffix, not a type tag.
- Archived object models are the legacy low-poly format; loose `.SRM` assets
  are the remaster high-detail family and are a separate problem.

## Proposed slice (do not build in this pass)

A small, verifiable reader that proves the format and unlocks real meshes:

1. `src/object.rs`: parse one object container — outer relocation directory,
   object header, names, model header, vertices, polygons, material offsets.
   Validate both array-boundary identities and every polygon index. No
   rendering, no textures.
2. `--object-info --bigfile <BIGFILE.DAT> --entry <INDEX>`: print name, vertex
   count, polygon count, index min/max and validation status. No coordinates.
3. `--object-mesh --bigfile <BIGFILE.DAT> --entry <INDEX>`: feed positions and
   triangle indices into the existing mesh pipeline so a chosen object renders
   instead of a cube marker.

Out of scope: bones/posing, textures/UVs, multi-part containers (`human`,
`vlgr`, `paths`), the auxiliary type-3 table, orientation.

## Forecast split

| Part | Lines |
|---|---:|
| Production (parser + two CLI arms + mesh bridge) | ~240 |
| Tests (synthetic containers; boundary/index/refusal cases) x1.5 | ~360 |
| Docs (note reader section + README line + change docs) | ~40 |
| **TOTAL** | **~640** |

## PR plan

Kept under the 400-line single-PR budget by splitting at the parser boundary.

1. **PR 1 — parser + `--object-info`** (`feat/object-model-parser`): parser,
   validation, counts-only CLI, synthetic tests. Production ~140, tests ~210,
   docs ~25, TOTAL ~375.
2. **PR 2 — `--object-mesh` render bridge** (`feat/object-mesh-render`):
   positions/indices into the existing mesh path, synthetic tests, README and
   note reader section. Production ~100, tests ~150, docs ~20, TOTAL ~270.

This pass adds no code; it adds the note and this proposal/spec/tasks set.

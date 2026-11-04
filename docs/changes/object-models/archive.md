# Archive: object-models — DONE (archived models parse, validate and render)

**Stage banner: DONE.** Placed-object meshes live in `BIGFILE.DAT` `.PCM` OBJECT
containers (loader tag `1`), not loose `.SRM` files. `--object-info`,
`--object-mesh` and `--level-render --objects` read one container in memory;
nothing is extracted and no temp file is written.

## Delivered and verified

- Outer container: the same relocation directory as a level `.PCM`; `u32 N`,
  `N` positions, padded data base `4 * roundup512(N)`, then the object header.
- Vertices: 8-byte `i16 x, y, z` plus `u16 normalID`. Positions are bone-local.
- Rest pose: 24-byte bone records at model `+0x1C` (`+8` first vertex, `+10`
  last vertex, `+12` three `i16` translation, `+18` parent id); adding the
  accumulated ancestor translations per vertex range reconstructs the rest pose.
- Polygons: 12-byte `u16 v1, v2, v3`, normal byte, flag byte, `u32` tail.
  Corpus: 394 containers, **18,740 V / 32,557 P**, 0 out-of-range indices.
- Names: **375/378** distinct placed-object names resolve; `campath`, `marker`
  and `splob` are placeholders with no mesh and keep a flat marker.
- Index correction: the fourth BIGFILE dword is the **last four characters of
  the name, uppercased** (`raziel` -> `ZIEL`), not a loader type.

## Limits and merge

Archived models are legacy low-poly assets, not the remaster `.SRM` meshes;
`human`, `vlgr` and `paths` are multi-part and refused, and the object record
stores no orientation. PR #71 (docs), #72 (`--object-info`) and #73
(`--object-mesh` + placed-object render) merged into `main` at `c18a222`. No game
bytes, absolute paths, coordinates or hashes were committed.

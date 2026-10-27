# `.SRM` bone block - decode ledger (2026-10-26)

Planning note for the `animation` change. Read-only probe of the SR1
remaster game data under `1\OBJECT\`; no writes to the install, no game
bytes copied into the repo, no raw coordinates or dumps below. Scratch
lived only in the session temp directory. This note records what the
bytes support and what they do not, so a later slice does not re-derive
it.

## Box

Three passes, no Ghidra, no `.SRA` decode:

1. Census and block arithmetic across every loose `.SRM`.
2. Block shape, per-vertex fields, and mesh-to-joint alignment on a small
   extra=0 humanoid.
3. Community tools and `.SRA` scoping.

## Census

- 349 loose `.SRM` files under `1\OBJECT\`.
- 349/349 parse to exact EOF under
  `4 + 4 + shaders*44 + 4 + materials*32 + 4 + (32+extra)*48 + [4 sentinel] + 128 flags + 8 counts + vertices*32 + faces*2`.
- `extra` distribution: 342 files at 0; the rest at 1, 7, 17, 25, 26, 26,
  26. The 32-record joint portion is present in every file.

Evidence class: observed for the arithmetic and distribution.

## Block shape

| Part | Shape | Evidence class |
|---|---|---|
| HD matrix class | First `extra` records, 48 bytes each, twelve f32 LE. On all 128 records across the 7 `extra > 0` files the upper 3x3 is near-orthonormal and the remaining three floats act as a translation: a 3x4 affine row-major transform | Derived (strong) |
| Joint pool | Next 32 records, 48 bytes each, four `vec3` f32 LE = 128 joint slots in file order. Plausible (finite, magnitude > 1 unit) slot counts across the census range 0..65, median 1; 164 static-prop files have none. The four points of a record read as a short chain; 17 records of the extra=0 humanoid have at least two plausible points | Derived + inferred grouping |
| Sentinel | Optional `0x80000000` word after the joint pool; both arms occur | Observed |
| Flags | 128 bytes after the sentinel; values are 0 or 1. Counts correlate with used bones on several extra=0 models but are not index-aligned; semantics UNKNOWN | Observed shape, UNKNOWN meaning |

The community Blender addon reads every 48-byte record as four `vec3`
positions and builds one unparented bone per point. Its own comments say
the structure is unknown and "supposedly only carries location vectors".
The split above (matrices first, then 32 joint records) is a clean-room
refinement from the orthonormality check, not copied code.

## Per-vertex bone fields

| Field | Offset in the 32-byte stride | Observation |
|---|---|---|
| Three bone indices | 20..22 | `u8`; maximum observed 67; no index reaches 128 across the 349 files |
| Three weights | 24..26 | `u8`; every triple sums to 255 across all parsed vertices |
| Material / U / V | 19 / 23 / 27 | Unchanged from the mesh-textures slice |

Evidence class: observed. Consequence: the pose math can index a fixed
128-slot joint pool without a lookup table, and the byte weights can be
used directly as `w / 255`.

## Alignment and mapping

On an extra=0 humanoid with 65 plausible joints (finite, magnitude > 1
unit):

- Raw joint positions require a Z negation to overlay the mesh. Under
  `(x, y, -z)`, the median nearest-vertex distance is about 21 units on a
  roughly 500-unit tall model; the max is about 252. Identity, X/Z swaps,
  and other sign conventions are materially worse.
- High-weight vertex clusters sit about 42..145 units from their slot
  joint. That is consistent with a bone head at one end of a limb and the
  cluster along the bone, but it does not prove anatomy or hierarchy.
- Some joint slots are unused and contain tiny denormal values rather
  than exact zero; all 349 files have zero non-finite joint values. A
  renderer or pose helper should therefore treat "plausible joint" as
  finite and above a small magnitude, not as `!= 0.0`.

Evidence class: derived for the axis; inferred for the slot-to-cluster
mapping. The mapping is playtest-gated in the `animation` change.

## What the block is not

- Not a hierarchy: no parent indices, no child lists, no names.
- Not clip data: no keyframe table, time base, or clip count in the
  `.SRM`. The `.SRA` sibling is the next candidate.
- Not self-describing: the 128 flags are 0/1 and their indexing is not
  established.
- Not uniform across model classes: the 7 `extra > 0` files prepend 3x4
  affine matrices whose relationship to the 128 joints is UNKNOWN. The
  `animation` slice refuses `--pose` on that class rather than guessing.

## Community tools (exact URLs + licenses)

| Tool | URL | License | Relevance |
|---|---|---|---|
| io_scene_xrm | https://github.com/TheDodylectableX/io_scene_xrm | No LICENSE file; GitHub API reports no license | Only remaster `.SRM` tool found that reads the bone block. Reads four positions per 48-byte record, builds unparented bones, no `.SRA` parser. Format facts only; no code copied |
| ModelEx | https://github.com/TheSerioliOfNosgoth/ModelEx | LICENSE present; GitHub classifies `NOASSERTION` | Original `.drm` / `.pcm`, not remaster `.SRM`; background only |
| Recombobulator | https://github.com/TheSerioliOfNosgoth/Recombobulator | GPL-3.0 | Original SR1 `.pcm`; has original-game animation structs, not remaster `.SRA`; background only |
| AsunderNoMore | https://github.com/TheSerioliOfNosgoth/AsunderNoMore | MIT | Bigfile packer; no model or animation |
| Tomb Raider Forums SRM thread | https://www.tombraiderforums.com/showthread.php?t=230457 | Forum terms | Prior material/UV/weight notes; no skeleton decode |

No documented remaster `.SRA` parser or animation importer was found.

## Next candidates

1. **Skeleton overlay** - draw the 128 joints and their four-point chains
   over the mesh. The geometry is already supported by this ledger; it
   needs a small line/point render path and the same extra=0 scope.
2. **`.SRA`** - 227 siblings, count-like first word, no text magic, no
   community parser. Scope it separately before any animation claim.
3. **`extra > 0` matrix semantics** - decide whether the 3x4 records are
   HD bind transforms and how they relate to the 128 joints. Refused by
   the first pose slice.
4. **Ghidra** - only after `.SRA` and the matrix class are scoped; the
   repo rule defers it until community formats are exhausted.

## Legal

Counts, field shapes, and aggregate distances only. No game bytes, no
absolute install paths, no per-file coordinates, no hashes, no dumps.
Game data was read in memory and never written.

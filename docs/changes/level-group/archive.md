# Archive: level-group — DONE (family merge)

**Stage banner: DONE — the opened level is four contiguous rooms.**

`--group <PREFIX>` scans the archive for `.PCM` entries whose self-name is the
prefix plus digits, sorts by suffix, and reports each room's `V`/`F` plus totals.
On the SR1 archive `--group under` discovers `under1..under4`: 1,468/2,922,
252/472, 657/1,174 and 269/508, merged to 2,646 vertices / 5,076 faces. Room
vertices are world space, so the merge concatenates positions and offsets each
room's `u16` face indices by its vertex base; no transform is applied. Bounding
boxes chain contiguously on `x`. Evidence: `docs/changes/level-group/proposal.md`.

## What landed
- PR #65 (`feature/level-group`, merge `53173ac`): `fae8f7a` discovers and merges
  a family, `0a51381` adds `--group`, `6697361` documents the slice.
- `--group` is mutually exclusive with `--entry`; empty prefix and no match are
  distinct exit-1 errors with no window.

## What stayed UNKNOWN
- Per-room transforms; none are applied or claimed.
- Textures and objects were out of scope here (objects followed in PR #66).
- The 16-bit vertex-index ceiling is a loud error, not a wrapped index.

## Next step
The family render is the shortest demo path; next is object markers on the merged
group (now supported by `--level-render ... --group under --objects`) and, later,
textures on level geometry.

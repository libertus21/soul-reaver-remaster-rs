# Archive: first-level — DONE (opening unit identified)

**Stage banner: DONE — the new game loads `under1`, not the loose `UNDRCT1`.**

The new-game branch seeds the current-unit name buffer with the literal `under1`;
the archive path builder composes `area\under\bin\under1.pcm`. The default archive
contains one level whose self-name is `under1`: read-only it parses to 1,515
offset-table records, 137 name strings, 1,468 vertices and 2,922 faces, with zero
out-of-range indices. The loose `UNDRCT1.PCM` is a different unit (`undrct1`,
3,689 vertices / 6,765 faces). Evidence: `docs/notes/first-level.md`, proposal
under `docs/changes/first-level/`.

## What landed
- PR #63 (`docs/first-level`, merge `8a11b9c`, commit `8c5f65b`) — docs only.
- Archive-entry access followed in PR #64 (`feature/level-archive`, merge
  `436421f`): `--level-geometry --bigfile <DAT> --entry 115` reads `under1`.
- Commit `3cf1cce` reads one `.PCM` unit from an archive entry in memory;
  `c14b4ad` adds the archive selectors; `3f61e55` documents the slice.

## What stayed UNKNOWN
- The save-field offset that overrides the default unit name.
- Runtime order (the state sequence was read statically, not measured).
- How the archive maps a path to an opaque name hash.
- When the loose `undrct` set is used, and `under1`'s spawn/initial object state.

## Next step
Dynamic-trace one new game to record the first archive path actually requested;
then locate the save field carrying the current unit name.

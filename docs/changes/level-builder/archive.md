# Archive: level-builder — DONE (structural model)

**Stage banner: DONE — the `.PCM` offset table is a relocation directory.**

Entry `i` of the table is the data-base-relative position of a dword that holds a
non-zero data-base-relative offset. The engine reads only `N`, rounds it up and
shifts the payload to the data base, then walks a fixed header schema: each slot
is a count or an offset with its own builder/copy. There is no size rule, no type
tag and no ordinal dispatch; type is decided by the header slot. The structural
reader `--level-structure` reports the integrity and the 36-byte table.
Evidence: `docs/notes/level-builder.md`.

## What landed
- PR #49 (`docs/level-builder`, merge `7d8f26d`, commit `44edf1b`) — relocation
  directory model.
- PR #50 (`docs/level-builder-table`, merge `568257c`, commit `7aa6e84`) —
  corrected origin (`data_base + entry`, not `file + entry`) and zero-entry read.
- PR #51 (`feature/level-structure`, merge `53b235c`, commits `394cc09`,
  `82b953d`, `6656066`) — reader plus docs.

## What stayed UNKNOWN
- Field meanings of the 36-byte record and the `0xBE 0xEF` tagged blob.
- The semantic role of the 48-strided pointer array and the two 0x40-byte header
  blocks (`0x60`/`0x61`).
- Whether any non-tag-2 path reads the directory; writer-tool identity.

## Next step
Follow the consumer of the 40-byte runtime array and the owner of the 48-strided
array; a whole-binary search for `base + offset[i]` should close the directory
question for good.

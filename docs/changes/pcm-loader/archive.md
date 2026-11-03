# Archive: pcm-loader — DONE (container base resolved)

**Stage banner: DONE — the `.PCM` container base is confirmed; field semantics are not.**

The loader queue reads raw bytes, and the drain takes the first dword `N`, rounds
the directory up to a 2048-byte boundary and shifts the payload to the data base:
`data_base = round_up(4 + 4*N, 2048)`, equivalently `4 * roundup512(N + 1)`. The
gap from `4 + 4N` to the data base is zero in 89/89 units. Tag 2 is the area
`.PCM` (a 0x248 level struct); tag 1 is the object container. Evidence:
`docs/notes/pcm-loader.md`.

## What landed
- PR #46 (`feature/pcm-loader-plan`, merge `0fdd6b9`, commits `dfac896`,
  `1472a2c`) — the loader walk and the L13 resolution.
- PR #47 (`fix/pcm-data-base`, merge `e7bbde1`, commit `af668a5`) — code now
  addresses blocks from the padded data base.
- Readers: `--level-structure`, `--level-geometry`, `--level-objects`.

## What stayed UNKNOWN
- The meaning of the level-header fields and the 36-byte source record.
- The `0xBE 0xEF` tagged-blob interior (only the length rule is known).
- The object `.PCM` decompressor body and format.

## Next step
Field semantics were taken up by `level-builder` and `level-geometry`; the object
container decompressor stays deferred until object meshes are in scope.

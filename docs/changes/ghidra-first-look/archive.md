# Archive: ghidra-first-look — DONE (baseline survey)

**Stage banner: DONE — read-only baseline of `1\sr1.dll`.**

Ghidra 12.1.4 auto-analysis recovered 5,243 functions and 2,445 defined strings.
All 86 external functions are `KERNEL32.DLL`; the image is a plain PE with no DRM,
packing or anti-cheat, and no PDB symbols. The single export resolves to `run`
(also ordinal 1), which references the default archive name `bigfile.dat`. The
survey mapped the bigfile loader/read/error clusters and the `.data` archive-name
table. Evidence: `docs/notes/sr1-ghidra-survey.md`; plan under
`docs/changes/ghidra-first-look/`.

## What landed
- PR #14 (`feature/ghidra-first-look`, merge `40cab52`, commit `a940a5f`) — the
  behavior survey, string inventory, spot checks and follow-up candidates.
- No `src/` change; 83 tests green at the time (docs-only baseline).

## What stayed UNKNOWN
- The launcher-to-`run` linkage (later addressed by S3).
- Which functions own the loader's post-open error rows (later S1/S2).
- Semantics of most recovered functions; no symbol names.

## Next step
S1/S2/S3 followed this baseline: loader error parity, call-edge tracing, and the
launcher handshake. S4 then mapped the ordered startup stages inside `run`.

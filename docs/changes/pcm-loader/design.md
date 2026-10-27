# Design: pcm-loader

## Method

Bounded Ghidra recon on `1\sr1.dll`, about two passes.

1. **Recon.** Scan defined strings for `.pcm`, `pmf`, `pnf`, `area`, `object`,
   `level`, `load`, `queue`, `can't` and related terms. Resolve every xref to
   its containing function.
2. **Walk.** From the path builder, follow each caller to the queue entry,
   the drain, the per-type builder and the completion callback. Decompile
   each step into `%TEMP%` and read it there.
3. **Negative checks.** Tag mapping, constant sizes, and count fields checked
   against the decompiled arithmetic rather than against names.

Not done: dynamic tracing, the object decompressor body, the name resolver
body, SR2, `.SRL`.

## Chain (evidence in `docs/notes/pcm-loader.md`)

- Path builder (L1, L2) -> area loader or object builder.
- Queue ring (L4) -> drain common prefix (L5) -> tag dispatch:
  - tag `1`: object `.PCM`, 0x6D90 decompressed instance, 8 sub-models (L6);
  - tag `2`: area `.PCM`, 0x248 level struct (L7);
  - tag `0x32`: `.sra` generic table builder (existing note).
- Level builder (L8, L9): 36-byte source to 40-byte runtime records.
- Completion (L11): level init and name walk; 0x60-byte records; 16-byte
  name-table stride.
- 48 (L10): tracker slot count, not a block.

## Decision record

| # | Decision | Why |
|---|---|---|
| D1 | Verdict is PARTIAL, not documented and not no-go | The pipeline and runtime record are CONFIRMED. On-disk field meaning is not, and L13 is unresolved |
| D2 | Treat "48-byte block" as a misreading | The only 48 in the object path is `MAX_OBJECTS`. A census correlation is not a block |
| D3 | Record the 40-byte class as "runtime record from 36-byte source" | Copy shape is CONFIRMED; the file-side block census is not reconciled with it, so the file block is not claimed |
| D4 | Do not name any function, address or field offset in the planning bundle except as role names | User legal rule. Structure is described by role and size, never by address or offset |
| D5 | Probe L13 before any reader | A reader built on a wrong first-dword rule breaks every unit |

## Evidence ledger

See `docs/notes/pcm-loader.md`, table L1-L16. Summary of classes:

- CONFIRMED: L1, L2, L3 (values), L4, L5 (mechanics), L6 (sizes), L7, L8
  (structure), L9 (copy shape), L10 (count), L11, L12.
- INFERRED: L3 (type-word binding for the area site), L6 (8 sub-models).
- UNRESOLVED: L13.
- UNKNOWN: L14, L15, L16.

## Risks carried into the next slice

- L13 may invalidate the header table described in the note. If the drain's
  first-dword rule is a wrapper, every table offset in the note is relative to
  the wrong base until the wrapper is identified.
- The name resolver (L15) is unread. The 36-byte record's first field depends
  on it.
- Census correlations (40/48) were used by an earlier note; this change says
  they are not evidence of block types.

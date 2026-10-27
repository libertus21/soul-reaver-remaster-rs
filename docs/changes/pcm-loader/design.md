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
- Drain prefix (L13): the first dword is the directory count N; the drain
  strips `4 * roundup512(N)` bytes and the level builder is called at that
  data base. Directory and header offsets are relative to it.
- Level builder (L8, L9): 36-byte source to 40-byte runtime records; the first
  source dword goes through the name resolver (L15).
- Completion (L11): level init and name walk; 0x60-byte records; 16-byte
  name-table stride.
- 48 (L10): tracker slot count, not a block.

## Decision record

| # | Decision | Why |
|---|---|---|
| D1 | Verdict stays PARTIAL, not "documented" | The pipeline, the container base and the runtime record are CONFIRMED. Field meaning is not, so "documented" would overclaim |
| D2 | Treat "48-byte block" as a misreading | The only 48 in the object path is `MAX_OBJECTS`. A census correlation is not a block |
| D3 | Record the 40-byte class as "runtime record from 36-byte source" | Copy shape is CONFIRMED; the file-side block census is not reconciled with it, so the file block is not claimed |
| D4 | Do not name any function, address or field offset in the planning bundle except as role names | User legal rule. Structure is described by role and size, never by address or offset |
| D5 | Probe L13 before any reader was correct; it is now resolved | The container base is settled (L13), so a reader may implement the container. Field semantics still gate the record layer |
| D6 | The `.PCM` data base is `4 * roundup512(N)`, not `4 + 4N` | Code (the drain arithmetic) plus 89/89 corpus (name field at the base, zero padding before it). The data-side note is corrected by a dated block, not rewritten |

## Evidence ledger

See `docs/notes/pcm-loader.md`, table L1-L16. Summary of classes:

- CONFIRMED: L1, L2, L3 (values), L4, L5 (mechanics, corrected), L6 (sizes),
  L7, L8 (structure), L9 (copy shape), L10 (count), L11, L12, L13 (code +
  89/89 corpus), L15 (resolver behavior).
- INFERRED: L3 (type-word binding for the area site), L6 (8 sub-models).
- UNKNOWN: L14, L15 (interior of the tagged record), L16.

## L13 resolution (what moved)

- The first dword is N. There is no wrapper.
- The drain strips `roundup512(N)` dwords, where `roundup512(N)` is the next
  multiple of 512 strictly above N. In bytes the final buffer is
  `4 * roundup512(N)`.
- For the sampled units the skip is 2,048 / 8,192 / 10,240 bytes, never the
  file size and never `4 + 4N` (see the note's three-number table).
- Consequence for the plan: the reader's base is the data base; directory and
  header offsets are relative to it; the old `4 + 4N` base is off by the
  padding (`4 * roundup512(N) - 4 - 4N`, 1,796 bytes for a 62-entry unit).

## Risks carried into the next slice

- Field semantics (L16) and the tagged-record interior (L15) are still
  UNKNOWN. A reader may parse the container and the level header shape but
  must not name fields.
- The original `area-levels` note's `4 + 4N` base is a live trap for any
  consumer; the dated correction block is the only guard.
- Census correlations (40/48) were used by an earlier note; this change says
  they are not evidence of block types.

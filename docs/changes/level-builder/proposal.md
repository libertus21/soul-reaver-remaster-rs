# Proposal: level-builder (outcome: partial; docs-only, one reader slice proposed)

## Intent

Answer what the SR1 level builder does with a `.PCM` level unit: the per-block
loop, the type-dispatch rule, the 36 -> 40 conversion, the "name resolver", the
48-byte class and placement. Four earlier byte-level hypotheses about blocks
had failed, so the binary was treated as the only source of truth.

## Outcome

**The `.PCM` offset table is a relocation directory, not a block table.** Entry
`i` is the file position of a dword that holds a non-zero base-relative offset.
The loader uses only `N` (to find the aligned data base) and discards the
directory; the level builder then walks a fixed header schema. Type is decided
by the header slot - not by size, block content or ordinal. The first word of a
"48-byte block" is therefore a base-relative pointer, and 48 is the distance to
the next pointer.

Also established:

- the exact 36-byte -> 40-byte field map (extra four bytes = the 4-byte offset
  widened to an 8-byte pointer);
- the "name resolver" is a tagged-blob copier (`0xBE 0xEF`, count byte, typed
  variable-length entries), not a string lookup; no magic -> null;
- names come from a separate 16-byte table at header slot 0x23 (terminated by
  `0xFF`) and from a C string at 0x26;
- the level builder applies no data-driven transform; only name-keyed hardcoded
  patch code nudges i16 fields.

Full evidence: `docs/notes/level-builder.md` (ledger B1-B13, UNKNOWN list).

## What changed versus the prior notes

- `pcm-loader` L9/L10: the copy shape stands; "no 48-byte block" becomes "no
  48-byte *type*; 48-byte spans are pointer spacings".
- `pcm-content-census`: its 4/40/48 classes are shape-only and cannot be read as
  a type taxonomy. The lag-6 period is payload between pointers.
- The earlier "objects with a link/parent" lead is **not supported**: the first
  dword is a link, but it is not a parent link and not a level object.

## Proposed next slice (smallest useful reader)

A read-only structural report, not a semantic decoder:

1. Parse the fixed header schema (counts and base-relative offset slots).
2. Validate the relocation directory against that schema: every non-zero offset
   slot is listed, zero slots are omitted, every entry's value is in range.
3. Decode the 36-byte table (count, stride, per-record blob length or null) and
   report it separately from the 4/40/48 shape census.
4. Rename the CLI's block wording from "shape/block" to "pointer span" where the
   old wording implies a record; keep `BlockClass` as a shape hint only.

No geometry, transform, audio or gameplay decode.

## Forecast split

| Part | Lines |
|---|---|
| Production (schema + directory validation + 36-byte report in `src/level.rs`, CLI arm) | ~120 |
| Tests (synthetic header/directory/36-byte cases, refusals) x1.5 | ~180 |
| Docs (note update + README line) | ~40 |
| **TOTAL** | **~340** |

Inside the 400-line single-slice budget. If the directory validation needs a
second shape (the tag-1 object container), split it into a follow-up.

## No-go recorded here

- Placement / transforms: still NO-GO for decoding. The builder composes no
  transform; downstream consumers of the 40-byte records were not traced.
- Geometry / mesh: still NO-GO. This pass names the node tree but does not
  decode vertices or faces.
- 48-byte semantic role: UNKNOWN. The mechanism is proven, the meaning is not.

## Next probes if the slice is not funded

1. Consumer of the 40-byte array (level +0x190/+0x198 and the blob pointer).
2. Owner of the 48-strided pointer array (second pointer of the room's 36-byte
   part record).
3. Whole-binary search for any reader of `base + offset[i]` outside tag 2.

## Non-goals

Game bytes in the repo, addresses, decompiled listings, mesh/vertex decode,
transform/placement decode, rendering, block type taxonomy.

# `level-builder` - what the engine actually does with a `.PCM` level unit

Bounded, read-only Ghidra 12.1.4 pass on the SR1 remaster binary (`1\sr1.dll`,
AMD64). Behaviour notes only: role names, field offsets and a few short engine
literals. No addresses, no disassembly, no decompiled text, no game bytes, no
absolute paths. Ghidra project, scripts and decompiler output lived in `%TEMP%`
only.

Date: 2026-10-27. Box: three passes (call-tree dump, real-unit cross-checks,
focused re-reads), inside the ~3 h budget. Evidence class per claim:
**CONFIRMED** (code and bytes agree), **INFERRED** (code only, or bytes only),
**UNKNOWN**.

## Verdict

**The `.PCM` offset table is not a block table.** It is a **relocation
directory**: entry `i` is the file position of a dword that holds a non-zero
base-relative offset. Consecutive entries therefore bracket pointer fields, and
the "block sizes" used by earlier notes (4/40/48) are *spacings between
pointers*, not record sizes and not types.

The level builder never reads that table. Before it runs, the loader reads only
`N`, rounds it up to the next 512-dword boundary, and shifts the payload down to
the data base, so the builder receives the payload at offset 0 and has no handle
on the directory. The builder then walks a **fixed header schema** at the data
base. Each slot is either a count or an offset, and each offset slot has its own
builder or copy. There is no per-block type tag, no size rule and no ordinal
rule.

This closes the long-standing unknown: **the first word of a "48-byte block" is
a base-relative pointer**, not a small integer of unknown meaning. Across the
89 SR1 area units all 46,124 directory entries hold an in-range offset; none is
zero or out of range.

## Method / box

1. **Re-pin + call tree.** Re-found the tag-2 path from the `.pcm` path
   literal, followed the queue drain and both tag-2 completion copies, then
   walked every callee of the level builder to a fixed point. Decompiled to
   `%TEMP%`; no listings entered the repo.
2. **Real-unit cross-checks.** Parsed all 89 SR1 area `.PCM` units read-only and
   compared the directory against the header schema and the builder's strides.
3. **Focused re-reads.** Re-read the 36-byte record loop, the tagged-blob
   copier, the room/geometry sub-builders and the name-keyed patch tables.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| B1 | Tag 2 reaches the level builder only after the loader shifted the payload down by a 512-dword-rounded `N`; the builder receives the payload at offset 0 and never reads the directory. Both tag-2 completion copies do the same shift. | CONFIRMED |
| B2 | The directory is a relocation map: in 5/5 sampled units every header dword slot that the builder treats as an offset is listed when its value is non-zero, and omitted when it is zero (24/24 slots per unit). | CONFIRMED |
| B3 | In all 89 area units, **46,124 / 46,124** directory entries hold a value in `1..data_len`; none is zero or out of range. | CONFIRMED |
| B4 | The level builder has **no loop over the directory**. Its only per-record loops are over header-declared arrays (36-byte records; 16-byte name entries). | CONFIRMED |
| B5 | Type is decided by the **header slot**, not by size, content or ordinal. Each slot has a dedicated builder or copy. | CONFIRMED |
| B6 | 36-byte source to 40-byte runtime is a nine-dword copy with the first dword widened to an 8-byte pointer (table in §3). | CONFIRMED |
| B7 | The "name resolver" is a **tagged-blob copier**, not a string lookup: `0xBE 0xEF` magic, count byte at +2, entries from +4 with a type byte and a length byte; types 0..4; whole blob copied; no magic -> null. | CONFIRMED |
| B8 | The tagged blob is variable length (24, 32, 36, 44, 68 bytes observed) and holds small integers; it is not a name. | CONFIRMED (shape) / UNKNOWN (meaning) |
| B9 | The 48-byte class is not a builder type. It is a directory spacing. Two observed cases: a 16-byte four-offset struct followed by 32 zero bytes, and a 48-strided array whose first dword is a pointer to a geometry block. | CONFIRMED (mechanism) / INFERRED (role) |
| B10 | Object names are a 16-byte-stride table at header slot 0x23, terminated by a first byte `0xFF`, and copied whole. | CONFIRMED |
| B11 | The level origin is three i16/u16 pairs copied from header bytes 0x44..0x4F to level +0x7C/+0x80/+0x84, with mirrors at +0x7E/+0x82/+0x86 and +0x88/+0x8A. No transform is composed. | CONFIRMED |
| B12 | The only positional arithmetic in the builder is **name-keyed hardcoded patch code** (short level-name literals plus a table of per-level patch functions). It nudges specific i16 fields; it is not a data-driven placement transform. | CONFIRMED |
| B13 | A shared sub-builder converts source i16 triples to f32 triples (vertex data) and builds geometry node trees with two child pointers per node. This is geometry, not object placement. | CONFIRMED (code) / INFERRED (role) |

## 1. The builder body

Input: `(level_struct, payload, payload)`, where `payload` begins at the data
base. The header is a C struct at payload offset 0. The builder copies it into a
runtime 0x248-byte struct by walking a fixed slot list. For each slot it either
copies a scalar, copies a `count x stride` array, or calls a dedicated builder
with `(count, offset)`. All offset values are base-relative.

The slots, in order:

- `0x00` -> room/zone record (0xA0 runtime). The room record owns the arrays
  listed below.
- `0x01` -> offset to a small struct {count, offset}; the builder copies a
  `count x 0x40`-byte table.
- `0x02`/`0x03` -> count + a dedicated builder (60-byte source records to
  88-byte runtime records, with variable-size sub-arrays).
- `0x0C`/`0x0D` -> count + a dedicated builder (40-byte runtime records with
  16-byte sub-entries and an 8-byte array).
- `0x15` -> the same builder as `0x01` (a second 0x40-byte table).
- `0x16`/`0x17` -> count + 24-byte table (raw copy).
- `0x1B`..`0x32` -> several counts/offsets and tagged sub-trees.
- `0x34`/`0x35` -> start/end of the **type-tagged tree** (§4).
- `0x36` -> a scalar; `0x37` -> a record array built from per-record offsets;
  `0x38`/`0x39` -> count x 8-byte raw copy.
- `0x3A`/`0x3B` -> count + **36-byte records to 40-byte runtime records**
  (§3).
- `0x3C` -> a scalar; `0x3D` -> a second C string (copied with `strlen`).
- `0x3E` -> a scalar; `0x3F`..`0x57` -> 8-byte groups copied verbatim.
- `0x60`/`0x61` -> two independent 0x40-byte blocks copied verbatim (not
  dereferenced).
- `0x23` -> the 16-byte object-name table (B10).
- `0x26` -> the level's own name, a C string.

There is no loop "for each of the `N` directory blocks". The builder is a struct
walker, not a block walker.

## 2. Type dispatch

Answer: **(d) something else - the header slot.**

- Not size: the same size is consumed by different builders. 36-byte records at
  slot `0x3B` become 40-byte runtime records; 36-byte part records inside the
  room record become 96-byte runtime records.
- Not a field inside the block: no type/tag word exists at a block start. The
  word earlier passes measured is the relocated pointer.
- Not ordinal: the runtime never sees the directory order.

Mechanism: the file is a serialized C struct. The writer recorded, in the
leading directory, the file position of every dword that holds a non-zero
base-relative offset. The reader needs only `N` (to find the aligned data base)
and then adds the base to the header fields. The individual directory offsets
are serialization metadata, not a runtime block index.

Corollary: a "block" is only the span between two consecutive pointer fields.
The corpus figures (4/40/48 bytes) are pointer spacings. That is why four
content hypotheses framed on "blocks" failed: the spans do not delimit typed
records, and a 48-byte span can be padding (B9).

Cross-check: in every sampled unit the directory positions match the header's
own offset slots exactly, including omission of zero-valued slots (B2). Over
the whole corpus every directory entry's value is an in-range offset (B3). Both
are expected under a relocation directory and accidental under a block table.

## 3. The 36 -> 40 conversion

Source array: `count = header[0x3A]`, `offset = header[0x3B]`, stride 0x24.
Destination: `count x 0x28`.

| source offset | size | runtime offset | note |
|---|---|---|---|
| 0x00 | u32 | 0x00 | base-relative offset; passed through the tagged-blob copier; stored as an 8-byte pointer |
| 0x04 | u32 | 0x08 | copied |
| 0x08 | 8 bytes | 0x0C | copied as two dwords |
| 0x10 | u32 | 0x14 | copied |
| 0x14 | u32 | 0x18 | copied |
| 0x18 | u16 | 0x1C | copied |
| 0x1A | u16 | 0x1E | copied |
| 0x1C | u32 | 0x20 | copied |
| 0x20 | u32 | 0x24 | copied |

The extra four bytes are exactly the widening of the first field from a 4-byte
relative offset to an 8-byte pointer. Nothing else grows.

Real cross-check: the table is populated in 7 area units. In 6 of them every
record position is itself a directory entry (because the first field is a
pointer) and the first field resolves to a `0xBE 0xEF` blob. In the seventh
(LAIR10) all three first fields are zero, so the positions are omitted from the
directory and the copier returns null. Stride is 0x24 in all seven.

## 4. The tagged-blob copier (the "name resolver")

Behaviour:

1. Read the 32-bit relative offset at record +0; if zero -> null.
2. At `base + offset`, require bytes `0xBE 0xEF`; otherwise -> null. This is a
   filter, not an error.
3. Count = byte at +2.
4. Walk `count` entries from +4. Each entry starts with a type byte and a length
   byte. Type 0 advances `0x12 + 2 x length`; types 1..4 advance
   `0x0E + 2 x length`. No other types were observed.
5. Allocate `end - start` and copy the blob **verbatim**; return the pointer.

It does not relocate, intern or resolve names. The blob is a variable-length
parameter block of small integers (B8). So the first field of a 36-byte record
is not a name offset, and treating it as a string is wrong.

Names come from elsewhere:

- the level's own name is a NUL-terminated C string at header `0x26`;
- object names are the 16-byte table at header `0x23`, terminated by `0xFF`
  (B10).

## 5. The 48-byte class

**What the builder does with it: nothing.** There is no 48-byte record type in
the level builder and no source stride of 0x30 in its call tree. The source
strides found are 0x24, 0x4C, 0x5C, 0x60, 0x0C, 0x08 and 0x10. 0x30 appears
only as a *runtime* stride in a shared sub-builder.

What the class actually is: a directory spacing. Two observed cases:

1. A four-offset struct (16 bytes) whose first dword is a pointer, followed by
   32 zero bytes before the next pointer. The "48-byte block" is the struct plus
   padding; its trailing bytes are not a record.
2. A 48-strided array in which each element begins with a base-relative pointer
   and carries 44 bytes of payload. The first pointer targets a geometry block.
   Column 0 rises monotonically because the targets are laid out in order; the
   lag-6 u16 period lives in the 44-byte payload.

So the "objects with a link/parent field" reading is **not supported**. The
first dword is a link (a base-relative pointer) - that part is CONFIRMED - but
it is not a parent link, and the record is not a level object. The only
parent/child structure found is the **geometry node tree** built by the room
sub-builder: source nodes carry two child offsets, and runtime nodes are 0x40
bytes with two child pointers (B13). Object hierarchy lives in a separate
48-slot tracker, not in these spans.

## 6. Placement

- The builder copies a **level origin** as three i16/u16 pairs from header
  0x44..0x4F into level +0x7C/+0x80/+0x84 (B11). It also copies two 0x40-byte
  blocks from header 0x60/0x61 verbatim.
- It does **not** compose or apply a transform to geometry or to the 36/40-byte
  records.
- The only positional arithmetic is the name-keyed hardcoded patch code (B12):
  for specific level names it adds or subtracts constants on i16 fields inside
  the per-object record array. That is a per-level fix-up, not a general
  placement rule.
- Per-model geometry does have an i16 -> f32 triple conversion and short/float
  mirrors (B13), but that is vertex data produced by the shared sub-builder, not
  object placement.

Conclusion: **no data-driven placement transform exists in the level builder.**
If placement lives in the area `.PCM` at all, it is carried as copied records
and must be interpreted downstream. This pass found no consumer that turns those
records into world transforms.

## Cross-checks against real units

- 89/89 area units: every directory entry's value is an in-range offset (46,124
  entries).
- 5/5 sampled units: all 24 header offset slots agree with the directory
  (non-zero => listed, zero => omitted).
- 7/7 units with the 36-byte table: stride 0x24. 6/7 have every record position
  in the directory and a `0xBE 0xEF` blob; LAIR10 has zero first fields and the
  positions are omitted.
- Name table: 16-byte stride, `0xFF` terminator, short lowercase object names.

## UNKNOWN (explicit)

1. Meaning of the 36-byte record's other eight dwords. The three mask-like
   dwords at 0x08/0x0C/0x10 and the u16 pair at 0x18/0x1A are copied but not
   interpreted.
2. Meaning of the tagged blob's fields (types 0..4). Only the length rule is
   established.
3. Semantic role of the 48-strided pointer array and of the object it points
   at. It is reached from the room/geometry chain, not from the level builder.
4. Whether any downstream consumer reads level +0x190/+0x198 (the 40-byte record
   array) and turns it into gameplay state. Not traced.
5. Whether the directory's individual offsets are ever read by a non-tag-2 path
   (a tool, or the object container). Only the tag-2 path was audited.
6. Whether the two 0x40-byte header blocks (0x60/0x61) are transforms. They are
   copied verbatim; their consumers were not traced.

## Next probes (ordered by cost)

1. **Consumer of the 40-byte array.** Find the readers of level +0x190/+0x198
   and of the blob pointer; that names the table.
2. **48-array owner.** Follow the second pointer of the room's 36-byte part
   record (the one that points near the 48-run) and identify the sub-builder
   that consumes it.
3. **Directory use outside tag 2.** Whole-binary search for a reader of
   `base + offset[i]`. Expect none; that would close the "relocation map vs
   block table" question for good.

## Consequences for the prior notes

- `docs/notes/pcm-loader.md` L9/L10: the 36 -> 40 copy shape stands. "No
  48-byte block found" is refined: there are 48-byte *spans*, but they are
  directory spacings, not records.
- `docs/notes/pcm-content-census.md`: its size classes are base-invariant but
  semantic-free. The lag-6 period and the monotone column-0 are properties of
  payload *between pointers*, not of a typed 48-byte record.
- `src/level.rs` `BlockClass` is a shape classifier only. It must not be read as
  a type taxonomy.

## Legal

Behaviour notes only. Role names, field offsets and a few short engine
literals. No addresses, no listings, no decompiled bodies, no game bytes, no
absolute install paths. Scratch stayed in `%TEMP%`. No protection or packing was
encountered, so no line was stopped.

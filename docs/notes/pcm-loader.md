# `pcm-loader` - how `sr1.dll` reads `.PCM` level units

Bounded, read-only Ghidra 12.1.4 survey of the SR1 remaster binary
(`1\sr1.dll`, AMD64). Behavior notes only: role names, string text and
field-level structure. No addresses, no disassembly, no decompiled text, no
game bytes, no absolute install paths. Ghidra project and decompiler output
lived in `%TEMP%` only.

## Verdict

**PARTIAL, but the container blocker is gone.** The loader pipeline and the
`.PCM` container base are now CONFIRMED end to end (L13 resolved below). A
reader may now safely implement the container: directory count, directory
offsets and the data base. What is still not established is the meaning of
the fields inside the level header and of the 36-byte source record, so a
reader must still stop at "structure known, semantics UNKNOWN" for those.
No reader may claim field meanings yet.

## Box (what was and was not done)

- Seven headless runs in this pass on the existing scratch project; every
  function was re-pinned by string/xref and the arithmetic was read
  instruction by instruction in scratch only.
- Recon: `.pcm` / `pmf` / `pnf` / `area` / `object` / `load` / `queue` literals
  with every xref resolved to its containing function.
- Walk: area path builder -> area loader -> queue -> drain -> per-tag builder;
  object slot builder -> object completion; tracker limits.
- Data check: offline arithmetic over all 89 SR1 area `.PCM` units
  (read-only; counts and offsets only, no bytes recorded here).
- Not done: dynamic tracing, the object `.PCM` decompressor body, SR2, `.SRL`.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| L1 | `.pcm` format literals exist for four roots: area, area_e3 (level units) and object, object_e3 (object containers). Each is formatted by a single path builder that also emits the `.crm` and `.pnf` siblings | CONFIRMED |
| L2 | The area path builder has three callers. One is the area loader, which walks the level's object-name table and calls the object builder once per name | CONFIRMED |
| L3 | The area `.PCM` request is queued with a stack-passed type word set to `2`. The object `.PCM` request uses type `1`. The `.sra` request uses `0x32`. The type word -> field mapping for the area site is inferred from the object site's layout | CONFIRMED for values; INFERRED for the field binding |
| L4 | The read queue is a 64-entry ring. Each entry stores the type, the output pointer and the completion callback. Resolution goes first to the bigfile index, then to a loose file. Only raw bytes are read; the queue does no parsing | CONFIRMED |
| L5 | The drain runs one common prefix for every type that opts in (the area and object requests both pass the opt-in flag; the generic request does not): it reads the first dword, rounds it **up to the next multiple of 512 dwords (2 KiB)**, shifts the payload down by that many dwords, shrinks the block by the same byte count, then dispatches on the allocation tag. The value is a **dword count**, not a byte count | CONFIRMED (mechanics; corrected by L13) |
| L6 | Tag `1` (object `.PCM`): allocates a 0x6D90-byte buffer, decompresses into it, then runs the object-instance builder. The instance region is 0x6D00 bytes, which equals 8 sub-model records of 0xDA0 each | CONFIRMED for sizes; sub-model count 8 INFERRED from arithmetic |
| L7 | Tag `2` (area `.PCM`): allocates a 0x248-byte level struct and calls the level builder with `(level_struct, base, base)`. The level loader reads count and pointer fields at fixed offsets within that struct | CONFIRMED |
| L8 | Level builder, source to runtime: header words give three tables. A 0x18-stride table; an 8-byte pointer list; and a record array. The record array is the one that matters for the block question (see L9) | CONFIRMED (structure) |
| L9 | **The 40-byte block.** The level builder allocates `count * 0x28` bytes. Each runtime record is built from a **36-byte (0x24) source record**: u32, u32, 8-byte field, u32, u32, u16, u16, u32, u32 (nine dwords). The first u32 is passed through the name resolver (L15) and stored as a pointer | CONFIRMED (copy shape); field semantics UNKNOWN |
| L10 | **The 48-byte class is not a block.** The object tracker's `0x30` value is `MAX_OBJECTS = 48`, the slot count of the tracker, printed in its own format string. No 48-byte record allocation was found in the level or object builders | CONFIRMED (the count); no 48-byte block found |
| L11 | The completion for area `.PCM` sets the level record, runs the level-init pass, then calls the name-table walk and the per-record flag pass. The name table is NUL-terminated, at a 16-byte stride; each record is 0x60 bytes, with its count at the level header | CONFIRMED |
| L12 | Strings with no consumer found: `Load Queue is full or corrupted`, `Can't locate file`, `Can't read from bigfile`, `Can't load big file named` (existing bigfile error notes) | CONFIRMED (by xref) |
| L13 | **Resolved.** There is no wrapper. The first dword is the directory count N. The drain strips `4 * roundup512(N)` bytes, where `roundup512(N) = ((N + 512) & ~511)`. That lands exactly on the `.PCM` data base, which is `4 * roundup512(N)` bytes into the file and zero-padded up to it. The file's first dword therefore sits `roundup512(N)` dwords before the level builder's base, not 4 bytes before it. 89/89 SR1 units agree | CONFIRMED (code + corpus) |
| L14 | The object `.PCM` decompressor (tag-1 path) body and its format are not read | UNKNOWN |
| L15 | The name resolver applied to each 36-byte record's first field: it checks a two-byte magic tag at the offset; records without the tag resolve to null; tagged records are walked (a count byte, then variable entries with a type byte and a length byte) and copied whole into the level's allocation. The copy is what the runtime record points at. The interior meaning of the tagged record is not read | CONFIRMED (behavior); interior semantics UNKNOWN |
| L16 | Meaning of each 36-byte source field. The first field is **not** a plain string offset (it must carry the tag from L15), so a reader that treats it as a name string is wrong. The remaining fields are a copy shape only | UNKNOWN |

### Prior state of L13 (history, not silently rewritten)

The earlier pass recorded L13 as "Conflict, not reconciled": the drain looked
like it treated the first dword as a 512-rounded byte count while the
documented header made it a count N. The reconciliation is that the drain
rounds **N dwords** to a 512-**dword** boundary and uses the result as a byte
offset. The "512-byte" wording was the error; the first dword was N all along.

## L13 resolution (the three numbers)

`.PCM` layout, as read by the engine:

```
[ u32 N ][ N x u32 ascending offsets ][ zero padding ][ level data ]
                                      \__ up to 4 * roundup512(N) __/
```

- The directory is `N + 1` dwords.
- The gap from `4 + 4N` to the data base is **zero in 89/89** units.
- The data base is `4 * roundup512(N)`, where
  `roundup512(N) = ((N + 512) & ~511)` (strictly greater than N; for N a
  multiple of 512 the code would jump one extra 512-dword block - INFERRED
  from the formula, no corpus case has such an N).
- Directory offsets and every level-header offset are relative to that data
  base. Record 0 (offset 0) is the level header itself.
- At the data base, the level header's name field points at the unit's own
  name in 89/89 units; that is the independent check that fixes the base.

Three numbers side by side for real units (bytes):

| Unit | file size | `4 + 4N` (old base) | drain skip | data base | builder base |
|---|---|---|---|---|---|
| `LAIR5` (N=62) | 9,136 | 252 | 512 dwords = 2,048 | 2,048 | file + 2,048 |
| `CATHY44` (N=326) | 35,264 | 1,308 | 512 dwords = 2,048 | 2,048 | file + 2,048 |
| `LAIR1` (N=1655) | 200,156 | 6,624 | 2,048 dwords = 8,192 | 8,192 | file + 8,192 |
| `MOVIE3` (N=2256) | 274,084 | 9,028 | 2,560 dwords = 10,240 | 10,240 | file + 10,240 |

The drain's value equals **neither** the file size nor `4 + 4N`. It equals the
padded directory length, and the base moves with N (2 KiB, 3 KiB, 4 KiB, 5 KiB
in dword terms). It is not a fixed header and it is not a wrapper.

**Answer to the blocker:** for one real area `.PCM` the drain hands the level
builder a pointer to the file's data base, `4 * roundup512(N)` bytes in. The
file's first dword (N) is `roundup512(N)` dwords before that base - 512 dwords
= 2,048 bytes for N < 512 - and the buffer starts at the raw file's first byte
(no wrapper).

## Correction to the data-side note

`docs/notes/area-levels.md` states "the data base is `4 + 4N`; record i starts
at `base + offset[i]`". The offset table itself is right (89/89), but the base
is wrong: it is `4 * roundup512(N)`. Its 89/89 check could not catch this
because both candidate bases keep the offsets ascending and in bounds. Any
record position derived from that note is short by
`4 * roundup512(N) - 4 - 4N` bytes (1,796 bytes for `LAIR5`, 740 for
`CATHY44`, up to several KiB for large units). The correction is recorded in
that note as a dated block; its history is left intact.

## Pipeline (behavior)

```
area / object name
  -> path builder (area | area_e3 | object | object_e3, pcm/crm/pnf/sra)
  -> bigfile index lookup, else loose file   (64-slot queue, raw bytes)
  -> drain: first-dword rule, 512-dword round, payload shift
       tag 1 : object .PCM -> 0x6D90 decompressed instance (8 x 0xDA0 sub-models)
       tag 2 : area .PCM   -> 0x248 level struct -> level builder at data base
                              -> 0x18 table, 8-byte pointer list,
                                 36-byte source -> 40-byte runtime records
       tag 0x32: .sra      -> generic resource-table builder (see sra-animation)
  -> completion callback (object: slot fill; area: level init + name walk)
```

## Consequences for the format notes

- The 4-byte stride finding stands. The 12-byte refutation stands. The
  36-byte source record is 9 dwords and is consistent with the 4-byte stride.
- `fixed48 = fixed40 + 1` census correlations should not be read as "a 48-byte
  block". Count fields and tracker limits explain the 48. The 40-byte class
  has a candidate source (the 36-byte record), but this is a copy shape, not a
  proven file block.
- `docs/notes/area-levels.md` and `docs/notes/room-geometry.md` remain the
  data-side authorities for the census and geometry, with the L13 base
  correction above. This note adds the code side for the level record only.

## Next probes (ordered by cost)

1. **Field semantics (L16).** With the base settled, label the level-header
   words and the 36-byte record against the block notes. Do not label before
   the tagged record (L15) is read further.
2. **Tagged-record body (L15 follow-up).** The resolver proves the first field
   points at a tagged record; the entry kinds are still unknown.
3. **Object `.PCM` decompressor (L14).** Only if object containers are in
   scope; otherwise defer.

## Legal

Behavior notes only. Role names, string text and field-level structure. No
addresses, no listings, no decompiled bodies, no game bytes, no absolute
install paths. Scratch files stayed in `%TEMP%`. No protection or packing
was encountered, so no line was stopped.

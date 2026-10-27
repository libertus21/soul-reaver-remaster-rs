# `pcm-loader` - how `sr1.dll` reads `.PCM` level units

Bounded, read-only Ghidra 12.1.4 survey of the SR1 remaster binary
(`1\sr1.dll`, AMD64). Behavior notes only: role names, string text and
field-level structure. No addresses, no disassembly, no decompiled text, no
game bytes, no absolute install paths. Ghidra project and decompiler output
lived in `%TEMP%` only.

## Verdict

**PARTIAL.** The loader pipeline is documented end to end with evidence. The
record layout of the level unit is documented only for the runtime side. The
on-disk field semantics and the decompressor for the object `.PCM` are not
established, so **no reader is safe to implement yet**. Next probes at the end.

## Box (what was and was not done)

- Six headless runs plus one scratch project, about two passes of work.
- Recon: `.pcm` / `pmf` / `pnf` / `area` / `object` / `load` / `queue` literals
  with every xref resolved to its containing function.
- Walk: area path builder -> area loader -> queue -> drain -> per-tag builder;
  object slot builder -> object completion; tracker limits.
- Not done: dynamic tracing, the object `.PCM` decompressor body, the name
  resolver, SR2, `.SRL`.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| L1 | `.pcm` format literals exist for four roots: area, area_e3 (level units) and object, object_e3 (object containers). Each is formatted by a single path builder that also emits the `.crm` and `.pnf` siblings | CONFIRMED |
| L2 | The area path builder has three callers. One is the area loader, which walks the level's object-name table and calls the object builder once per name | CONFIRMED |
| L3 | The area `.PCM` request is queued with a stack-passed type word set to `2`. The object `.PCM` request uses type `1`. The `.sra` request uses `0x32`. The type word -> field mapping for the area site is inferred from the object site's layout | CONFIRMED for values; INFERRED for the field binding |
| L4 | The read queue is a 64-entry ring. Each entry stores the type, the output pointer and the completion callback. Resolution goes first to the bigfile index, then to a loose file. Only raw bytes are read; the queue does no parsing | CONFIRMED |
| L5 | The drain runs one common prefix for every type: it reads the first dword, rounds it up to a 512-byte multiple, shifts the payload, then dispatches on the allocation tag | CONFIRMED (mechanics) |
| L6 | Tag `1` (object `.PCM`): allocates a 0x6D90-byte buffer, decompresses into it, then runs the object-instance builder. The instance region is 0x6D00 bytes, which equals 8 sub-model records of 0xDA0 each | CONFIRMED for sizes; sub-model count 8 INFERRED from arithmetic |
| L7 | Tag `2` (area `.PCM`): allocates a 0x248-byte level struct and calls the level builder. The level loader reads count and pointer fields at fixed offsets within that struct | CONFIRMED |
| L8 | Level builder, source to runtime: header words give three tables. A 0x18-stride table; an 8-byte pointer list; and a record array. The record array is the one that matters for the block question (see L9) | CONFIRMED (structure) |
| L9 | **The 40-byte block.** The level builder allocates `count * 0x28` bytes. Each runtime record is built from a **36-byte (0x24) source record**. Source fields: a dword name offset resolved through a name resolver to a pointer; a dword; an 8-byte field; two dwords; two words; two dwords; and a trailing dword. The 36-byte source stride is 9 dwords, so it is consistent with the corpus-wide 4-byte stride | CONFIRMED (copy shape); field semantics UNKNOWN |
| L10 | **The 48-byte class is not a block.** The object tracker's `0x30` value is `MAX_OBJECTS = 48`, the slot count of the tracker, printed in its own format string. No 48-byte record allocation was found in the level or object builders | CONFIRMED (the count); no 48-byte block found |
| L11 | The completion for area `.PCM` sets the level record, runs the level-init pass, then calls the name-table walk and the per-record flag pass. The name table is NUL-terminated, at a 16-byte stride; each record is 0x60 bytes, with its count at the level header | CONFIRMED |
| L12 | Strings with no consumer found: `Load Queue is full or corrupted`, `Can't locate file`, `Can't read from bigfile`, `Can't load big file named` (existing bigfile error notes) | CONFIRMED (by xref) |
| L13 | **Conflict, not reconciled.** The drain treats the first dword of the buffer as a byte count (rounded to 512). The established `.PCM` header (count N, then N ascending offsets) treats it as N. The two cannot both describe the raw file without a wrapper | UNRESOLVED |
| L14 | The object `.PCM` decompressor (tag-1 path) body and its format are not read | UNKNOWN |
| L15 | The name resolver applied to each source record's name offset: its inputs and output are not read | UNKNOWN |
| L16 | Meaning of each 36-byte source field, and what makes a record a 40-byte "block" versus any other class in the file census | UNKNOWN |

## Pipeline (behavior)

```
area / object name
  -> path builder (area | area_e3 | object | object_e3, pcm/crm/pnf/sra)
  -> bigfile index lookup, else loose file   (64-slot queue, raw bytes)
  -> drain: first-dword rule, 512-byte round, payload shift
       tag 1 : object .PCM -> 0x6D90 decompressed instance (8 x 0xDA0 sub-models)
       tag 2 : area .PCM   -> 0x248 level struct -> level builder
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
  data-side authorities for the census and geometry. This note adds the code
  side for the level record only.

## Next probes (ordered by cost)

1. **Resolve L13 first.** Dynamic or static check: does the buffer handed to
   the drain start with the `.PCM` header (N, then offsets), or with a wrapper
   whose first dword is a size? One real area `.PCM` answers it.
2. **Name resolver (L15).** Read what the lookup does with the source name
   offset. It fixes the record's first field and links the 36-byte record to
   the name table already documented in L11.
3. **Object `.PCM` decompressor (L14).** Only if object containers are in
   scope; otherwise defer.
4. **Field semantics (L16).** Once 1 and 2 are settled, label the 36-byte
   fields against the existing block notes. Do not label them before that.

## Legal

Behavior notes only. Role names, string text and field-level structure. No
addresses, no listings, no decompiled bodies, no game bytes, no absolute
install paths. Scratch files stayed in `%TEMP%`. No protection or packing
was encountered, so no line was stopped.

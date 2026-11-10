# slot-descriptor-parse - the per-slot descriptor array is the model's 24-byte
# table, and its aim data has no established record length (2026-11-09)

Read-only parse slice. The archived object-container payload already carried a
located, byte-verified per-slot record block; this slice makes the locator
generic, fixes the record length at 24 bytes, parses the records, exposes the
per-slot aim reference, and answers the three open questions. Nothing was
launched, attached or instrumented for this slice. No engine behaviour changed,
no golden moved, and no game bytes, addresses, file offsets or install paths are
recorded here.

Evidence classes per claim: **CONFIRMED** (observed bytes with a falsifiable
shape), **PARTLY** (structure observed, one side open), **UNKNOWN** (not
established).

> Update (2026-11-09): `docs/notes/aim-stream-bounds.md` delimits the aim
> sub-slices (24-byte header + `count * 20`-byte frames), decodes the 16-byte
> per-slot track header, and pins the Q12 fixed-point unit. The aim-length
> UNKNOWN below is superseded by that note.

## Headline

The block is the model's **24-byte bone/descriptor table**: each record carries
the same 24 bytes in the file form and is expanded to 32 bytes in memory, where
the loader relocates the base-relative aim offset at `+20` into an absolute
pointer. The array count equals the model bone count by construction.

The stated `0xFFFF` gate at `+6`/`+10` is **not** an all-records invariant. On a
live raziel dump only the first three slots carry `0xFFFF`; the remaining 63
records are active and carry small values there. The array is located
generically from the object container's own model header (the declared
bone-table base), with the `0xFFFF` run kept as the headerless signature path.

| Question | Verdict | Class |
|---|---|---|
| Where is the array, generically? | The model header's declared bone-table base (no hardcoded offset). The `0xFFFF` run is a fallback for header-less payloads. | CONFIRMED |
| Does the `bone_count` gate hold? | Yes, when the array is located by the model header. The pure `0xFFFF` signature covers only inactive-slot objects. | CONFIRMED |
| What is the file record length? | 24 bytes (memory form 32 bytes). | CONFIRMED |
| What is the per-slot aim record length? | Not established; recorded as UNKNOWN. | UNKNOWN |
| What do the i16 values mean? | Small signed fixed-point components consistent with a tri-axial aim, not angles and not a key list. | PARTLY |

## A. Generic locate - agreement rate

The corpus scan covered **394 valid object containers** (entries with a
non-zero bone count) out of 1325 archive entries. Two locators were measured:

| Locator | Exact `count == bone_count` | Agreement |
|---|---|---|
| Pure `0xFFFF` run at `+6`/`+10` | 201 / 394 | **51.0%** |
| Model-header bone-table anchor | 394 / 394 | **100%** |

The pure signature is a real but partial gate: it finds objects whose slots are
all inactive (`+6 == +10 == 0xFFFF`). A single active slot breaks the run, which
is why the live raziel array (three inactive slots, 63 active) is not found by
that gate alone. The header anchor returns the same base the model header
declares for the bone table, so it is generic, not a hardcoded position.

The three records with a non-zero aim offset on raziel are slots 1, 31 and 41;
the parsed count is 66.

## B. Record length of the per-slot aim data - UNKNOWN

The three aim offsets point into one contiguous stream of 16-bit signed values.
Their separations are **64 bytes** and **44 bytes**, so the data is **not** a
fixed-stride array. The stream was checked for the three candidate length
markers:

| Candidate | Observation | Verdict |
|---|---|---|
| Length/count field before the record | the words immediately before each offset are ordinary small values, not a length | refuted |
| Count byte at the record start | the first bytes are ordinary signed components (`0x14`, `0xE6`, `0xFA`), not plausible counts | refuted |
| `0xFFFF` / `0` sentinel terminator | zeros occur inside a record and the values continue after them; no `0xFFFF` terminator | refuted |
| Fixed stride | the two separations differ (64 and 44) | refuted |

`aim_record` therefore returns the raw slice bounded by the next known aim
offset (or the payload end) with an explicit `AimRecord::UnknownLength` marker.
It never invents a length.

## C. i16 semantics - tri-axial aim components, not angles, not a key list

The first eight i16 of each referenced run are:

| Slot | First eight i16 | Range |
|---|---|---|
| 1 | 20, 36, 45, -33, 14, 42, 0, 30 | -33..45 |
| 31 | -26, 0, 14, -35, 39, 0, 0, -22 | -35..39 |
| 41 | -6, -9, 44, 0, -17, 17, 25, 53 | -17..53 |

Observations:

- Overall first-eight range: **-35..53**. The surrounding contiguous stream
  stays in the same small signed band in the sampled window (about -50..225).
- Every value is a 16-bit signed word with `0x00`/`0xFF` sign-extension bytes;
  there is no per-axis single scalar and no key/index pairing.
- Taking the leading three words as one vector gives plausible direction
  components: `(20, 36, 45)`, `(-26, 0, 14)`, `(-6, -9, 44)`.

Verdict: the values read as **tri-axial fixed-point aim components** (a vector
stream), not as Euler angles and not as a key list. No scale or denominator
word is present in the referenced data, so the fixed-point unit stays UNKNOWN.
No rotation is composed in this slice.

## D. Implementation

`src/slot_descriptor.rs` (pure, no I/O, not wired into the viewer):

- `SlotDescriptor` - raw 24-byte fields; only `aim_offset` carries a spec name.
- `locate_descriptor_array(payload, bone_count)` - model-header anchor first,
  then the headerless `0xFFFF` signature run.
- `parse_descriptor_array(payload, array_base, bone_count)` - raw record parse;
  active slot words do not invalidate the block.
- `aim_record(payload, descriptors, array_base, slot)` - bounded raw slice with
  `AimRecord::Absent` / `AimRecord::UnknownLength`.
- 11 unit tests: exact 24-byte layout, `0xFFFF` signature locate, header anchor,
  bone-count gate, offset-0 path, aim bounding, out-of-range offsets and
  truncated/malformed payloads. Synthetic fixtures only; no game bytes.

`src/main.rs` registers the module behind `#[allow(dead_code)]`; it is not wired
into any CLI arm, so headless digests are unchanged.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The array is the model's 24-byte table | a live dump that does not match the archive block at the model's bone-table base | Tested: the raziel dump matches 63/66 records, differing only in the three relocated aim offsets |
| The `0xFFFF` signature is a universal locator | an array with an active slot that breaks the `0xFFFF` run | Refuted: active slots carry small values; the pure gate covers 51.0% |
| The aim record length is fixed | two equal separations, or a length/count prefix | Refuted: separations are 64 and 44; no prefix or terminator |
| The i16 values are angles | one scalar per axis with an angular scale | Not observed; the leading words form 3-component vectors |
| The i16 values are a key list | an explicit key/index pairing or a count of keys | Not observed; no key structure in the referenced data |

## Limits

- The header anchor uses the object container's model header; a payload that is
  not an object container must use the `0xFFFF` fallback.
- The pure `0xFFFF` signature is not a universal gate; its 51.0% coverage is a
  property of the corpus, not a parser failure.
- The per-slot aim record length stays unknown; only the first eight words were
  sampled at each offset.
- The fixed-point unit and the meaning of the words after the leading vector are
  not established.
- This slice parses file-form records only; it does not decode the object
  container, relocate pointers, or compose rotations.

## Legal

Read-only. No game process was launched or instrumented. No game file was
written, no game bytes were copied into the repository, and no disassembly,
address, file offset or install path appears here. Only structure, field roles
and format constants are recorded.

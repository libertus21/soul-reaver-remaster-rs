# producer-selector - the three producers are located and their dispatch is a
# runtime channel/track record, not a per-slot bit in the 24-byte bone table
# (2026-11-12)

Read-only static pass on the shipped SR1 module (Ghidra 12.1.4) plus a bounded
offline differential over the shipped object containers. The question was the
per-slot producer selector: which field decides whether a slot's angle triple
comes from the packed channel, the quaternion track, or the aim/mode path. The
short answer: **the three producers and their dispatch are found; the selector
is a runtime animation-channel/track record, not a field of the 24-byte bone
record. Its file-side placement is not established, so the per-slot mapping is
still UNKNOWN for the four body models.**

Nothing was launched, attached or instrumented. No engine code changed, no
golden moved, `cargo test` stays 618 green.

Evidence classes: **CONFIRMED** (instruction-level, or two independent
observations agree), **INFERRED** (one side observed, the other reasoned),
**UNKNOWN** (not established).

## Verdict

**PARTIAL.**

- **FOUND:** the three producer entry points and the internal dispatch that
  chooses between them, including the exact switching field in the runtime
  records. The packed-channel decoder is located, its bit-plane/literal framing
  is decoded, and the quaternion-track sampler is separated from it.
- **FOUND (negative, strong):** the 24-byte file bone record carries **no**
  producer selector. Of its twelve words, only `+20` separates any two slot
  classes; `+0`, `+2`, `+4` and `+22` are constant zero on all four models.
- **FOUND (negative, strong):** file-resident **rotation** tracks are
  effectively absent from the corpus and **zero** on the four body models, so
  the quaternion producer cannot be sourced from file data for those bodies.
- **UNKNOWN:** the file offset at which the channel/track records live. The
  runtime shape is not present in the shipped containers or the `.SRA` files in
  any scanned form. The per-slot census therefore gains a class (not
  quaternion) but cannot be split into packed versus mode-applied.

## Quick path

1. Read "The dispatch, by role" for the three producers and the switch.
2. Read "The switching fields" for the exact offsets and values.
3. Read "Evidence: file-side separation" before reusing any bone-record field.
4. Read "Updated census" for what changes and what does not.
5. Read "The next probe" and "Falsification ledger" before planning follow-up.

## The dispatch, by role

The sampler is per object, once per frame, inside the per-object pose pass
(unchanged from `docs/notes/angle-array-source.md`). This slice resolves the
internal branch that was open there:

```text
per-object pose builder
  -> animation sampler (per object)
       -> channel loop (N channel records, 0x50 stride)
            keyed frame table present?
               yes -> keyed frame-table sampler
                        interpolate 20-float frames, write QUATERNION form
               no  -> packed-channel decoder
                        3 bit planes, 3 bits per frame per axis,
                        palette index or literal, blend, write ANGLE form
       -> node angle triple (channels + window blend) -> staging records
  -> per-slot descriptor evaluator
       -> staging record kind flag
            angle form      -> forward Euler constructor (3 angles)
            quaternion form -> quaternion-to-matrix
       -> track entry tagged with the slot? -> mode applier (rotation /
            translation / matrix / identity, per its mode byte)
```

Two consequences matter:

- The producer choice is **per channel**, and a channel covers a **contiguous
  range of slots** (a first-slot index plus a slot count), not one slot.
- A **track entry** can override an individual slot after the channel pass; its
  own mode byte decides what it writes. This is the mode applier.

The aim path is object-level, not per-slot: the object-root constructor reads
the object's live aim words, and the attachment path reads an attachment
channel. Neither is selected by a bone-record field.

## The switching fields

All offsets below are **memory-form record offsets**, observed in the shipped
module's control flow. They are named because a decoder that runs against a
relocated animation record must test them. They are **not** file-form offsets.

### Channel record (0x50-byte stride, contiguous array)

| Field | Offset | Role | Class |
|---|---:|---|---|
| sampled marker | `+0`, bit 7 | set after the channel is sampled this frame; also consumed/masked by the slot builder | CONFIRMED |
| first slot | `+2` | first staging/pose slot index covered by this channel | CONFIRMED |
| slot count | `+3` | number of contiguous slots the channel covers | CONFIRMED |
| frame time | `+4`, `+6` | current and previous frame position | CONFIRMED |
| packed table | `+0x38` | frame-table header for the packed decoder (divisor at `+6` of the header) | CONFIRMED |
| **keyed table** | `+0x48` | frame-table header for the keyed sampler; the intermediate pointer's `+0x10` must also be non-null | **CONFIRMED — this is the producer switch** |
| blend window | `+0x3c`, `+0x3e` | two frame positions used by the window blend | CONFIRMED |

The switch is exactly:

```text
producer(channel) =
    KEYED_QUATERNION   if channel.keyed_table != 0
                       and channel.keyed_table.frame_array != 0
    PACKED_CHANNEL     otherwise
```

and the two writers are distinct routines: the keyed branch writes four floats
and sets the staging kind flag; the packed branch writes three angles and leaves
the flag clear.

### Track entry (0x40-byte stride, pool, chained by the node's `+2` word)

| Field | Offset | Role | Class |
|---|---:|---|---|
| next entry | `+0` | chain link (index into the same pool) | CONFIRMED |
| mode byte | `+2` | selects the apply semantic; observed values {1, 8, 0x0A, 0x0E, 0x10, 0x12, 0x16, 0x20, 0x22, 0x26, 0x4C} | CONFIRMED |
| target slot | `+3` | the slot this entry overrides; the chain lookup compares it to the slot being built | CONFIRMED |
| flags | `+0x10` | bit 0x4000 = interpolate, bit 0x2000 = direct, bit 0x8000 = consumed/disabled | CONFIRMED |
| fraction | `+0x14` | i16 fraction, `/4096` | CONFIRMED |
| base triple | `+0x20`, `+0x24`, `+0x28` | authored value | CONFIRMED |
| delta triple | `+0x2c`, `+0x30`, `+0x34` | interpolation delta | CONFIRMED |
| quaternion | `+0x30`.. | four floats for the matrix modes; overlaps the delta triple, and the mode byte decides which reading applies | CONFIRMED |

Mode values map to: rotation triple (set / add / matrix-multiply), translation
triple (set / add), identity, and an indirect callback. The mode applier is the
only path that can write a translation or a matrix into the staging record.

### Node header (the record the slot builder evaluates)

| Field | Offset | Role | Class |
|---|---:|---|---|
| channel count | `+0` | number of channel records; each 0x50 bytes, array starts at `+0x38` | CONFIRMED |
| track head | `+2` | index of the first track entry; `0` = no per-slot track overrides | CONFIRMED |
| trimmed angle triple | `+8`, `+0xc`, `+0x10` | three floats copied into the staging record; zeroed when the flag test fails | CONFIRMED |
| flags / channel base | `+0x38` | byte tested as `(value & 0x88) == 0x80` (use own triple) and `value & 4` (negate), then cleared with `& 0x7f`. In the sampler the same offset is the **base of the channel array**, so this byte is plausibly the first channel's format byte. Which reading is intended is UNKNOWN. | PARTLY |
| processed | `+0x14`, bit 0 | cleared after the build | CONFIRMED |

### Staging record (0x30-byte stride) and the bone record

| Field | Offset | Role | Class |
|---|---:|---|---|
| angles | `+0`, `+4`, `+8` | angle form: three angles (the forward constructor's input) | CONFIRMED |
| quaternion | `+0`..`+0xc` | quaternion form: four floats | CONFIRMED |
| scale | `+0x10`, `+0x14`, `+0x18` | Q12 scale, identity is 4096.0 | CONFIRMED |
| translation | `+0x20`, `+0x24`, `+0x28` | translation triple | CONFIRMED |
| **kind flag** | `+0x2c` | 0 = angle form, non-zero = quaternion form; this is the final angle-vs-quaternion switch | **CONFIRMED** |

The bone record keeps its documented file layout (24 bytes; `+12`/`+14`/`+16`
translation, `+18` parent, `+20` aim offset). In memory it is 32 bytes, and the
packed decoder reads the translation triple at `+0xc`/`+0xe`/`+0x10` and the
parent at `+0x12` — the same fields, so the bytes the consumers read map 1:1 and
the record is padded to 32 (**INFERRED** for the padding boundary).

### Why the switch is a runtime record and not file data

The channel array and the track-entry pool are addressed through relocated
pointers and allocated from a runtime pool; the shipped image's staging region
is zero and the loader relocates the records in place
(`docs/notes/animation-loader-trace.md`). The record shapes name the data a
decoder must consume, not a file offset to read.

## Evidence: file-side separation

The differential was run offline against the four census models, comparing
**the only two slot classes that file data can separate**: slots with a
non-zero `+20` aim offset (the aim class) and slots with `+20 == 0`. Values are
`u16` in the 24-byte file record; "distinct" counts distinct values.

raziel (66 slots, 3 aim):

| Offset | aim distinct | non-aim distinct | Separates? |
|---:|---:|---:|---|
| `+0` | 1 (`0`) | 1 (`0`) | no — identical |
| `+2` | 1 (`0`) | 1 (`0`) | no — identical |
| `+4` | 1 (`0`) | 1 (`0`) | no — identical |
| `+6` | 3 | 22 | no — overlapping |
| `+8` | 3 | 56 | no — overlapping |
| `+10` | 3 | 57 | no — overlapping |
| `+12` | 3 | 35 | no — overlapping |
| `+14` | 2 | 30 | no — overlapping |
| `+16` | 2 | 30 | no — overlapping |
| `+18` | 3 | 38 | no — overlapping |
| `+20` | 3 | 1 (`0`) | **yes — disjoint** |
| `+22` | 1 (`0`) | 1 (`0`) | no — identical |

skinner (35, 4 aim), sluagh (24, 5 aim) and morlock (37, 4 aim) reproduce the
same result: every offset overlaps or is identical **except `+20`**. The
never-before-measured `+0`/`+2`/`+4` and `+22` are constant zero on all four
models, so there is no hidden mode byte in the bone record.

Corpus facts used by the verdict:

| Check | Result | Class |
|---|---|---|
| Containers scanned for the file track-descriptor shape | 1325 entries | CONFIRMED |
| Containers with a valid **rotation** track (10-byte Q12 quaternion frames) | **2** (`font`, one unnamed 13-bone model) | CONFIRMED |
| Rotation tracks on raziel / skinner / sluagh / morlock | **0 / 0 / 0 / 0** | CONFIRMED |
| Containers with a **position** track candidate (32-byte frames) | raziel 10, skinner 1, sluagh 0, morlock 0, glphicon 1, `font` 1 | INFERRED (shape gate is loose) |
| Channel array shape (0x38 header + N×0x50 records) in body containers | 0 hits | CONFIRMED negative (bounded) |
| Same channel shape in 287 loose `.SRA`/`.SRL` files | 0 hits | CONFIRMED negative (bounded) |
| Runtime descriptor dump records a track-table pointer for raziel | no (existing dump) | CONFIRMED |

Two independent observations therefore agree that the quaternion producer is
not file-sourced for the body models: no rotation track in the container, and
no track table in the runtime dump.

## The confirmed rule

Stated so a decoder can implement it (against a relocated animation record, not
against the file):

```text
# per channel, covering slots [c.first .. c.first + c.count)
kind(c) = QUATERNION_TRACK   if c.keyed_table and c.keyed_table.frame_array
          PACKED_CHANNEL     otherwise

# after the channel pass, per slot
if track_entry t exists with t.target == slot:        # node.track_head chain
    apply t.mode (rotation / translation / identity / matrix)
    # the final form is whatever the writer leaves at staging+0x2c;
    # only the keyed sampler was observed to set it non-zero
else:
    staging_kind = c.kind produced for that slot

# final construction
staging[slot].kind_flag == 0 -> Euler forward constructor(staging.angles)
staging[slot].kind_flag != 0 -> quaternion-to-matrix(staging.quaternion)

# object-level, not per slot
object root            -> AIM (live aim words)
attachment present     -> AIM/attachment channel
```

Equivalent statement of the selector: **`channel+0x48` (keyed frame table) is
the packed-vs-quaternion switch; `staging+0x2c` is the angle-vs-quaternion
switch; `track_entry+2` is the mode-applier semantic; `track_entry+3` is the
slot binding.**

## Updated census

The only change the evidence supports is to **exclude the quaternion track** for
the four bodies and to merge the previously UNCLASSIFIED slots into the
staging route. They are **not** relabelled PACKED, because the channel payload
is still not readable from file data; "staging route" means "packed channel or
channel mode applier", both of which write the staging record.

| Actor | Slots | AIM_MODE | STAGING (packed/mode) | QUATERNION_TRACK | UNCLASSIFIED |
|---|---:|---:|---:|---:|---:|
| raziel (player) | 66 | 4 | 62 | 0 | 0 |
| skinner | 35 | 5 | 30 | 0 | 0 |
| sluagh | 24 | 6 | 18 | 0 | 0 |
| **total (three)** | **125** | **15** | **110** | **0** | **0** |
| morlock (extra) | 37 | 5 | 32 | 0 | 0 |

What changed versus `docs/notes/slot-producer-census.md`:

- `QUATERNION_TRACK = 0` is now **evidence-backed for these four models**
  (two independent negatives), not merely unassigned.
- The 110 are no longer "UNCLASSIFIED"; they are **STAGING**, a named
  superclass whose two concrete writers are located.
- `PACKED_CHANNEL` is still **not** assigned per slot: the per-channel
  first-slot/count records are not readable from the file, so "which channel
  covers slot N" remains UNKNOWN.
- The aim class is unchanged (15 including the object-root joint).

## The next probe (closes the per-slot split)

**Runtime: one bounded hook on the channel loop, plus one on the staging kind
flag.** At the sampler entry for each object, for the first N objects and one
frame each, record per channel: `first_slot`, `slot_count`, `keyed_table != 0`,
and whether any track entry targets a slot in its range; and per staging record
the kind flag. Emit one aggregate line per object (no per-call text, hard line
cap), then join with the existing descriptor dump by slot index.

Expected signal:

- If every raziel channel reports `keyed_table == 0` and every staging kind flag
  is 0, then all 62 staging slots are **PACKED_CHANNEL** and the census becomes
  `4 AIM + 62 PACKED` with a per-slot channel map.
- Any channel with a non-zero keyed table splits its covered slots into
  `QUATERNION_TRACK`; the channel's `first_slot`/`slot_count` gives the exact
  slot range, which is the mapping the file side cannot supply today.
- A staging flag of 1 with no keyed channel would falsify the "kind flag follows
  the producer" rule.

**File-side complement (cheaper, independent):** the model header points at a
16-byte-stride table (present on raziel, `glphicon` and `font`). It is not the
bone table and has not been decoded. Decode its entry fields and test whether
its entry count/offsets reproduce the per-channel `first_slot`/`slot_count`
ranges. That would tie the runtime channel geometry back to file data.

## UNKNOWN

1. The file offset of the channel array and the track-entry pool. Neither the
   container data nor the `.SRA` files contain the runtime shape under a bounded
   signature scan.
2. Which channel covers each slot in the file, and therefore each slot's
   `first_slot`/`slot_count` membership, without a runtime capture.
3. Whether the packed channel is populated per slot (the decoder is located, but
   the payload is not readable without the animation-record carrier).
4. Whether the `+0x38` byte tested by the slot builder is a node flag or the first
   channel's format byte.
5. The exact semantics of that byte's bits outside 0x80 and 0x04.
6. The mode-byte set is observed, not enumerated exhaustively; values outside
   the observed set are UNKNOWN.
7. Coverage: one shipped build, four body models, no runtime log this slice.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The producer choice is not a per-slot field of the 24-byte bone record | an aim slot and a non-aim slot differing in a located field other than `+20` | Tested on four models: only `+20` separates; `+0/+2/+4/+22` constant zero |
| `channel+0x48` selects keyed versus packed | a channel with a non-null keyed table running the packed decoder, or vice versa | Holds in the decompiled branch and in both writers |
| `staging+0x2c` selects angle versus quaternion form | a slot constructed by the Euler path while the flag is set, or the quaternion path while clear | Holds at the constructor dispatch |
| The quaternion producer is not file-sourced for the four bodies | a rotation track descriptor in raziel/skinner/sluagh/morlock, or a raziel track-table pointer | Not observed: 0 rotation tracks; dump has no pointer |
| The channel array is not stored in the runtime shape | an N×0x50 run with ascending first-slot fields in a body container or `.SRA` | Not found in 1325 containers or 287 `.SRA`/`.SRL` files |
| The 110 staging slots are all packed | a channel with a non-null keyed table in a body model | Not tested at runtime; this is the next probe |
| The tested file track-descriptor shape (`frame offset`, `frame count`, `kind`) is a rotation/position selector | a validated 10-byte Q12-quaternion frame run under `kind != 1` | Holds on the two models where it fires; the bodies never fire |

## Method and legal

- Static pass on the shipped SR1 module with headless Ghidra from a scratch
  project outside the repository and the game directory; the decompiled routines
  were re-verified against the shipped image bytes at the probed sites.
- Offline differential over the shipped object containers, read-only, using the
  repository's documented 24-byte bone-record layout and model-header anchor.
  Scratch probes stayed in the session temp directory and are not part of this
  branch.
- No game process was launched, attached or instrumented by this slice. No game
  file was written and no game byte is reproduced here.
- This note records field offsets, role names, value sets, counts and
  control-flow shapes only. It contains no addresses, module-relative numbers,
  disassembly listings, raw game bytes or absolute install paths.

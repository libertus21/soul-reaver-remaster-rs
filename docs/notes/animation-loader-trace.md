# animation-loader-trace - the per-slot descriptor array and track table are
# relocated out of the archived object container (2026-11-09)

Static, read-only Ghidra trace of the shipped SR1 remaster module. The question
was the last open route to body animation: **where does the per-slot animation
descriptor array physically come from, and is there any authored per-slot
rotation to recover?** This note answers both, records the data-source verdict
as the headline, and keeps the byte-exact carrier as an explicit UNKNOWN where
the control flow does not settle it. Nothing was launched, attached or
instrumented; no production code changed and no golden moved.

Evidence classes per claim: **CONFIRMED** (observed control flow or bytes with a
falsifiable shape), **PARTLY** (structure observed, one side still open),
**UNKNOWN** (not established). Functions are named by role only.

## Headline - the data source

The per-slot descriptor array and the track table are **serialized,
base-relative-pointer records carried inside the archived object container**
(the `.PCM`-style object resource), decompressed and **relocated in place by the
object loader before anything reads them**. They are *not* generated
procedurally from the bone table, *not* a compiled table in the module image,
and *not* the `.SRA` or `.SRM` families.

| Candidate the task asked to test | Verdict |
|---|---|
| A field in the archived object container we had not mapped | **YES - this is the carrier kind.** The records are relocatable sub-records of the object-container model definition, reached through a pointer array indexed by animation id. |
| A global/static table in the module (`.rdata`/`.data`) | **NO.** The record region is zero in the pristine image and populated only at load. |
| Generated procedurally from the bone table | **NO.** The descriptor array and track table are stored records with internal pointers, not synthesized. The pose *math* is procedural, but the *data* is not. |
| An asset family we have not identified | **NO new extension.** It is the object container, loaded by the object loader path. |

The exact bigfile entry and the byte-level placement of the records inside it
stay UNKNOWN (see "What stays UNKNOWN"); the *kind* of carrier is settled.

## Verdict table - the five questions

| # | Question | Answer | Class | Evidence kind |
|---|---|---|---|---|
| 1 | The allocator/filler of the descriptor array, sized by bone count | There is **no runtime malloc sized by bone count**. The array is a serialized block whose element count is the record's own per-slot count (equal to the model bone count at runtime, e.g. 66). The "fill" is the loader's decompress-plus-relocate pass, not a `bone_count x 32` allocation loop. | CONFIRMED | Pose-builder read shape + relocator write shape; contradicts a malloc model |
| 2 | Who calls it, and with what arguments, in the load/activation path | The **object-container loader drain** decompresses the object resource and calls the **object-container relocator** with the loaded buffer base. The relocated model definition is then dispatched by class index to the actor/model builder, which loads the `.SRM` sub-models and publishes the animation table the pose builder walks. The individual store that installs the animation table into the actor is reached through a class dispatch and was not pinned instruction-by-instruction. | PARTLY | Drain + relocator + actor-builder control flow; one indirect dispatch open |
| 3 | THE DATA SOURCE - what pointer/table/asset the filler reads | **Archived object container data.** The descriptor array (32-byte stride, count = per-slot count) and the track table (16-byte entries, destination pointer + frame count + divisor) are internal, base-relative records of the object-container model definition; the relocator rebases them and every nested pointer. Not `.SRA`, `.SRM`, `.SRL`, a compiled module table, or procedural. | CONFIRMED for the carrier kind; PARTLY for the exact entry/placement | Relocator fields match the consumer fields exactly; the `.SRA` and `.SRM` paths do not produce them |
| 4 | The track table the pose builder consumes | The **track selector** reads the table from the animation record and, per track, copies a 4-float frame from the frame array into the destination head cell. The same table is one of the structures the relocator rebases (destination pointers). Source: the object container. | CONFIRMED | Track-selector + relocator control flow |
| 5 | Whether authored per-slot rotation exists at all | **No authored per-slot rotation for the body.** Descriptor mode bits select procedural aim/face orientation: a per-slot authored **fixed-point aim vector** (three signed shorts) plus the object transform and a world target produce the rotation at runtime. There is no per-slot rotation matrix in the data. The authored 4-float **track frames are empty/unused for the observed body models** (runtime saw the track selector but emitted no frames). Authored track frames exist only for classes whose tables are populated. | CONFIRMED negative for body rotation; PARTLY for which classes carry frames | Descriptor/aim math + runtime absence of track-frame emissions |

## Data flow (roles only)

```text
object resource request (object-container loader tag)
  -> loader drain
     -> decompress the object-container model definition
     -> object-container relocator  (rebases every internal base-relative pointer)
        -> pointer array of animation records, indexed by animation id
           -> per record:
              - per-slot count  (equals model bone count at runtime)
              - descriptor array (32-byte stride; signed-16 active marker,
                mode bits, three-short fixed-point aim vector, nested pointers)
              - track table (16-byte entries: destination frame pointer,
                frame count, divisor)
     -> class-index dispatch
        -> actor/model builder
           -> loads `.SRM` sub-models (shaders, materials, HD matrices,
              128 rest translations, 128 activation flags)
           -> publishes the animation table the pose builder indexes
  -> pose builder (reads descriptor array -> 13-float per-slot pose)
  -> track selector (reads track table -> copies 4-float frames)
  -> palette assembler (13-float pose + `.SRM` rest/flags -> 3x4 matrix palette)
```

The relocator's structure and the consumer's structure are the **same record**:
the pointer array is indexed by animation id exactly as the pose builder indexes
it; the per-slot count drives a 32-byte-stride array exactly as the pose builder
walks descriptors; and the track table (count, entries after a small header,
16-byte stride, destination pointer) is exactly what the track selector copies
from. That three-way match across relocator, pose builder and track selector is
what identifies the carrier rather than coincidence.

### Why the `.SRA` and `.SRM` paths are not the carrier

- The object-container loader tag routes to the relocator that rebases the
  descriptor array and track table. The `.SRA` tag routes to a **different**
  generic tagged-resource builder that copies nested tagged blobs and does not
  produce a 32-byte descriptor array or a track table. The descriptor/track
  relocation is specific to the object-container path.
- The `.SRM` sub-model loader fills only the sub-model record: shader count and
  records, material count, extra HD-matrix records, the 128 rest translations and
  the 128 activation flags. It never writes the descriptor array or the track
  table.
- The `.SRM` activation flags (one per 128 joint slot) are a *different*
  structure from the descriptor's per-slot active marker (a signed 16-bit value
  with -1 meaning off). The palette assembler reads the `.SRM` flags; the pose
  builder reads the descriptor markers. They are two distinct gates.

## Correction to the earlier `.PCM` negative

`docs/notes/pcm-animation-record.md` reported a CONFIRMED negative: no
`bone_count x 32` descriptor array in the object container. That scan's gate was
a **0/1 marker column whose set/clear pattern matched the vertex-bearing
("ranged") bones**, over a flat contiguous window. The shape this trace actually
finds fails three of that gate's premises:

1. the per-slot active marker is a **signed 16-bit value** (-1 means off), not a
   0/1 byte - so a 0/1 column test cannot match it;
2. the block is **pointer-indirected** (reached through the model definition's
   animation-record pointer array), not a flat window that a byte walker expects
   to see as `bone_count` contiguous records at one offset;
3. the block is **base-relative and relocated in place**, so its stored pointers
   are offsets, not the absolute pointers a "descriptor array" heuristic expects.

That does not prove the block is on every model, and it does not re-derive the
byte-exact placement. It does mean the earlier negative does not refute a
pointer-relocated, signed-16-marker descriptor array, and the honest status is
PARTLY rather than a clean NO. The decisive re-test is listed under Next probes.

## What this means for the project

**Body animation is not a hidden data problem - it is a procedural-orientation
problem, and the descriptor/track data lives in the object container.** The
consequences:

- **No authored per-slot rotation exists to extract for the body.** The pose is
  computed from the descriptor's mode bits, a per-slot authored fixed-point aim
  vector, and the actor's own transform against a world target. Recovering "the
  body clip" as a rotation table is not possible because no such table exists in
  the body path.
- **A file-only replay of the procedural pose is reachable in principle:** parse
  the object container, decompress it, apply the same in-place relocation, then
  evaluate the pose builder's aim/face math. The blocker is not the descriptor
  array anymore; it is the **object-container decompression** and the exact
  sub-record placement, both still UNKNOWN.
- **The observable body motion over time is the object transform (root motion
  and facing), not a skin clip.** The runtime capture shows the animated slots
  changing with the object translation and yaw while the track selector emits no
  frames. So "body animation" as seen in play is root/actor motion plus
  procedural limb orientation, not a per-bone keyframe clip.
- **Authored 4-float track frames do exist for track-bearing classes** (the
  facial/HD class whose tables are populated), and those live in the same object
  container structure. If the project ever needs those, the same
  parse-relocate-replay pipeline applies.

**If exact runtime poses must be reproduced without a decoder**, a capture would
have to record, per object per sample: the object transform (position and
facing), the selected animation id, and the per-slot pose (or, equivalently, the
descriptor mode and aim vector plus the transform) - which is what the existing
runtime probe already reads. A capture is *sufficient* but no longer the only
route, because the input data is file-resident.

## What stays UNKNOWN

1. **The exact bigfile entry** that stores the animation records and their
   byte-level placement inside it. The carrier *kind* is settled (object
   container, relocatable records); the specific entry and on-disk offsets are
   not.
2. **Whether the relocated records belong to the character's own mesh container
   or to a named dependency object.** The loader resolves tagged name references
   to other loaded objects, so the animation records could live in a separate
   archived object that the model definition names.
3. **The object-container decompression format** (the path allocates a fixed
   instance buffer and decompresses into it; the codec is not decoded here).
4. **Descriptor bytes beyond what the consumers read:** the mode bits, the
   signed-16 active marker and the three-short fixed-point aim vector are
   established; the remaining descriptor fields and the nested pointers are not
   decoded.
5. **Track-frame semantics** (quaternion, position, scale or packed) and the
   exact clock unit / divisor meaning.
6. **Which models carry populated track tables.** The observed body models did
   not emit track frames; a per-class census was not run.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The descriptor array and track table are relocated from the object container | the relocator's descriptor/track fields are a different structure, or the object-container path never touches them | Tested by structure match; no contrary path found |
| The carrier is not procedural and not a compiled module table | the pose builder reads a computed buffer, or the pristine image already holds the records non-zero | Tested: pristine region is zero; relocator populates it |
| The carrier is not `.SRA` | the `.SRA` builder produces a 32-byte descriptor array or a track table | Tested negative: `.SRA` builder is a generic tagged-resource copier |
| The carrier is not `.SRM` | the `.SRM` loader writes the descriptor array or track table | Tested negative: it writes only shaders/materials/HD/rest/flags |
| Body pose is procedural orientation, not an authored rotation clip | a populated body track table whose frames rotate slots, or a per-slot rotation matrix in the descriptor | Tested: descriptor is mode+aim; observed body track tables empty |
| The earlier `.PCM` "no descriptor array" negative stands | a pointer-relocated, signed-16-marker, count-equal array exists in the object container | Not re-scanned at byte level; gate mismatch identified |

## Next probes (ordered by cost)

1. **Re-scan the object container with the correct gate.** Look for a
   pointer-indirected array of `slotCount` 32-byte records whose active column is
   a signed 16-bit value (not 0/1 marker) and whose nested fields are base-relative
   offsets; confirm on a body model and a track-bearing model. This closes or
   re-confirms the placement UNKNOWN.
2. **Pin the class-dispatch handler** that installs the animation table into the
   actor, to close question 2's one indirect step.
3. **Identify whether the records are in the mesh container or a named
   dependency**, by following the tagged name resolution from the model
   definition.
4. **Decode the object-container decompression** only if a file-only pose replay
   is pursued; otherwise leave it documented as the remaining blocker.

## Method and legal

- Static pass on the shipped SR1 module with headless Ghidra, read-only. The
  module was opened once and analyzed with default auto-analysis; the analysis
  project lived outside the repository and the game directory.
- Role identities reused from the existing authority notes; all conclusions are
  control-flow or structure observations, distinguished from inference inline.
- No game process was launched, attached or instrumented by this slice. No
  production code changed and no golden moved.
- This note records role names, structural shapes, field roles and format
  constants only: no addresses, no byte offsets, no disassembly listings, no raw
  game bytes, no absolute install paths.

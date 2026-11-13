# angle-array-source - the per-slot angle array is a per-frame staging cache, fed by serialized channels, plus a live root (2026-11-12)

Read-only static pass on the shipped SR1 module with Ghidra 12.1.4. The only new
inputs are the two runtime backtraces recorded before this slice; everything below
is derived from the module's control flow, data references and shipped image. No
game process was launched, attached or instrumented here. No engine code changed.

The question was where the per-slot angle array comes from and who writes it. The
short answer: **there is no file-resident angle array.** In the two observed
paths the constructors read two different things. The forward-order constructor
reads a per-frame **staging record** that the animation sampler rebuilds for each
object before the slot builder consumes it. The reverse-order constructor reads
the object's **live aim words**, converted to floats in place. The staging record's *values* are unpacked
from serialized per-slot channels, so the provenance splits: content is
file-derived, assembly is per-frame, and the root is live state.

## Quick path

1. Read "The decisive answer" - FILE-BACKED, COMPUTED or MIXED, and why.
2. Read "Verdict table" for the per-question result and its evidence kind.
3. Read "The chain, by role" for the dispatcher, the per-slot caller and the
   writer, in order.
4. Read "What the code actually reads" for the named inputs.
5. Read "The single next experiment" and "UNKNOWN" before planning follow-up.

## Verdict table

Evidence class: **CONFIRMED** (instruction-level or two independent static
observations agree), **PARTLY** (one side pinned, the other inferred), **UNKNOWN**.

| # | Question | Answer | Class | Evidence kind |
|---|---|---|---|---|
| 1 | Is the angle array file-backed or computed? | **MIXED.** The array is a per-frame staging cache, never a loaded table. Its per-slot content is unpacked from serialized channels in the object-container animation record; its root content is live object state. | CONFIRMED for the cache and the cadence; PARTLY for the per-slot content split | Write/read references, shipped-image bytes in the staging region, sampler control flow |
| 2 | What is the common dispatcher? | The **per-object pose-update pass**. It walks the object list, sizes and double-buffers each object's pose storage from the per-frame arena, and invokes the per-object router. It does **not** iterate slots itself; the slot loops live one level down in the builders. | CONFIRMED | Decompiled control flow of the pass and its router; both runtime chains pass through the same call |
| 3 | Immediate caller of the reverse constructor? | The **attachment-root builder**. It converts the object's three signed 16-bit aim words to floats in the same frame, then passes a pointer to those floats. The source is **not** `base + slot*stride` into a static array. | CONFIRMED | Instruction-level: three integer-to-float conversions immediately before the call; pointer to the object field |
| 4 | Who writes the array, and when? | The **animation sampler** and its per-node decoders, for each object, once per frame, in the same per-frame subtree and before the slot builder reads the record. Not at load; not at spawn. | CONFIRMED for role and cadence; PARTLY for which decoder drives each slot | Call graph from the per-frame pass; sampler precedes the slot build in the same call tree |
| 5 | Immediate caller of the forward constructor, and the "same page" scratch? | A **thin adapter** invoked by the per-slot descriptor evaluator. It copies the staging record's three values to a **stack temporary** and calls the constructor. The underlying data is the global per-slot staging area; the 0.99 same-page signal is the adapter's stack frame. | CONFIRMED caller and source; PARTLY for the page-fraction mapping | Instruction-level copy to the stack and call; the adapter has no other pointer source |

## The chain, by role

Roles only; the same order as the two runtime backtraces.

```text
game-loop step
  -> per-frame simulation step                 advances the frame clock, walks active units
     -> per-object pose-update pass            DISPATCHER
        -> per-object update router            picks a builder, sizes pose storage, allocates
           -> attachment-root builder          REVERSE constructor: live aim words
              -> slot-build driver              samples animation, then builds the slot chain
                 -> animation sampler           WRITER of the staging record
                    -> quaternion-track sampler  slerps a 4-float keyframe pair when present
                    -> packed-channel decoder    unpacks per-slot per-axis values and interpolates
                    -> mode applier              applies the descriptor mode
                 -> per-slot descriptor evaluator
                    -> thin adapter             stack copy of three values
                       -> forward constructor    angles -> 3x3 block
           -> main per-slot builder             root from aim or proxy; descriptor modes
              -> per-slot aim evaluator         mode that derives angles from a world target
```

Both runtime chains share the dispatcher and the per-object router, then separate:
one enters the attachment-root builder, the other continues through the
slot-build driver into the per-slot descriptor evaluator. That is why the
backtrace saw one constructor "in module data" (the object's own aim words) and
the other "in anonymous memory" (the adapter's stack copy of the staging record).

The writer sits *inside* the same per-frame subtree as the reader, not in a loader
or a stream: the slot-build driver begins by sampling the animation, then walks
the slots. There is no separate upstream producer to chase.

## The decisive answer: MIXED

Terminology, because the three labels are easy to blur:

- **File-backed**: the values exist in the shipped data and can be recovered by
  reading files alone.
- **Computed**: the values are produced by code from live state and cannot be read
  from data alone.
- **MIXED**: one half each, with the split stated explicitly.

### Why it is not FILE-BACKED as an array

The staging region is a **single global base indexed by slot**. In the shipped
image the whole region is zero. Every reference to it is a runtime reader or a
runtime writer; no load-path or spawn-path function references it. It is refilled
per object, per frame, and consumed immediately by that same object's slot build.
So there is no serialized angle array to load: the array is scratch.

### Why it is not purely COMPUTED

The per-slot content is not invented from gameplay state. The sampler unpacks
**per-slot, per-axis channels** carried by the object-container animation record.
The shipped loader trace already places that record, its per-slot descriptors and
its track table in the object container, relocated in place at load. The channel
payload is compressed, not a float array:

- three bit planes, one per axis, select which axes carry a value for the slot;
- each value is either a small palette index into a per-node float palette or an
  inline literal;
- adjacent frames are blended with a frame fraction over the angle difference,
  wrapping at a half turn;
- a per-slot fixed-point triple from the descriptor supplies translation, and the
  scale triple is a fixed-point identity.

So the per-slot angle values are present in the data, in a packed form, and are
recovered by arithmetic rather than observed as finished floats.

### Why it is MIXED, precisely

| Half | Where it comes from | Loadable offline? |
|---|---|---|
| Per-slot orientation (forward constructor) | Unpacked and interpolated from serialized per-slot channels in the object-container animation record | In principle yes, but only through the engine's packing, palette, frame-divisor, wrap and interpolation semantics |
| Root and attachment orientation (reverse constructor) | Live object aim words, and one mode computes them from the object position versus a world target | No: this half is live state, not file data |

That is the decisive answer: **you cannot load the array, but the per-slot half is
reconstructible from the data, and the root half is not.**

## What the code actually reads

| Input | Consumer | Role | Cadence |
|---|---|---|---|
| Three signed 16-bit aim words on the object | Attachment-root builder; main builder root | Converted to float in place and passed to the reverse constructor | Per frame per object |
| Packed per-node channel (three axis planes, palette indices or literals) | Packed-channel decoder -> staging record | Per-slot angle triple, interpolated between adjacent frames | Per frame per object |
| Four-float keyframe pair in a track table | Quaternion-track sampler -> staging record | Slerped quaternion when the table is populated | Per frame per object |
| Descriptor mode and its float triple | Mode applier / per-slot evaluator | Selects the content: channel composition, a mode-table triple, or a target-derived aim | Per frame per slot |
| Object position versus a world target | Per-slot aim evaluator | Yaw and pitch for the aim/face modes | Per frame per active slot |
| Per-slot fixed-point descriptor triple | Channel decoder / evaluator | Translation and a mode marker | Per frame per slot |

Two independent constants tie this together. The matrix-to-angle encoder scales
its output by `4096 / (2*pi)`, the same 4096 units-per-turn the constructor
consumes; and the staging record's identity scale is the Q12 value `4096.0`. The
angle triple therefore round-trips: composed matrix -> 4096-unit angles -> the
constructor's `Rz(c)*Ry(b)*Rx(a)` matrix. That round-trip is why the array looks
like an authored angle table at runtime while being a derived cache.

## The staging record

One record per slot, at a fixed stride from the global base. The record has two
orientation forms, selected by a representation flag:

- **Angle form**: three 4096-unit values, the constructor's direct input.
- **Quaternion form**: four values, written by the slerp path.

Plus a fixed-point scale triple, a translation triple, and small markers. The
angle form is the one the observed forward-constructor path reads; the quaternion
form is a different producer writing the same record for a different slot or
track kind. The representation flag is what makes the record polymorphic, and it
is why a slot cannot be classified without reading its descriptor/annotation bits.

## Offline decodability

If the goal is offline replay, the path is now concrete but not free:

1. Decode the object-container animation record: per-node packed bit planes, the
   per-node float palette, the frame count and divisor.
2. Reimplement the unpack and the frame blend, including the half-turn wrap for
   angle deltas.
3. Apply the descriptor mode: channel composition, mode-table triple, or the
   aim/target computation.
4. Reproduce the matrix-to-angle encoding, then feed the existing rotation model.

Steps 1-2 are the work the loader trace already listed as open (frame-table
semantics). Step 3 is the per-slot mode census. Step 4 is already solved and
implemented in the engine.

## The single next experiment

**A per-slot producer census.** For one actor, decode each slot's descriptor and
annotation bits and classify which producer fills its staging record: packed
channel, quaternion track, or aim/target. Confirm the classification with one
runtime log at the thin adapter: record the representation flag and the descriptor
mode for every forward-constructor call over a short walking capture, and join
the two.

This is the highest-value test because the MIXED verdict is exact at the role
level and open only at the per-slot level. One capture plus one static decode
converts "some slots are file-derived and some are live" into "these slots are
file-derived, these are live", which is exactly what offline replay needs.

## UNKNOWN

1. **Which producer filled each observed slot.** The capture recorded the caller
   chain, not the descriptor mode or the representation flag.
2. **Whether the body's packed angle channel is populated.** The loader trace
   observed empty 4-float track frames; the packed angle channel is a distinct
   track kind and was not censused. This is the decisive open item for offline
   decode.
3. **Frame-table semantics**: divisor, frame wrap, per-axis bit-plane layout and
   palette ordering. Listed open by the loader trace; needed for step 1 above.
4. **Scope of the representation flag**: per slot, per node, or per track, and
   whether it can change between frames.
5. **Origin of the mode-table float triple**: engine constant or load-derived from
   the object's animation data. The table is written through an indirect
   registration path, not a direct store, so its content source was not resolved.
6. **Per-object reuse of the staging base.** The base is global and indexed by
   slot; the pass is single-threaded, but the exact interleaving between two
   objects in one frame was not traced.
7. **Coverage**: one build, one actor, the slots the watch and the backtrace
   observed.
8. **The page-fraction mapping.** The 0.99 same-page figure is explained by the
   adapter's stack temporary; that mapping is static inference, not a measured
   page identity.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The staging region is a runtime cache | a load-path or spawn-path writer, or non-zero content in the shipped image | Holds: region is zero at rest; only runtime readers and writers reference it |
| The staging base is global, not per-object | a base reached through the object pointer | Holds: fixed module base plus a per-slot stride |
| The reverse constructor's source is `base + slot*stride` | a slot index in the pointer computation | Refuted: the pointer is an object field filled from the aim words immediately before the call |
| The forward constructor's source is a per-object field | the adapter passing an object pointer | Refuted: the adapter copies three values to its own stack frame |
| The dispatcher iterates slots | a slot loop inside the pass | Refuted: it walks objects; slot loops are in the builders |
| The array is written at load or spawn | a reference from a loader | Refuted: the sampler hangs off the per-frame pose subtree |
| The per-slot angles are pure aim/target | the observed forward path reading the aim words | Refuted: the aim path writes the pose slot directly; the observed forward path reads the staging record |
| The per-slot content is unavoidable runtime invention | a serialized per-slot channel with recoverable values | Refuted: the packed decoder unpacks an authored channel |
| The 336/336 rotation model is unaffected | a different encoding at the constructor | Holds: the encoder uses the same 4096-unit turn |

## What this note does not claim

- It does not claim the body animation is fully decoded; it names the producer
  family and the open per-slot split.
- It does not re-open the descriptor byte layout or the frame-table format; those
  stay where the loader trace left them.
- It does not identify any address, module-relative number, field offset or
  routine name. Routines appear by role only.
- It does not change engine behaviour, production code or any golden.

## Method and legal

Static pass only. The shipped module was opened once in headless Ghidra from a
temporary project outside the repository and outside the game directory. Each
runtime backtrace address was resolved to its containing function, decompiled,
and followed upward through callers and downward to the data it reads and writes.
The staging region was classified by enumerating every reference to it and by
reading its shipped-image bytes; the shipped bytes were not copied here.

No game process was launched or instrumented by this slice. No game file was
written and no game byte is reproduced. This note records roles, control-flow
shape, format constants and counts only: no addresses, no module-relative
numbers, no field offsets, no disassembly listings and no absolute install paths.

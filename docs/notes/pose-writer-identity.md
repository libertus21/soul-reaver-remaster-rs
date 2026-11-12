# pose-writer-identity - the two write-watch sites are the pose builder's own
# orientation constructors (2026-11-12)

Bounded, read-only static pass on the shipped SR1 module with Ghidra 12.1.4.
The starting facts are only the two writer sites reported by the live
`MemoryAccessMonitor` write-watch on the pose buffer the earlier probes called
`arg1`; everything else here is derived by static cross-reference. Nothing was
launched, attached or instrumented. No engine behaviour changed and no golden
moved. Roles, paraphrases and counts only: no addresses, no module-relative
numbers, no field offsets, no disassembly text and no absolute paths.

Evidence class per claim: **CONFIRMED** (instruction-level observation, or two
independent static observations agree), **PARTLY KNOWN** (structure pinned on one
side, the other inferred), **UNKNOWN** (explicitly not established).

## Headline

1. Both writers are **two sibling orientation constructors**. Each writes a 3x3
   rotation block (nine floats) into a caller-supplied pose slot, computed as an
   identity followed by three in-place axis rotations of three scalar angles.
   They are **producers of orientation, not copies**: the write path contains
   trigonometry and arithmetic, not a source pointer plus a length.
2. Both constructors are called from the **per-object pose builders**, and the
   destination is the object's **own per-frame pose array**. The watch therefore
   caught the builders **writing their own output**, not an upstream producer.
   That is the third option in the task: the pool identification was wrong.
3. The builder entry reads **only the object pointer**. The register the earlier
   probes sampled as a "second argument" is a **dead register** at the hook
   point, most plausibly carrying a pointer into the same per-frame pose arena.
   The earlier reading "the builder receives an already-animated pose" is
   consequently **not supported by the code**. The procedural-orientation account
   survives, and the producer it left unnamed is now named.

Verdict on the task's decisive question: **OUTPUT**, produced by a **producer**
routine. The two statements are not in conflict - the routine computes, and the
buffer it computes into is the builder's own output.

## Quick path

1. Read "Verdict table" - the two roles and the decisive classification.
2. Read "The two writers by role" - what each routine computes.
3. Read "Why the pool identification was wrong" - the register story.
4. Read "Callers and phase" and "What feeds the writers".
5. Read "Chain state" - what is closed and what remains.
6. Read "UNKNOWN list" and "Falsification ledger" before quoting anything.

## Verdict table

| # | Question | Answer | Class | Evidence kind |
|---|---|---|---|---|
| 1 | What is the first writer site? | An interior store inside its **own small routine**: the **orientation constructor, reverse composition order** (identity, then the three axis rotations applied third angle first). It writes **one slot's nine-float rotation block**, not a whole array and not a scalar. | CONFIRMED | Containing-function resolution + decompiled body + assembly of the store |
| 2 | What is the second writer site? | Interior store inside a **different but sibling routine**: the **orientation constructor, forward composition order**. Same inputs, same output size, opposite rotation order. It is **not** another instruction of the first routine. | CONFIRMED | Two distinct containing functions; bodies differ only in call order |
| 3 | Producer, copy, or the builder's own output? | The **routine is a producer** (identity init plus three trigonometric axis rotations). The **watch event is an output write**: the destination is the object's own pose array, written by the pose builder. | CONFIRMED for producer; CONFIRMED for output destination of the main builder path; PARTLY for which builder produced each individual event |
| 4 | Who calls them, from which phase? | The **per-object pose builders** (main and alternate), from the **per-object pose-update pass** that runs inside the **main frame update**. One path reaches them through the **per-slot descriptor evaluator**. This is the per-frame actor path, not the loader and not a stream. | CONFIRMED | Call graph: constructors -> builders -> pose-update pass -> main frame update |
| 5 | What feeds them? | Three **scalar angles** per call, from one of: the object's own aim triple, a per-slot descriptor triple interpreted as Euler angles, or two sampled animation-channel frames. A fourth path takes a shared-transform proxy (nine fixed-point words plus three integer translations) and does not use these constructors. | CONFIRMED for the three angle sources; PARTLY for which one drove the watched slots | Decompiled builders and descriptor evaluator |
| 6 | Does this close the chain? | **Yes for naming the pose producer and its data source.** No separate upstream writer of the pose buffer exists. What remains open is file-side descriptor layout and frame semantics for offline replay, plus one bounded question about the dead register's exact runtime value. | CONFIRMED for the producer; remaining items listed as UNKNOWN | Combination of 1-5 |

## The two writers by role

Both routines have the same contract: take a pointer to **three scalar angles**
and a pointer to a **pose slot**, return the slot pointer. Both write **the same
nine floats** (the rotation block of one slot) and nothing else; the remaining
slot components are written by the callers.

### Writer A - orientation constructor, reverse composition order

- Starts by writing the 3x3 identity into the slot's rotation block.
- Applies three in-place plane rotations in the order **third angle, second
  angle, first angle**. Each step is the standard rotation form
  `row_i' = row_i * c - row_j * s`, `row_j' = row_j * c + row_i * s` over a
  disjoint pair of matrix rows, so the result is a product of the three axis
  rotations composed in that order.
- Returns the slot pointer.

### Writer B - orientation constructor, forward composition order

- Identical prologue (identity) and identical per-axis rotation step.
- Applies the three rotations in the **opposite order** (first, second, third).
- Therefore A and B produce the two Euler composition orders of the same three
  angles - the classic pair of "world" and "local" (or row-vector and
  column-vector) conventions.

Both call the same three single-axis helpers. The helpers are the three distinct
axes: one rotates the first and third matrix rows, one the first and second, one
the second and third. Each is a no-op at angle zero.

### Why this is not a copy

- The write path contains no source pointer, no count and no loop over
  elements: the destination is the only memory pointer, and every written value
  is derived from the three angle scalars through products and sums.
- The two routines differ only in the order of three calls, which a copy routine
  could not, and a copy routine would not write an identity before anything
  else.
- The stores the watch caught are the identity-initialisation stores that
  *begin* a slot build, which is exactly what a rebuilt slot writes first.

## The decisive classification: OUTPUT

The task's three options must be applied at two different levels, and both
answers are true at their own level:

- **Routine level: PRODUCER.** The writer computes a rotation from angles. It is
  not a `memcpy`-like link and does not read another pose buffer.
- **Watch-event level: OUTPUT.** The buffer being written is the object's own
  pose array, reached through the object pointer. The pose buffer the watch
  armed is that same per-object array (see next section), not an upstream
  "animated input".

So the correct summary is: **the watch caught the pose builder producing its own
output**, and the pool identification inherited from the earlier content probe was
wrong. There is no evidence of an upstream animation producer writing a separate
input pose that the builder later reads.

### Why the pool identification was wrong

Three independent static observations combine:

1. **The hooked builder takes one argument.** At the entry of the main pose
   builder the second and third integer argument registers are overwritten from
   memory before any operand use that could carry a pointer; instruction-level
   review of the whole prologue shows no read of either, and the one call made
   before the overwrite takes a single integer and reaches a fixed scratch
   array. The alternate builder decompiles with a single object parameter and
   likewise never consumes them. A register the routine never reads is not an
   argument.
2. **The pose array is reached through the object.** Both builders read the
   object's pose-array pointer field and index it by 13-float slot stride; the
   main builder's slot loop walks the per-slot descriptor array and writes each
   slot in place. That array is the builder's output.
3. **The array lives in a per-frame bump arena.** The allocation used for it is a
   monotonic cursor over a fixed arena with a hard limit, advanced by one slot
   stride per requested slot; the arena cursor is reset at the start of each
   frame. The watched addresses sit in the module's writeable data region and
   move by whole allocation sizes, which is the fingerprint of this arena, not
   of a persistent input buffer. This also explains the small set of rotating
   base addresses and the changing content between calls that the earlier probe
   read as "animation of an input".

The register therefore carries a leftover pointer into the same arena: a
double-buffer copy in the pose-update pass leaves its source pointer (the
previous frame's pose array) in the second argument register, and the per-slot
loop leaves slot and scratch pointers in the same registers before the tail call
into the builder. Any of those values is pose-shaped and 13-float-slot sized -
which is exactly what the earlier probes measured.

The verdict does not depend on which of those leftover pointers it was: whether
the dead register held the current array, the previous array or a slot inside
either, the address is the builder's own pose storage, so the watch event remains
an output write. Which leftover pointer it was is a bounded UNKNOWN.

## Callers and phase

The call chain, by role:

```text
main frame update
  -> per-object pose-update pass  (walks the object list; runs every frame)
     -> per-object pose builder, main path   OR   alternate path
        -> per-slot descriptor evaluator        (descriptor modes)
           -> orientation constructor A or B    (the two write-watch sites)
        -> orientation constructor A or B       (object aim / attachment frames)
     -> child objects: same pass, recursive
```

- The **main frame update** is the ordinary per-frame game update: it advances
  frame counters, runs the unit list and the object pose pass. It is not the
  loader path and not the stream path.
- The **pose-update pass** walks objects, decides per object whether to rebuild
  the pose, rotates the pose storage in the per-frame arena, and then invokes a
  builder for each object whose update callback says it is active.
- The **main builder** walks the animation record's per-slot descriptor array and
  builds each active slot in place. Its per-slot path chooses between the
  descriptor-driven route and a shared-transform proxy route.
- The **alternate builder** handles the attachment/root case: it builds the
  object's root orientation from the object's own aim triple, or combines two
  sampled animation-channel frames, and then propagates a translation delta down
  the slot list.

Both constructors are shared helpers: they are also reached from the per-slot
descriptor evaluator (two descriptor modes select the two composition orders) and
from a few unrelated transform call sites. A writer event therefore identifies
the **store instruction**, not the caller; the caller attribution for a single
event is an inference from the destination, not an observation.

## What feeds the writers

Per call, the constructors take three scalar angles. Those angles come from one
of three sources, all of them object-local or descriptor-local:

| Source | Role | Cadence |
|---|---|---|
| Object aim triple | Three signed 16-bit angle words on the object, converted to floats by the builder before the call | Per frame per object |
| Per-slot descriptor triple | The descriptor's three-scalar field, interpreted as Euler angles when the descriptor mode selects an orientation mode | Per active slot per frame |
| Sampled channel frames | Two animation-channel samples (seven floats each) converted by two constructor calls and combined | Per frame when an attachment channel is present |

The **shared-transform proxy** path is the fourth way a slot's rotation is
produced; it does not use these constructors. It copies twelve floats from a
3x4 transform whose rotation part is stored as nine signed 16-bit fixed-point
words (one part in 4096) and whose translation part is three integers, converting
as it stores.

Four further facts about the input side:

- The per-slot descriptor carries an active marker; inactive slots are skipped
  entirely before the constructors are reached.
- The per-slot descriptor's low mode bits select between the descriptor route and
  the proxy route; the descriptor route forwards to the per-slot evaluator, which
  dispatches on a mode byte.
- The three-scalar descriptor field is **mode-dependent**: orientation modes read
  it as Euler angles, while other modes read the same field as a translation
  component. The field is shared; only the mode decides how it is consumed.
- The descriptor array and its optional frame table are the records already
  identified as living in the **object container** and relocated in place at load
  (`docs/notes/animation-loader-trace.md`). This note adds no new carrier.

## Chain state

| Link | State |
|---|---|
| The watched stores -> the routines that perform them | **Closed.** Two named roles, both producers. |
| Those routines -> their callers | **Closed.** Both pose builders, both reached from the per-object pose-update pass in the main frame update. |
| The pose builders -> the pose buffer the watch armed | **Closed for role.** The buffer is the object's own per-frame pose array; the dead-register provenance is a bounded open item that does not change the verdict. |
| The pose builders -> their inputs | **Closed at role level.** Object aim triple, per-slot descriptor triple, sampled channel frames, shared-transform proxy. |
| The inputs -> file-resident data | **Closed by the existing loader trace** for the descriptor array and frame table (object container, relocated in place). Per-descriptor byte layout and frame semantics remain open there, not here. |

**Links remaining before exact offline replay of a body pose: none on the
"who produces it" question.** What remains is the file-side work already listed
in the loader trace (descriptor bytes beyond mode and aim, frame table semantics,
object-container decompression) and the optional register-identity check below.

## Correction to the earlier runtime-capture readings

This slice changes the interpretation of the merged `arg1` plans, not their
measurements:

- The content probe's numbers stand: it did read a pose-shaped, changing region.
- Its **role assignment** does not. The builder does not consume a pose
  argument; the measured pointer was a dead register aliasing the builder's own
  pose storage. "The builder receives an already-animated pose" is refuted by
  the builder's signature.
- The practical consequence is a **simplification**: there is no upstream
  animation producer to chase. The body pose is produced by the per-object pose
  builders from the object transform, the object aim triple and the per-slot
  authored descriptors.
- The procedural-orientation conclusion is unchanged and now has a concrete
  production chain behind it.

## If a copy had been found

Not applicable: the write path does not copy. For the record, had the watch
resolved to a copy, the next watch target would have been the copy's source
pointer; here the analogous open item is narrower: confirm which arena address
the dead register carried. That is answered by a value log, not a write-watch,
because the question is about a register value, not about a second buffer.

## Next probes (ordered by cost, all read-only)

1. **Register provenance (cheap, closes the bounded UNKNOWN).** At the pose
   builder entry, log the dead second/third argument registers and the object's
   pose-array pointer field, and report equality or containment. If the register
   is inside the same arena block as the pose-array field, the alias story is
   confirmed at runtime; either result leaves the producer verdict intact.
2. **Per-slot mode census (static, no runtime).** Count which descriptor modes
   occur in raziel's animation record and which of the four production paths each
   slot takes. This turns "the builders produce the pose" into "these slots come
   from these sources" without a new capture.
3. **Frame-table semantics (static, file-side).** Decode the sampled channel
   record shape already used by the attachment path; this is the same open item
   the loader trace lists, now with a concrete consumer.
4. **Do not re-arm a write-watch on the pose arena** to answer "who writes it".
   The answer is known: the pose builders do, and the arena is reset every frame.

## UNKNOWN list

1. **Which leftover arena pointer the dead register held** at each watched event:
   the current array, the previous array, a slot inside either, another object's
   array in the same arena, or scratch. Bounded; does not affect the verdict.
   A runtime value log closes it.
2. **Which builder produced each individual watch event.** The store instruction
   is shared; caller attribution is inferred from the destination and page, not
   observed.
3. **Which of the four production paths drove raziel's watched slots** in that
   session (root aim, descriptor Euler, attachment frames, or proxy transform).
   Static mode census can bound it; the capture did not record per-slot modes.
4. **The three angles' exact units and scaling per path.** The constructors use
   the values directly in trigonometry; the callers supply either floats derived
   from signed 16-bit object angles or sampled channel floats. The per-source
   quantization is not re-derived here.
5. **The per-slot descriptor's remaining bytes and its optional frame table**
   (same open items as the loader trace; not re-opened here).
6. **Whether non-body classes share the same constructors and paths.** The
   callers are generic transform helpers; a per-class census was not run.
7. **Whether the main and alternate builders can both run for one object in one
   frame.** The pass chooses one per object; the selection flag's semantics are
   not decoded here.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The two sites are two distinct routines, not two instructions of one | both sites resolve to the same containing function | Holds: each site resolved to a different containing function |
| The routines compute; they do not copy | a source pointer and a count in the write path, or writes not derived from the angle inputs | Holds: every written value derives from the three scalar inputs; no source pointer |
| The destination is the object's own pose array | the destination is a buffer not reached from the object pointer, or a persistent input buffer | Holds: destination is the object's pose-array field, indexed by the 13-float slot stride |
| The builder does not read its second/third argument registers | a read of either register in the builder prologue | Holds for the main builder at instruction level; the alternate builder decompiles with one parameter |
| The pose arena is per-frame and reset | no reset of the arena cursor, or allocations that persist across frames | Holds: cursor and limit state are reset in the frame update; allocations are monotonic within a frame |
| The earlier "builder receives an animated pose" reading | the builder consuming a pose pointer from a register | Refuted: the register is dead at entry |
| There is no separate upstream pose producer | a non-builder writer that computes a pose array later read by the builder | Holds so far: no such writer was found; the observed writers are the builders themselves |

## What this note does not claim

- It does not claim the write-watch was useless. It named the exact store class
  and led, through the destination, to the producer.
- It does not claim any address, offset, type name or source structure. The
  routines are described by behaviour only.
- It does not re-open the descriptor/frame-table layout questions; those stay
  where the loader trace left them.
- It does not change engine behaviour, production code or any golden.

## Checklist for review

- [ ] The decisive OUTPUT verdict is stated before the supporting detail.
- [ ] The two writers are described by role and composition order, never by
      number.
- [ ] Producer and output are distinguished at routine level and event level.
- [ ] The dead-register argument is stated with its instruction-level evidence
      and its bounded UNKNOWN.
- [ ] The phase is named (main frame update, per-object pose pass) and separated
      from the loader.
- [ ] The input sources are role-named and linked to the existing loader trace.
- [ ] The remaining open items are explicit and ordered by cost.
- [ ] No addresses, module-relative numbers, field offsets, disassembly text,
      game bytes or absolute paths appear in this note.

## Method and legal

- Static pass only: the shipped SR1 module was opened once in headless Ghidra
  with default analysis, from a temporary project outside the repository and
  outside the game directory.
- The two watch sites were resolved to their containing functions, decompiled,
  and cross-referenced to callers and inputs. Claim strength is annotated inline.
- No game process was launched, attached or instrumented by this slice. No game
  file was written and no game byte was copied into the repository.
- This note records roles, structural shapes, call relationships and behaviour
  paraphrases only. It contains no addresses, no module-relative numbers, no
  disassembly listings, no raw game bytes and no absolute install paths.

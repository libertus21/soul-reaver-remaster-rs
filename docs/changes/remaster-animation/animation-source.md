# Remaster animation source - who writes the `.SRM` slot transforms

The spline line is closed: the selector fired, the sampler never did, and only
`font` carried a set. This note follows the other path. In the remaster the
`.SRM` 128-slot pool is consumed by a **skin-palette assembler** that runs from
the draw path, and the per-slot pose it consumes is produced by a **slot-pose
builder** that reads a per-model **animation record**. The animation record is
not a `.SRA` buffer and not the original spline set: it carries a per-slot
descriptor array plus a separate per-frame track table.

This is a static, read-only result on the shipped SR1 module. A runtime capture
that would confirm the cadence and the data flow is prepared but deliberately
not run in this slice.

## Quick path

1. Read the verdict table for the five questions.
2. Read the data flow for the writer chain.
3. Read the evidence ledger for what is observed versus inferred.
4. Treat the UNKNOWN list as the honest boundary; do not fill it by inference.
5. Use "What would unblock this" to order the next slice.

## Verdict table

| # | Question | Answer | Confidence |
|---|---|---|---|
| 1 | Who writes the 128 slot transforms per frame? | Two roles. The **slot-pose builder** writes a per-object slot pose (one 13-float record per slot: a 3x3 rotation, a scale marker and a translation). The **skin-palette assembler** runs once per visible sub-model and writes one 12-float 3x4 matrix per active slot into a global palette, publishing the written count. Neither role is the spline system. | CONFIRMED for role identity, call chain and record shapes; cadence of the pose builder is delta-driven (CONFIRMED by its caller condition) |
| 2 | Where does the animation DATA come from? | A per-model **animation record** selected by the object's animation id through the actor block's animation table. It carries a per-slot descriptor array and a separate track table of 4-float frames with a per-track frame count and divisor. `.SRA` is not read on this path. `.SRL` is read by a global loader/sampler and feeds only the `extra > 0` HD-matrix class. The exact file field that populates the in-memory animation record is UNKNOWN. | CONFIRMED for the in-memory structures and the `.SRL` branch; UNKNOWN for the exact file carrier |
| 3 | Is it real skeletal animation or something else? | It is **GPU skinning**: a 3x4 matrix palette combined with the `.SRM` per-vertex three bone indices and byte weights. The per-slot pose is **procedural orientation**: descriptor mode bits choose aim/face computations relative to the object transform and a world target, with the object translation added. It is not vertex morphs, blend shapes or texture skinning. Whether an authored clip also rotates slots is UNKNOWN. | CONFIRMED for skinning + procedural slot orientation; UNKNOWN for clip-driven slot rotation |
| 4 | What is the cadence and the time source? | The palette is assembled per visible sub-model per drawn frame. The slot pose is rebuilt when the object transform or pose is dirty, or when the pose buffer is missing, not unconditionally every frame. Track selection uses an integer time counter: `index = (time / divisor) % frame_count`, plus a one-step frame base. The `.SRL` sampler uses a header scale against a global clock with a start/count window. The clock unit (frame vs millisecond) is UNKNOWN. | CONFIRMED for the formulas and the delta-driven pose condition; PARTLY for the real runtime cadence |
| 5 | Is a pose reconstructable from file data alone? | **NO for the body today.** The per-slot descriptors, the track table and the object transform are all file-derived in principle, but the animation-record carrier, the descriptor layout and the 4-float track semantics are not pinned. **YES for `.SRL`**: its per-track 3x4 stream is fully file-derived (already documented), but it binds to the facial/HD class, not to body bones. | CONFIRMED negative for the body; CONFIRMED for `.SRL` |

## Data flow

```text
object animation id
  -> actor-block animation table
     -> animation record
        -> per-slot descriptor array (32-byte records: active marker + mode bits)
        -> track table (track count + per-track frame count, divisor, 4-float frames)
  -> slot-pose builder
     -> per-object slot pose (13 floats per slot: 3x3 rotation, scale marker, translation)
  -> skin-palette assembler
     -> reads the .SRM 128 rest translations and 128 activation flags
     -> global palette of 12-float 3x4 matrices (one per written slot) + published count
  -> GPU skinning
```

The assembler only writes slots whose `.SRM` activation flag is set, so the
palette count is the number of active slots, not always 128.

Two independent systems feed the assembler:

- **Slot pose** - the main path, from the animation record's descriptor array.
  It is per object and is the same buffer the draw path reads.
- **HD matrices** - an alternate path, active only when the sub-model carries
  extra 3x4 records (the `extra > 0` class of 7 files). That path composes the
  extra matrix with the global `.SRL` frame for the current time. This is the
  only `.SRL` consumer found.

## Cross-check against the corpus

Counts only; read-only over the user's own copy.

| Asset | Count | Structural fact used here |
|---|---:|---|
| `.SRM` | 349 | All parse to the 128-slot pool. Activation flags are 0/1. `extra` is 0 in 342 files; the seven non-zero files carry 1, 7, 17, 25, 26, 26, 26 extra records |
| `.SRL` | 60 | Track counts are 1, 7, 17, 25 and 26. Every one of the seven `extra > 0` `.SRM` counts is present in the `.SRL` track set |
| `.SRA` | 227 | No reader on the animation path; the loader still hands the buffer to a no-op consumer |

The `extra`/track-count match is the strongest structural link between the
`.SRM` HD-matrix class and `.SRL`. It is a count correlation, not a decode of
the `.SRL` values, so the facial/dialogue reading remains INFERRED at the
semantic level while the branch itself is CONFIRMED.

## Evidence ledger

| # | Claim | Class | Evidence type |
|---|---|---|---|
| E1 | The `.SRM` loader stores, per sub-model, 128 rest translations (12 bytes each), 128 activation flags (1 byte each), the extra-matrix array and the per-vertex skin weights | Observed | Loader behaviour + existing parser |
| E2 | The skin-palette assembler loops over the animation record's slot count, reads the activation flags and rest translations, reads the per-slot pose, and writes one 12-float 3x4 matrix per active slot into a global palette | Observed | Static control flow and arithmetic |
| E3 | The palette assembler is called once per visible sub-model by the sub-model draw, which is called from the room render pass; a separate animation/visibility pass also calls the draw | Observed | Call graph |
| E4 | The slot-pose builder writes 13 floats per slot into the object's pose buffer; the record is a 3x3 rotation, a scale marker and a translation | Observed | Static math |
| E5 | The pose builder is delta-driven (object transform and rotation compare) and also runs when the pose buffer is absent or the animation id changes | Derived | Caller condition |
| E6 | The per-slot descriptor is a 32-byte record with an active marker and mode bits; the first two mode bits select aim/face computations | Observed for the bits read; rest UNKNOWN | Static control flow |
| E7 | The animation-track selector copies the selected 16-byte frame into the head slot of each track, indexed by time divided by a per-track divisor modulo the track's frame count | Observed | Static control flow |
| E8 | The `.SRL` loader reads the whole file into a global buffer and a global sampler returns the frame for the current time; the palette assembler consumes it only in the `extra > 0` branch | Observed | Writer/reader references + branch |
| E9 | The seven `extra > 0` `.SRM` counts are exactly the set of `.SRL` track counts | Observed | Corpus census |
| E10 | `.SRA` is not read on this path | CONFIRMED negative | Prior loader ledger + this traversal |

## What stays UNKNOWN

1. The exact file field that populates the in-memory animation record. The
   actor-block animation table is observed in memory; the container field that
   builds it is not pinned.
2. The 32-byte descriptor layout beyond the active marker and the first two
   mode bits.
3. The meaning of the 4-float track frame (quaternion, position, scale or a
   packed value) and how a track binds to a slot.
4. Whether any slot rotation is authored or clip-driven; the observed slot
   math is procedural.
5. The clock unit (frame index vs milliseconds) and the exact divisor meaning.
6. The real runtime cadence of the pose builder. The static condition is
   delta-driven; the runtime hit rate was not observed.
7. The `.SRL` channel-to-bone binding and the non-rigid cell decomposition,
   unchanged from the `.SRL` ledger.
8. Whether the `extra > 0` class is exclusively dialogue/facial. The count
   match supports it; the file naming and the voice/subtitle correlation
   support it; it is not proven per file.

## Falsification notes

| Claim | Observation that would refute it |
|---|---|
| The palette assembler is the per-frame slot writer | a moving actor never enters it, or the published palette count stays zero while the actor visibly deforms |
| The 13-float slot pose is the assembler input | the assembler reads a different structure, or the pose builder writes a different stride |
| The animation record supplies the slot pose | the descriptor array does not correspond to the slot count, or the pose is built from another table |
| `.SRL` feeds only the `extra > 0` class | a sub-model with `extra == 0` consumes the global `.SRL` frame, or the count sets diverge |
| `.SRA` is not consumed | a reader of the `.SRA` buffer is found outside the observed loader path |
| The pose builder is delta-driven | a probe shows it running at a constant per-frame rate for an idle object |

## What would unblock this

Ordered by cost:

1. **Run the prepared runtime probe** on a moving actor. It reports whether the
   palette assembler and the pose builder fire, their slot counts, the pose
   signature and the track index/time. That confirms or refutes the writer
   chain without decoding anything new.
2. **Pin the animation-record carrier.** Follow the actor-block animation table
   from the object-container loader to the file field. This is the shortest
   route to a file-only replay.
3. **Decode the 32-byte descriptor** by comparing models with visibly different
   behaviour and correlating the mode bits. A runtime probe that logs the
   descriptor per slot is the cheapest discriminator.
4. **Decode the 4-float track frame** by correlating the selected frame with
   the object transform at the same time. Only then is the track type proven.
5. **Write the minimal replay** once 2-4 are pinned: object transform plus
   per-slot pose at time `t`, then the palette, then the existing `.SRM` skin
   weights.

## Palette-hook correction (2026-11-09)

The prepared capture reported `palette=0` while `pose=627` and `track=9454`
fired. Read that correctly: the palette-assembler hook did not fire. It does
**not** mean the palette assembler is absent, and the earlier shorthand of
"palette=0" must not be read as "no palette". The pose builder and the track
selector being observed confirms the rest of the chain was live; only the
palette hook missed.

The palette assembler is re-pinned here **by identity, not by name or call
site.** Its distinguishing behaviour is the whole sequence:

1. reads the model's 128 rest translations;
2. reads the 128 activation flags;
3. reads the per-slot 13-float pose;
4. writes one 12-float, row-major 3x4 matrix per active slot into a global
   palette;
5. publishes the written count.

A routine that does not do all five is not this role. The count is the number of
slots whose activation flag is set, so "assembler ran, all flags clear" and
"assembler never ran" are different observations and must be reported apart.

The probe (outside the repository, since it carries module-relative offsets) was
corrected accordingly:

- the palette role is identified by that behaviour signature, so a hook that
  fires with a zero published count is labelled `assembler-ran-empty` and a hook
  that never fires is labelled `unobserved` — the ambiguous `palette=0` is gone;
- the per-call dumps were replaced with de-duplicated per-object signatures plus
  periodic summaries, because the previous run exhausted its line cap on
  repeated per-call dumps instead of recording distinct events.

The capture was **not** re-run in this slice. Role identity is CONFIRMED from the
static writer chain above; the runtime hit rate of the palette assembler and the
specific reason the earlier hook missed it remain UNOBSERVED.

## Method and legal

- Static pass on the shipped SR1 module with headless Ghidra, read-only; the
  project lived outside the repository and the game directory.
- Corpus cross-checks were counts only over the user's own copy.
- No addresses, memory or file offsets, disassembly listings, raw game bytes or
  absolute install paths are recorded here. Functions are named by role.
- No game process was launched, attached or instrumented by this slice. No
  production code changed and no golden moved.

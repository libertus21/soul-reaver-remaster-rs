# Design: anim-binding

docs-only analysis slice. "Design" here is the investigation design: the
bounded method, the loader chain as reconstructed, and the decision record
for the no-go. No production interface changes.

## Approach

1. **Anchor before reading.** Find the animation-ish literals in the
   complete string inventory, then resolve every reference to its containing
   function. This is the same re-pin discipline as `sr1-ghidra-survey.md`:
   `FUN_` names shift between analyses, strings and RVAs do not.
2. **Walk one path to the end.** From the `.sra` literal follow the only
   caller down to the load queue, the completion, the drain and the
   callback. Stop when the data is delivered or dropped.
3. **Contrast, don't speculate.** Reconstruct the model path and the spline
   path only far enough to show they are *different* consumers, so the
   no-go is not "we gave up".
4. **Negative test the alternative reading.** The generic codec post
   processor is the one place a hidden parser could live; test it against
   real files before claiming absence.

Two passes, one 75-minute box, one auto-analysis run and seven cheap
`-noanalysis` script runs.

## Loader chain (reconstructed)

    object name
      -> object-slot allocator: 48 slots x 0x38, name +0, state +0x18
         -> formats "\\kain2\\object\\%s.sra"      (and _e3 variant)
         -> formats "\\kain2\\object\\%s\\%s.crm"
         -> formats "\\kain2\\object\\%s\\%s.pcm"
      -> async queue (64 entries x 0x78): path, callback, context, type byte
      -> completion: store (buffer, size, state=1) only
      -> drain: post-process by codec tag, then call stored callback(buffer, ...)
      -> .sra callback = 0x115ef0 = RET 0        <-- data dies here
      -> .pcm callback = model builder
         -> sub-model count -> one "OBJECT\\%s_%d.SRM" per part (0xda0 stride)
      -> later: spline sampler reads numkeys/currkey keys, per-object frames

The `.sra` buffer is stored at `slot[0x30]`. The model buffer is stored at
`slot[0x10]` and is what the ready/fixup/cleanup consumers dereference.

## Evidence ledger (summary; full text in the note)

| Claim | Class | Basis |
|---|---|---|
| One owner for both `.sra` literals; no other `.sra` string reference | CONFIRMED | complete defined-string inventory + raw ASCII scan |
| That owner queues a raw load with callback `0x115ef0`, context `&slot[0x30]`, type `0x32` | CONFIRMED | instruction-level call site |
| `0x115ef0` is `RET 0` in inter-function padding; no defined function | CONFIRMED | bytes + neighbour listing |
| Completion stores the buffer; drain calls the stored callback with it | CONFIRMED | both functions read end to end |
| The codec-`0x32` post-processor expects `0x0FACE0FF` records; real `.SRA` has none in the sampled entries and its offsets are `4,8,12,...` | Observed | 3 files, 64/52/64 leading entries |
| `.PCM` selects N sub-models, each an `OBJECT\%s_%d.SRM` load into a 0xda0 array | CONFIRMED (loop/format); INFERRED (container role) | model loader |
| A separate per-object spline runtime exists with frame deltas and 12-bit fixed-point key triples | CONFIRMED | sampler + debug formats + constants |
| That sampler writes the instance transform, not a 128-joint skin | Derived (strong) | field targets and call shape |
| No reader of the `.SRA` buffer exists on the inspected paths | CONFIRMED (path); UNKNOWN (whole program) | consumers of the slot table |

## Why no-go, argued

- **A binding needs data flow.** The only path that knows a `.sra` exists
  ends in a no-op. A table or loop that never runs cannot be cited as a rule.
- **The obvious hiding place is ruled out.** If the engine secretly parsed
  `.SRA`, the generic raw-asset post-processor would be it. Its record
  contract (tagged records, offset pointers) disagrees with the files on
  every sampled entry, and the delivered result is discarded anyway.
- **The real animation machine is elsewhere.** The spline runtime is a
  transform track system with a different key layout. Documenting "channel i
  -> joint i" from `.SRA` would be an invention, not a recovery.

## Decisions

| Decision | Options | Choice | Rationale |
|---|---|---|---|
| Slice type | decode `.SRA` semantics; answer the binding question in code | answer in code, document no-go | the data side is already documented as non-binding; only code could overturn it |
| Depth | full loader reconstruction; one path to the end | one path, plus contrast | the question dies at the callback; deeper work buys nothing |
| Negative test | trust the decompiler shape; probe real files | probe real files | absence claims need a test, not an impression |
| Artifact form | code, note, both | note + planning bundle | nothing to implement; a code stub would be dead code |
| Address style | names only; RVAs; both | role names, RVAs only for load-bearing points | names drift, RVAs are stable, readers need anchors |

## Threat matrix

N/A. No routing, shell, subprocess, VCS/PR automation, executable
classification, or process-integration boundary. The change reads one local
binary and writes Markdown.

## Risks

| Risk | Mitigation |
|---|---|
| "Absence" is overclaimed | every absence names its search; residual `UNKNOWN` 3 is spelled out |
| The `RET 0` is a build artifact, not intent | stated as "this build does not parse it", not as "the format is dead" |
| A reader trusts a `FUN_` name in a future run | re-pin rule is stated in both note and spec |
| Legal slip (listing pasted) | artifacts contain paraphrases only; scratch stayed in `%TEMP%` |

## Open questions

- [ ] Where the spline keys are filled (cheapest probe; does not block this
      no-go).
- [ ] Whether SR2 parses `.sra` (would overturn the "legacy for both"
      reading, not the SR1 verdict).

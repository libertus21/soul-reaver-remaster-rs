# Proposal: anim-binding

**NO-GO.** A bounded Ghidra survey of `1\sr1.dll` answers the question this
change was created to answer, and the answer is the negative one: the SR1
remaster **never parses `.SRA`**. The file is opened, queued, decompressed
and then handed to a three-byte `RET 0` stub. There is no channel table, no
per-joint binding loop, and no parent/child initialisation on that path, so
no binding rule can be extracted and documented for later implementation.

The slice is still worth landing. It retires the last open hypothesis about
`.SRA` - "the binding must be in code" - with binary evidence, and it names
the one structure a future animation change should chase instead: the
per-object spline key table and whoever fills it.

## Quick path

1. Read `docs/notes/anim-binding.md`: the bound, the evidence ledger, the
   no-go, and the ordered next probes.
2. Read `design.md` for the loader chain and the per-claim evidence class.
3. Read `tasks.md` for the (docs-only) forecast and the apply plan.

No code is produced. No viewer behavior changes. `cargo test` is the
regression proof and is a no-op baseline here (208 passed at plan time).

## Details

| Topic | Decision |
|---|---|
| Intent | Answer whether `.sra` curve channels are bound to joints in code, and document the rule if so |
| Verdict | **No rule exists to document.** The only `.sra` consumer is an empty callback (`0x115ef0`, `RET 0`) |
| Why the answer is credible | Single string owner, instruction-level call site, byte-level callback, queue completion and drain read end to end, plus a negative test of the generic post-processor against real files |
| What the binary does instead | A per-object spline runtime with `numkeys`/`currkey`, per-key frame deltas, 12-bit fixed-point triples and clip frames - fed from a source other than the `.sra` path |
| Scope | Docs only. No `src/` change, no format change, no CLI change |
| Reuse rule | `docs/notes/sra-animation.md` and `docs/notes/srm-bone-block.md` stay authoritative for the data side; this note only adds the code side |
| Evidence policy | Every claim carries CONFIRMED / INFERRED / UNKNOWN. Absence claims state the search that was run |
| Legal policy | Behavior notes only. No listings, no decompiled text, no game bytes, no absolute paths, Ghidra project outside the repo |
| Non-goals | Playback, skinning, `.SRL` decode, SR2, dynamic tracing, any rewrite of the spline system |
| Slice boundary | If a reviewer can still claim "maybe the binding is in some other function", the note has failed; the ledger and the string scan are the answer to that |

## Explicit no-go condition

This change is GO as a documentation slice while all four hold:

1. The `.sra` path literals have exactly one owning function (whole-string
   scan, not a sample).
2. That function's load call passes a callback that is an empty function at
   byte level.
3. The queue drain really delivers the buffer to that callback, and no
   reader of the stored field is found on the inspected paths.
4. A real-file spot check shows the generic post-processor's record shape
   (`0x0FACE0FF` tagged records) does not match `.SRA`.

If (1), (2) or (3) fails during apply, STOP and rewrite the note as
"binding exists, not yet decoded" naming the failed check and the function
that disproved it. Do not preserve the no-go wording.

## What this buys

- The project stops treating `.SRA` as "the animation candidate" and stops
  budgeting slices against it.
- The next animation attempt starts from the spline key table, which is
  real, running code with a documented layout, instead of from a dead file
  loader.
- The negative result is reproducible: it is a string scan plus two
  function walks, not a month of reverse engineering.

## Checklist

- [ ] Reviewer can state the verdict without re-reading the note.
- [ ] Reviewer can state which single function owns `.sra` and what its
      callback is.
- [ ] Reviewer can state what remains UNKNOWN and which probe is cheapest.
- [ ] No claim of a channel->joint mapping, hierarchy or clip catalog
      appears anywhere in the bundle.

## Next step

`design.md` for the chain and the evidence classes, `tasks.md` for the
forecast; the note is the deliverable.

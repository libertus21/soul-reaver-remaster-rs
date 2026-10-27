# `anim-binding` - does `sr1.dll` bind `.SRA` channels to joints?

Bounded, read-only Ghidra 12.1.4 survey of the SR1 remaster binary
(`1\sr1.dll`, AMD64, 2,416,008 bytes). Behavior notes only: role names,
string text, structural paraphrases and RVAs. No disassembly or decompiled
text is reproduced here, no game bytes, no absolute install paths.

## Box (what was and was not done)

Two passes, roughly 75 minutes, all inside the agreed box:

1. **String/xref recon.** Whole-binary scan of defined strings for
   animation-ish terms (`.sra`, `.srl`, `.srm`, `skeleton`, `anim`, `joint`,
   `bone`, `channel`, `keyframe`, `curve`, `spline`, `frame`, `playto`,
   `interp`, `bind`, ...) with every code reference resolved to its
   containing function.
2. **Loader walk.** From the `.sra` literals down the async load queue, the
   completion handler and the drain, then across to the model and spline
   paths for contrast.

Not done: dynamic tracing, `.SRL` decode, SR2 binary. Nothing under the
install was written. Ghidra's project lived in `%TEMP%` only.

## Question

The `.SRM` block is a flat 128-slot joint pool with no hierarchy, and the
`.SRA` container has no bone index, no name table and no clip catalog.
Therefore, if a binding exists at all, it must be code. This slice asks:

> Is there a channel->joint binding in `sr1.dll` - a mapping table indexed
> by channel, a per-joint binding loop, or a parent/child initialisation -
> that makes `.SRA` curves usable?

## Answer

**NO. In this binary the `.SRA` is never parsed.**

The engine formats the `.sra` path, queues an asynchronous raw load,
decompresses the file, stores the buffer in the object slot - and hands it
to an **empty callback**, a three-byte `RET 0`. Nothing downstream reads
that buffer as curves. There is no channel table, no per-joint loop, and
no name/index association anywhere on the `.sra` path.

The animation runtime that *does* exist in the binary is a per-object
**spline** system, and it is fed from somewhere other than this loader.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| A1 | Exactly one function owns the `.sra` literals (`\kain2\object\%s.sra`, `\kain2\object_e3\%s.sra`). The whole defined-string inventory contains no other `.sra` reference, and a raw ASCII scan agrees | CONFIRMED |
| A2 | That function is the object-slot allocator/builder: a table of 48 slots, stride 0x38, name at +0, state at +0x18; it also formats the `.crm` and `.pcm` siblings for the same name | CONFIRMED |
| A3 | It enqueues an asynchronous load with: the `.sra` path, callback argument `0x115ef0`, context `&slot[0x30]`, type byte `0x32`. Verified at instruction level - the callback address is loaded immediately before the queue call | CONFIRMED |
| A4 | `0x115ef0` decodes as `C2 00 00` = `RET 0`, three bytes inside the padding after the neighbouring function. Ghidra defines no function there. It performs no work and cannot parse anything | CONFIRMED (bytes + neighbours) |
| A5 | The queue completion only records `(buffer, size, state=1)` into the request. The drain then post-processes by codec tag (`1`, `2`, `0x32`) and finally calls the stored callback with the buffer as first argument | CONFIRMED |
| A6 | Therefore the `.SRA` buffer is delivered to `RET 0` and left in `slot[0x30]`. No consumer was found | CONFIRMED for the loader path; residual in "Unknowns" 3 |
| A7 | The `0x32` post-processor is a generic resource-table builder: it reads the leading dwords as offsets, expects records tagged `0x0FACE0FF`, and copies child tables. Sampled real `.SRA` entries (64, 52 and 64 leading entries of the three largest files) contain no such tag, and the offsets form a `4,8,12,...` run. This processor does not describe `.SRA` content either | Observed (3-file sample) |
| A8 | `.PCM` is the object container: a count reads N sub-models, each loaded as `OBJECT\%s_%d.SRM` (1-based) into a 0xda0-stride record array. The `.SRM` loader decodes 44-byte shader and 48-byte material records and dedups their names through global hash tables. Nothing on this path maps curves to joints | CONFIRMED for the count/loop/format; the "container" role is INFERRED |
| A9 | A separate per-object spline runtime exists: debug formats print `Spline %s%d playto %d ... maxFrames=%d`, `Spline %s%d : clip(%d,%d) ...`, `Spline %s%d prevFrame=...`, and a sampler prints `def->currkey=%d spline->numkeys=%d`. Keys begin with a frame delta; sampled values are 12-bit fixed point (`>>12`, scale 4096); key stride is 0x1c or 0x30 depending on a per-spline flag byte | CONFIRMED (control flow + constants) |
| A10 | That spline sampler writes the instance's own transform fields (two three-component groups, short + float mirrors), i.e. object transform animation over frames, not a 128-slot skin deformation | Derived (strong) |
| A11 | No function on any inspected path consumes the `.SRA` buffer, and the spline key tables are not populated from it | CONFIRMED by absence of a reader on that path |

### Why this is a no-go and not "not found yet"

A binding rule needs a consumer. A1-A6 show the only `.SRA` consumer is an
empty function. A7 shows the generic post-processor does not match the file
shape. A9-A10 show the animation system that runs is a different machine
with a different key layout. There is no seam left where a channel->joint
table could hide on this path.

## What the binary does contain (for the next slice)

- **Object graph loader.** 48 object slots; each slot can name up to 8
  child objects; the tracker waits until dependencies reach the loaded
  state. The `.SRA` is loaded as part of the same slot creation as `.pcm`
  and `.crm`, but only the `.pcm` result is consumed.
- **Sub-model array.** Each object instance owns N `0xda0`-byte records,
  one per `name_%d.SRM`. This is where a per-part joint pool lives; a real
  binding would have to address `(part, slot)` today and does not.
- **Spline keys.** A track table with `numkeys`/`currkey`, per-key frame
  deltas, 12-bit fixed-point triples, and clip (`playto`) logic. If `.SRA`
  ever turns out to hold curves, this is the structure they would have to
  be converted into.

## What stays UNKNOWN

1. **Where the spline keys come from.** The writer that fills `numkeys`
   and the key array was not reached in this box. It is the shortest route
   to the real animation source.
2. **Whether the remaster replaced the parser with a stub.** A `RET 0` at a
   real call site is either an empty function in the shipped build or a
   deliberately gutted one. It is not proof that the original build parsed
   `.SRA`, only that this one does not.
3. **Whether any non-string path reaches `slot[0x30]`.** A script or AI
   layer could read the field without referencing `.sra`. No such reader
   appeared in the inspected consumers, but a whole-program data-flow
   search for that field was not run.
4. **`.SRA` offset semantics.** The offsets run `4,8,12,...` in the leading
   entries, which fits one 4-byte item per declaration rather than a
   pointer per record. What each item means is not established.
5. **`.SRL` record semantics.** 60 files, `SRL.` magic, a count and a
   ~1200-byte stride; not decoded.
6. **The two spline key strides (0x1c / 0x30)** and the per-spline flag byte
   that selects them.

## Next probes (ordered by cost)

1. **Find the spline-key writer.** Locate the store into the `numkeys`
   field and the key-array allocation. That names the real animation source
   without guessing at `.SRA`.
2. **Dynamic confirmation (cheap, read-only).** With an object that has a
   loose `.SRA`, break on the drain's indirect call for codec `0x32` and on
   the first-dword read of `slot[0x30]`. Expect the stub to return
   immediately and the field never to be read. This upgrades A6 to observed
   runtime behavior.
3. **`.SRL` scoping.** The only remaining sibling large enough to carry
   clips (up to 1.7 MiB, ~1200-byte records). Test for a per-object clip
   catalog: name list, count, frame range.
4. **SR2 parity.** Check `2\sr2.dll` for the same two `.sra` literals and
   compare the callback target. If SR2 parses it, the parser exists and was
   dropped in SR1; if SR2 also stubs it, the format is legacy for both.
5. **Give up on `.SRA` runtime use.** Keep the container census as a
   diagnostic and stop claiming that `.SRA` is "the animation candidate".

## Method (reproducible)

- Project: `%TEMP%\opencode\anim-recon\proj`, imported once with default
  auto-analysis; every later run uses `-noanalysis` plus a Java post-script
  that (a) prints candidate strings with their xref functions and (b) writes
  decompiled text to `%TEMP%` for reading. Nothing is copied into the repo.
- **Re-pin rule:** never trust `FUN_` names between runs. Anchor on a string
  literal or an import, read the containing function, then follow calls.
  RVAs are file-relative and stable for this PE; names are not.
- Raw spot checks (offset tables, magic probes) were done with a small
  in-memory reader, never a dump.

## Legal

Counts, role names, string text, field shapes and RVAs only. No listings,
no disassembly, no decompiled bodies, no game bytes, no absolute install
paths. Game data read-only; Ghidra project outside every repo and game
directory.

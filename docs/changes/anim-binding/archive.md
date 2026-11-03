# Archive: anim-binding — NO-GO

**Stage banner: NO-GO — the SR1 remaster never parses `.SRA`.**

A docs-only Ghidra pass answered the code-side question: the `.sra` path literals
have exactly one owning function, which enqueues an asynchronous raw load and
hands the buffer to a three-byte `RET 0` stub (`0x115ef0` at instruction level).
No channel table, per-joint loop or parent/child initialisation exists on that
path. The animation runtime that does exist is a per-object spline key system fed
from a different source. Evidence: `docs/notes/anim-binding.md`; plan and ledger
under `docs/changes/anim-binding/`.

## What landed
- PR #41 (`docs/animation-investigation`, merge `4a878d1`, commit `fd02c97`) —
  docs only: loader-chain walk, evidence ledger A1–A11, UNKNOWN list, next probes.
- No `src/` change; `cargo test` was a no-op baseline.

## What stayed UNKNOWN
- The writer that fills the spline `numkeys`/key array: the real animation source.
- Whether this build deliberately stubbed a parser the original shipped.
- Whether a non-string path reaches the stored `.SRA` buffer (`slot[0x30]`).
- `.SRL` record semantics and the two spline key strides (`0x1c` / `0x30`).

## Next step
Find the spline-key writer, then run a cheap read-only dynamic confirmation that
the drain's callback returns immediately and the buffer is never read. Scope
`.SRL` (60 files) and compare SR2's callback before claiming `.SRA` is usable.

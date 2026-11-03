# Archive: sra-animation — NO-GO

**Stage banner: NO-GO — `.SRA` has no hierarchy, no clip catalog, no `.SRM` binding.**

A read-only census of all 227 `.SRA` files found a structural header (declaration
count, ascending offsets, `0` terminator, 2 KiB floor) and a payload of packed
integer records plus smooth float runs bounded to `0..360`. There is no parent
array, no name table, no duration/frame-rate field, and only 1/227 basenames
match an `.SRM`. Binding a curve to a joint would be fabricated. Evidence:
`docs/notes/sra-animation.md`, proposal under `docs/changes/sra-animation/`.

## What landed
- PR #41 (`docs/animation-investigation`, merge `4a878d1`, commit `fd02c97`) —
  the `.SRA` census and the no-go.
- The proposed `--sra-info` diagnostic did NOT ship; the slice concluded docs-only.
  The code-side follow-up `anim-binding` then proved this build never parses
  `.SRA` (the loader callback is a `RET 0` stub).

## What stayed UNKNOWN
- Channel-to-joint binding, time base/frame rate, clip boundaries.
- The packed header fields and the `N` semantics.
- `.SRL` record semantics (60 files, `SRL.` magic, ~1200-byte stride).

## Next step
Stop treating `.SRA` as the animation source. The only live structure is the
per-object spline key table; `.SRL` scoping and SR2 parity are the next probes.

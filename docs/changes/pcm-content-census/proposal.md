# Proposal: pcm-content-census (outcome: no-go, docs-only)

## Intent

Redo the `.PCM` block content census at the verified data base (PR #47), and
re-evaluate the placement and geometry no-gos that earlier content claims
supported.

## Outcome

- Census by content: no all-zero blocks in five probed units. 4-byte blocks
  are small integers in three units. A u16 lag-6 (12-byte) period appears in
  48-byte blocks on the correct base.
- The earlier f32 field, 12-byte `[u32][f32][u32]` group, and placement reading
  are **invalid**. f32 is at chance (54.5% for random bits, versus 51-91% in
  measured 48-blocks).
- Placement: **NO-GO** in this change.
- Geometry: **NO-GO** for decoding. A falsifiable probe exists for the surviving
  lag-6 period.

Full evidence: `docs/notes/pcm-content-census.md`.

## Forecast (only if a later change is proposed)

No code is proposed in this change. If the probes above are funded later, the
forecast is a single read-only CLI arm (about 150 authored lines) plus tests,
well under the 400-line limit. It would reuse `data_base()` and the census
helpers in `src/level.rs`.

## Next probes (not started)

1. Geometry: split 48-byte blocks into 6-u16 records and test per-column
   monotonicity or delta coherence across units.
2. Placement: test whether column 0 of 48-byte blocks is a pointer to the block
   at that relative offset, and compare the target size class.

## Non-goals

Mesh or vertex decode, transform or placement decode, rendering, block type
taxonomy, and any game bytes in the repo.

# Archive: pcm-content-census — DONE (census; later reinterpreted)

**Stage banner: DONE — content censused at the corrected data base; the block model was later corrected.**

The census re-read `.PCM` block content at the verified data base
(`round_up(4 + 4N, 2048)`) and explicitly invalidated the earlier `4 + 4N`
content claims. The surviving signal is a u16 equality period at lag 6 (12 bytes)
inside 48-byte spans, at 20–34% in every probed unit; the old f32
`|f| < 5000` reading sits at chance and was refuted as evidence. Evidence:
`docs/notes/pcm-content-census.md`.

## What landed
- PR #48 (`docs/pcm-content-census`, merge `7c06c61`, commit `8eeaef9`) — the
  per-unit census and the re-evaluated no-gos.
- Depends on PR #47 (`fix/pcm-data-base`, merge `e7bbde1`) for the correct base.

## What stayed UNKNOWN
- What the lag-6 period encodes; no type label was assigned.
- Whether column 0 of 48-byte spans is a pointer (unit-dependent 0–19%).
- Geometry and placement were still NO-GO in this change.

## Next step
`level-builder` then showed the spans are pointer spacings, not records, and
`level-geometry` resolved the mesh at slot `0x00`. This census remains a
shape-only record; its probes no longer decide block semantics.

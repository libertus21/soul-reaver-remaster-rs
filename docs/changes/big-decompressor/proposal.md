# Proposal: big-decompressor

## Intent

Find the compression behind `.BIG` slot payloads and plan the smallest slice that
decodes ONE slot. The investigation answered the first half negatively: the
payload is **not compressed**. Every `data` slot is a raw, headerless
**256 × 256 A1R5G5B5 image** (131072 bytes = 256 × 256 × 2), verified by byte
statistics and by rendering sampled slots into coherent images with correct
colours (`docs/notes/s7-big-slot-payload-format.md`).

So the slice that "decodes one slot" becomes a **format converter**, not a
decompressor: read one slot, convert A1R5G5B5 to RGBA8, show it through the
window path that already exists. `--tshow` stops being a guaranteed error and
starts showing one real slot. Empty and all-zero slots stay loud, because they
carry no payload.

The name `big-decompressor` is kept for traceability with the dispatch that
commissioned it; the body of the slice is the decoder that turned out to be
needed.

## No-go condition (explicit)

This slice is a **no-go** — and `--tshow` must stay loud — if any of these holds
at apply time:

1. A real `data` slot does not render as a recognizable image (noise, sheared
   rows, wrong colours, or wrong orientation) under the A1R5G5B5 256 × 256 read.
2. The payload length invariant breaks for a geometry-valid container (any slot
   that cannot be read as exactly 131072 bytes).
3. A protection or anti-tamper layer appears on this path. Stop that line and
   report; never defeat anything.

If (1) or (2) happens, do not "fix" it by guessing a second format. Revert the
claim, keep the loud `--tshow`, and re-open the investigation with pass 3 (the
bounded Ghidra call-path probe) as the next candidate.

## Quick path

1. `cargo run -- --textures <TEXTURES.BIG> --tshow 4` reads slot 4, decodes
   256 × 256 A1R5G5B5 to RGBA8, prints one verifying line, and shows the image.
2. `--tshow 0` (all-`0xFF`) and an all-zero slot stay loud: no payload, no
   window, exit 1.
3. Selector, header, and geometry errors keep their exact prior contracts.
4. `cargo test` covers the converter and every error path on synthetic bytes;
   the visible render is a manual playtest.

## Details

| Topic | Decision |
|---|---|
| Verified format | 256 × 256, little-endian `u16`, A1R5G5B5 (bit 15 alpha, 14–10 R, 9–5 G, 4–0 B), row-major, top-down |
| Why not compressed | Raw bytes render as coherent images with no transform; lag-2 byte autocorrelation equals the pixel unit; exact 256·256·2 fit leaves no header room |
| Slice scope | One slot, one converter, one window. No batch export, no atlas slicing |
| Empty/zero slots | Still loud, exit 1, no window. They carry no payload |
| Output line | One deterministic stdout line naming slot, offset, decoded dimensions and pixel format, so the decode is checkable without the window |
| Existing contracts | Header parsing, geometry validation, `--tlist`, selector/range errors, and every other CLI mode stay unchanged |
| Tests | Synthetic bytes only. Known `u16` patterns assert the RGBA8 conversion; CLI tests cover error paths; the render is a manual playtest |
| Dependencies | None new; `render::show_rgba8` already exists |
| Legal | Format facts only; no payload dumps, no install paths; scratch analysis stays outside the repo |

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `src/slot_texture.rs` | New | Pure A1R5G5B5 → RGBA8 converter with dimension/length guards and unit tests |
| `src/textures.rs` | Modified | One-slot decode helper; empty/zero messages drop the "decompressor UNKNOWN" text; module doc corrected |
| `src/main.rs` | Modified | `--tshow` decodes a `data` slot instead of always erroring; usage/help text |
| `README.md` | Modified | `.BIG` bullet: slot payloads are raw 256 × 256 A1R5G5B5; `--tshow` shows one slot |
| `docs/notes/s7-big-slot-payload-format.md` | New | Evidence ledger so the finding survives |
| `docs/changes/big-decompressor/*.md` | New | This planning bundle |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| A sampled slot contradicted by the full set | Low | The format fits every sample across all four containers; the apply playtest shows several slots |
| Channel order is B/G/R, not R/G/B | Low | Visual evidence (natural skin/stone tones) says R/G/B; the playtest catches a swap immediately |
| `--tshow` success path is untestable headlessly | Medium | Decode is a pure function with unit tests; only the window call is manual, same pattern as `--bigfile --show` |
| 5→8 expansion differs from the engine's | Low | Bit replication is standard; a wrong ramp is visible as banding on real slots |
| Scope drift into slot→name mapping or atlases | Medium | Design lists both as out of scope; tasks stop at one slot |

## Rollback Plan

Revert the converter, the `--tshow` wiring, and the README bullet. `--tshow`
returns to its loud `slot … is data: payload is compressed, decompressor
UNKNOWN` verdict, and `src/textures.rs` to container-only behavior. No data, no
state, no config.

## Success Criteria

- [ ] `--tshow <i>` on a `data` slot prints one verifying line and shows a
  recognizable 256 × 256 image from the real container (manual playtest).
- [ ] `--tshow` on empty/zero slots stays loud, exit 1, no window.
- [ ] Header, geometry, selector, `--tlist`, and every other mode are unchanged.
- [ ] `cargo test` green on synthetic bytes only; no game bytes in the diff.
- [ ] `docs/notes/s7-big-slot-payload-format.md` records the algorithm verdict
  and the ruled-out set.

## Capabilities

### New Capabilities

- `big-slot-payload`: decode one `.BIG` `data` slot as a raw 256 × 256
  A1R5G5B5 image and render it, with loud refusal for slots that carry no
  payload.

### Modified Capabilities

- `big-slots`: `--tshow` changes from "always a loud non-decodable verdict" to
  "decode a `data` slot; still loud for empty/zero". Listing and classification
  are untouched.

## Next step

`spec.md` (requirements and message/exit contracts), then `design.md` and
`tasks.md`. Apply follows as a single PR.

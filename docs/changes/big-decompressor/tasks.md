# Tasks: big-decompressor

Ordered work units. Each keeps its tests with its behavior and is independently
reviewable. `test_command` for apply: `cargo test`.

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~240-300 authored (`slot_texture.rs` ~100 new; `textures.rs` ~60 add / ~10 del incl. tests; `main.rs` ~45 add / ~15 del incl. tests; README ~10; doc comments ~10) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (4 work-unit commits) |
| Delivery strategy | single-pr (cached) |
| Chain strategy | pending — not needed at this size |

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low
```

### Suggested Work Units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | A1R5G5B5 → RGBA8 converter | PR 1 | `cargo test slot_texture` | N/A — pure function | `src/slot_texture.rs` |
| 2 | One-slot decode helper + payload errors | PR 1 | `cargo test textures` | N/A — synthetic I/O | `decode_slot_at` and `SlotNoPayload` in `src/textures.rs` |
| 3 | `--tshow` shows one slot | PR 1 | `cargo test cli_tests` | `cargo run -- --textures <one real .BIG> --tshow 4` (user playtest, read-only) | `run_textures` + CLI tests |
| 4 | README truth-up + legal gate | PR 1 | `cargo test` | N/A — docs only | `.BIG` bullets in `README.md` |

## Task 1 — `src/slot_texture.rs`: A1R5G5B5 converter (≤150 authored lines, tests included)

- [ ] 1.1 Create `src/slot_texture.rs` with `WIDTH = 256`, `HEIGHT = 256`,
  `PIXEL_BYTES = 2`, `SLOT_BYTES = WIDTH * HEIGHT * PIXEL_BYTES` (131072).
- [ ] 1.2 Add `SlotDecodeError::WrongLength { found, expected }` with the
  `spec.md` message text and `Display`/`Error` impls.
- [ ] 1.3 Add `expand5(v: u8) -> u8` as `(v << 3) | (v >> 2)`.
- [ ] 1.4 Add `decode_a1r5g5b5(bytes: &[u8]) -> Result<Vec<u8>, SlotDecodeError>`:
  length guard first, then little-endian `u16` per pixel in row-major order;
  `a` = bit 15 ? 255 : 0; `r/g/b` from bits 14–10/9–5/4–0 via `expand5`.
- [ ] 1.5 Unit tests (synthetic values only): `expand5(0) == 0`,
  `expand5(31) == 255`, `expand5(16) == 132`; all-zero slot → all pixels
  `(0, 0, 0, 0)`; all-ones slot → `(255, 255, 255, 255)`; pure-red, pure-green,
  pure-blue, and alpha-set/alpha-clear pixels map exactly; first pixel lands at
  output byte 0 and pixel 256 at output byte 1024 (row-major check).
- [ ] 1.6 Wrong lengths (0, 131071, 131073) return `WrongLength` naming both
  lengths; nothing is padded or truncated.
- [ ] 1.7 Verify: `cargo test slot_texture` green; `cargo build` warning-free.

## Task 2 — `src/textures.rs`: one-slot decode helper (≤150 authored lines, tests included)

- [ ] 2.1 Add `pub fn decode_slot_at(path, header, index) -> Result<(u64, Vec<u8>), TexturesError>`:
  `debug_assert!` the range (caller checks), seek `slot_offset(index)`, read
  exactly 131072 bytes, classify; `Data` → `slot_texture::decode_a1r5g5b5`,
  `Empty`/`Zero` → the new payload error. Short read stays `Io`.
- [ ] 2.2 Replace `SlotNotDecodable` with `SlotNoPayload { index, offset, class }`
  whose messages are exactly the two `spec.md` lines (empty/zero, "no payload to
  decode"). Delete the "payload is compressed, decompressor UNKNOWN" text.
- [ ] 2.3 Update the module doc: payload is raw 256 × 256 A1R5G5B5, not
  compressed; `--tshow` decodes one slot. No claim about other pixel formats.
- [ ] 2.4 Update the existing `slot_not_decodable_messages_*` test to the new
  variant/messages; keep every other error message assertion.
- [ ] 2.5 Unit tests (synthetic containers): `decode_slot_at` on a data slot
  returns 262144 RGBA8 bytes and the slot offset; on all-`0xFF` and all-`0x00`
  slots returns `SlotNoPayload` with the right class; range-checked caller is
  not bypassed.
- [ ] 2.6 Verify: `cargo test textures` green.

## Task 3 — `src/main.rs`: `--tshow` decodes one slot (≤150 authored lines, tests included)

- [ ] 3.1 `run_textures --tshow`: after the existing selector/range checks, call
  `textures::decode_slot_at`. On `Ok((offset, rgba))`, print the single
  `spec.md` stdout line, then `render::show_rgba8(256, 256, &rgba)`; map a render
  error to `error: <text>` + exit 1; return `ExitCode::SUCCESS` on window close.
- [ ] 3.2 Keep `error: {e}` + exit 1 for `SlotNoPayload`, `SlotPayloadLength`,
  `Io`, and header/geometry errors. No window on any error path.
- [ ] 3.3 Update the usage/help text: `--tshow <entry>` shows one slot's raw
  256 × 256 A1R5G5B5 image; drop the "compressed/UNKNOWN" wording.
- [ ] 3.4 Update the existing `--tshow` CLI tests: selector/range still exit 1;
  synthetic all-zero slots still exit 1 with the new zero-class message; add a
  synthetic data-slot test that asserts the decode helper's output length (not
  the window).
- [ ] 3.5 Verify: `cargo test` green; `cargo run -- --textures <one real .BIG>
  --tshow 4` shows the image, exit 0 on close (user playtest, read-only).

## Task 4 — README + playtest + legal gate (docs only)

- [ ] 4.1 Update the `.BIG` bullets in `README.md`: slot payloads are raw
  256 × 256 A1R5G5B5 (uncompressed); `--tshow` shows one slot; empty/zero slots
  stay loud.
- [ ] 4.2 Record the playtest note (file, slot, what was seen — recognizable
  image, upright, correct colours) in the commit/PR body.
- [ ] 4.3 Legal: `git status --short` shows only `src/*.rs` and `README.md`; no
  game bytes, install paths, or content dumps in the diff.
- [ ] 4.4 Verify: `cargo test` green; whitelist clean.

## Apply gate

- [ ] All four work units land as one PR from fresh `main`.
- [ ] `cargo test` green; `--tshow` on at least three real slots shows
  recognizable images (different containers), and empty/zero slots still exit 1.
- [ ] No decompressor, no new dependency, no game bytes; `--tlist`, header,
  geometry, and all other modes unchanged.
- [ ] No-go gate: if the playtest contradicts the raw-format model, stop,
  revert the claim, keep `--tshow` loud, and name the bounded Ghidra call-path
  probe as the next candidate. Do not guess a second format.

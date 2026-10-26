# Tasks: big-slots

Ordered work units. Each keeps its tests with its behavior and is independently reviewable. `test_command` for apply: `cargo test`.

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~330-390 authored (`textures.rs` ~150 add / ~10 del incl. tests; `main.rs` ~40 add / ~5 del incl. tests; README ~10) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (4 work-unit commits) |
| Delivery strategy | single-pr (cached) |
| Chain strategy | pending - not needed at this size |

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low
```

### Suggested Work Units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | Slot model, classification, geometry, errors | PR 1 | `cargo test textures` | N/A - pure/unit surface | Slot block in `src/textures.rs` + its tests |
| 2 | Whole-slot scan, report, single-slot lookup | PR 1 | `cargo test textures` | N/A - synthetic I/O surface | Scan/report block + its tests |
| 3 | CLI `--tlist` / `--tshow` contract | PR 1 | `cargo test cli_tests` | `cargo run -- --textures <one real .BIG> --tlist` (user playtest, read-only) | `run_textures` + CLI tests/fixtures |
| 4 | README truth-up + legal gate | PR 1 | `cargo test` | N/A - docs only | `.BIG` bullet in `README.md` |

## Task 1 - `src/textures.rs`: slot model, classification, geometry (<=150 authored lines, tests included)

- [x] 1.1 Add `SLOT_SIZE: u64 = 131072`, `SLOT_DATA_START: u64 = 4096`, `expected_len(count)`, `slot_offset(index)` (pure arithmetic).
- [x] 1.2 Add `SlotClass { Empty, Zero, Data }` with `Display` (`empty`/`zero`/`data`) and `classify_slot(&[u8])`: `all(0xFF)` -> `Empty`, else `all(0x00)` -> `Zero`, else `Data`; empty slice -> `Zero`.
- [x] 1.3 Add `validate_geometry(file_len, header)` rejecting anything except `4096 + count * 131072`; add `SlotGeometryMismatch { file_len, count, expected }` with the exact message from `spec.md`; call it in `describe` after `parse_header`.
- [x] 1.4 Clamp only the data probe offset to `min(DATA_PROBE_OFF, file_len - PROBE_LEN)` so a geometry-valid short container no longer fails on a probe; real containers keep 1 MiB.
- [x] 1.5 Add `SlotSelectorInvalid { selector, count }` and `SlotIndexOutOfRange { index, count }` variants with the exact messages; keep every existing variant and message.
- [x] 1.6 Unit tests (synthetic): offsets/expected lengths; classification of `0xFF`, `0x00`, mixed, `0xFF`-except-last-byte -> `Data`; geometry exact pass; one byte short and one byte long -> `SlotGeometryMismatch` with both lengths in the message.
- [x] 1.7 Verify: `cargo test` green; `cargo build` warning-free.

## Task 2 - `src/textures.rs`: whole-slot scan, report, single-slot lookup (<=150 authored lines, tests included)

- [x] 2.1 Add `SlotScan { classes }` with `count(class)` and `report()`: `addressing:` line, `slots (whole-slot scan; no sampling):`, `slot <i> offset <off> <class>` per index, footer `slots: <total> total, <empty> empty, <zero> zero, <data> data`.
- [x] 2.2 Add `scan_slots(path, header)`: one reusable 131072-byte buffer, seek to `SLOT_DATA_START`, read every slot whole, classify; short read -> existing `Io` error.
- [x] 2.3 Add `slot_class_at(path, header, index) -> (offset, class)`: seek to `slot_offset(index)`, read that slot only, classify.
- [x] 2.4 Add `SlotNotDecodable { index, offset, class }` with the three exact class-specific messages from `spec.md`.
- [x] 2.5 Unit tests (synthetic containers of exactly `4096 + count * 131072` bytes): one all-`0xFF`, one all-`0x00`, rest invented data; report line count, offsets arithmetic, footer totals; `slot_class_at` returns the same class; no-sampling test (last byte differs -> `Data`).
- [x] 2.6 Verify: `cargo test` green.

## Task 3 - `src/main.rs`: CLI contract (<=150 authored lines, tests included)

- [x] 3.1 `run_textures` `--tlist`: after `describe`, print `layout.summary()`, then `scan_slots(...).report()`, return `ExitCode::SUCCESS`.
- [x] 3.2 `run_textures` `--tshow`: after `describe`, parse the selector as `usize`; non-integer -> `SlotSelectorInvalid`; `>= count` -> `SlotIndexOutOfRange`; else `slot_class_at` + `SlotNotDecodable`; all stderr `error: ...`, exit 1, no window. Header/geometry errors still win.
- [x] 3.3 Update existing synthetic fixtures (`synthetic_big`, `synthetic_textures`) to exact geometry lengths; keep every prior header/usage exit-code test and add geometry-mismatch, `--tlist` class listing, `--tshow` selector/range/class tests.
- [x] 3.4 Verify: `cargo test` green; `cargo run -- --textures <one real .BIG> --tlist` lists every slot with class + footer, exit 0 (user playtest, read-only); `--tshow 0` and `--tshow <count>` are loud, exit 1.

## Task 4 - README + legal gate (docs only)

- [x] 4.1 Update the `.BIG` bullets in `README.md`: geometry validated, `--tlist` lists index/offset/class + footer, `--tshow` loud with class and UNKNOWN decompressor.
- [x] 4.2 Legal: `git status --short` shows only `src/*.rs` and `README.md`; no game bytes, install paths, or content dumps in the diff.
- [x] 4.3 Verify: `cargo test` green; whitelist clean.

## Apply gate

- [x] All four work units land as one PR from fresh `main`.
- [x] `cargo test` green; `--tlist` outcome recorded for at least one real container; `--tshow` loud behavior recorded.
- [x] Geometry mismatch is distinct and never clamped; classification is whole-slot (no sampling); `--tshow` still never decodes.

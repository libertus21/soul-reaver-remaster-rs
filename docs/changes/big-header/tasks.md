# Tasks: big-header

Ordered work units. Each is independently committable, keeps its tests with its behavior, and states its verify step. `test_command` for apply: `cargo test`.

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~190-230 authored (`textures.rs` ~95 add / ~30 del incl. tests; `main.rs` ~55 add / ~15 del incl. tests; README ~15) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (3 work-unit commits) |
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
| 1 | Header parse + validation + summary | PR 1 | `cargo test textures` | N/A — pure/unit surface, no CLI change yet | `src/textures.rs` header block + its tests |
| 2 | CLI `--tlist` / `--tshow` contract | PR 1 | `cargo test cli_tests` | `cargo run -- --textures <one real .BIG> --tlist` (user playtest, read-only) | `run_textures` + its CLI tests in `src/main.rs` |
| 3 | README truth-up + legal gate | PR 1 | `cargo test` | N/A — docs only | the `textures-big` bullet and "What's missing" entry in `README.md` |

## Task 1 — `src/textures.rs`: header model, validation, summary (≤150 authored lines, tests included)

- [ ] Add `HEADER_SIZE: u16 = 512` and `BigHeader { header_size: u16, entry_count: u16 }` (both `Copy`, `PartialEq`).
- [ ] Add `parse_header(prefix: [u8; 4], file_len: u64) -> Result<BigHeader, TexturesError>`: LE `u16` at `0..2` and `2..4`; reject `file_len < 512`, header size != 512, count outside `1..=32768`.
- [ ] Add `TruncatedHeader { len }`, `WrongHeaderSize { found }`, `ImplausibleCount { count }` variants with the distinct messages from `spec.md`; keep existing variants.
- [ ] Replace `TexturesLayout::count_word_sane` with `header: BigHeader`; make `describe` read 4 bytes (short read → truncated) and keep the existing windows + table hunt unchanged.
- [ ] Update `summary()`: `BIG header: 512-byte header, <count> entries` first, then size class, window classes, `entry table: UNKNOWN (location not established; no stride inferred)`; never print exact byte lengths.
- [ ] Unit tests on synthetic bytes only: valid parse (including count 3), wrong size 256, truncated 100 and 2 bytes, count `0` and `32769`, summary content + no-exact-length assertion; replace the old insane-count-was-success test.
- [ ] Verify: `cargo test` green; `cargo build` warning-free.

## Task 2 — `src/main.rs`: CLI contract for `--tlist` and `--tshow` (≤150 authored lines, tests included)

- [ ] `run_textures`: valid header + `--tlist` → print `summary()`, return `ExitCode::SUCCESS`. Invalid header → existing `error: {e}` path, exit 1, no summary.
- [ ] Keep `--tshow` loud: after a valid `describe`, print `TexturesError::BadSelector(sel)` and exit 1; no window, no entry-data read.
- [ ] Remove the duplicated "no entry table identified … next candidate" stderr block and the now-unused `NEXT_CANDIDATE`; keep usage errors unchanged.
- [ ] CLI tests: synthetic temp files where `--tlist` exits 0 (valid), 1 (truncated, wrong size, implausible count); `--tshow` exits 1 on valid and invalid; missing path/verb still exits 1.
- [ ] Verify: `cargo test` green; `--tlist` on one real `.BIG` reports 512 + its count and exits 0 (user playtest); `--tshow 0` exits 1 loudly.

## Task 3 — docs + legal gate (docs only)

- [ ] Update the `textures-big` bullet in `README.md`: four `.BIG` containers share a verified 512-byte header + entry count; `--tlist` reports it and exits 0; entry table still UNKNOWN; `--tshow` still loud.
- [ ] Update the "What's missing" TEXTURES.BIG entry: header parsed, index location still open; name the four containers without install paths.
- [ ] Legal: `git status --short` shows only `src/*.rs` and `README.md`; no game bytes, install paths, or extracted data in commits.
- [ ] Verify: `cargo test` green; `git status --short` whitelist clean.

## Apply gate

- [ ] All three work units land as one PR from fresh `main`.
- [ ] `cargo test` green, `--tlist` header outcome recorded for at least one container, `--tshow` loud behavior recorded, legal whitelist clean.
- [ ] Entry table still UNKNOWN in code, output, and docs — no stride, no index offset, no entry lines.

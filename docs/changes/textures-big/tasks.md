# Tasks: textures-big

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

## Task 1 — `src/textures.rs`: region classifier + `describe` + `--tlist` summary arm (with unit tests)

- [ ] Add `RegionClass` (`ZeroFill` / `FfFill` / `Data` / `DdsMagic`) + `classify_window(bytes) -> RegionClass` (pure, no I/O): all-zeros, all-`0xFF`, `DDS ` prefix, else data. Add `describe(path) -> Result<TexturesLayout, TexturesError>` doing bounded read-only reads (header word, first-4K census samples, fill windows, sparse data windows — every offset+length recorded in the apply PR per the design P1–P6 ledger) and returning shapes only (file size, region classes, table status). New `TexturesError` (`NotTextures` / `BadEntry` / `BadSelector` / `NotDds` / `Io`) — spec R-scenarios gate message + exit 1, not the type name.
- [ ] Wire `--textures <path> --tlist`: table identified → entry lines to stdout, exit 0 (format fixed in Task 2); no table → layout summary (counts/codes/field-shapes only) to stdout + loud stderr naming the file and the `2\BIGFILE.DAT` next candidate + exit 1. Existing file/dir/`--bigfile` arms UNTOUCHED (spec R1).
- [ ] Unit tests on SYNTHETIC bytes only (hand-built windows in temp dirs via `tempfile`; no game files): each `RegionClass` from crafted windows; count-word rejection (tens-of-millions word → `NotTextures`); `--tlist` no-table arm exits 1 with the candidate named.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — bounded table-hunt + `parse_table` + range-checked `read_entry` (with unit tests)

- [ ] Table-hunt at apply (read-only, ≤1 KB across ≤5 new windows, every offset+length documented in the apply PR): file-tail 1 KB (index-at-end hypothesis) + first data window past the fill region. FOUND → add `parse_table` (fields per observed layout, nothing invented; opaque `u64` ranges, `offset + length <= file_len` overflow-safe) + `read_entry` (pre-validated single bounded read into memory, never writes). NOT FOUND → record the hunt windows + negative result in the apply PR body, keep the Task-1 loud arms as the specified no-table behavior (spec R2/R3) — the task still completes, it completes as a documented no-go.
- [ ] `--tlist` table-found arm: `index identifier offset length` per line (labels per identified layout), exit 0; retail values stay on stdout, never in docs/commits.
- [ ] Unit tests on SYNTHETIC bytes only: fake table buffer parses to exact fields; truncated table rejected; entry range past EOF rejected; `offset + length` overflow rejected. (No-table outcome needs no parser tests — the Task-1 rejection tests already gate it.)
- [ ] Verify: `cargo test` passes; apply PR body records hunt windows + verdict (table N or confirmed no-table); `git status --short` whitelist check.

## Task 3 — `--tshow` DDS path + README + playtest evidence + legal gate

- [ ] Wire `--textures <path> --tshow <entry>`: resolve selector (decimal index OR the stable identifier printed by `--tlist`; unknown → loud + exit 1) → `read_entry` → require `DDS ` magic via EXISTING `has_dds_magic` → EXISTING `decode_memory` → EXISTING `show_texture` tail. Non-DDS entry, unknown selector, out-of-range index, or no-table → loud stderr + exit 1 (spec R3/R5). Zero changes to `dds.rs` / `render.rs` / `gallery.rs` / `bigfile.rs`.
- [ ] Update `README.md` "What works" for `--tlist`/`--tshow` (observed layout verdict, conditional listing, DDS-only rule, in-memory guarantee, next candidate) without adding game paths/hashes/offsets.
- [ ] Playtest (acceptance, user): `--tlist` on the real TEXTURES.BIG — record N or confirmed no-table + exit code; `--tshow` one real DDS entry when one exists — compare vs loose-file render of the same texture if identifiable, else vs reference viewer, record entry + comparison + result + date in the commit/PR body per archive-report convention. Corrupt-input run (truncated copy in a temp dir — NEVER mutate the game dir) confirms loud exit 1.
- [ ] Legal: `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes; no extracted bytes, no game paths/hashes/offsets/content dumps committed (list output stays on stdout, never in docs).
- [ ] Verify: `cargo test` passes; playtest recorded; `git status --short` clean of non-whitelisted files.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~230-340 (Task 1 ~90-120 incl. tests, Task 2 ~80-130 incl. tests, Task 3 ~60-90 incl. README; no generated files; `Cargo.lock` untouched — zero new crates) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~340 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main (chain strategy stays deferred: chaining is not needed) |
| Work-unit mapping | Task 1 -> commit 1 (classifier + loud arms + tests), Task 2 -> commit 2 (table-hunt verdict + parser-or-no-go + tests), Task 3 -> commit 3 (`--tshow` + README + playtest evidence; tests travel with their behavior per work-unit-commits) |
| Rebase point for apply | Updated `main` at `5f88e85` (merge of PR #8 bigfile-index); this plan branch carries docs only, so apply branches fresh from `main` |

## Apply gate

- [ ] `cargo test` green, `--tlist` outcome recorded (N or confirmed no-table) + `--tshow` playtest note written when applicable (entry + comparison + result), `git status --short` clean of non-whitelisted files, single PR ≤400 authored lines.

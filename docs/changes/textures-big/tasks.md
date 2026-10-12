# Tasks: textures-big — LANDED (documented no-go, PR #10 MERGED)

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

Landed reality (reconciled at archive, 2026-10-12): Task 1 landed as the classifier + `--tlist` no-table arm (`ea70073`); Task 2 landed as the bounded table-hunt with a confirmed no-table verdict, keeping the Task-1 loud arms (`50d6986`); Task 3 landed as the `--tshow` loud no-go arm + README + playtest evidence (`e4ee7a7`). No table was found, so the table-found arms and parser tests were correctly N/A — the slice completed as a documented no-go per the spec R2/R3 no-table arms.

## Task 1 — `src/textures.rs`: region classifier + `describe` + `--tlist` summary arm (with unit tests) — [x] LANDED (`ea70073`)

- [x] Add `RegionClass` (`ZeroFill` / `FfFill` / `Data` / `DdsMagic`) + `classify_window(bytes) -> RegionClass` (pure, no I/O): all-zeros, all-`0xFF`, `DDS ` prefix, else data. Add `describe(path) -> Result<TexturesLayout, TexturesError>` doing bounded read-only reads (header word, first-4K census samples, fill windows, sparse data windows — every offset+length recorded in the apply PR per the design P1–P6 ledger) and returning shapes only (file size, region classes, table status). New `TexturesError` (`NotTextures` / `BadEntry` / `BadSelector` / `NotDds` / `Io`) — spec R-scenarios gate message + exit 1, not the type name.
- [x] Wire `--textures <path> --tlist`: table identified → entry lines to stdout, exit 0 (format fixed in Task 2); no table → layout summary (counts/codes/field-shapes only) to stdout + loud stderr naming the file and the `2\BIGFILE.DAT` next candidate + exit 1. Existing file/dir/`--bigfile` arms UNTOUCHED (spec R1).
- [x] Unit tests on SYNTHETIC bytes only (hand-built windows in temp dirs via `tempfile`; no game files): each `RegionClass` from crafted windows; count-word rejection (tens-of-millions word → `NotTextures`); `--tlist` no-table arm exits 1 with the candidate named.
- [x] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — bounded table-hunt + `parse_table` + range-checked `read_entry` (with unit tests) — [x] LANDED as documented no-go (`50d6986`)

- [x] Table-hunt at apply (read-only, ≤1 KB across ≤5 new windows, every offset+length documented in the apply PR): file-tail 1 KB (index-at-end hypothesis) + first data window past the fill region. LANDED as NOT FOUND → hunt windows + negative result recorded in the apply PR body, Task-1 loud arms kept as the specified no-table behavior (spec R2/R3) — the task completed as a documented no-go.
- [ ] `--tlist` table-found arm: `index identifier offset length` per line (labels per identified layout), exit 0; retail values stay on stdout, never in docs/commits. — N/A (no table found; correctly skipped).
- [ ] Unit tests on SYNTHETIC bytes only: fake table buffer parses to exact fields; truncated table rejected; entry range past EOF rejected; `offset + length` overflow rejected. — N/A (no-table outcome; the Task-1 rejection tests already gate it).
- [x] Verify: `cargo test` passes; apply PR body records hunt windows + verdict (confirmed no-table); `git status --short` whitelist check.

## Task 3 — `--tshow` DDS path + README + playtest evidence + legal gate — [x] LANDED as loud no-go arm (`e4ee7a7`)

- [x] Wire `--textures <path> --tshow <entry>`: LANDED as the loud no-table arm (no table → loud stderr naming the file + next candidate + exit 1, no window). Zero changes to `dds.rs` / `render.rs` / `gallery.rs` / `bigfile.rs`.
- [x] Update `README.md` "What works" for `--tlist`/`--tshow` (observed layout verdict, conditional listing, DDS-only rule, in-memory guarantee, next candidate) without adding game paths/hashes/offsets.
- [x] Playtest (acceptance, user): `--tlist` on the real TEXTURES.BIG — confirmed no-table + exit 1 (twice); `--tshow 0` — loud no-table error + exit 1, no window; single-file regression — stone door renders. Recorded in the archive report.
- [x] Legal: `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes; no extracted bytes, no game paths/hashes/offsets/content dumps committed (list output stays on stdout, never in docs).
- [x] Verify: `cargo test` passes (49/49 at archive); playtest recorded; `git status --short` clean of non-whitelisted files.

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

## Apply gate — [x] MET (single PR #10, documented no-go)

- [x] `cargo test` green (49/49 at archive), `--tlist` confirmed-no-table outcome recorded + `--tshow` loud no-go playtest note written, `git status --short` clean of non-whitelisted files, single PR to main.

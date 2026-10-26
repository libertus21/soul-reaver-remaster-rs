# Archive: big-header — DONE (header verified, entry index still UNKNOWN)

The four SR1 `.BIG` containers now parse and validate a shared 512-byte header
with a `u16` entry count, and `--tlist` exits 0 on a valid header instead of
exiting 1 on a no-go. The entry table stays explicitly UNKNOWN: no index
location, no stride, no entry lines, and `--tshow` stays a loud exit 1. PR #25 is
merged into `main`; nothing outside `src/textures.rs`, `src/main.rs`, `README.md`,
and the change docs was touched.

## Quick path

1. `git checkout main && git pull` — HEAD is `6516726` (merge of PR #25).
2. `cargo run -- --textures <any .BIG from your own install> --tlist` — prints
   the header report and exits 0.
3. `cargo test` — 111/111 green on synthetic bytes only.

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| `--tlist` TEXTURES.BIG | `BIG header: 512-byte header, 1287 entries` plus the size class, the three window classes, and `entry table: UNKNOWN (location not established; no stride inferred)`; exit 0. |
| `--tlist` MOVIES.BIG | Same report shape, `29 entries`; exit 0. |
| `--tshow 0` | `no entry table identified for selector '0': index location UNKNOWN (no stride inferred)`; exit 1. |
| Regression suite | `cargo test` 111 passed / 0 failed (re-verified on merged `main`). |
| Build hygiene | Zero warnings after a forced rebuild (`cargo clean` + `cargo build --all-targets`). |
| Legal / repo | `git status` clean, whitelist-only files committed, repository private. |

Planning-probe counts for the other two containers were recorded as 268
(`BONUS.BIG`) and 178 (`E3.BIG`); only TEXTURES.BIG and MOVIES.BIG were
re-confirmed in this playtest pass.

## Details

| Topic | Outcome |
|---|---|
| PR | [#25 — feat(big-header): parse and validate the .BIG header](https://github.com/libertus21/soul-reaver-remaster-rs/pull/25), state MERGED (`mergedAt` 2026-10-26T02:45:14Z), base `main`, head `feature/big-header`, 7 files, +536/−76. |
| Commits on `main` | `00fef43` plan bundle (proposal/spec/design/tasks) → `3fd03c4` parse + validate the header words → `5c0d203` `--tlist` exits 0, `--tshow` stays loud → `1c8e5a9` README truth-up → `0f97437` mark work units complete → `6516726` PR #25 merge. |
| Verified contract | Bytes `0..2` = header size (`u16` LE, MUST be 512); bytes `2..4` = entry count (`u16` LE, MUST be in `1..=32768`). Read budget: 4 bytes. Bytes `4..512` are observed but NOT validated; the pre-existing bounded probe windows and table hunt are unchanged. |
| CLI behavior | `--tlist` on a valid header prints `BIG header: 512-byte header, <count> entries`, the size class, the header/fill/data window classes, and the UNKNOWN entry-table line; exit 0. Three distinct failures keep exit 1: truncated file, wrong header size, implausible count. `--tshow <sel>` is always a loud exit 1 (no window, no entry-data read). All other modes are byte-for-byte unchanged. |
| Delivered in code | `src/textures.rs`: `HEADER_SIZE`, `BigHeader`, `parse_header`, three new error variants (`TruncatedHeader`, `WrongHeaderSize`, `ImplausibleCount`), `TexturesLayout.header` replacing the old count-word sanity flag, updated `summary()`. `src/main.rs`: `--tlist` success path, loud `--tshow`, removal of the duplicated no-table block and `NEXT_CANDIDATE`, CLI exit-code tests. |
| Deliberately not built | Any index offset, stride, entry-field inference, entry listing, entry decoding, or new probe of the header tail. `bigfile.rs` untouched (this is not the SR1PC layout). |
| Warnings (recorded) | (1) Review forecast under-estimated: 352 authored lines outside the 260-line planning bundle (raw diff +536/−76 over 7 files) against a ~190–230 planned range. (2) `tasks.md` checkboxes were stale at the end of apply; fixed in follow-up commit `0f97437` rather than amended. |
| Branch cleanup | Deleted only branches verified fully contained in `main` (see the report below the table). `feature/big-header-plan` was NOT contained (its tip predates the rebased planning commit) and was left in place. |
| Follow-ups (next slice) | Locate the entry index: its byte location and per-entry stride are still UNKNOWN, so no entry can be listed or shown. The other containers need no new probe for the header contract, but the index hunt must be re-run against each container it claims to cover. No audio/gameplay scope is opened here. |
| Legal | Behavior and format facts only. No game bytes, no absolute game paths, no extracted data, no offsets, no hashes, and no content dumps in any artifact or commit. Tests use synthetic bytes in temp dirs; the real containers are read-only playtest inputs. |

## Checklist

- [x] PR #25 MERGED and `main` contains all slice commits (HEAD `6516726`)
- [x] Playtest evidence recorded verbatim (`--tlist` on two containers, `--tshow 0`, 111/111, zero warnings)
- [x] Header contract stated as parsed vs UNKNOWN, with the read budget and the unvalidated tail called out
- [x] Contained branches deleted after containment checks; uncontained branches kept with the reason stated
- [x] README "What works" carries the verified contract and "What's missing" keeps the index hunt as the next item
- [x] No game bytes, absolute game paths, offsets, or hashes added by this archive

## Next step

Open the entry-index slice (`bigfile-index`-style discipline, but for the `.BIG`
layout): establish WHERE the per-container index begins and WHAT its stride is,
using read-only probes and synthetic tests, then let `--tlist` list entries and
`--tshow` resolve one. Until that lands, this slice's exit-0 contract means
"header verified" and nothing more.

# Tasks: bigfile-index

Ordered work units. Each is independently committable, each keeps tests/docs with the behavior, each states its verify step. `test_command` for the apply phase: `cargo test`.

## Task 1 — `src/bigfile.rs`: SR1PC index parser + range-checked entry reader (with unit tests)

- [ ] Add `BigfileEntry { name_hash: u32, offset: u64, length: u64 }` + `parse_index(bytes, file_len) -> Result<Vec<BigfileEntry>, BigfileError>`: u32 LE count at offset 0 (reject `> 32768`, reject index overrunning `file_len`), then 16-byte LE entries `[hash, length, offset]` with per-entry `offset + length <= file_len` in u64 arithmetic (overflow-safe). 4th dword read and ignored (design UNKNOWN rule — hex-carried, never interpreted).
- [ ] Add `read_entry(path, entry, file_len) -> Result<Vec<u8>, BigfileError>`: pre-validate range, single bounded read into memory; never writes. New `BigfileError` variants (`NotBigfile` / `BadEntry` / `Io`) or `ViewerError` extension — apply picks the smaller diff; spec R-scenarios gate message + exit 1, not the type name.
- [ ] Unit tests on SYNTHETIC bytes only (hand-built fake bigfiles in temp dirs via `tempfile`; no game files): 1-entry + multi-entry parse with exact field asserts; count-too-large rejected; truncated index rejected; entry range past EOF rejected; `offset + length` u32-overflow rejected.
- [ ] Verify: `cargo test` passes; `git status --short` shows only intended files.

## Task 2 — `dds.rs`: in-memory decode gate + `--show` entry path (with unit tests)

- [ ] Add `pub fn decode_memory(bytes: &[u8]) -> Result<LoadedTexture, ViewerError>` sharing `load_texture`'s validation (magic + DX10/`dxgiFormat` + BC7-shape checks, `dxgiFormat` 99 stays loud); refactor `load_texture` onto it with the smaller diff (single-file behavior byte-for-byte unchanged).
- [ ] Wire `--show` selection: decimal index OR 8-hex-digit hash (case-insensitive, optional `0x`) matched against `parse_index` output; resolve → `read_entry` → require `DDS ` magic → `decode_memory` → EXISTING render tail (`show_rgba8`-family). Non-DDS entry, unknown selector, or out-of-range index → loud stderr + exit 1 (spec R3/R5).
- [ ] Unit tests on SYNTHETIC bytes only: `decode_memory` accepts minimal synthetic DDS bytes and rejects non-DDS magic; selector parsing (index vs hash vs garbage) without touching game files or a GPU.
- [ ] Verify: `cargo test` passes; `git status --short` whitelist check.

## Task 3 — `main.rs`: `--bigfile` dispatch + `--list` + playtest evidence + README

- [ ] Add `--bigfile <path> --list | --show <entry>` dispatch: `--list` prints `index hash offset length` per line to stdout, exit 0; flag errors (missing path/verb/selector) print usage to stderr + nonzero exit. Existing file/dir/usage arms UNTOUCHED (spec R1/R6 — single-file and gallery modes byte-for-byte unchanged).
- [ ] Update `README.md` "What works" for `--list`/`--show` (selector forms, DDS-only rule, in-memory guarantee, modes unchanged) + credit line for the Soul Spiral spec source (GPL-3.0-only, facts-only reuse — no THIRD-PARTY-NOTICES.md entry; see design).
- [ ] Playtest (acceptance, user): `--list` on the real BIGFILE.DAT — record authoritative N + exit code; `--show` one real DDS entry — compare vs loose-file render of the same texture if identifiable, else vs reference viewer, record entry + comparison + result + date in the commit/PR body per archive-report convention. Corrupt-input run (e.g. truncated copy in a temp dir — NEVER mutate the game dir; e.g. `--show` on a known non-DDS entry) confirms loud exit 1.
- [ ] Legal: `git status --short` shows only `*.rs`/`*.md`/`Cargo.*` changes; no extracted bytes, no game paths/hashes/offsets committed (list output stays on stdout, never in docs).
- [ ] Verify: `cargo test` passes; playtest recorded; `git status --short` clean of non-whitelisted files.

## Review Workload Forecast

| Item | Value |
|---|---|
| Estimated authored changed lines, total | ~270-360 (Task 1 ~100-130 incl. tests, Task 2 ~110-140 incl. tests, Task 3 ~60-90 incl. README; no generated files; `Cargo.lock` untouched — zero new crates) |
| Per-task ceiling | Each task ≤150 authored lines, each committable alone |
| Chained PRs recommended | No — total fits one reviewable slice |
| 400-line budget risk | Low (forecast ~360 max vs 400 budget) |
| Decision needed before apply | No — proceed as SINGLE PR to main (chain strategy stays deferred: chaining is not needed) |
| Work-unit mapping | Task 1 -> commit 1, Task 2 -> commit 2, Task 3 -> commit 3 + playtest evidence (tests travel with their behavior per work-unit-commits) |
| Rebase point for apply | Updated `main` at `530ec61` (gallery archived); this plan branch carries docs only, so apply branches fresh from `main` |

## Apply gate

- [ ] `cargo test` green, `--list` N recorded + `--show` playtest note written (entry + comparison + result), `git status --short` clean of non-whitelisted files, single PR ≤400 authored lines.

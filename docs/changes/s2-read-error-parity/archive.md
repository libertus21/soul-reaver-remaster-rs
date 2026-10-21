# Archive: s2-read-error-parity — DONE (docs-only)

Post-open read-error parity for the bigfile loader is closed as a
docs-only slice: the bounded call-edge trace (2 of 2 probe passes used)
found zero caller-to-callee edges into either error half, so no row
resolved to PROVEN, no production message was aligned, and all five
post-open failure rows stay HELD with their existing prefixes pinned by
new regression tests. Termination behavior is unchanged. PR #17 is
merged into `main`; contained slice branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `8d5874a` (merge of PR #17).
2. `cargo test` — 88/88 green on synthetic bytes (user-confirmed).
3. `cargo run -- --bigfile <absent-path> --list` (your own path at
   runtime) — stderr starts with `Can't load big file named "<path>"`,
   exit 1 (unchanged; S1 open-stage row).

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| Missing file | `Can't load big file named "<path>" (Spanish OS detail, os error 2)`, exit 1 (user-confirmed). |
| Regression suite | `cargo test` 88 passed / 0 failed (user-confirmed). |
| Scope hold | DOCS-ONLY by plan box rule — static call-edge tracing (2 passes) found ZERO caller-to-callee edges to the read/error halves; all 5 post-open rows recorded HELD with prefixes pinned by new tests; termination unchanged. |

## Details

| Topic | Outcome |
|---|---|
| PR | [#17 — docs+test(s2-read-error-parity): edge trace holds all post-open rows, docs-only slice](https://github.com/libertus21/soul-reaver-remaster-rs/pull/17), state MERGED (`mergedAt` 2026-10-14T04:21:26Z), base `main`, head `feature/s2-read-error-parity`, 7 files, +359/-13. |
| Commits on `main` | `e2dbbb9` plan proposal/spec/design/tasks · `a768835` record call-edge trace addendum, all post-open rows held · `c59a40b` record per-row held verdicts, narrow slice to docs-only · `e1a6835` pin held-row prefixes and exit-1 contract on synthetic bytes · `8d5874a` PR #17 merge. |
| Edge trace result | Re-pin reproduced S1 exactly (`function_count=5243`, `matched_string_count=3`). Pass 1 (depth-1): `direct_edge_count=0`, both `overlap_count=0`. Pass 2 (depth-2): `read-error-half` has 3 single-callee wrapper callers (3-site / 0-site / 0-site shapes), none reaching an IO import (`EDGE2=0`); `open-error-half` has 1 caller (the second `bigfile.dat` referrer) — the only observed loader edge, already covered by S1's open-stage row. Nothing resolved to PROVEN. |
| Scope kept | SR1 only. Zero production-line changes: `BigfileError` Display prefixes untouched (no PROVEN row). Code diff is tests only — `src/bigfile.rs` `mod tests` hold-pin assertions and one `src/main.rs` `mod cli_tests` exit-1 assertion. Five post-open rows HELD (truncated count/bad count, truncated index table, entry past EOF, seek failure, short read); `BadSelector`/`NotDds` unchanged; loud-stderr + exit-1 contract untouched everywhere. Termination stays UNKNOWN (static headless only, no new observable). |
| Branch cleanup | `feature/s2-read-error-parity` and `feature/s2-read-error-parity-plan`: verified fully contained in `main`, deleted locally and from origin. Contained leftover archive branch deleted the same way (local + origin): `docs/archive-s1`. Explicitly kept (out of scope / not contained): `origin/dev`, `origin/dev-3.0`, `origin/Visor-flesh-3d-galeria` (long-lived/other lines); `origin/dev-2-0` and `origin/feature/texture-gallery-plan` (NOT contained in `main`). |
| Follow-ups (TBD by user) | Dynamic-trace slice candidate for the still-HELD rows — static call edges resolve none; dynamic tracing is the only remaining route to the read/error halves. S3 launcher-to-`run` default-archive linkage candidate (launcher handshake slice). Neither is scoped by this archive. |
| Legal | Behavior notes only; no listings, no Ghidra DBs, no game bytes/paths committed by this slice. Evidence uses message prefixes, counts, and exit codes only. No DRM encounters to report (none observed on this line). |

## Checklist

- [x] PR #17 MERGED and `main` contains all slice commits
- [x] Playtest evidence recorded verbatim (missing-file prefix + exit 1; 88/88 suite; docs-only scope hold)
- [x] Contained slice branches deleted after containment check; uncontained / out-of-scope branches explicitly kept with reason
- [x] README "What works" gains the S2 line; "What's missing" keeps the next binary slice open (S3 / dynamic trace TBD by user)
- [x] No game bytes, paths, hashes, or offsets added by this archive

## Next step

User picks the next binary slice: a dynamic-trace candidate to resolve the still-HELD post-open rows, or S3 (launcher handshake) — each with its own proposal → spec → tasks → code → playtest cycle under the same slice discipline.

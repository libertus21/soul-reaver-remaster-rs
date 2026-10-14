# Archive: s1-bigfile-loader — DONE

Open-stage error parity with the original SR1 loader is complete and playtest-accepted: a missing/unopenable archive now fails with the original loader's load-failure prefix, the five unconfirmed failure rows correctly keep their existing messages, and termination behavior is unchanged. PR #15 is merged into `main`; contained slice branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `cfcdac3` (merge of PR #15).
2. `cargo test` — 86/86 green on synthetic bytes (user-confirmed, incl. gallery + mesh suites).
3. `cargo run -- --bigfile <absent-path> --list` (your own path at runtime) — stderr starts with `Can't load big file named "<path>"`, exit 1.

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| Missing file | `Can't load big file named "<path>" (<Spanish OS detail, os error 2>)`, exit 1 (user-confirmed verbatim; prefix is the contract, suffix locale-dependent). |
| Regression suite | `cargo test` 86/86 green (user-confirmed, incl. gallery + mesh suites). |
| Scope hold | Open-stage row ALIGNED; five failure rows correctly HELD unknown; termination behavior unchanged. |

## Details

| Topic | Outcome |
|---|---|
| PR | [#15 — feat(bigfile): align open-stage errors with the original load-failure prefix](https://github.com/libertus21/soul-reaver-remaster-rs/pull/15), state MERGED (`mergedAt` 2026-10-14T04:06:00Z), base `main`, head `feature/s1-bigfile-loader` |
| Commits on `main` | `f433139` plan proposal/spec/design/tasks · `8cdc9f7` plan docs onto feature branch · `5720b54` re-pin loader path by string xref + behavior notes · `b14dbe0` confirm parity mapping, exclude unconfirmed rows from code · `e14aed2` open-stage failures report the load-failure prefix (`BigfileError::OpenFailed`, pre-open `metadata`/`open` only) · `ebe221f` regression tests pinning the prefix + unknown-mapping holds on synthetic bytes · `cfcdac3` PR #15 merge |
| Branch cleanup | `feature/s1-bigfile-loader` and `feature/s1-bigfile-loader-plan`: verified fully contained in `main`, deleted locally and from origin. Other contained slice leftovers deleted the same way (local + origin): `feature/ghidra-first-look`, `feature/ghidra-first-look-plan`, `feature/models-3d`, `feature/models-3d-plan`, `docs/archive-textures-big` — all merged, no commits lost. Explicitly kept (out of scope for this cleanup): `origin/dev`, `origin/dev-3.0`, `origin/Visor-flesh-3d-galeria` (long-lived/other lines, not slice leftovers); `origin/dev-2-0` and `origin/feature/texture-gallery-plan` (NOT contained in `main` — kept). |
| Scope kept | SR1 only. Exactly one row ALIGNED: pre-open failures (`metadata`/`open` in `load_index`/`read_entry`) report `Can't load big file named "{path}" ({detail})`. Five rows HELD unknown per spec R3 (truncated count/bad count, truncated table, entry past EOF, seek failure, short read — existing texts pinned by tests). `BadSelector`/`NotDds` unchanged (no original counterpart). Loud-stderr + exit-1 contract untouched everywhere. |
| Follow-ups (TBD by user) | Post-open message parity needs call-edge tracing into the read-error half = S2 candidate (discovery/selection table slice). Launcher-to-`run` default-archive linkage = S3 candidate (launcher handshake slice). Neither is scoped by this archive. |
| Legal | Behavior notes only; no listings, no Ghidra DBs, no game bytes/paths committed by this slice. Evidence uses message prefixes, counts, and exit codes only. No DRM encounters to report (none observed across slices). |

## Checklist

- [x] PR #15 MERGED and `main` contains all slice commits
- [x] Playtest evidence recorded verbatim (missing-file prefix + exit 1; 86/86 suite; scope hold)
- [x] Contained slice branches deleted after containment check; uncontained / out-of-scope branches explicitly kept with reason
- [x] README "What works" gains the S1 parity line; "What's missing" keeps the next binary slice open (S2/S3 TBD by user)
- [x] No game bytes, paths, hashes, or offsets added by this archive

## Next step

User picks the next binary slice: S2 (post-open parity via call-edge tracing) or S3 (launcher handshake), each with its own proposal → spec → tasks → code → playtest cycle under the same slice discipline.

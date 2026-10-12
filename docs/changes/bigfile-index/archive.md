# Archive: bigfile-index — DONE

The `BIGFILE.DAT` milestone is complete and playtest-accepted: `cargo run -- --bigfile <BIGFILE.DAT> --list` prints the SR1PC archive index and exits 0, and `cargo run -- --bigfile <BIGFILE.DAT> --show <entry>` reads one entry in memory and shows it when it is a DDS texture, failing loud otherwise. PR #8 is merged into `main`; contained feature branches are deleted.

## Quick path

1. `git checkout main && git pull` — HEAD is `5f88e85` (merge of PR #8).
2. `cargo test` — 43/43 green on synthetic bytes; `cargo build` shows zero warnings.
3. `cargo run -- --bigfile <BIGFILE.DAT> --list` (your own install path at runtime) — prints the entry index, exits 0.

## Playtest evidence

| Item | Value |
|---|---|
| `--list` on the real BIGFILE.DAT | Exit 0, N=1325 entries (authoritative, user-confirmed). |
| `--show 0` on the real BIGFILE.DAT | Loud `not a DDS entry` error, exit 1, no window (per spec — this archive holds PS1-era data, 0 DDS among 1325 entries; data fact, not failure). |
| Headless gates from verify | 43/43 tests, zero warnings, legal clean. |
| Reporting rule learned | Container listings stay on the user machine/chat; repo artifacts carry counts and exit codes only. |

## Details

| Topic | Outcome |
|---|---|
| PR | [#8 — feat(bigfile): index BIGFILE.DAT and show one archived DDS from memory](https://github.com/libertus21/soul-reaver-remaster-rs/pull/8), state MERGED (`mergedAt` 2026-10-12T01:27:12Z), base `main`, head `feature/bigfile-index` |
| Commits on `main` | `a912945` SR1PC index parser + range-checked entry reader · `fc014e7` in-memory decode entry point sharing `load_texture` validation · `8043133` `--bigfile` dispatch reusing the single-file render tail · `9f310a7` plan docs merge (proposal/spec/design/tasks) · `5f88e85` PR #8 merge |
| Branch cleanup | `feature/bigfile-index`: verified fully contained in `main` (`git merge-base --is-ancestor`), deleted locally and from origin. `feature/bigfile-index-plan`: verified fully contained in `main`, deleted locally and from origin. Prior-slice leftovers explicitly kept: `feature/texture-gallery-plan` is NOT contained in `main` (local + remote kept); `docs/archive-gallery` (PR #7) is contained but not obviously superseded by this slice — kept, not assumed; `docs/archive-bc7-slices` and `origin/dev` likewise kept (out of scope for this cleanup). |
| Scope kept | SR1PC layout only (u32 LE count, 4×u32 LE entries, raw byte offsets); `--list` prints index lines to stdout; `--show` accepts decimal list index or stored name hash (case-insensitive, optional `0x`); DDS entries only — every other archived type is listed, never decoded; entry bytes live in process memory only, never written to disk or repo; single-file and gallery modes byte-for-byte unchanged |
| Deferred (unchanged) | sRGB correctness pass, resize handling, full mipmap-chain UI; hash-name reverse lookup table; archive browser UI; extraction-to-disk; non-DDS decoding; other `.BIG` files; audio, models, gameplay, Ghidra |
| Legal | No game bytes added by this archive; evidence uses counts and exit codes only (no hashes, offsets, or paths of game data) |

## Checklist

- [x] PR #8 MERGED and `main` contains all slice commits
- [x] Playtest note recorded (`--list` N + exit code; `--show` loud non-DDS exit 1; user acceptance)
- [x] Headless gates recorded (43/43 tests, zero build warnings, loud-exit cases)
- [x] Contained branches deleted after containment check; uncontained / out-of-scope branches explicitly kept with reason
- [x] README "What works" marks bigfile-index done; "What's missing" queues the next milestone
- [x] No game bytes, hashes, offsets, or paths of game data added by this archive

## Next step

The chosen next milestone is TEXTURES.BIG: the expected home of the remaster's packed textures. A 2236-entry sibling container is noted as a candidate for that milestone's scoping probe. sRGB/resize/mipmap-chain stay deferred unless the TEXTURES.BIG proposal pulls them in.

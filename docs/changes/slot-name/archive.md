# Archive: slot-name — DONE (slots carry the engine's own names; ambiguity fails loud)

`TEX_HD.MAP` is now decoded and exposed through the `.BIG` CLI. `--tlist --map`
annotates every slot line with the names the engine derives from its alias
matrix, and `--tshow <name> --map` accepts a texture name as a slot selector.
Names shared by several slots are rejected with the candidate indices instead of
silently picking the first match. Without `--map`, every prior line, message,
and exit code is byte-for-byte unchanged. PR #32 is merged into `main`; the
change touched `src/tex_hd_map.rs`, `src/main.rs`, `README.md`, and the change
docs only.

## Quick path

1. `git checkout main && git pull` — HEAD is `7e782f6` (merge of PR #32).
2. `cargo run -- --textures <TEXTURES.BIG> --tlist --map <TEX_HD.MAP>` — exit 0,
   every slot line carries `; names ...` (or `; names -`).
3. `cargo run -- --textures <TEXTURES.BIG> --tshow <unique name> --map <TEX_HD.MAP>`
   — shows that slot in the existing window; a shared name exits 1 with the
   candidate indices and opens no window.
4. `cargo test` — 157/157 green on synthetic bytes only.

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| `--tlist --map` on `TEXTURES.BIG` | Names on every slot; footer `1287 total, 1 empty, 0 zero, 1286 data`; 1287/1287 slots named (0 `names -`). |
| `--tshow 1_0 --map` | `slot 1 ... decoded: 256x256 A1R5G5B5 -> RGBA8; showing image; names 1_0,1_1,1_2,1029_12`; window showed the font atlas, consistent with the loose `FONT0.DDS`. |
| `--tshow 100_10 --map` | `error: texture name '100_10' is referenced by 519 slots (e.g. 2, 3, 4, 5, 9, 10, 11, 12); select one by index`; exit 1. |
| `E3.BIG` with `--map` | `error: cannot map names for 'E3.BIG': expected TEXTURES.BIG, MOVIES.BIG, or BONUS.BIG`; exit 1. |
| Name statistics | 5,876 distinct names, 2,070 shared between more than one slot. |
| Regression | `--tlist` without `--map` byte-identical to the prior output (SHA256 proven). |
| Gates | `cargo test` 157 passed / 0 failed after the rebase; zero warnings; legal whitelist clean; repository private. |

The single `--tshow` probe that resolved showed a real image that matched the
equivalent loose file; the shared-name probe exited 1 without opening a window.
No payload bytes were printed or committed.

## Details

| Topic | Outcome |
|---|---|
| PR | [#32 — feat(textures): TEX_HD.MAP slot names for --tlist/--tshow](https://github.com/libertus21/soul-reaver-remaster-rs/pull/32), state MERGED (`mergedAt` 2026-10-26T05:27:24Z), base `main`, head `feature/slot-name`, 8 files, +1,831/−23. |
| Commits on `main` | `4a95410` plan bundle + `TEX_HD.MAP` findings (`docs/changes/slot-name/*`, `docs/notes/s8-big-slot-names.md`) → `2d3689d` decode slot alias codes (`src/tex_hd_map.rs`) → `04344e9` append names to `--tlist` → `d99eace` accept names for `--tshow` → `c4a3e1b` mark completed work units → `7e782f6` PR #32 merge. |
| Rebase note | The branch was rebased onto the merged pipe-safety fix (PR #33, `68eb74a`) and force-pushed as `c4a3e1b`; the new `--map` stdout path moved to `cli_out::outln!`, so pipe safety now covers every stdout path. Pre-rebase hashes are not part of `main`'s history. |
| Verified contract | `TEX_HD.MAP` is exactly 384,000 bytes = 6,000 rows x 64 bytes, 16 little-endian `u32` alias codes per slot row, `0xFFFFFFFF` = none. Blocks: `TEXTURES` base 0, `MOVIES` base 2,000, `BONUS` base 4,000; `E3.BIG` has no block. Code decode: `v < 32000` → `<v/16>_<v%16>`, `32000 ≤ v < 64000` → `M\<(v−32000)/16>_<(v−32000)%16>`, `v ≥ 64000` → `B\<(v−64000)/16>_<(v−64000)%16>`; `0` is valid, only `0xFFFFFFFF` means "none". |
| CLI behavior | `--tlist --map` keeps the summary, addressing line, class computation, and footer unchanged and appends `; names <n1>,<n2>,...` (deduped, first-seen order) or `; names -`. `--tshow <name> --map` parses `<id>_<sub>`, `M\<id>_<sub>`, or `B\<id>_<sub>`, resolves against the group's rows, and refuses to guess when more than one slot matches (loud error listing up to 8 candidates plus the total). Exactly one match decodes and shows through the existing path. Unknown names, invalid syntax, wrong map length, unreadable maps, unknown stems, and row capacity all fail loud (exit 1). Header/geometry errors keep precedence. |
| Delivered in code | `src/tex_hd_map.rs`: `Group`, `Bank`, `TextureName`, `TextureMap`, `MapError`, constants and codec, with synthetic-bytes tests. `src/main.rs`: `--map` argument shapes, list suffix, name selector and ambiguity path, help text, CLI tests. |
| Deliberately not built | Descriptive labels ("stone wall", "face") — the data only carries numeric asset names; mapping those to PS1 page/room catalogues is separate scope. No implicit map discovery, no new dependency, no game bytes. |
| Process warnings (recorded) | (1) The slice exceeded the 800-line review budget: 1,024 code-only changed lines (1,854 with docs) against a ~340-line forecast; the user reviewed and accepted merging as a single PR. (2) A broken-pipe panic surfaced during playtest (`\| head` → exit 101) and was fixed separately in PR #33; this branch was then rebased onto that fix and its new `--map` output moved to `cli_out::outln!` (force-push `c4a3e1b`), so pipe-safety now covers every stdout path. |
| Branch cleanup | Deleted only branches verified fully contained in `main`; see the report below. `feature/slot-name-plan` is NOT contained and was left in place. |
| Follow-ups | (1) Descriptive names are out of scope: the numeric names match the engine's loose-file basenames, not human vocabulary; any friendly labels need PS1 catalogues and a separate slice. (2) Exhaustive slot → name correlation across all 1,287 `TEXTURES.BIG` slots is sampled-but-complete for names (every slot printed a name), while content identity of each slot remains unproven beyond the playtested images. |
| Legal | Format and behavior facts only. No game bytes, no decoded imagery, no absolute game paths, no extracted data, and no content dumps in any artifact or commit. Tests use synthetic bytes in temp dirs; the real files are read-only playtest inputs. |

## Checklist

- [x] PR #32 MERGED and `main` contains all slice commits (HEAD `7e782f6`)
- [x] Playtest evidence recorded verbatim (name coverage, unique-name image, shared-name refusal, `E3.BIG` stem refusal, name statistics, SHA256 regression)
- [x] Name model stated as verified (map shape, block bases, code decode) with numeric-vs-descriptive scope called out
- [x] Contained branches deleted after containment checks; uncontained branches kept with the reason stated
- [x] README "What works" carries the `--map` name mapping and "What's missing" points at audio and animation
- [x] No game bytes, absolute game paths, offsets, or hashes added by this archive

## Next step

Open the audio slice: the paired `.SND`+`.SMP` tracks under `MUSIC/<track>/`
are still unread and have no format verdict beyond community tooling. Probe the
pair contract read-only before writing code, then scope animation for the
existing mesh path. Texture work has no open blocker at this level.

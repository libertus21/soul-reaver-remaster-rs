# Archive: unified-viewer — DONE (one discoverable entry point, legacy argv untouched)

The viewer now has a grouped, discoverable entry point without changing how any
existing command behaves. `--help` lists the five capability groups, a new
`<group> <action> [operands]` grammar translates to the exact legacy argv, and an
optional `menu` picker walks a terminal user through group, command, and required
operands over the same dispatcher. The chain shipped as three PRs (#56, #57, #58),
all merged into `main`; every legacy invocation keeps its stdout, stderr, and exit
code because the new forms are rewritten into the old argv and handled by the
existing code. `Cargo.toml` and `Cargo.lock` are byte-identical at every PR.

## Quick path

1. `git checkout main && git pull` — HEAD is `6985627` (merge of PR #58).
2. `cargo test` — 260/260 green, zero warnings.
3. `cargo run -- --help` — five groups, one usage line per capability, exit 0.
4. `cargo run -- texture list <BIG>` — same result as
   `cargo run -- --textures <BIG> --tlist`.
5. `cargo run -- menu` — interactive picker (needs a terminal); `q` or EOF exits.

## Accepted evidence (verbatim)

| Item | Value |
|---|---|
| PR chain | 3 PRs, all MERGED: #56 catalog + grouped help, #57 router, #58 picker. |
| Dependencies | Zero new dependencies: `Cargo.toml` and `Cargo.lock` diff is empty at every PR. |
| PR1 `--help` catalog | 15 modes across 5 groups (`texture`, `archive`, `mesh`, `audio`, `level`); `help <group>` filters to that group; an unknown group exits 1 and lists the valid groups. |
| PR2 router | New subcommands translate to the exact legacy argv; verified byte-identical on 4 pairs (level links, level info, texture list, archive list). Legacy flags still work. |
| PR3 picker | `menu` enters an interactive picker (group → command → required operands) over injected `BufRead`/`Write`, and returns argv to the same dispatcher. Empty argv with a non-terminal stdout exits 1 in ~112 ms without hanging (no pipe hang). `q` / Ctrl-D cancels. |
| Playtest (menu) | The picker displayed groups, listed commands, re-prompted on an empty required value with `error: this value is required`, then treated a typed `2` as a path, ending in a real `cannot read 2` file error with exit 1. Working as designed; the confusion was UX (the `SND>` prompt does not say it wants a path). |
| Gates | 260/260 tests at PR3, zero warnings, repository private, legal clean. |
| Authored per PR | 180 / 335 / 196 lines (includes imported planning docs where noted). |

## PR chain

| PR | Title | Merged | Files | Lines | Commits |
|---|---|---|---|---|---|
| [#56](https://github.com/libertus21/soul-reaver-remaster-rs/pull/56) | `feat(cli): add the capability catalog and grouped help` | 2026-11-03T17:19:23Z | 6 | +704 / −0 | `docs(cli): import the unified-viewer planning documents` → `feat(cli): add the capability catalog and grouped help` |
| [#57](https://github.com/libertus21/soul-reaver-remaster-rs/pull/57) | `feat(cli): route grouped subcommands to the legacy dispatch` | 2026-11-03T17:33:14Z | 2 | +351 / −14 | `refactor(cli): extract the dispatch chain from main` → `feat(cli): route grouped subcommands to the legacy dispatch` |
| [#58](https://github.com/libertus21/soul-reaver-remaster-rs/pull/58) | `feat(cli): add the interactive terminal picker` | 2026-11-03T17:48:20Z | 4 | +205 / −3 | `feat(cli): add the interactive terminal picker` → `docs(readme): document the grouped entry point and terminal picker` |

Merges on `main`: `2e108ea` (#56) → `9a29e5b` (#57) → `6985627` (#58).

## Details

| Topic | Outcome |
|---|---|
| Outcome | One entry point that can be discovered (`--help`, grouped subcommands) and driven interactively (`menu`), with the legacy surface preserved. |
| Catalog | `src/cli_catalog.rs` owns the 15 capabilities and their groups; help text is generated from that table, never hand-duplicated. `--help`, `-h`, and `help` print the group index; `help <group>` prints one group. |
| Router | `src/cli_argv.rs` maps each new form to the exact legacy argv, and `src/main.rs` routes that argv through the extracted `dispatch` chain. Unknown first words and plain paths fall through to the legacy parser untouched. |
| Picker | `src/menu.rs` is pure over injected `BufRead`/`Write`: it prompts only for the declared operands, builds the same canonical argv, and returns it to the dispatcher. It validates no paths or file contents itself. |
| Headless safety | The picker runs only with a terminal; `menu` under redirected stdout keeps the legacy usage error and exits 1 without reading stdin. `--help` and every subcommand are fully non-interactive. |
| Reserved words | `texture`, `archive`, `mesh`, `audio`, `level`, `menu`, `help`, `--help`, and `-h` are reserved in `argv[0]`; a path with one of those names is addressed as `./name`. Covered by a test. |
| Compatibility guarantee | Legacy flags are unchanged, and the equality between old and new forms is asserted as an **argv vector comparison** in unit tests (not as output snapshots). On four representative pairs the legacy and new forms were also compared byte-for-byte. |
| Legal | The change is CLI wiring only: no game bytes, no absolute install paths, no extracted assets. Tests use synthetic argv and temp directories. |

## Follow-up — UX polish (not a defect)

The picker prompts for an operand using only its token name (`SND>`, `<BIG>`
stripped), with no example or hint that a filesystem path is expected. In the
playtest a typed `2` was accepted as a path and failed downstream with
`cannot read 2` (exit 1). That is the designed contract — the picker never
validates paths — but it reads like a menu choice. A later polish item should
add a short hint or example to each operand prompt (for example
`path to .SND bank> `). Recorded as a small polish item, **not** a defect and
not a blocker for this archive.

## Branch cleanup report

Containment was checked with `git merge-base --is-ancestor <ref> main`. For
rebased duplicates, `git diff <ref> main -- <paths the branch touched>` was used:
an empty diff means the same content already landed on `main`.

| Branch | State | Action |
|---|---|---|
| `feature/unified-viewer-catalog` | ancestor of `main` | Deleted local + remote. |
| `feature/unified-viewer-router` | ancestor of `main` | Deleted local + remote. |
| `feature/unified-viewer-menu` | ancestor of `main` | Deleted local + remote. |
| `feature/unified-viewer-plan` | not an ancestor (rebased copy); its four planning docs are identical on `main` | Deleted local + remote. |
| `docs/archive-room-geometry` | ancestor of `main` | Deleted local + remote. |
| `docs/level-builder` | ancestor of `main` | Deleted local + remote. |
| `docs/level-builder-table` | ancestor of `main` | Deleted local + remote. |
| `docs/level-links-archive` | ancestor of `main` | Deleted local + remote. |
| `docs/pcm-content-census` | ancestor of `main` | Deleted local + remote. |
| `feat/link-target-id` | ancestor of `main` | Deleted local + remote. |
| `feature/level-links` | ancestor of `main` | Deleted local + remote. |
| `feature/level-structure` | ancestor of `main` | Deleted local + remote. |
| `feature/pcm-loader-plan` | ancestor of `main` | Deleted local + remote. |
| `fix/level-info-links` | ancestor of `main` | Deleted local + remote. |
| `fix/pcm-data-base` | ancestor of `main` | Deleted local + remote. |
| `feature/placement-plan` | not contained; its placement docs are absent from `main` | Kept (unmerged work). |
| `origin/dev`, `origin/dev-2-0`, `origin/dev-3.0` | long-lived development branches | Kept deliberately. |

## Checklist

- [x] PRs #56, #57, #58 MERGED and `main` contains the whole chain (HEAD `6985627`)
- [x] Zero dependency change verified across the chain (`Cargo.toml` / `Cargo.lock` diff empty)
- [x] Accepted evidence recorded verbatim, including the playtest menu session
- [x] UX gap recorded as a polish follow-up, explicitly not a defect
- [x] Compatibility stated as legacy-unchanged and proven by argv vector comparison
- [x] Contained branches deleted after containment checks; uncontained work kept with the reason stated
- [x] README "What works" and "What's missing" reflect the unified entry point and the open follow-up
- [x] No game bytes, absolute game paths, offsets, or hashes added by this archive

## Next step

The unified entry point is complete and archived. The next open slices are
unchanged by this work: room placement and transforms inside `.PCM`, `.SRA`
clip/hierarchy scoping for full-body animation, audio loop rendering and the
`QSMa` sequence interpreter, and the still-unscoped binary-loader rows. The
picker prompt hint is a one-file polish item inside `src/menu.rs`.

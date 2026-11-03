# Design: unified-viewer

`main()` today is a linear chain of `if args.first() == "--flag"` arms,
each calling a `run_*` handler that takes the tail slice and returns
`ExitCode`. All parsing, validation, and messages already live inside
those handlers. Therefore the cheapest correct unification is not a
parser rewrite: it is a **translator in front of the same chain**. New
subcommands are rewritten into the exact legacy argv, and the chain runs
unchanged.

## Quick path

`main()` -> `strip_launcher_noops` (unchanged) -> `cli_argv::route`
(pure) -> one of {help text, picker, legacy argv, usage error} -> the
existing chain, extracted verbatim as `dispatch(&[String]) -> ExitCode`
-> the unchanged `run_*` handler -> the unchanged decode/render path.

## Option comparison

| Criterion | (a) Terminal menu only | (b) In-window UI (`egui`/`eframe`) | (c) Unified subcommand CLI | (c)+(a) chosen |
|---|---|---|---|---|
| New dependencies | none | `eframe`, `egui`, their tree (dozens of crates), plus a font atlas and an accesskit stack | none | none |
| GPU/render risk | none | HIGH: a second event loop beside `winit`, a second paint path, and an `eframe`-pinned `wgpu` major that may not match the project's `wgpu 30.0.1` (mismatch means two GPU stacks or a project-wide downgrade) | none | none |
| Testability | HIGH: pure over `BufRead`, drive with a `Cursor` | LOW: needs a window and a GPU; headless CI cannot assert it | HIGH: `&[String] -> Vec<String>`, table-driven | HIGH |
| Risk to existing behavior | none (additive) | HIGH: touches the event loop and the only render path | LOW if implemented as argv canonicalization; HIGH if it re-parses flags | LOW by construction |
| Scriptable / CI usable | no | no | yes | yes |
| Discoverability | medium (guided) | high (visual) | medium-high (`--help` groups) | high |
| Effort | ~85 production lines | weeks, plus a dependency audit | ~200 production lines | ~285 lines, 3 PRs |
| Verdict | insufficient alone | rejected | primary | **chosen** |

### Dependency cost/benefit, stated explicitly

This change adds **zero** dependencies. The `Cargo.toml` diff at every PR
must be empty. `eframe`/`egui` was evaluated and rejected: it would add a
large transitive tree to a project whose entire dependency list is five
crates, introduce a second GPU backend generation beside the pinned
`wgpu 30.0.1`, and require reworking the `winit 0.30` event loop that
every rendering test and playtest depends on. Its benefit over the
terminal picker is visual polish, not capability. For a change whose
defining constraint is "nothing that works may change", that trade is
inverted. `std::io::IsTerminal` (stable since Rust 1.70) covers the only
terminal-detection need, so even the usual `is-terminal` crate is
unnecessary.

## Why argv canonicalization is the compatibility proof

Byte-compatibility is not asserted by re-testing outputs; it is
guaranteed by construction. `route` returns a `Vec<String>`; the
dispatcher consumes it exactly as it consumes `std::env::args()` today.
If `route("texture list", ["TEXTURES.BIG"])` returns
`["--textures", "TEXTURES.BIG", "--tlist"]`, then every downstream byte
is produced by the same code that produces it today, because it *is* the
same code. The test therefore asserts vector equality (S6), not output
text, and no second source of truth can drift.

The one structural edit is the verbatim extraction of the existing chain:

```rust
fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = strip_launcher_noops(raw);
    match cli_argv::route(&args) {
        Route::Help(group) => { cli_catalog::print_help(group); ExitCode::SUCCESS }
        Route::Menu => menu::run_and_dispatch(),
        Route::Dispatch(argv) => dispatch(&argv),
        Route::Error(message) => { eprintln!("{message}"); ExitCode::from(1) }
    }
}

fn dispatch(args: &[String]) -> ExitCode {
    // existing pose_mode_conflict + if-chain, moved without edits
}
```

`pose_mode_conflict` stays at the top of `dispatch`, so its message and
ordering are unchanged. It can never fire for a new form because only
`mesh show` can emit `--pose`, and it always emits it in `--mesh` mode.

## Router precedence (fixed, tested)

| # | Condition on `argv[0]` | Route | Exit |
|---|---|---|---|
| 1 | argv is empty | `Dispatch([])` -> existing usage block | 1 |
| 2 | `--help`, `-h`, `help` | `Help(argv[1])` | 0 |
| 3 | `menu` | `Menu` (TTY-gated) | handler |
| 4 | `texture`, `archive`, `mesh`, `audio`, `level` | parse new grammar -> `Dispatch(canonical)` or `Error` | 0/handler/1 |
| 5 | anything else (any `--flag`, any path, any unknown word) | `Dispatch(argv)` verbatim | as today |

Rule 5 is the safety net: a token we do not reserve can never be
reinterpreted, so no existing script changes meaning. Rule 3 and rule 4
are the only new namespaces, and they are the documented reserved words.

### Reserved-word shadowing

A local directory named `mesh` is now shadowed in `argv[0]`. Resolve it
as `./mesh`. This is the only observable behavior change of the whole
change, it affects a name that is not an existing flag, and it is
documented in `--help` plus covered by S12. The alternative (probing the
filesystem before routing) would make dispatch depend on the working
directory and silently prefer a directory over a command, which is
worse.

## Canonical argv mapping

The capability table is the single source for all four consumers: the
router, `--help`, group help, and the picker. Each `Cap` declares its
group, summary, operands (name + required), and `argv()`.

| Group | New form | Legacy argv emitted |
|---|---|---|
| texture | `texture list <BIG> [--map <MAP>]` | `--textures <BIG> --tlist [--map <MAP>]` |
| texture | `texture show <BIG> <SEL> [--map <MAP>]` | `--textures <BIG> --tshow <SEL> [--map <MAP>]` |
| texture | `texture file <FILE.DDS>` | `<FILE.DDS>` |
| texture | `texture gallery <DIR>` | `<DIR>` |
| archive | `archive list <DAT>` | `--bigfile <DAT> --list` |
| archive | `archive show <DAT> <SEL>` | `--bigfile <DAT> --show <SEL>` |
| mesh | `mesh show <SRM> [--pose <B>]` | `--mesh <SRM> [--pose <B>]` |
| mesh | `mesh textured <SRM> --tex-dir <TEX>` | `--mesh-textured <SRM> --tex-dir <TEX>` |
| mesh | `mesh gallery <DIR> --tex-dir <TEX>` | `--mesh-all <DIR> --tex-dir <TEX>` |
| audio | `audio list <SND>` | `--audio <SND> --alist` |
| audio | `audio wave <SND> <N> --out <OUT>` | `--audio <SND> --wave <N> --out <OUT>` |
| level | `level info\|records\|structure\|links <PATH>` | `--level-info\|--level-records\|--level-structure\|--level-links <PATH>` |

Option flags are accepted in any position after the first operand and are
re-emitted in the canonical order. This is strictly more forgiving than
today's legacy handlers (which require exact positions); the legacy form
itself stays strict because it never passes through this table.

## Module boundaries

| File | Contents | Pure? | PR |
|---|---|---|---|
| `src/cli_catalog.rs` | `Cap` enum, `Spec`, `GROUPS`, `Cap::argv(operands)`, `help_text()`, `group_help(group)` | yes (no IO) | 1 |
| `src/cli_argv.rs` | `Route`, `route(&[String])`, `parse_group`, `CliError` | yes (no IO) | 2 |
| `src/menu.rs` | `run(reader, writer) -> Option<Vec<String>>`, prompt loop, TTY gate wrapper | reader/writer injected | 3 |
| `src/main.rs` | `mod` declarations, router hook, verbatim `dispatch` extraction | — | 2 (+10 in 3) |

No handler is edited. `cli_out::outln` is NOT used by the catalog: help
goes to stdout through the existing `print_line` so a closed pipe still
exits 0, matching R2's "help is CLI output" and the existing
broken-pipe contract.

## Picker semantics

- Prints numbered capabilities grouped by header, then `select>`.
- Accepts a number, or `q`/EOF -> `None` -> exit 0.
- For the chosen capability, prompts each declared operand in order,
  showing `[optional]` for non-required ones.
- Empty required operand re-prompts; non-numeric selection re-prompts.
- Returns the canonical argv; `main` then calls `dispatch`, so the picker
  never validates a path or opens a file itself.
- One-shot: after the chosen capability's handler returns (including
  after a window closes with Esc), the process exits with that handler's
  code. It is a launcher, not a UI host.

## What is NOT built

- No in-window UI, no `egui`/`eframe`, no new crate of any kind.
- No second render path, no `winit` event-loop change, no `render.rs`,
  `srm.rs`, `textures.rs`, `bigfile.rs`, `dds.rs`, `snd.rs`, or `level.rs`
  edit.
- No capability added, removed, or reimplemented; no new format support.
- No install auto-discovery, no saved paths, no config file, no env vars.
- No shell completions, no man page, no new binary name or alias.
- No change to the no-argument usage block, to any legacy message, or to
  any legacy exit code.
- No help text reuse: `--help` is additive; the legacy error-path usage
  blocks stay exactly as they are.

## Test strategy (no GPU, no game bytes)

| Layer | How | Why it is enough |
|---|---|---|
| Router | table-driven over every accepted and rejected form, asserting the exact `Vec<String>` | vector equality is the byte-compat proof (see above) |
| Catalog | help/group-help contain every capability key and group, and no capability is ungrouped | one table, one place to forget |
| Legacy passthrough | unknown words, unknown flags, paths, empty argv route to `Dispatch(argv)` verbatim | proves rule 5 |
| Picker | `Cursor` scripted sessions: select, bad select, empty operand, `q`, EOF | no TTY or process needed |
| Regression | the 245 existing tests run untouched; new tests are additive only | a changed assertion is a design leak |

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Reserved word shadows a real path | `mesh` directory unreachable by bare name | documented, `./mesh` escape, S12 test |
| Router drifts from the chain | new form silently does something else | router emits legacy argv only; no handler logic in it; S6 asserts equality |
| Help text diverges from reality | users told about a capability that moved | help generated from the same table the router uses |
| Picker blocks a pipe | CI hangs | TTY gate with `IsTerminal`; loud refusal; S11 |
| Menu becomes a second validation surface | two error vocabularies | picker does no validation; it returns argv and lets the handler speak |
| Scope creep toward a real UI | dependency and render risk | non-goals list is explicit; `Cargo.toml` diff must stay empty |

## Next step

See `tasks.md` for the atomic units, the line forecast, and the three-PR
chain.

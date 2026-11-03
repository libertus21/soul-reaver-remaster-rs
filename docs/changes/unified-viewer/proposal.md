# Proposal: unified-viewer

GO for a unified entry point built as **argv canonicalization**, plus a
terminal picker that shares the same capability table. No new dependency,
no rewrite of any existing handler, no second render path.

The tool today exposes eight independent CLI shapes with no shared
front door: `--textures ... --tlist|--tshow`, `<FILE.DDS>` and `<DIR>`
(positional legacy), `--bigfile ... --list|--show`, `--mesh`,
`--mesh-textured`, `--mesh-all`, `--audio ... --alist|--wave`, and the
four `--level-*` reports. Discovery is a 17-line `eprintln!` block that
only appears when the invocation is already wrong, capabilities are not
grouped, and there is no `--help`. A user who owns the game but not this
codebase cannot answer "what can this thing do with a `.BIG`" without
reading `main.rs`.

This change makes the tool discoverable and scriptable **without**
touching anything that already works: new grouped subcommands are
translated into the exact legacy argv and handed to the existing
dispatch chain, which stays byte-identical.

## Quick path

1. `cargo run -- --help` prints five capability groups and one example
   per capability; `cargo run -- help mesh` prints one group.
2. `cargo run -- texture list <BIG>` runs exactly the same code as
   `cargo run -- --textures <BIG> --tlist`, with the same stdout bytes
   and exit code.
3. `cargo run -- menu` lists the catalog and prompts for operands when
   both stdin and stdout are terminals; with piped stdin it refuses
   loudly instead of blocking.
4. Every legacy flag keeps working, unchanged, forever.

## Why this shape and not a UI

An in-window UI (`egui`/`eframe`) is the richest option and the worst fit
here. It adds a large dependency tree, its own `wgpu` major pin (the
project pins `wgpu 30.0.1`; a mismatch means either two GPU stacks in one
binary or a project-wide downgrade), a second event loop next to the
existing `winit` loop, and a whole new render path that cannot be
verified headless. The compatibility contract of this change is "nothing
that works may change"; a new rendering path is the largest possible
violation of that contract for the smallest discoverability gain.

A terminal picker has almost all the discoverability value of a window
menu, costs zero dependencies (`std::io::IsTerminal` is std since 1.70),
and is testable with a `Cursor` instead of a GPU. A grouped subcommand
CLI costs the same and is also scriptable, which a window menu is not.
Both are cheap enough to ship together, so the change does both: the CLI
is the primary entry point and the picker is a thin front end over the
same table.

## Details

| Topic | Decision |
|---|---|
| Problem | Eight loose CLI modes, no grouped help, no `--help`, no machine-discoverable capability list. Nothing is broken; the problem is discovery |
| Chosen option | (c) unified subcommand CLI as the primary entry point, plus (a) terminal picker as a second front end over the SAME capability table. (b) in-window `egui` UI is rejected |
| Compatibility mechanism | New grammar is translated to the exact legacy argv (`texture list X` -> `--textures X --tlist`) and handed to the existing dispatch chain. The chain is extracted verbatim into `fn dispatch(&[String]) -> ExitCode`; no handler, parser, or message changes |
| Reuse rule | `run_bigfile`, `run_textures`, `run_mesh*`, `run_audio`, `run_level_*`, `render.rs`, `srm.rs`, `textures.rs`, `bigfile.rs`, `dds.rs`, `snd.rs`, `level.rs` are NOT modified. Tests keep calling them directly |
| New modules | `src/cli_catalog.rs` (capability table + argv builder + help text), `src/cli_argv.rs` (pure router/grammar), `src/menu.rs` (prompt loop over `BufRead`) |
| Dependencies | Zero added. No `clap`, no `egui`, no `is-terminal` crate (`std::io::IsTerminal`) |
| Failure policy | New grammar errors are LOUD on stderr + exit 1 with the offending token. Legacy errors keep their exact text and exit codes. The picker refuses when not on a TTY instead of blocking a pipe |
| Reserved words | `texture`, `archive`, `mesh`, `audio`, `level`, `menu`, `help`, `--help`, `-h` are reserved as `argv[0]`. A local path with one of those names is reached as `./name`. Documented and tested, never silent |
| Non-goals | In-window UI, new crates, a second render path, install auto-discovery, saved paths/config, shell completions, a new binary alias, changing any legacy message or exit code, rewriting help text of existing error paths, gameplay |
| Slice boundary | Grouped CLI + `--help` + picker + tests + README. No capability is added, removed, or reimplemented; only reached differently |

## Explicit no-go condition

This change is GO only while all four hold:

1. `route()` is a pure `&[String] -> Route` and every new form maps to
   an argv that the existing chain already accepts, with no handler edit.
2. The 245 existing tests stay green without being modified (new tests
   are additive; a changed assertion means the design leaked into
   behavior and the slice stops).
3. `Cargo.toml` gains no dependency.
4. The no-argument invocation still prints the current 17-line usage and
   exits 1, byte for byte.

If (1) needs a handler change, stop and re-scope: that is a rewrite, not
a unification. If (2) fails, stop and name the two options that actually
collide instead of loosening the legacy contract.

## Checklist

- [ ] Reviewer can state the deliverable in one sentence: grouped
      subcommands, `--help`, and a terminal picker that all funnel into
      the unchanged dispatch chain.
- [ ] Reviewer can state why the CLI wins: zero dependencies, pure
      function testability, byte-compatibility by construction, full
      scriptability.
- [ ] Reviewer can state what is NOT built: no egui, no new crate, no
      install discovery, no config file, no completions.
- [ ] Legal guardrails hold: no game bytes, no absolute install paths,
      relative model names only, synthetic test bytes only.

## Next step

Read `spec.md` for the acceptance scenarios and exit codes, then
`design.md` for the option table, the router precedence, and the exact
argv mapping, then `tasks.md` for the forecast and the three-PR chain.

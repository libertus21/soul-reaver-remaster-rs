# Proposal: f4-launcher-cli

Document the command-line argument set found in `SRX.exe` strings and replicate it as a
pure Rust parser with unit tests, then expose three of those flags (`--nosteam`,
`-nocheck`, `-skipintro`) as documented no-ops in the viewer CLI. Docs + parser only:
no engine code changes, no binary invocation in tests.

## Quick path

1. Re-extract and tabulate the launcher flags from the packed `SRX.exe` string
   (evidence quotes below); mark every observed effect `UNKNOWN` (static only).
2. Add `src/launcher_cli.rs` — a pure parser + `LauncherArgs` + `LauncherCliError`,
   with unit tests on synthetic argv only.
3. Wire three compatibility no-ops into `src/main.rs` (strip + ignore).
4. Land as a single PR <=400 authored lines.

## Decisions

| Topic | Decision |
|---|---|
| Scope | F4 only: launcher CLI args. S1/S2/S3 stay out. SR1 only. |
| Evidence rule | Every arg cites the exact packed-string evidence. Effects are `UNKNOWN` unless directly observable from strings; no invented semantics. |
| Clean-room | Behavior notes only. No disassembly, no listings, no game bytes/paths/hashes. Binaries read-only. |
| Parser | Pure function `parse(&[String]) -> Result<LauncherArgs, LauncherCliError>`; no IO, no `std::env`, no process spawning. Synthetic argv tests only. |
| Viewer surface | `--nosteam`, `-nocheck`, `-skipintro` are accepted as **no-ops**: stripped before dispatch, no behavior change, no output, exit code unaffected. Other discovered flags are **not** wired into the viewer (parser-only). |
| Budget | Single PR <=400 authored lines. |

## Checklist

- [ ] Args table (spec) cites the packed-string evidence for each flag.
- [ ] Pure parser + tests land with `cargo test` green.
- [ ] Viewer accepts the three no-op flags without changing existing behavior.
- [ ] `git status` shows only intended files.

## Next step

Spec (`spec.md`) fixes the args table and requirements; design (`design.md`) fixes the
parser API and no-op wiring; `tasks.md` lists atomic units.

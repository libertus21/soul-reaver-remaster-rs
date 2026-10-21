# Design: f4-launcher-cli

Pure parser in a new module plus three no-op flags stripped in `main.rs`. No engine
code changes.

## Quick path

`SRX.exe packed string` -> `src/launcher_cli.rs` (`parse` + `LauncherArgs` +
`LauncherCliError`) -> unit tests -> `src/main.rs` strips no-ops -> `cargo test`.

## Method

| Step | Detail |
|---|---|
| New module | `src/launcher_cli.rs`, declared in `main.rs` as `mod launcher_cli;` |
| Struct | `LauncherArgs` with one field per discovered flag (bools, `Option<u32>` for width/height, `Option<String>` for Epic values, `Option<Game>` for -1/-2, `Vec<String> unknown`) |
| Enums | `Game { Sr1, Sr2 }`, `EgsEnv { Dev, Stage, Live }` (parsed from -egs_*), `LauncherCliError { MissingValue(String), InvalidValue { flag, value } }` |
| Parse | Single pass over `&[String]`. `=`-suffixed flags split in place; space-separated values consume the next element. Unknown tokens pushed to `unknown`. |
| No-ops | `src/main.rs` removes `--nosteam`/`-nocheck`/`-skipintro` from `args` before the first dispatch; no logging, no exit change. |
| Tests | `#[cfg(test)]` in `launcher_cli.rs`; synthetic argv only. |

## Details

| Topic | Decision |
|---|---|
| Module purity | No `std::env`, no IO, no process spawn. `parse` takes `&[String]` and returns `Result`. |
| Error type | `thiserror` not available; implement `Display` + `std::error::Error` manually (no new crate). |
| Viewer surface | Only the three named flags become no-ops. Other discovered flags are parser-only and are **not** added to viewer usage (avoids scope creep). |
| Compatibility | `-nocheck`/`-skipintro` are accepted by the parser even though absent from this build; marked compat-only. |
| No-op placement | Strip in `main()` immediately after `std::env::args().skip(1).collect()`, before any `--bigfile`/path dispatch. |

## Deliberately NOT built

Engine-side effect of any flag, Steam/Epic integration, resolution/window changes,
runtime execution, any binary invocation in tests.

## Checklist

- [ ] Reviewer can state the parser's value-consumption rule (`=` vs space).
- [ ] Reviewer can confirm no-ops leave stdout/stderr/exit code unchanged.
- [ ] No new crates; `cargo test` green.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

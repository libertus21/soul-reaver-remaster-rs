# Spec: f4-launcher-cli

Launcher CLI argument set discovered in `SRX.exe` strings, plus the pure-parser and
viewer no-op contract. Behavior notes only; no engine code changes.

## Quick path

1. Apply extracts the packed flag string from `SRX.exe` (evidence below) and builds
   the table.
2. `src/launcher_cli.rs` implements `parse` + `LauncherArgs` + error type.
3. `src/main.rs` strips `--nosteam`/`-nocheck`/`-skipintro` as no-ops.
4. `cargo test` is the gate.

## Args discovered (SRX.exe packed string)

Evidence (single packed string, SRX.exe):  
`...-width -height MAINICONLoKLegacy of Kain? Soul Reaver 1&2 Remastered-nosteamsteam_api64.dll-noegs-AUTH_PASSWORD=-epicsandboxid=-epiclocale=-epicuserid=-egs_dev-egs_stage-egs_live-dev-novsync-nolegal-width-height1\sr1.dll2\sr2.dll-1-2...`

| Arg | Value? | Evidence token | Observed effect | Inference (unverified) |
|---|---|---|---|---|
| `-nosteam` | no | `-nosteam` | UNKNOWN | disables Steam API init |
| `-noegs` | no | `-noegs` | UNKNOWN | disables Epic Games Store init |
| `-nolegal` | no | `-nolegal` | UNKNOWN | skips legal screen |
| `-novsync` | no | `-novsync` | UNKNOWN | disables vsync |
| `-dev` | no | `-dev` | UNKNOWN | dev mode |
| `-ldpi` | no | `-ldpi` | UNKNOWN | DPI-awareness setup |
| `-egs_dev` | no | `-egs_dev` | UNKNOWN | select EGS dev env |
| `-egs_stage` | no | `-egs_stage` | UNKNOWN | select EGS stage env |
| `-egs_live` | no | `-egs_live` | UNKNOWN | select EGS live env |
| `-width` | yes (u32) | `-width` | UNKNOWN | window width |
| `-height` | yes (u32) | `-height` | UNKNOWN | window height |
| `-AUTH_PASSWORD=` | yes (string) | `-AUTH_PASSWORD=` | UNKNOWN | Epic auth password |
| `-epicsandboxid=` | yes (string) | `-epicsandboxid=` | UNKNOWN | Epic sandbox id |
| `-epiclocale=` | yes (string) | `-epiclocale=` | UNKNOWN | Epic locale |
| `-epicuserid=` | yes (string) | `-epicuserid=` | UNKNOWN | Epic user id |
| `-1` | no | `-1` | UNKNOWN | select SR1 (1\sr1.dll) |
| `-2` | no | `-2` | UNKNOWN | select SR2 (2\sr2.dll) |
| `-nocheck` | no | not present in this build's SRX.exe strings | UNKNOWN (absent) | compatibility no-op only |
| `-skipintro` | no | not present in this build's SRX.exe strings | UNKNOWN (absent) | compatibility no-op only |

`=`-suffixed tokens take their value after `=` in the same argv element. Space-separated
value tokens (`-width 1920`) are also accepted by the parser.

## Requirements

- R1: Parser SHALL be pure (no IO, no env, no process spawn) and total over `&[String]`.
- R2: Unknown tokens SHALL be collected, not rejected (launcher-tolerant).
- R3: Value-taking flags with a missing/invalid value SHALL return a typed error.
- R4: All effects remain `UNKNOWN`; no invented semantics in code or docs.
- R5: Viewer no-ops (`--nosteam`, `-nocheck`, `-skipintro`) SHALL be stripped before
  dispatch and SHALL not alter stdout/stderr/exit code.
- R6: Tests use synthetic argv only; no binary invocation, no game paths.
- R7: No game bytes/paths/hashes committed.

## Acceptance scenarios

- [ ] `parse(&["-nosteam".into(), "-width".into(), "1920".into()])` yields
      `nosteam=true`, `width=Some(1920)`, `unknown=[]`.
- [ ] `parse(&["-nocheck".into(), "-skipintro".into()])` yields both true.
- [ ] Missing value (`-width` with no follower) returns `MissingValue`.
- [ ] Invalid number (`-width abc`) returns `InvalidValue`.
- [ ] Unknown token (`-foo`) lands in `unknown`.
- [ ] Viewer invoked with `--nosteam` behaves like the same invocation without it.
- [ ] `cargo test` green; `git status` shows only intended files.

## Next step

Design (`design.md`) fixes the module/API and no-op wiring; `tasks.md` makes apply atomic.

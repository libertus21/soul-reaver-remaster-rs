# Design: ghidra-first-look

Bounded shipped-artifact survey: cheap static reads first, headless Ghidra second, strings/imports/xrefs only — full decompilation is explicitly out of bounds.

## Quick path

1. Reuse the TEMP Ghidra project; run time-boxed auto-analysis on `sr1.dll`.
2. Xref the evidence strings to code; write behavior paraphrases into the survey.
3. Prove hygiene (`check-ignore` + `git status`) and close with follow-up slice proposals.

## Method (bounded)

| Step | Bound | Tool |
|---|---|---|
| Static pre-read (done in Phase 1) | Read-only; offsets + lengths recorded | Python PE/strings probes, no Ghidra |
| Headless import | Proven: 26.1 s wall, `-noanalysis` | `support/analyzeHeadless.bat <projdir> probe -import 1\sr1.dll -noanalysis` |
| Auto-analysis | Time-box 30 min; if it overruns, keep the import-only DB and narrow to string-xref scripting | `analyzeHeadless` default analyzers, command + wall time logged |
| Candidate hunt | Strings + imports + xrefs + entry points ONLY; no function-body reading beyond what naming requires | Ghidra `findStrings` / xref script, results paraphrased |
| Survey write-up | Behavior notes; short quoted strings (filenames, error formats) allowed as evidence | `docs/notes/sr1-ghidra-survey.md` (new file, apply phase) |

Proven environment: Ghidra 12.1.4 at `C:\Users\dell\Downloads\ghidra_12.1.4_PUBLIC`, Temurin JDK 25, project at `C:\Users\dell\AppData\Local\Temp\opencode\ghidra-probe\probe` (reusable; import already saved). First headless call printed benign `GhidraMCP Module.manifest` warnings — non-blocking. No GUI fallback needed.

## Candidate areas (from Phase 1 evidence; apply confirms/refutes)

| ID | Area | Evidence strings | Expected finding |
|---|---|---|---|
| F1 | Bigfile open/read/error path | `Can't read from bigfile: %s` @`0x1a57b0`, archive-name cluster @`0x200958`–`0x200984`, `CreateFileW`/`ReadFile`/`SetFilePointerEx` imports | Name the loader function(s); paraphrase open→seek→read→error flow |
| F2 | Module/entry bootstrap | Entry `0x131ff0`, single export (name UNKNOWN), `LoadLibraryExW`/`GetProcAddress`/`FreeLibrary` imports, SRX's `1\sr1.dll` literal | How the launcher reaches game code; possible E3-vs-retail module selection |
| F3 | Engine global-state layout | `.data` ~96 MB zero-init span | Anchor major globals via data xrefs (layout only, no semantics invented) |
| F4 (low priority) | Launcher CLI surface | `-nosteam`, `SR1_*` IDs in `SRX.exe` | Only if needed to launch/playtest later; SR1-only scope keeps this optional |

## Hygiene proof (apply gate)

- Ghidra projects stay under `%TEMP%` (never repo/game dirs); whitelist `.gitignore` (`*` + narrow re-allows) already excludes `*.gpr`/`*.rep` — re-prove with `git check-ignore` in the apply commit.
- `git status --short` shows only intended `*.md` files; binaries read-only throughout.

## Out of scope

Full decompilation, any function-body copying, SR2 binaries, platform shims, runtime debugging, GUI sessions.

## Next step

Tasks (`tasks.md`) — four atomic units plus the workload forecast.

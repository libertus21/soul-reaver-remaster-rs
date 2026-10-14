# Spec: ghidra-first-look

Analysis slice: the deliverable is a reproducible SURVEY, not code. Tests are N/A with reason (below); verification is a command log plus a second-eyes spot check.

## Quick path

1. Apply runs the bounded Ghidra pass on `1\sr1.dll` and records every command + wall time.
2. Survey doc lands under `docs/notes/` with the binary-role table, string-evidence inventory, and candidate function list.
3. Hygiene gate passes: `git status --short` shows only intended `*.md` files; `check-ignore` proves Ghidra DBs stay excluded.

## Probe evidence (Phase 1, static reads only — no Ghidra yet)

All PEs are AMD64. Sizes: `SRX.exe` 989,576; `1\sr1.dll` 2,416,008; `Galaxy64.dll` 14,389,760; `steam_api64.dll` 2,668,544.

| Binary | Role verdict | Key evidence |
|---|---|---|
| `1\sr1.dll` | PRIMARY — game logic + archive loading | `bigfile.dat`, `Can't read from bigfile: %s`, `textures.big` / `movies.big` / `bonus.big` / `e3.big`; KERNEL32 imports `CreateFileW`, `ReadFile`, `SetFilePointerEx`, `GetFileSizeEx`, `FindFirstFileExW`, `LoadLibraryExW`, `GetProcAddress` (93 total); entry RVA `0x131ff0`; no Steam/Galaxy strings |
| `SRX.exe` | Launcher shell | Literal `1\sr1.dll`; `steam_api64.dll`, `SteamUser022`, `-nosteam`, `SR1_*` achievement IDs, OpenGL loader strings; zero bigfile references |
| `Galaxy64.dll` | OUT — platform networking | WS2_32 / WININET / CRYPT32 / MSWSOCK imports, 31 exports |
| `steam_api64.dll` | OUT — GOG shim | Imports `Galaxy64.dll`, 2084 exports |

String file offsets in `sr1.dll` (lengths exact; RVAs UNKNOWN until Ghidra import confirms):

| Offset (file) | Length | Content (behavior label) |
|---|---|---|
| `0x1a1d60` | 11 | `bigfile.dat` — default archive name |
| `0x1a57b0` | 27 | `Can't read from bigfile: %s` — loader error path |
| `0x200958` / `0x200968` / `0x200978` / `0x200984` | 12 / 10 / 9 / 6 | `textures.big` / `movies.big` / `bonus.big` / `e3.big` — archive name cluster |
| import names | — | `CreateFileW`, `ReadFile`, `LoadLibraryExW`, `GetProcAddress` — file-IO + dynamic-load surface |

Structural observations (unverified = UNKNOWN): `.data` virtual size `0x5c21758` (~96 MB) vs raw `0x15000` — large zero-init globals, typical engine state; single export, name UNKNOWN (parse did not resolve — confirm in Ghidra or record UNKNOWN).

## Requirements

- R1: The survey SHALL name the ONE primary binary with import/string evidence (done above; apply re-confirms after import).
- R2: The survey SHALL list 2–4 candidate follow-up functions/areas, each with ≥1 evidence string (offset + content), as behavior paraphrases — no listings.
- R3: Every Ghidra invocation SHALL be logged (command + wall time); the project SHALL live outside repo and game dirs.
- R4: If any target shows DRM/anti-cheat/packing blocking static analysis, that line STOPS and the survey reports it (no defeating anything). None observed in Phase 1 (plain PE imports, readable strings).
- R5: No decompiled code, disassembly, Ghidra DBs, game bytes, or game paths beyond the install-relative binary names SHALL be committed.
- R6: Existing viewer behavior SHALL be byte-for-byte unchanged (docs-only slice; `cargo test` is a no-op baseline proving nothing broke).

## Tests: N/A

No code ships, so no unit tests apply. Verification instead: (a) reproducible command log with wall times any reviewer can re-run; (b) second-eyes spot check of ≥2 evidence strings with an independent strings tool; (c) `cargo test` green + `git status` clean of non-whitelisted files.

## Next step

Design (`design.md`) fixes the bounded headless method; tasks (`tasks.md`) make it atomic.

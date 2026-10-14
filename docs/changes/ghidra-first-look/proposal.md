# Proposal: ghidra-first-look

Survey the SR1 (PC remaster) binaries with Ghidra to FIND where game logic lives — a bounded scoping probe, not unbounded decompilation. Deliverable is a survey doc: binary-role table, analysis inventory, candidate function list with string evidence, and follow-up slice proposals.

## Quick path

1. Primary target is `1\sr1.dll` (proven below) — Ghidra headless import + auto-analysis, both time-boxed and logged.
2. Xref bigfile/file-IO strings to code; name 2–4 follow-up areas as behavior paraphrases (never pasted listings).
3. Land the survey under `docs/notes/` + hygiene proof (`git status` / `check-ignore` gate); `cargo test` stays green as a no-op baseline.

## Decisions

| Topic | Decision |
|---|---|
| Primary target | `1\sr1.dll` (2,416,008 bytes, AMD64, GUI subsystem): owns every `*.big` filename string plus the only bigfile error format string; zero platform (Steam/Galaxy) references. |
| Launcher shell | `SRX.exe` (989,576 bytes, AMD64) contains the literal `1\sr1.dll`, Steam interface strings, a `-nosteam` flag, and `SR1_*` achievement IDs — it loads the game module, it is not the game. Secondary at most. |
| Out of scope | `Galaxy64.dll` (networking imports: WS2_32/WININET/CRYPT32, 31 exports) and `steam_api64.dll` (imports `Galaxy64.dll`, 2084 exports: a GOG shim) — platform shims, excluded. SR2 side (`2\`) excluded per slice scope. |
| Headless feasibility | PROVEN: `analyzeHeadless` import-only run on `sr1.dll` completed in 26.1 s wall (Ghidra 12.1.4, Temurin JDK 25). No GUI fallback needed. Full auto-analysis still time-boxed in apply. |
| Clean-room rule | Behavior notes only ("loader reads X then Y"). No decompiled code, no disassembly listings, no Ghidra project databases in the repo — the whitelist `.gitignore` already excludes `*.gpr`/`*.rep` (verified by `check-ignore`). Ghidra projects live under `%TEMP%`, never in repo or game dirs. Game binaries are read-only inputs, never modified or copied into the repo. |
| Community decomps | `soul-re` / KAIN2 PS1 sources stay BEHAVIOR reference only (different binary, different CPU) — never conflate addresses or structs. |

## Checklist

- [ ] Survey names concrete follow-up slices with string evidence, not vague "analyze more".
- [ ] Every probed offset/length documented; anything unverified marked UNKNOWN.
- [ ] Single PR ≤400 authored lines (docs-heavy slice, almost no code).

## Next step

Spec (`spec.md`) gates the survey shape; design (`design.md`) fixes the bounded method; tasks (`tasks.md`) list the atomic apply units.

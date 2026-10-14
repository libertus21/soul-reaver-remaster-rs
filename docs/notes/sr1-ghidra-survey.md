# SR1 Ghidra survey (`ghidra-first-look` apply)

Bounded first-look survey of the SR1 (PC remaster) game binary with Ghidra 12.1.4.
Deliverable is BEHAVIOR notes only: functional paraphrases, string references with
RVAs, call-graph shapes. No disassembly, no decompiled listings, no game bytes.

- Primary target: `1\sr1.dll` (2,416,008 bytes, AMD64), read-only input, never copied into the repo.
- Ghidra project lives at `%TEMP%\opencode\ghidra-probe\probe` (outside repo and game dirs).
- Image base: `0x180000000`. All addresses below are RVAs (address minus image base).

## Coverage box (time-boxed auto-analysis: COMPLETE, box not hit)

| Item | Result |
|---|---|
| Default auto-analysis (`-process sr1.dll`) | SUCCEEDED, 153 s analysis / 174 s wall (30-min box not hit) |
| Functions recovered | 5,243 |
| External functions | 86, all `KERNEL32.DLL` (KERNEL32-only verdict confirmed) |
| Defined strings | 2,445 (25 matched the evidence filter) |
| Memory blocks | 9 (`.text`, `.rdata`, `.data`, `.pdata`, `.fptable`, `.rsrc`, `.reloc` + headers/tdb) |
| `.data` span | RVA `0x221000`, size `0x5c21758` (~96 MB, mostly zero-init globals — confirmed) |
| Protections hit | NONE — plain PE, readable strings/imports; no DRM, packing, or anti-cheat observed |
| Symbols | Absent (no PDB; headless log notes "Skipping PDB processing"), so functions keep `FUN_` names |

## Binary-role table (spec R1: re-confirmed after import)

| Binary | Role verdict | Key evidence |
|---|---|---|
| `1\sr1.dll` | PRIMARY — game logic + archive loading | Owns every `*.big` filename string, both bigfile error formats, file-IO imports; entry RVA `0x131ff0`; single export now resolved as `run` @ `0x11fb10` |
| `SRX.exe` | Launcher shell (secondary at most) | Literal `1\sr1.dll` (file offset `0x85570`), `-nosteam`, `SteamUser022`, `SR1_*` IDs; zero bigfile references |
| `Galaxy64.dll` / `steam_api64.dll` | OUT — platform shims | Unchanged from spec; not opened in Ghidra |

## Reproducibility (spec R3: every invocation logged)

| # | Command (read-only inputs) | Wall time |
|---|---|---|
| 1 | `analyzeHeadless <probe-dir> probe -import 1\sr1.dll -noanalysis` (Phase 1, plan) | 26.1 s |
| 2 | `analyzeHeadless <probe-dir> probe -process sr1.dll -postScript SurveyProbe.java -scriptPath <survey-dir> -analysisTimeoutPerFile 1500` | 174 s (analysis 153 s) |
| 3 | Same as 2 plus `-noanalysis` (probe-script fix re-run; analysis already saved) | 18 s |
| 4 | `python3 spotcheck.py` (independent raw-byte spot check, § Spot check) | < 5 s |
| 5 | `python3 srxcheck.py` (launcher-literal check, § F2) | < 5 s |
| 6 | `cargo test` (no-op baseline, § Tests) | < 60 s |

Benign notes: `GhidraMCP Module.manifest` warnings on every headless start (non-blocking);
first probe run of `SurveyProbe.java` failed to compile on `getExternalFunctionCount()`
(not present in 12.1.4) — fixed by manual counting, re-ran clean. Passing a script
output path as an extra `-postScript` argument is rejected (`Bad argument`), so the
script writes its fixed TEMP path.

## String-evidence inventory (offsets re-confirmed; RVAs new from Ghidra)

File offsets re-checked byte-for-byte against Phase 1; Ghidra RVAs differ because
RVAs are post-load addresses (`.rdata` loads at RVA `0x1a0000`).

| File offset | RVA | Content | Referenced from within |
|---|---|---|---|
| `0x1a1d60` | `0x1a2960` | `bigfile.dat` | export `run`@`0x11fb10` (+`0x8d`), FUN@`0xd14f0` |
| `0x1a57b0` | `0x1a63b0` | `Can't read from bigfile: %s` | FUN@`0x12f620` |
| — (new) | `0x1a63e8` | `Can't load big file named "%s"` | FUN@`0x126300` |
| `0x200958` | `0x201558` | `textures.big` | pointer table in `.data` @`0x230c78` |
| `0x200968` | `0x201568` | `movies.big` | pointer table in `.data` @`0x230c90` |
| `0x200978` | `0x201578` | `bonus.big` | pointer table in `.data` @`0x230ca8` |
| `0x200984` | `0x201584` | `e3.big` | pointer table in `.data` @`0x230cc0` |
| — (new) | `0x1a32b8` | `save %d is too big! (type %d)` | FUN@`0x101600` |
| — (new) | `0x1a5530` / `0x22d534` | `movie%d` / `movie1` | FUN@`0x11e330` / FUN@`0x119ae0` |
| — (new) | `0x1a2888` / `0x1a28c0` | `PIX\BIG_BKGDMENU_OG` / `PIX\BIG_BKGDMENU_HD` | no code xrefs found (UNKNOWN use) |

## F1 — bigfile loader candidates (behavior paraphrases)

- Error-path pair: FUN@`0x12f620` touches the read-failure format string and
  FUN@`0x126300` touches the load-failure format string — the two halves of an
  open-then-read loader reporting failures at each stage (paraphrase, not verified bodies).
- Open cluster: `CreateFileW` is called from FUN@`0x197b50` (3 call sites),
  FUN@`0x197484`, and FUN@`0x19c5c4` (remaining callers are CRT console helpers).
- Seek/size/read cluster: FUN@`0x189e50` calls BOTH `SetFilePointerEx` and
  `GetFileSizeEx` (seek-plus-size pair); `ReadFile` callers FUN@`0x16bfec`,
  `0x1852e8`, `0x184a00`, `0x184b98`, `0x184dfc` and further `SetFilePointerEx`
  callers `0x185f04`/`0x185fb4`/`0x186078` sit in one `0x184xxx`–`0x189xxx`
  neighborhood — the shape of an archive random-access reader (inference).
- Discovery cluster: `FindFirstFileExW` callers FUN@`0x18b0c4`, `0x18ad64`,
  `0x18a658` — directory scanning adjacent to the reader neighborhood,
  plausibly archive discovery (inference).

## F2 — entry bootstrap (partially answered)

- The single export's name is resolved: **`run` @ `0x11fb10`** (Phase 1 UNKNOWN).
  It references `bigfile.dat` within its first `0x8d` bytes, which reads as the
  launcher-to-game handoff using the default archive name (inference).
- Entry `0x131ff0` (symbol + function both named `entry`, confirmed) sits inside a
  `LoadLibraryExW`/`GetProcAddress`/`FreeLibrary` caller cluster whose neighbors
  are CRT delay-load/thunk helpers (e.g. `__acrt_uninitialize_winapi_thunks`) —
  the dynamic-load surface is CRT machinery, not game bootstrap (inference).
- Launcher side: `SRX.exe` carries the `1\sr1.dll` literal but no plain-text `run`
  reference at string level, so the exact launcher→`run` linkage is UNCONFIRMED
  (needs an import-table parse of `SRX.exe`; proposed as slice S3, SR1 scope permitting).

## F3 — engine global-state layout (layout only, no invented semantics)

- The four archive-name strings are pointed to by a contiguous pointer table in
  `.data` (`0x230c78`–`0x230cc0`) — the archive selection state (layout fact).
- Movie slots `movie 1`–`movie 6` plus `Movie Areas...` are referenced from `.data`
  tables (`0x225558`–`0x225620`, `0x222c30`) — menu/movie selection state (layout fact).
- Bonus state: `bonus`, `BONUS_CONTENT`, `BM_BONUS_CONTENT`, `BM_SR2BONUS` strings
  referenced from `.data` (`0x226e20`, `0x22fea8`, `0x22ff10`, `0x230b60`); the
  `save %d is too big! (type %d)` message in FUN@`0x101600` evidences a save-type
  bounds check touching that area (paraphrase).

## Follow-up slice candidates (spec R2)

| ID | Slice | String evidence |
|---|---|---|
| S1 | Bigfile open/read/error path: pin the loader among FUN@`0x126300`/`0x12f620` (error halves), `0x197b50`/`0x197484` (open), `0x184xxx`–`0x189xxx` (seek/size/read) | `Can't read from bigfile: %s`, `Can't load big file named "%s"`, `bigfile.dat` |
| S2 | Archive discovery + selection table: directory-scan cluster `0x18a658`–`0x18b0c4` feeding the `.data` table @`0x230c78`; includes `e3.big`/`bonus.big` selection | `textures.big` / `movies.big` / `bonus.big` / `e3.big` |
| S3 | Launcher handshake via export `run`: confirm from `SRX.exe`'s import table how `run`@`0x11fb10` is reached; default-archive handoff | `bigfile.dat` (referenced +`0x8d` into `run`), `1\sr1.dll` in launcher |

F4 (launcher CLI: `-nosteam`, `SR1_*` IDs) stays optional per plan — only if playtest needs demand it.

## Spot check (Tasks 2 verify: independent second-eyes tool)

`spotcheck.py` re-read the binary raw (no Ghidra): all six Phase-1 file offsets
CONFIRMED byte-for-byte (`bigfile.dat` `0x1a1d60`, error string `0x1a57b0`,
archive cluster `0x200958`–`0x200984`); all ten file-IO/dynamic-load import names
found (`CreateFileW`, `ReadFile`, `SetFilePointerEx`, `GetFileSizeEx`,
`FindFirstFileExW`, `LoadLibraryExW`, `GetProcAddress`, `FreeLibrary`).

## Tests (spec R6: no-op baseline proving nothing broke)

`cargo test`: **83 passed, 0 failed** — viewer behavior byte-for-byte unchanged
(docs-only slice, no code touched).

## Hygiene proof (Task 4 gate; spec R5)

- `git status --short` on the feature branch shows ONLY this new `*.md` file
  (plus the 4 planning `*.md` files merged from the plan branch) — no Ghidra DBs,
  no binaries, no listings.
- `git check-ignore -v probe.gpr probe.rep tmp/survey.gpr` → all three ignored via
  `.gitignore:5:*` (blanket whitelist rule).
- `git check-ignore docs/notes/sr1-ghidra-survey.md` → exit 1, NOT ignored (committable as intended).
- Ghidra projects/commands/outputs stayed under `%TEMP%\opencode\` (probe + survey
  sibling dirs); game binaries only ever read.

## Workload

- This survey: ~150 authored lines (1 new file, docs only, no code).
- Plan docs (merged from `feature/ghidra-first-look-plan`): 166 lines across 4 files.
- PR total forecast: ~316 authored lines — inside the 400-line single-PR budget.

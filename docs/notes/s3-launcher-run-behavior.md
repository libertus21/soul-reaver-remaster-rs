# Behavior notes: s3-launcher-run launcher-to-engine handshake (apply-time)

Tasks 1-2 of `docs/changes/s3-launcher-run/tasks.md`. Behavior paraphrases
only — no disassembly, no decompiled code, no Ghidra databases in the repo.
Raw function addresses are deliberately WITHHELD per repo hygiene
(`AGENTS.md` rule 3); every role label below was assigned fresh at apply
time and is re-derivable from the strings alone (see Re-run). No game
bytes, absolute paths, hashes, or retail offsets are recorded here.

## Method (bounded, headless, 2 passes used of 2)

- Ghidra `12.1.4` headless. A new TEMP project under
  `%TEMP%\opencode\ghidra-srx` (outside the repo and the game dir) held
  `SRX.exe` imported read-only; auto-analysis COMPLETE (76 s, box not hit).
  The existing `%TEMP%\opencode\ghidra-probe` project (engine) was reopened
  read-only with analysis OFF (`-noanalysis`).
- Pass 1 (launcher): enumerate the import table (external symbols and
  external functions: library + name), re-pin the launcher literal by
  string xref, and list `LoadLibrary*` / `GetProcAddress` call sites with
  the strings referenced at each site.
- Pass 2 (argument resolution): for each `GetProcAddress` call site,
  resolve the second argument (the procedure-name operand) to its raw
  bytes; for the engine, re-pin the `run` export and the `bigfile.dat`
  xref.
- Box closed by the pass budget (2 of 2 used). No DRM, anti-cheat, or
  packing was encountered on this line.

## Re-pin (fresh labels + counts)

- Launcher `SRX.exe`: `function_count=1205`, `external_symbol_count=228`,
  `external_function_count=215`, `ordinal_import_count=0`.
- Engine `sr1.dll`: `export_count=3` (`run`, `Ordinal_1`, `entry`),
  `run_export_count=1`, `matched_string_count=1` (the default archive name).

## Per-claim verdicts

| # | Claim | Verdict | Evidence (class) |
|---|---|---|---|
| L1 | SRX.exe statically imports `run` from `sr1.dll` | NOT CONFIRMED (no static import) | Import-table parse: `sr1_library_symbol_count=0`, `sr1_external_function_count=0`, `ordinal_import_count=0`; the only `run`-named import is `STEAM_API64.DLL!SteamAPI_RunCallbacks` (unrelated). Class L1 static import — absent. |
| L2 | SRX.exe dynamically loads `sr1.dll` and resolves `run` | CONFIRMED | A `LoadLibraryA` call site in the launcher-bootstrap function references the string `1\sr1.dll`; a `GetProcAddress` call site in the SAME function references the string `run`. Class L2 dynamic load — import-table absence plus the call-edge pair. |
| L3 | The linkage is L1 or L2 | CONFIRMED (dynamic) | L2 CONFIRMED; L1 absent. |
| L4 | `run` is the engine entry receiving the default archive name | CONFIRMED | Engine export `run` (also exported as ordinal 1) references the string `bigfile.dat` within its first bytes; the re-pin reproduces the survey. Class L4 handoff. |
| L5 | Engine startup contract | PARTIAL | The handshake method is dynamic (L2) and the default-archive handoff is observable (L4). What `run` does after entry is UNKNOWN (static only; nothing executed). |

Adjacent observation (same launcher-bootstrap function, SR1 focus kept):
the launcher also resolves a second engine procedure name `exec` from the
same library, and repeats the load-and-resolve shape for the sibling
engine library. Neither changes the L1-L5 verdicts.

## Engine startup contract (bounded paraphrase)

The launcher dynamically loads the engine library and resolves its entry
by name: it calls `LoadLibraryA` with the engine-library literal, then
`GetProcAddress` with the string `run` (the engine's single export, also
ordinal 1). The engine entry `run` references the default archive name
`bigfile.dat` within its first bytes — the launcher-to-game default-archive
handoff. What `run` does with the archive name after entry is UNKNOWN
(static analysis only; nothing was executed).

## Re-run (second pair of eyes)

1. Open the TEMP launcher project read-only, `-noanalysis`.
2. Enumerate external symbols and external functions; confirm no library
   named `sr1.dll` and no ordinal imports.
3. Search defined strings for `sr1.dll`; follow the xref to the
   `LoadLibraryA` call site; in the same function, find the
   `GetProcAddress` call site whose second argument resolves to the raw
   string `run`.
4. Open the TEMP engine project read-only, `-noanalysis`; confirm the
   export named `run` and its `bigfile.dat` xref.
5. The verdict table above must reproduce exactly from those outputs.

## Legal

Binaries read-only; Ghidra projects outside the repo under `%TEMP%`. No
game bytes, absolute paths, hashes, or retail offsets are recorded here.
The short string quotes above are evidence quotes already present in the
committed survey.

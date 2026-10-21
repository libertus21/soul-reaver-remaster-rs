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

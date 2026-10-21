# Design: s3-launcher-run

Import-table first, xref second: parse SRX.exe's external-symbol table, re-pin
the launcher literal by xref, trace the load call sites, then record the
confirmed handshake and engine startup contract. Behavior notes only; no engine
code changes.

## Quick path

`survey + S1/S2 notes` -> import SRX.exe into a TEMP project -> import-table
parse (library/name/ordinal) -> string xref re-pin -> load call-site trace ->
behavior notes -> hygiene gate.

## Method (bounded, time-boxed)

| Step | Bound | Tool |
|---|---|---|
| SRX.exe project | New TEMP project under `%TEMP%\opencode\ghidra-srx` (sibling of the existing `ghidra-probe`); import SRX.exe read-only; auto-analysis (989 KB, fast). Never the repo or the game dir. | `analyzeHeadless` 12.1.4 (smoke-tested at plan time — usage prints; benign `GhidraMCP Module.manifest` warnings) |
| Import-table parse | Enumerate external symbols with library + name + ordinal; flag any library named `sr1.dll` and any import named `run` (or an ordinal resolving to `run`). | `getExternalManager()` / symbol table |
| String xref re-pin | Search defined strings for the launcher literal; follow xrefs to referencing functions. Re-pin `sr1.dll`'s `run` export and `bigfile.dat` in the existing probe project (read-only, `-noanalysis`). | findStrings + xref |
| Load call-site trace | List callers of `LoadLibrary` / `LoadLibraryEx` / `LoadLibraryA` / `GetProcAddress`; for each, list the strings referenced at the call site (the literal / `run`). | Call-graph + xref queries |
| Box | Max 2 probe passes, 45-minute wall box. The box closes by pass budget; unresolved claims stay UNKNOWN. | Addendum records the no-evidence result per claim |
| Note shape | New file `docs/notes/s3-launcher-run-behavior.md`: fresh labels + counts, the import entry or load call sites, per-claim CONFIRMED/UNKNOWN verdict, engine startup contract paraphrase. | Behavior paraphrases; short string quotes as evidence |

## Details

| Topic | Decision |
|---|---|
| Import-table parse | Enumerate the launcher's external symbols; for each, record library name, import name (or ordinal), and the resolved target. The presence of `sr1.dll` as a library is the L1 signal; the import name/ordinal is the L2 signal. |
| Dynamic-load trace | For each `LoadLibrary*` caller, list the strings referenced at the call site; for each `GetProcAddress` caller, list its referenced arguments. A caller referencing the launcher literal plus a caller referencing `run` (name or ordinal) is the L2 signal. |
| Re-pin | Every address is runtime-resolved from its string xref. Survey RVAs (for example the `run` export address) are starting hints only — never written into notes or code. |
| Engine startup contract | Record only what the evidence supports: the handshake method (L1/L2) and the default-archive handoff (`bigfile.dat` referenced inside `run`). What `run` does after entry stays UNKNOWN unless a bounded paraphrase is justified. |
| Notes file | New `docs/notes/s3-launcher-run-behavior.md` (no rewrite of S1/S2 notes). Fresh labels + counts, per-claim verdict, no-evidence records, contract paraphrase. |
| Dependencies | ZERO new shipped crates; no engine code changes. |

## Deliberately NOT built

F4 launcher CLI (`-nosteam`, `SR1_*` IDs), Steam/achievement behavior, `sr1.dll`
body decompilation, any runtime execution, any GUI Ghidra sessions, any
committed game data, any box extension without user approval.

## Checklist

- [ ] Reviewer can state the import-table parse procedure and why string presence
  alone is not proof.
- [ ] Reviewer can trace each CONFIRMED claim to a named import entry or load
  call site (or its explicit no-evidence hold).
- [ ] Reviewer can state what stays UNKNOWN (runtime behavior after entry; F4
  CLI).
- [ ] No retail addresses, listings, or game bytes appear in the notes.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

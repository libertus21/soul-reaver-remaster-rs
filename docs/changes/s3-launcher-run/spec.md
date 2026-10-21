# Spec: s3-launcher-run

Confirm how SRX.exe reaches the `run` export of `sr1.dll`, and document the
engine startup contract that follows. The survey resolved the export name and
the launcher literal; S3 must supply the import-table / linkage evidence.
Behavior notes only.

## Quick path

1. Apply parses SRX.exe's import table and re-pins the launcher literal by xref
   (Task 1); each claim becomes CONFIRMED or stays UNKNOWN (Task 2).
2. Behavior notes record the confirmed handshake and the engine startup contract
   (Task 2); `cargo test` is the no-op gate.

## Original behavior (S3 target)

The survey established: `sr1.dll`'s single export is `run` (referenced
`bigfile.dat` within its first bytes — the default-archive handoff), and SRX.exe
carries the `1\sr1.dll` literal but no plain-text `run` at string level. The
linkage is UNCONFIRMED.

| # | Claim | Evidence required | Verdict |
|---|---|---|---|
| L1 | SRX.exe statically imports `run` from `sr1.dll` | External-symbol entry for library `sr1.dll` with import name `run`, OR an import-by-ordinal entry for `sr1.dll` whose ordinal resolves to `run` | UNKNOWN until the import table is parsed |
| L2 | SRX.exe dynamically loads `sr1.dll` and resolves `run` | A `LoadLibrary*` call site whose referenced string is the launcher literal, plus a `GetProcAddress` call site whose referenced argument is `run` (name) or the ordinal | UNKNOWN until the call sites are traced |
| L3 | The linkage is L1 or L2 | At least one of L1/L2 CONFIRMED | UNKNOWN until L1/L2 |
| L4 | `run` is the engine entry receiving the default archive name | `sr1.dll` export name `run` plus a `bigfile.dat` reference inside its first bytes (re-pin by string xref, not hardcode) | CONFIRMED at survey level; re-pin to reproduce |
| L5 | Engine startup contract (what `run` does with the archive name) | Behavior paraphrase of `run`'s archive-name use; no body listing | UNKNOWN unless a bounded paraphrase is justified |

## Evidence requirements per claim

| Claim | Minimum evidence |
|---|---|
| L1 | Import-table entry: library name `sr1.dll`, import name `run` (or ordinal plus the resolved export name). Cite the symbol name and ordinal. |
| L2 | Two call sites: `LoadLibrary*` referencing the launcher literal; `GetProcAddress` referencing `run` (or the ordinal). Cite the call-site function and the referenced string. |
| L4 | `sr1.dll` export `run`; `bigfile.dat` xref inside `run`'s first bytes. |
| L5 | A one-line behavior paraphrase only if L1/L2 is CONFIRMED and the archive-name use is observable; else UNKNOWN. |

## UNKNOWN rules

- A claim with no import entry or call edge stays UNKNOWN; the notes record the
  explicit no-evidence result (for example `import_sr1_found=0`,
  `loadlib_callers=0`).
- No address from the survey is treated as a reference; re-pin by string xref at
  runtime.
- Static analysis only; nothing is executed, so runtime behavior (what `run`
  actually does after entry) stays UNKNOWN beyond the paraphrase.
- If DRM/anti-cheat/packing blocks this line, the line STOPS and the slice
  reports it (none observed to date).

## Requirements

- R1: Apply SHALL parse SRX.exe's import table and re-pin every string/address by
  xref at runtime; no survey RVA SHALL appear as a reference in notes or code.
- R2: Every linkage verdict SHALL cite an import entry or a load call site with
  its referenced string; string presence alone SHALL NOT count as proof.
- R3: Any claim still UNKNOWN after the bounded box stays UNKNOWN and is recorded
  with its no-evidence result.
- R4: Behavior notes SHALL be paraphrases only — no disassembly, no decompiled
  listings, no Ghidra DBs in the repo.
- R5: No game bytes, paths, hashes, or retail offsets SHALL be committed;
  binaries stay read-only.
- R6: If DRM/anti-cheat/packing blocks analysis on this line, that line STOPS and
  the slice reports it (none expected).

## Acceptance scenarios

- [ ] **Static import:** SRX.exe import table shows `sr1.dll!run` (name or
  ordinal) -> L1 CONFIRMED; notes name the import entry.
- [ ] **Dynamic load:** `LoadLibrary*` + `GetProcAddress` call sites reference
  the launcher literal and `run` -> L2 CONFIRMED; notes name both call sites and
  strings.
- [ ] **Unknown:** neither L1 nor L2 holds inside the box -> L3 UNKNOWN; notes
  record the no-evidence result and the contract stays partial.
- [ ] **Docs-only:** `cargo test` green (no code touched); `git status --short`
  shows only the plan docs plus behavior notes; no listings, DBs, or game data
  committed.

## Next step

Design (`design.md`) fixes the import-table parse + xref method, the time box,
and the note shape; `tasks.md` makes apply atomic.

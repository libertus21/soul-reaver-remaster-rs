# Proposal: s3-launcher-run

Confirm the launcher-to-engine handshake (SRX.exe reaching the `run` export of
`sr1.dll`) and document the engine startup contract. The survey resolved the
export name (`run`) and the launcher literal (`1\sr1.dll`) but left the exact
linkage UNCONFIRMED; S3 parses the launcher's import table and traces the
reference chain, then records the contract as behavior notes. Docs-only slice:
no engine code changes.

## Quick path

1. Import SRX.exe into a TEMP Ghidra project; parse its import table and re-pin
   the launcher literal by string xref (FUN@ names shift — never hardcode).
2. Trace how `run` is reached: a static import by name/ordinal, or a
   `LoadLibrary*` + `GetProcAddress` pair whose referenced strings are the
   launcher literal and `run`; record import names/ordinals and load call sites.
3. Write `docs/notes/s3-launcher-run-behavior.md` with the confirmed handshake
   and the engine startup contract; anything unproven stays UNKNOWN.

## Decisions

| Topic | Decision |
|---|---|
| Slice scope | S3 only (survey follow-up candidate): launcher-to-engine handshake via export `run` plus the default-archive handoff. S1/S2 loader rows and F4 launcher CLI stay out. SR1 only. |
| Proof rule | A linkage claim is CONFIRMED only on import-table evidence: the launcher's external-symbol entry for `sr1.dll!run` (name or ordinal), OR a load call site whose referenced string is the launcher literal / `run`. String presence alone is not proof — the import entry or call edge is. |
| UNKNOWN rule | If neither a static import entry nor a load call site ties the launcher to `run`, the linkage is UNKNOWN and the contract records the gap. Never guess. |
| Re-pin rule | Every address is re-resolved at runtime from its string xref; survey RVAs are starting hints only, never references in notes or code. |
| Clean-room rule | Behavior notes only. No disassembly, no decompiled listings, no Ghidra DBs in the repo. Binaries read-only; projects under `%TEMP%`. The two short user-visible formats already in the survey are evidence quotes. |
| Budget guard | Single PR <=400 authored lines (plan + behavior notes, docs-only). If the box expires with the linkage unresolved, S3 narrows to the partial evidence and records UNKNOWN — never silently absorbed. |
| Tests | `cargo test` is a no-op baseline (no code touched). No synthetic bytes needed for a docs-only slice. |

## Checklist

- [ ] Every linkage claim cites an import entry (library + name/ordinal) or a load call site with its referenced string.
- [ ] Unresolved claims are marked UNKNOWN with the explicit no-evidence record.
- [ ] Behavior notes name the handshake method and the engine startup contract.
- [ ] Single PR <=400 authored lines; `cargo test` green; `git status` shows only intended files.

## Next step

Spec (`spec.md`) fixes the evidence requirements per claim and the UNKNOWN rules;
design (`design.md`) fixes the import-table parse + xref method and time box;
`tasks.md` lists the atomic apply units.

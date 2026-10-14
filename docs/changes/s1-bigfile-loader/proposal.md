# Proposal: s1-bigfile-loader

Pin down the original's bigfile open/read/error path and bring our reader to parity. First behavior slice from the Ghidra survey: document the original's open→seek→read→error sequence as behavior notes, specify the delta vs our `src/bigfile.rs`, and implement only the parity gaps that fit the single-PR budget (error-message/exit-code alignment + regression tests on synthetic bytes).

## Quick path

1. Re-pin the loader path in Ghidra by STRING XREF at runtime (FUN@ names shift between runs — never hardcode): error pair, open cluster, seek/read cluster.
2. Write behavior notes (`docs/notes/s1-bigfile-loader-behavior.md`): call order, per-step failure conditions, which user-visible message maps to which failure.
3. Align our error messages/exit codes to the confirmed mapping + regression tests on synthetic bytes only; overflow becomes a scoped follow-up.

## Decisions

| Topic | Decision |
|---|---|
| Slice scope | S1 only (survey § Follow-up slice candidates): bigfile open/read/error path. S2 (discovery/selection table) and S3 (launcher handshake) stay out. SR1 only. |
| Re-pin rule | Every function address is re-resolved from its string xref at apply time; survey addresses are starting hints, not references. A stale hardcoded address is a plan violation. |
| Clean-room rule | Behavior notes only ("open fails → load-failure message"). No decompiled code, no disassembly listings, no Ghidra DBs in the repo. Binaries read-only; projects under `%TEMP%`. The two short user-visible error formats are evidence quotes, already in the survey. |
| Message adoption | Adopting the original's user-visible message formats as our Display prefixes is parity, not copying: short functional labels with our own diagnostic detail appended. No original code or data is reused. |
| Budget guard | Single PR ≤400 authored lines (docs + targeted code/tests). Only message/exit-code alignment + tests ship; anything bigger (new decoding, hash-name lookup, other `.BIG`s) is a scoped follow-up, not scope creep. |
| Tests | Synthetic bytes only — hand-built fake bigfiles in temp dirs. The real archive is never touched by tests; game bytes/paths never committed. |

## Checklist

- [ ] Behavior notes name the call order and map each failure to its message, with UNCONFIRMED items marked UNKNOWN.
- [ ] Parity table covers every failure case: missing file, truncated table, entry past EOF, seek/read failure (original vs ours).
- [ ] Apply changes only what the confirmed mapping justifies; deviations are recorded, not silently absorbed.
- [ ] Single PR ≤400 authored lines; `cargo test` green; `git status` shows only intended files.

## Next step

Spec (`spec.md`) fixes the parity table and exit-code scenarios; design (`design.md`) fixes the re-pin method and message-adoption shape; tasks (`tasks.md`) list the atomic apply units.

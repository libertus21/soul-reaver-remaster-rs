# Proposal: s2-read-error-parity

Close the bigfile loader parity end-to-end: trace call edges from the S1-pinned open/seek/read clusters to the two error halves, and align every S1-HELD failure row whose routing the trace PROVES. Anything still unproven stays held under the same exclusion rule — no guessing.

## Quick path

1. Re-pin strings + clusters by STRING XREF at runtime (FUN@ names shift — never hardcode), then trace call edges cluster → error half (bounded, time-boxed: see design).
2. Record verdicts per S1-held row in a behavior-notes addendum: PROVEN (edge observed) or still HELD (no edge).
3. Align `BigfileError` Display prefixes for PROVEN rows only, with synthetic-bytes regression tests; single PR ≤400 authored lines.

## Decisions

| Topic | Decision |
|---|---|
| Slice scope | S2 only: post-open read-error parity (truncated count/table, past-EOF, seek-fail, short-read) + termination re-check. S1 open-stage row stays as-is. S3 launcher handshake stays out. SR1 only. |
| Proof rule | A row aligns ONLY on observed call-edge evidence (caller→callee pair into an error half). Cluster existence alone is not proof — that is exactly what S1 lacked. |
| Exclusion rule | Inherited from S1 spec R3: any row still UNKNOWN after the bounded pass stays UNKNOWN, keeps its current text, and is pinned by a hold test. No speculative alignment. |
| Clean-room rule | Behavior notes only ("seek fails → read-failure message via edge A→B"). No decompiled code, no listings, no Ghidra DBs in the repo. Binaries read-only; projects under `%TEMP%`. The two short user-visible formats are evidence quotes already in the survey. |
| Alignment shape | Same Display-prefix discipline as S1 unless the trace proves a different shape (design § Details). |
| Budget guard | Single PR ≤400 authored lines (plan + addendum + targeted code/tests). If edges resolve only partially, S2 narrows to the resolved rows — never silently absorbs the rest. |
| Tests | Synthetic bytes only — temp-dir fake bigfiles. The real archive is never touched; game bytes/paths never committed. |

## Checklist

- [ ] Every S1-held row has a traced-or-held verdict with named edge evidence or an explicit no-edge record.
- [ ] Code changes map 1:1 to PROVEN rows; held rows keep current text, pinned by tests.
- [ ] Single PR ≤400 authored lines; `cargo test` green; `git status` shows only intended files.

## Next step

Spec (`spec.md`) fixes the per-row verdict targets; design (`design.md`) fixes the edge-tracing method and time box; tasks (`tasks.md`) list the atomic apply units.

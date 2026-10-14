# Spec: s2-read-error-parity

Post-open behavior parity on the bigfile path: our seek/read/index-table failures report through the same message family the original loader routes them to — proven by call-edge evidence, pinned by regression tests on synthetic bytes.

## Quick path

1. Apply traces call edges from the pinned clusters to the error halves (Task 1); each S1-held row becomes PROVEN or stays HELD (Task 2).
2. Code aligns Display prefixes for PROVEN rows only (Task 3); exit contract stays loud-nonzero.
3. Regression tests pin every row — new prefixes where proven, held text where not (Task 4); `cargo test` is the gate.

## Original behavior (S1-confirmed + S2 edge targets)

S1 confirmed the two message families at xref level (each format has exactly one code referrer; open vs seek/size/read clusters are disjoint). S2 must supply the missing call edges. Source: `docs/notes/s1-bigfile-loader-behavior.md` + S2 addendum.

| # | Original stage (behavior paraphrase) | Evidence | User-visible message |
|---|---|---|---|
| O1 | Open fails (S1 CONFIRMED, unchanged) | Load-failure format + `CreateFileW` open cluster | `Can't load big file named "%s"` |
| O2 | Seek/size/read fails after a successful open (family CONFIRMED, per-case routing = S2 target) | Read-failure format + seek/size/read cluster (`SetFilePointerEx`/`GetFileSizeEx`/`ReadFile`) | `Can't read from bigfile: %s` |
| O3 | Process termination on loader failure | UNKNOWN — static headless only; S2 re-checks for any observable, else stays UNKNOWN | UNKNOWN |

## Parity table (per S1-held row; Task 2 verdicts recorded 2026-10-14)

| Failure case | Ours today (`src/bigfile.rs`) | Task 2 verdict → apply action |
|---|---|---|
| Truncated count word / bad count | `not a BIGFILE index: {detail}`, exit 1 | HELD — no count-word reader identified, no edge to either half (addendum § Per-row verdicts); text unchanged, hold test stays |
| Truncated index table mid-read | `not a BIGFILE index: truncated index table`, exit 1 | HELD — no table-read stage identified, no edge (addendum); text unchanged, hold test stays |
| Entry range past EOF (bad index data) | `bad bigfile entry: {detail}`, exit 1 | HELD — no bounds-check → error-half edge, as predicted (addendum); text unchanged, hold test stays |
| Seek failure during entry read | `cannot read bigfile: {os}`, exit 1 | HELD — no seek-cluster → `read-error-half` edge after 2 probe passes (addendum); text unchanged, hold test stays |
| Short read of entry bytes | `cannot read bigfile: {os}`, exit 1 | HELD — no read-cluster → `read-error-half` edge after 2 probe passes (addendum); text unchanged, hold test stays |
| Termination behavior | stderr + exit 1, no window | NO CHANGE expected — static analysis cannot observe termination; record observable only, never invent parity |

Rules: the adopted prefix is the original O2 format with the archive identity substituted; our diagnostic detail survives as a parenthesized suffix (same discipline as S1). Selector (`BadSelector`) and non-DDS (`NotDds`) errors have no original counterpart in scope — UNCHANGED.

## Requirements

- R1: Apply SHALL re-pin every string and cluster by xref at runtime; no survey or S1 function label SHALL appear as a reference in code, tests, or notes except as a historical starting hint.
- R2: Every verdict SHALL cite an observed caller→callee edge or record an explicit no-edge result; cluster existence alone SHALL NOT count as proof.
- R3: Code changes SHALL implement exactly the PROVEN mapping — no speculative alignment for HELD rows.
- R4: Every failure case SHALL keep (or gain) a loud stderr message + nonzero exit; no silent success, no panic, no short read presented as success.
- R5: Tests SHALL use synthetic bytes only (temp-dir fake bigfiles); no game bytes, paths, hashes, or offsets committed.
- R6: If DRM/anti-cheat/packing blocks analysis on this line, that line STOPS and the slice reports it (none observed to date).

## Acceptance scenarios

- [ ] **Proven row:** VACUOUS for this slice — no row resolved to PROVEN, so Task 3 makes zero production-line changes (box rule).
- [x] **Held row:** every row keeps its current prefix (pinned by test in Task 4); stderr still names the entry/detail; exit 1.
- [x] **Happy path unchanged:** valid synthetic index still lists / reads exact bytes; existing `cargo test` suite stays green (88 passed).
- [x] **Unit level:** `cargo test` covers each parity-table row with message-prefix assertions on synthetic bytes (hold pins, Task 4).
- [x] **Legal:** `git status --short` shows only intended source/doc/test files; no listings, DBs, or game data committed.

## Next step

Design (`design.md`) fixes the edge-tracing method, the time box, and the message-adoption shape; `tasks.md` makes apply atomic.

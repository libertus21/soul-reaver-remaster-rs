# Spec: s1-bigfile-loader

Behavior parity on the bigfile path: our `--bigfile --list | --show` reader fails the way the original loader fails — same stage → same message family, nonzero exit — verified by regression tests on synthetic bytes.

## Quick path

1. Apply confirms the stage→message mapping by string xref (Task 2); anything unconfirmed stays UNKNOWN and is NOT implemented.
2. Code aligns our `BigfileError` Display prefixes to the confirmed mapping (Task 3); exit contract stays loud-nonzero.
3. Regression tests pin each failure case to its message + exit code on synthetic bytes (Task 4); `cargo test` is the gate.

## Original behavior (survey-pinned, mapping to-confirm in apply)

Source: `docs/notes/sr1-ghidra-survey.md` §§ F1, String-evidence inventory. The stage→message mapping below is INFERENCE from string roles, not verified bodies — apply Task 2 confirms or corrects each row.

| # | Original stage (behavior paraphrase) | Evidence | Inferred user-visible message |
|---|---|---|---|
| O1 | Open fails (archive cannot be opened) | Load-failure format string + `CreateFileW` open cluster | `Can't load big file named "%s"` |
| O2 | Seek/size/read fails after a successful open | Read-failure format string + seek/size/read cluster (`SetFilePointerEx`/`GetFileSizeEx`/`ReadFile`) | `Can't read from bigfile: %s` |
| O3 | Process termination on loader failure | UNKNOWN — never observed; no exit-code/dialog/return claim is made | UNKNOWN (apply records the observable only) |

## Parity table (per failure case)

| Failure case | Original (per table above) | Ours today (`src/bigfile.rs` + `main.rs`) | Verdict → apply action |
|---|---|---|---|
| Missing file (open fails) | O1 load-failure format (to confirm) | `error: cannot read bigfile: {os}`, exit 1 | DIVERGE (message) → adopt load-failure prefix |
| Truncated count word / bad count | O1 or O2 (to confirm which stage reads it) | `error: not a BIGFILE index: {detail}`, exit 1 | UNKNOWN mapping → adopt confirmed prefix, keep detail suffix |
| Truncated index table mid-read | O2 read-failure format (to confirm) | `error: not a BIGFILE index: truncated index table`, exit 1 | DIVERGE (message) → adopt read-failure prefix |
| Entry range past EOF (bad index data) | UNKNOWN (to confirm) | `error: bad bigfile entry: {detail}`, exit 1 | UNKNOWN → record; adopt read-failure prefix only if mapping confirms |
| Seek failure during entry read | UNKNOWN (`SetFilePointerEx` cluster) | `error: cannot read bigfile: {os}`, exit 1 | UNKNOWN → adopt read-failure prefix (seek is post-open by construction) |
| Short read of entry bytes | O2 read-failure format (to confirm) | `error: cannot read bigfile: {os}`, exit 1 | DIVERGE (message) → adopt read-failure prefix |
| Termination behavior | O3 UNKNOWN | stderr + exit 1, no window | NO CHANGE — record observable; never invent parity |

Rules: the adopted prefix is the original format with the path/name substituted; our diagnostic detail survives as a parenthesized suffix. Selector (`BadSelector`) and non-DDS (`NotDds`) errors have no original counterpart in scope (no `--show` UI exists in the original) — UNCHANGED.

## Requirements

- R1: Apply SHALL re-pin every loader address by string xref at runtime; no survey FUN@ address SHALL appear as a reference in code, tests, or notes except as a historical starting hint.
- R2: Behavior notes SHALL state the open→seek→read→error call order, the per-step failure conditions, and the confirmed stage→message mapping; every unconfirmed item SHALL be marked UNKNOWN.
- R3: Code changes SHALL implement exactly the confirmed mapping — no speculative alignment for UNKNOWN rows.
- R4: Every failure case in the parity table SHALL keep (or gain) a loud stderr message + nonzero exit; no silent success, no panic, no short read presented as success.
- R5: Tests SHALL use synthetic bytes only (temp-dir fake bigfiles); no game bytes, paths, hashes, or offsets committed.
- R6: If DRM/anti-cheat/packing blocks analysis on this line, that line STOPS and the slice reports it (none observed in the survey; inherited from the survey spec R4).

## Acceptance scenarios

- [ ] **Missing file:** `--bigfile <absent> --list` exits 1; stderr starts with the confirmed load-failure prefix naming the path.
- [ ] **Truncated table:** `--bigfile <synthetic-truncated> --list` exits 1; stderr starts with the confirmed read/load prefix per the Task 2 mapping.
- [ ] **Entry past EOF:** `--show` of a synthetic entry whose range overruns EOF exits 1; stderr names the entry; no window opens.
- [ ] **Short read:** entry bytes cut after index parse exits 1 with the read-failure prefix (covered at unit level; CLI manual run optional).
- [ ] **Happy path unchanged:** valid synthetic index still lists / reads exact bytes; existing `cargo test` suite stays green.
- [ ] **Unit level:** `cargo test` covers each parity-table failure with message-prefix + exit-code assertions on synthetic bytes.
- [ ] **Legal:** `git status --short` shows only intended source/doc/test files; no listings, DBs, or game data committed.

## Next step

Design (`design.md`) fixes the re-pin method, the message-adoption shape, and the test strategy; `tasks.md` makes apply atomic.

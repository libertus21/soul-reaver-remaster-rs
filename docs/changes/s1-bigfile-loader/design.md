# Design: s1-bigfile-loader

Cheap confirmation first, smallest diff second: re-pin the loader by string xref, write behavior notes, then change only `BigfileError` Display prefixes (+ tests) to match the confirmed stage→message mapping. Parser, range checks, CLI arms, and exit-1 contract stay untouched.

## Quick path

`survey strings` → runtime xref re-pin → `docs/notes/s1-bigfile-loader-behavior.md` (call order + mapping) → `BigfileError` Display prefix alignment → synthetic-bytes regression tests → `cargo test` + hygiene gate.

## Method (bounded)

| Step | Bound | Tool |
|---|---|---|
| Reuse TEMP Ghidra project | Read-only; reuse `%TEMP%\opencode\ghidra-probe` or a sibling, never repo/game dirs | `analyzeHeadless` (proven 12.1.4); import already saved |
| Re-pin by string xref | Search the two error-format byte strings, follow xrefs to their referencing functions, name the open/seek/read callers around them — paraphrase only | Ghidra findStrings + xref; string content is the key, FUN@ names are runtime labels |
| Call-order reconstruction | Xref + caller naming + error-block shape ONLY; no function-body reading beyond what naming the stage requires, never pasted | Behavior paraphrases into the notes file |
| Termination record | Observable only (exit/dialog/return as seen, or UNKNOWN) — never inferred from message text | Notes file, O3 row |
| Message alignment | Display-prefix swap only; variants, parser, `read_entry`, CLI arms unchanged | `src/bigfile.rs` edit, ~10-30 lines |

Proven environment (inherited): Ghidra 12.1.4 headless works (re-verified at plan time — usage prints, benign `GhidraMCP Module.manifest` warnings); survey DB + scripts under `%TEMP%\opencode\`.

## Details

| Topic | Decision |
|---|---|
| Behavior notes file | NEW `docs/notes/s1-bigfile-loader-behavior.md`: open→seek→read→error call order, per-step failure conditions, confirmed stage→message mapping, O3 termination observable, UNKNOWN list. Behavior paraphrases; short format-string quotes as evidence. |
| Message shape | `Can't load big file named "{path}"` for pre-open failures; `Can't read from bigfile: {path}` for post-open failures; our current detail appended as ` ({detail})` suffix. `main.rs` keeps its `error: ` + exit-1 wrapper — only the `BigfileError` Display strings change. |
| Variant mapping (provisional — Task 2 confirms) | `Io` on open/metadata → load-failure prefix; `Io` on seek/read_exact + truncated-table `NotBigfile` → read-failure prefix; bad-count `NotBigfile` → whichever stage Task 2 confirms; `BadEntry` → read-failure prefix only if confirmed, else unchanged text. `BadSelector`/`NotDds` UNCHANGED (no original counterpart). |
| Exit codes | Ours stays exit 1 everywhere (spec R4). If Task 2 observes a concrete original termination behavior that is cheap to mirror, record it as a follow-up — do NOT change exit semantics on inference. |
| Tests | Extend the existing `mod tests` in `src/bigfile.rs` following its synthetic-container pattern: one test per parity-table failure asserting the Display prefix + (at CLI level, `main.rs` `cli_tests` style) `ExitCode(1)`. New tests: missing-file message, truncated-table message, past-EOF message, short-read message. `tempfile` already a dev-dep; no new crates. |
| Dependencies | ZERO new shipped crates; no decoder/window changes. |

## Deliberately NOT built

Hash-name reverse lookup, non-DDS decoding, other `.BIG` files, archive discovery/selection (S2), launcher handshake (S3), exit-semantics changes, any GUI Ghidra sessions, any committed game data.

## Checklist

- [ ] Reviewer can state the re-pin procedure and why no address is hardcoded.
- [ ] Reviewer can trace each Display prefix to its confirmed spec row (or its UNKNOWN hold).
- [ ] Reviewer can state what stays unchanged (parser, range checks, CLI, exit codes, selector/DDS errors).
- [ ] `offset + length` validation and never-write-to-disk rules are untouched.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

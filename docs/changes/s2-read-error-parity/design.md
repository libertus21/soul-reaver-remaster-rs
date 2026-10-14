# Design: s2-read-error-parity

Edges first, smallest diff second: re-pin by string xref, trace caller→callee edges from the pinned clusters into the two error halves, record proven/held verdicts, then change only the Display prefixes the trace justifies. Parser, range checks, CLI arms, and exit-1 contract stay untouched.

## Quick path

`survey strings + S1 notes` → runtime xref re-pin → edge trace (callers/callees, max 2 probe passes) → behavior-notes addendum (proven/held per row) → Display prefix alignment for proven rows → synthetic-bytes regression tests → `cargo test` + hygiene gate.

## Method (bounded, time-boxed)

| Step | Bound | Tool |
|---|---|---|
| Reuse TEMP Ghidra project | Read-only; reopen `%TEMP%\opencode\ghidra-probe` (or sibling), `-noanalysis`, never repo/game dirs | `analyzeHeadless` 12.1.4 (re-verified at plan time — usage prints, benign `GhidraMCP Module.manifest` warnings) |
| Re-pin by string xref | Search the two error-format strings + default archive name; follow xrefs to referrers; re-list open/seek/read cluster callers with per-function site counts | findStrings + xref; string content is the key, function labels are runtime-only |
| Edge trace (THE new work vs S1) | From each cluster function, list callees; from each error-half function, list callers; record every caller→callee pair connecting cluster → error half. Max 2 probe passes, 45 min wall box. No function-body reading beyond what naming the stage requires, never pasted | Call-graph queries only; paraphrases into the addendum |
| Box expiry | If edges do not resolve inside the box, unresolved rows stay HELD — no box extension without an explicit user call; S2 narrows to resolved rows | Addendum records the no-edge result per row |
| Message alignment | Display-prefix swap only for PROVEN rows; variants, parser, `read_entry`, CLI arms unchanged | `src/bigfile.rs` edit, ~10-30 lines |

## Details

| Topic | Decision |
|---|---|
| Behavior-notes addendum | Append to `docs/notes/s1-bigfile-loader-behavior.md` (new section, no rewrite of S1 rows): fresh function labels + xref counts, every observed cluster→error-half edge as a caller→callee pair, per-row PROVEN/HELD verdict, termination re-check result. Behavior paraphrases; short format-string quotes as evidence. |
| Message shape | Proven post-open failures adopt `Can't read from bigfile: "{path}" ({detail})` — same Display-prefix discipline as S1 (original format + our detail as parenthesized suffix). The exact `%s` substitution (full path vs bare name) follows whatever the trace proves; default is the path unless proven otherwise. `main.rs` keeps its `error: ` + exit-1 wrapper. |
| Variant mapping (provisional — Task 2 confirms) | Seek-failure `Io` + short-read `Io` → read-failure prefix if their cluster→read-error-half edges are observed. Truncated-count/table `NotBigfile` → read-failure prefix only if the count/table read stage is identified AND its error edge is observed. `BadEntry` (past-EOF) → read-failure prefix only if a bounds-check→error-half edge is observed, else unchanged. `BadSelector`/`NotDds` UNCHANGED (no original counterpart). |
| Exit codes | Ours stays exit 1 everywhere (spec R4). Termination stays UNKNOWN unless the trace surfaces a new observable — record only, never invent. |
| Tests | Extend `mod tests` in `src/bigfile.rs` following the synthetic-container pattern: proven rows assert the new `Can't read from bigfile:` prefix; held rows keep asserting current text. CLI-level `ExitCode(1)` assertions in `main.rs` style where cheap. No new crates. |
| Dependencies | ZERO new shipped crates; no decoder/window changes. |

## Deliberately NOT built

Hash-name reverse lookup, non-DDS decoding, other `.BIG` files, archive discovery/selection, launcher handshake (S3), exit-semantics changes, any GUI Ghidra sessions, any committed game data, any box extension without user approval.

## Checklist

- [ ] Reviewer can state the edge-tracing procedure and why cluster existence alone is not proof.
- [ ] Reviewer can trace each Display prefix to a named caller→callee edge (or its explicit no-edge hold).
- [ ] Reviewer can state what stays unchanged (parser, range checks, CLI, exit codes, selector/DDS errors, S1 open-stage row).
- [ ] `offset + length` validation and never-write-to-disk rules are untouched.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

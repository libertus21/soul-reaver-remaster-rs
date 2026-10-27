# Tasks: anim-binding

Docs-only analysis slice. There is nothing to implement: the investigation
answered the question and the answer is the no-go. `test_command` for every
phase is `cargo test`, used only as a no-op baseline. Per the repo
convention, the planning bundle is committed on the planning branch and is
**not** counted in an apply forecast - and here there is no apply.

## Review Workload Forecast

Split as required: production / tests (x1.5) / docs / total.

| Bucket | Lines | Note |
|---|---|---|
| Production | **0** | no `src/` change, no CLI change, no format change |
| Test code | 0 x 1.5 = **0** | no new behavior to prove; existing suite is the baseline |
| Docs - planning bundle | **511** | note 142, proposal 79, spec 90, design 100, this file 100 |
| Docs - apply delta | **0** | nothing is generated at apply; the note is final |
| **TOTAL authored by this change** | **511** | all Markdown, all review-facing |

- **Per-PR ceiling:** 400 authored lines applies to *implementation* PRs.
  This change has no implementation PR; the bundle is the deliverable and
  is reviewed against the session review budget.
- **Session review budget:** 800. The bundle lands at 511, leaving ~289
  for review notes and any addendum.
- **PR split plan:** **single PR, docs only.** Splitting a 490-line note
  into two PRs would split one argument across two reviews for no gain.
- **Stop rule:** if the bundle grows past 700 lines, stop and cut the
  loader-chain section down to the chain diagram plus the ledger; the note
  must stay readable in one sitting.
- **Explicit statement:** this is a docs-only analysis slice. No production
  code, no tests, no forecast inflation from test lines.

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: none (docs only)
400-line budget risk: N/A (no implementation PR)
```

## Phase 1 - Preconditions (done at plan time)

- [x] 1.1 `main` equals `origin/main`; branch cut from `main`.
- [x] 1.2 Ghidra headless + JDK verified; project created under `%TEMP%`,
      outside the repo and the install.
- [x] 1.3 Prior survey read for method and re-pin rule.

## Phase 2 - Recon (done at plan time)

- [x] 2.1 Whole defined-string inventory filtered for animation terms, with
      every xref resolved to its containing function.
- [x] 2.2 `.sra` / `.srm` / `.srl` literals identified; `.sra` owner found
      to be the only one.
- [x] 2.3 Spline debug formats catalogued as the contrast system.

## Phase 3 - Loader walk (done at plan time)

- [x] 3.1 Object-slot allocator read: slot layout, sibling path formats,
      queue call arguments.
- [x] 3.2 Callback address inspected at byte level (`RET 0`).
- [x] 3.3 Queue completion and drain read end to end; callback invocation
      confirmed.
- [x] 3.4 `0x32` post-processor read; negative test against three real
      `.SRA` files.
- [x] 3.5 Model path and spline sampler read for contrast.

## Phase 4 - Artifacts (done at plan time)

- [x] 4.1 `docs/notes/anim-binding.md` with ledger, verdict, unknowns and
      ordered probes.
- [x] 4.2 Proposal with the explicit no-go and the overturn condition.
- [x] 4.3 Spec with R1-R8 (scope, evidence classes, no-go bar, legal,
      verification).
- [x] 4.4 Design with the chain, the argued no-go and the decision record.
- [x] 4.5 This forecast.

## Phase 5 - Verify (apply gate, nothing to build)

- [ ] 5.1 `cargo test` green; record the count as the no-op baseline.
- [ ] 5.2 `git status --short` shows only the five new Markdown files; no
      Ghidra database, no listing, no scratch file.
- [ ] 5.3 Grep the bundle for absolute paths, hex dumps and decompiled
      fragments; expect zero hits.
- [ ] 5.4 Reviewer restates the verdict and the cheapest next probe without
      re-reading the note.

## Apply gate

- [ ] Merged only if 5.1-5.4 pass.
- [ ] No text in the bundle claims a channel->joint mapping, a hierarchy, a
      clip catalog, or that `.SRA` is usable at runtime.
- [ ] The no-go names the empty callback and the negative post-processor
      test, not just "we did not find it".

## Follow-up (not this change)

1. Spline-key writer: find the store into `numkeys` and the key allocation.
2. Runtime confirmation of the stub and of the unused slot field.
3. `.SRL` scoping.
4. SR2 parity.

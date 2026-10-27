# Tasks: pcm-loader

Docs-only analysis. Nothing to implement in this change. `test_command` is
`cargo test`, used only as a no-op baseline.

## Review Workload Forecast

| Bucket | Lines | Note |
|---|---|---|
| Production | **0** | no `src/` change |
| Test code | **0** | no new behavior |
| Docs - planning bundle | 346 | note 92, proposal 54, spec 64, design 59, this file 77 |
| **TOTAL** | **346** | all Markdown |

- Within the 400-line per-PR ceiling and the 800-line session review budget.
- PR split: **single PR, docs only.** Not recommended to split.
- Chained PRs: No. Decision needed before apply: No.

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: none (docs only)
400-line budget risk: Low
```

## Phase 1 - Preconditions

- [x] 1.1 `main` up to date with `origin/main`.
- [x] 1.2 Ghidra headless and the JDK verified; project in `%TEMP%`.
- [x] 1.3 Prior method notes read (`anim-binding`, `sr1-ghidra-survey`).

## Phase 2 - Recon

- [x] 2.1 Defined-string scan for `.pcm`, `pmf`, `pnf`, `area`, `object`,
      `level`, `load`, `queue`, error terms; xrefs resolved.
- [x] 2.2 Path builder and its three callers identified.

## Phase 3 - Loader walk

- [x] 3.1 Queue ring and bigfile/loose resolution read.
- [x] 3.2 Drain common prefix and tag dispatch read.
- [x] 3.3 Tag-1 object instance size confirmed; tag-2 level builder read.
- [x] 3.4 36-to-40-byte record relation confirmed; 48 identified as the
      tracker count.
- [ ] 3.5 **Not done (budget):** tag-1 decompressor body (L14).
- [ ] 3.6 **Not done (budget):** name resolver body (L15).

## Phase 4 - Artifacts

- [x] 4.1 `docs/notes/pcm-loader.md`: ledger L1-L16, pipeline, probes.
- [x] 4.2 Proposal with the PARTIAL verdict and the implementation no-go.
- [x] 4.3 Spec R1-R8.
- [x] 4.4 Design with the decision record D1-D5.
- [x] 4.5 This forecast.

## Phase 5 - Verify

- [ ] 5.1 `cargo test` green (no-op baseline); record the count.
- [ ] 5.2 `git status --short` shows only the planning bundle and the note.
- [ ] 5.3 Grep the bundle for absolute paths, hex and decompiled text: zero
      hits expected.
- [ ] 5.4 Reviewer restates the verdict and the L13 probe without re-reading.

## Next slice (not this change)

1. **Resolve L13.** Show the buffer the drain hands to the level builder for
   a real area `.PCM`: does the first dword match the documented count, or is
   there a wrapper? This gates everything else.
2. **Name resolver (L15).** Read the lookup on the source name offset.
3. **Field semantics (L16).** Label the 36-byte fields only after 1 and 2.
4. **Object decompressor (L14).** Only if object containers are in scope.

## Apply gate

- [ ] Merge only if 5.1-5.4 pass.
- [ ] No text claims a working `.PCM` reader, a block type for 48, or a
      confirmed field meaning for the 36-byte record.

# Tasks: pcm-loader

Docs-only analysis. Nothing to implement in this change. `test_command` is
`cargo test`, used only as a no-op baseline.

## Review Workload Forecast

The original bundle was 346 authored lines. The L13 pass adds a delta; the
forecast below is the **delta of this PR**, measured with `git diff --stat`
before commit, not the whole-file totals.

| Bucket | Lines | Note |
|---|---|---|
| Production | **0** | no `src/` change |
| Test code | **0** | no new behavior |
| Docs - planning bundle | ~150 | note resolution + ledger corrections; design/spec/tasks updates; one dated correction block in `area-levels.md` |
| **TOTAL** | ~150 | all Markdown |

- Delta stays under the 400-line per-PR ceiling and the 800-line session
  review budget, even though the note itself is now longer than before.
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
- [x] 3.6 Name resolver body read (L15): tagged-record copy, null on no tag.
- [x] 3.7 L13 probe: the drain strips `4 * roundup512(N)` bytes; the first
      dword is N; the base is the data base. Verified on 89/89 units.

## Phase 4 - Artifacts

- [x] 4.1 `docs/notes/pcm-loader.md`: ledger L1-L16, pipeline, probes.
- [x] 4.2 Proposal with the PARTIAL verdict and the implementation no-go.
- [x] 4.3 Spec R1-R8.
- [x] 4.4 Design with the decision record D1-D6.
- [x] 4.5 This forecast.
- [x] 4.6 L13 resolution section with the three-number table and the
      corpus result.
- [x] 4.7 Dated base correction in `area-levels.md` (history left intact).

## Phase 5 - Verify

- [x] 5.1 `cargo test` green (no-op baseline): 231 passed, 0 failed.
- [x] 5.2 `git status --short` shows only the planning bundle and the note.
- [x] 5.3 Grep the bundle for absolute paths, hex and decompiled text: zero
      hits expected.
- [x] 5.4 Reviewer restates the verdict and the L13 resolution without
      re-reading.

## Next slice (not this change)

1. **Field semantics (L16).** With the base settled, label the level-header
   words and the 36-byte record against the block notes. The first field is a
   tagged record (L15), not a string.
2. **Tagged-record body (L15 follow-up).** Read the entry kinds inside the
   tagged record.
3. **Object decompressor (L14).** Only if object containers are in scope.

## Apply gate

- [ ] Merge only if 5.1-5.4 pass.
- [ ] No text claims a working `.PCM` reader, a block type for 48, or a
      confirmed field meaning for the 36-byte record.

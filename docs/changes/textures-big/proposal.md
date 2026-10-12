# Proposal: textures-big

Probe first, then read the minimum: `1\TEXTURES.BIG` does NOT match the SR1PC bigfile layout (count-at-0 test fails — `src/bigfile.rs` must not be reused), so this slice documents the observed custom layout, adds loud `--tlist` / `--tshow` arms that list entries when an entry table is identified at apply time (and fail loud otherwise), and renders one DDS entry through the proven in-memory decode-to-window path when such an entry exists. If no table or no DDS entry is found, the slice still delivers layout documentation, loud behavior, synthetic-only tests, and names the next container candidate (`2\BIGFILE.DAT`, 2236 entries).

## Quick path

1. `cargo run -- --textures <TEXTURES.BIG> --tlist` exits 0 with an entry listing when an entry table is identified, else exits 1 loud with the observed layout summary.
2. `cargo run -- --textures <TEXTURES.BIG> --tshow <entry>` renders one DDS entry in the existing window (user playtest); anything else fails loud, exit 1.
3. Verified with `cargo test` (synthetic bytes only), a user playtest on the real file, and a `git status` whitelist check.

## Details

| Topic | Decision |
|---|---|
| Intent | Prove whether the remaster's packed textures live in `TEXTURES.BIG` and are readable without an extraction step; the planning probe already answers the first half (custom layout, no SR1PC index) so apply hunts only for the entry table, never for a full format reverse-engineer |
| Layout verdict (planning probe, read-only) | CUSTOM, not SR1PC: count word at 0 reads in the tens of millions (far above the `MAX_ENTRIES` 32768 sanity limit); the SR2 hash-table header reading fails its pad rule on the same bytes; header region is near-zero fill with a short tag-like prefix, then `0xFF` fill, then high-entropy data with no `DDS ` magic at any sampled window (see `design.md` for the full probe ledger) |
| Reuse rule | `src/bigfile.rs` is NOT reused for parsing (its `NotBigfile` rejection is the correct answer for this file, not its parser); `dds.rs` (`decode_memory`, `has_dds_magic`) and the `show_texture` render tail ARE reused unchanged |
| Input contract | The textures path arrives via `--textures <path>` at runtime only; absolute game paths never committed; no game bytes in repo or on disk outside process memory (same legal rule as all prior slices) |
| Failure policy | LOUD everywhere (mirrors `bigfile-index`, not gallery skipping): unidentified table, out-of-range entry, unknown selector, or non-DDS `--tshow` target each print a clear stderr error + exit 1; the error names the next container candidate when nothing is listable |
| Non-goals | Extraction-to-disk (FORBIDDEN — memory only), `E3.BIG` / `BONUS.BIG` / `MOVIES.BIG`, audio, models, hash-name reverse lookup, browser UI, Ghidra |
| Slice boundary | `--tlist` + `--tshow`-one-DDS (both conditional on what apply finds) + layout docs; the documented no-go variant (doc + loud stubs + tests + next candidate) counts as a complete slice, not a failure |
| Next candidate | `2\BIGFILE.DAT` (SR2 side): count word parses as 2236 with the SR1PC table-size sanity holding — layout unverified beyond the count word, counts only, to be scoped by its own probe |

## Checklist

- [ ] Reviewer can state the slice's one deliverable without re-reading (list TEXTURES.BIG entries, render one archived DDS from memory — or document why neither is possible and name the next candidate).
- [ ] Reviewer can state why `src/bigfile.rs` is not reused (count-at-0 fails; custom layout observed).
- [ ] Reviewer can state the failure policy and the no-go completion rule (loud exit 1; doc + loud stubs + tests + next candidate still closes the slice).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes committed or extracted, game paths via CLI args only, counts/codes/field-shapes only in artifacts.

## Next step

Read `spec.md` for acceptance scenarios (with exit codes), then `design.md` and `tasks.md`. Apply phase follows with a SINGLE PR (forecast in `tasks.md`).

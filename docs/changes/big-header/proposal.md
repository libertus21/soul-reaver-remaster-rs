# Proposal: big-header

The four SR1 `.BIG` containers (TEXTURES, MOVIES, BONUS, E3) currently fall into the `textures-big` blind spot: `--tlist` reads a `u32` at offset 0, rejects it as an implausible count, and reports that no entry table was identified — a verdict about the wrong field. §6 of the decompiled-pseudocode findings describes the real header as two `u16` fields, and a fresh bounded probe confirms it on all four files. This slice parses and validates that header, exposes both fields, replaces the misleading count-word verdict with a byte-verified header report, and keeps the entry table explicitly UNKNOWN.

## Quick path

1. `cargo run -- --textures <.BIG path> --tlist` prints the parsed header (header size, entry count) and exits 0 when the header validates.
2. Truncated, wrong-size, or implausible-count input fails loud with a DISTINCT stderr message and exit 1.
3. The entry table stays UNKNOWN: no stride, no index offset, no entry lines; `--tshow` stays loud (exit 1).
4. Verified with `cargo test` on synthetic bytes only; the real files are read by nothing but the bounded planning probe.

## Details

| Topic | Decision |
|---|---|
| Intent | Close the "no table identified" false negative by parsing the header that actually exists, without pretending the entry index was found |
| Verified header (planning probe) | Read-only, 16 bytes per file, 64 bytes total: `u16[0] = 512` in all four; `u16[1] = 1287 / 29 / 268 / 178` for TEXTURES / MOVIES / BONUS / E3; bytes 4..16 are zero in all four (observation, NOT a validated rule) |
| Parsed bytes | Exactly bytes `0..2` (header size) and `2..4` (entry count), little-endian. Bytes `4..512` and everything after stay unparsed |
| Validation | Header size MUST equal 512; entry count MUST be within `1..=32768` (the existing shared sanity bound, used as a guard — not a format claim); file length MUST be at least 512 |
| Still UNKNOWN | Per-file index location, entry stride, entry field layout, payload encoding — no stride, offset, or entry format is invented |
| Reuse rule | `src/bigfile.rs` stays untouched: the SR1PC layout still does not apply. The existing bounded probe windows and table-hunt evidence stay as-is |
| Failure policy | LOUD and distinct per class: truncated header, wrong header size, implausible count, unresolvable `--tshow` selector — each a clear stderr message + exit 1 |
| Behavior change | `--tlist` on a valid header moves from exit 1 to exit 0, because exit 0 now means "header verified", not "entries listed". The stdout report states that the entry table is still UNKNOWN |
| Non-goals | Index discovery, entry decoding, extraction, SR2 containers, other formats, Ghidra |
| Slice boundary | One module change + CLI rewire + README; synthetic-only tests |
| Legal | No game bytes or install paths in artifacts; file names and header field values only; runtime path arrives via CLI |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Exit-code change surprises a consumer | Low | Documented in spec and README; no known consumer relies on the old exit 1 |
| A count bound (32768) misclassifies a future large archive | Low | Bound is an explicit sanity guard with its own distinct message; measured counts are 29–1287 |
| Scope creep toward index hunting | Medium | Design states the index stays UNKNOWN; tasks contain no probe or stride work |

## Rollback Plan

Revert the `--tlist` / `--tshow` commits and the README bullet. `src/textures.rs` returns to the count-word verdict and the prior exit-1 contract; no data, no persisted state, no config to undo.

## Success Criteria

- [ ] All four `.BIG` headers parse to `512` plus their measured counts through the same code path.
- [ ] Invalid headers produce three distinct messages and exit 1; a valid header produces the report and exit 0.
- [ ] Entry table is still reported UNKNOWN and `--tshow` still exits 1 loudly.
- [ ] `cargo test` green; `git status` shows only intended files.

## Next step

Read `spec.md` for the requirement and message/exit contracts, then `design.md` and `tasks.md`. Apply follows as a SINGLE PR (forecast in `tasks.md`).

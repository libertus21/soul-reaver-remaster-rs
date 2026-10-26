# Archive: big-slots — DONE (slots listed and classified; decompressor still UNKNOWN)

The four SR1 `.BIG` containers now have their fixed-slot geometry validated and
their slots listed with a per-slot class. `--tlist` exits 0 with one
`slot <i> offset <off> <class>` line per slot plus a per-class footer, and
`--tshow <i>` stays a loud exit 1 that names the slot class and the compression
verdict. Decompression itself remains unimplemented and unguessed, so `data`
slots are listed and described but cannot be rendered yet. PR #28 is merged into
`main`; nothing outside `src/textures.rs`, `src/main.rs`, `README.md`, and the
change docs was touched.

## Quick path

1. `git checkout main && git pull` — HEAD is `19ecedc` (merge of PR #28).
2. `cargo run -- --textures <any .BIG from your own install> --tlist` — prints
   the header report, the addressing line, every slot, and the footer; exit 0.
3. `cargo run -- --textures <same file> --tshow 1` — names the class and
   `decompressor UNKNOWN`; exit 1.
4. `cargo test` — 122/122 green on synthetic bytes only.

## Playtest evidence (accepted, recorded verbatim)

| Item | Value |
|---|---|
| `--tlist` MOVIES.BIG | `slots: 29 total, 1 empty, 0 zero, 28 data`; exit 0. |
| `--tlist` TEXTURES.BIG | `slots: 1287 total, 1 empty, 0 zero, 1286 data`; exit 0. |
| `--tlist` BONUS.BIG | 268 total, 1 empty, 0 zero, 267 data. |
| `--tlist` E3.BIG | 178 total, 0 empty, 2 zero, 176 data. |
| `--tshow 0` | `slot 0 (offset 4096) is empty (0xFF marker): no payload; decompressor UNKNOWN (nothing to decode)`; exit 1. |
| `--tshow 1` | `slot 1 (offset 135168) is data: payload is compressed, decompressor UNKNOWN (cannot decode)`; exit 1. |
| `--tshow 9999` / `--tshow abc` | Distinct out-of-range message and distinct non-integer selector message; exit 1 each. |
| Geometry mismatch (synthetic, one byte short) | `slot geometry mismatch: … refusing to clamp`; exit 1. |
| Regression suite | `cargo test` 122 passed / 0 failed. |
| Build hygiene | Zero warnings on a cold rebuild. |
| Legal / repo | Whitelist clean, `git status` shows only intended files, repository private. |

Both `--tlist` runs on real containers produced footer totals equal to the
header count, with the three class counts summing to that total. The two
real-container `--tshow` probes exercised an `empty` slot and a `data` slot;
neither opened a window and neither printed payload bytes.

## Details

| Topic | Outcome |
|---|---|
| PR | [#28 — feat(big-slots): list fixed slots and refuse unknown decompression](https://github.com/libertus21/soul-reaver-remaster-rs/pull/28), state MERGED (`mergedAt` 2026-10-26T03:26:15Z), base `main`, head `feature/big-slots`, 7 files, +866/−48. |
| Commits on `main` | `12eb312` plan bundle (proposal/spec/design/tasks) → `151a805` geometry validation + whole-slot classification → `69ca09a` whole-slot scan and class report → `581f090` CLI slot listing and loud non-decodable verdict → `f2d8aaa` README truth-up → `c99285b` acceptance boxes ticked → `19ecedc` PR #28 merge. |
| Verified contract | Slot `i` occupies exactly 131072 bytes at `4096 + i * 131072`, and the container length MUST equal `4096 + count * 131072`. Classes are computed over the whole slot: `empty` = every byte `0xFF`, `zero` = every byte `0x00`, else `data`. No sampling, no head heuristic. |
| CLI behavior | `--tlist` prints the unchanged header summary, then `addressing: fixed 131072-byte slots …`, one line per slot, then the footer; exit 0. Any length deviation is a distinct `slot geometry mismatch` error, exit 1, never clamped. `--tshow` rejects non-integer and out-of-range selectors distinctly and, for a valid index, exits 1 naming class, offset, and `decompressor UNKNOWN`. All other modes are byte-for-byte unchanged. |
| Delivered in code | `src/textures.rs`: `SLOT_SIZE`, `SLOT_DATA_START`, `expected_len`, `slot_offset`, `validate_geometry`, `SlotClass`/`classify_slot`, `SlotScan`/`scan_slots`/`report`, `slot_class_at`, four new error variants, data-probe clamp for short-but-valid containers, synthetic tests. `src/main.rs`: `--tlist` slot report, `--tshow` selector/range/class paths, exact-geometry fixtures, CLI exit-code tests. |
| Deliberately not built | Any decompressor, texture decode from a `.BIG` slot, `TEX_HD.MAP` name mapping, or other-container support. No new dependency. `bigfile.rs` untouched (this is not the SR1PC layout). |
| Warnings (recorded) | (1) Review forecast under-estimated: 607 authored lines against a ~360-line forecast, concentrated in `src/textures.rs` tests; the 400-line budget stayed Low only because the change landed as chained work units. (2) Verify found an additive banner line in the `--tlist` output that is absent from the spec message table — the table describes the pre-existing summary and the new slot section, not that banner; no behavior deviates from a requirement. (3) Spec and proposal acceptance boxes were stale at the end of apply and were ticked in follow-up commit `c99285b` rather than amended. (4) `design.md` "open questions" were deliberately left unticked: they are genuinely unresolved, not a checkbox slip. |
| Branch cleanup | Deleted only branches verified fully contained in `main` (see the report below the table). `feature/big-slots-plan` was NOT contained (its tip predates the rebased planning commit) and was left in place. |
| Follow-ups | (1) Identify the decompressor for `data` slots — the next and ONLY blocker between listing and rendering. (2) `TEX_HD.MAP` name mapping (slot index to texture name) is still open; slots are addressable by index only. (3) Sentinel-slot and non-131072-slot containers remain open questions, both already made loud by the geometry check. |
| Legal | Behavior and format facts only. No game bytes, no absolute game paths, no extracted data, and no content dumps in any artifact or commit. Tests use synthetic bytes in temp dirs; the real containers are read-only playtest inputs. |

## Checklist

- [x] PR #28 MERGED and `main` contains all slice commits (HEAD `19ecedc`)
- [x] Playtest evidence recorded verbatim (four-container footers, `empty` and `data` `--tshow`, index errors, geometry mismatch, 122/122, zero warnings)
- [x] Slot model stated as verified vs UNKNOWN, with classification scope and the decompressor called out
- [x] Contained branches deleted after containment checks; uncontained branches kept with the reason stated
- [x] README "What works" carries the slot listing and "What's missing" names the decompressor as the remaining blocker
- [x] No game bytes, absolute game paths, offsets, or hashes added by this archive

## Next step

Open the decompressor-identification slice. Establish WHICH algorithm produces
the `data` slot payloads using read-only probes and synthetic tests, so a
`--tshow` slot can be decoded and shown; `TEX_HD.MAP` name mapping can follow in
the same or a later slice. Until a decompressor lands, this slice's exit-0
contract means "slots listed and classified" and nothing more.

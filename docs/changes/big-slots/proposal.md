# Proposal: big-slots

## Intent

The `big-header` slice proved the `.BIG` header but still reports the entry table as UNKNOWN, so `--tlist` lists nothing and `--tshow` cannot name a slot. The slot-geometry probe (PR #27) settled the layout: the four SR1 containers are arrays of fixed 131072-byte slots with no index table (`file_len == 4096 + count * 131072` exactly; slot `i` at `4096 + i * 131072`). This slice turns that verified arithmetic into behavior: validate geometry against the real file length, list every slot with an honest class, and make `--tshow` fail loud with the slot class and the compression verdict. No decompressor is invented.

## Quick path

1. `cargo run -- --textures <path> --tlist` validates geometry, prints the existing header report, then one line per slot (`index`, `offset`, `class`) and a per-class footer, exit 0.
2. A length that disagrees with `4096 + count * 131072` is a DISTINCT loud error, exit 1, never clamped.
3. `--tshow <i>` validates `i` in `0..count`, classifies that one slot, and fails loud: class named, payload compressed, decompressor UNKNOWN, exit 1.
4. `cargo test` on synthetic containers only; real files stay read-only and outside the repo.

## Details

| Topic | Decision |
|---|---|
| Intent | Convert confirmed slot arithmetic into listing/validation; keep compression explicit |
| Verified geometry | `file_len == 4096 + count * 131072` for all four containers; slot `i` at `4096 + i * 131072`; measured counts 1287 / 29 / 268 / 178 (TEXTURES / MOVIES / BONUS / E3); slot 0 is all `0xFF`, slot 1 all zero, payload from ~slot 2 |
| Classification | Whole-slot read, all 131072 bytes: `empty` (all `0xFF`), `zero` (all `0x00`), else `data`. No sampling, so no "sampled" wording is needed |
| Listing cost | One sequential pass over the file; largest container is 168 MB; one reusable 128 KiB buffer; constant memory. Accepted for a CLI |
| Geometry failure | Distinct `slot geometry mismatch` error naming observed length, count, and expected length; exit 1; no clamping, no listing |
| `--tshow` | Valid index still fails loud by design: payload compression is UNKNOWN, so no decoding and no guess. Reads only that one slot |
| Contract preservation | `parse_header`, its three header errors, and `summary()` keep their exact messages. The new slot section is appended |
| Non-goals | Decompression, texture decoding, `TEX_HD.MAP` name mapping, other containers, SR2, Ghidra |
| Dependencies | None new; std only |
| Legal | Format facts and arithmetic only; no game bytes, no install paths; tests synthetic |

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `src/textures.rs` | Modified | Slot constants, `SlotClass`, geometry validation, whole-slot scan/report, slot lookup, new errors + tests |
| `src/main.rs` | Modified | `--tlist` prints the slot report; `--tshow` parses/validates the index and reports class + compression + tests |
| `README.md` | Modified | Truth-up the `.BIG` bullet for slot listing |
| `docs/changes/big-slots/*.md` | New | This planning bundle |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Full scan is slow on the largest container | Low | Sequential read, fixed buffer; 168 MB is fine for a CLI; `--tshow` reads one slot |
| Geometry check rejects a hypothetical variant layout | Low | It is an exact equality only after the verified four-container measurement; mismatch is loud, never clamped |
| Existing report test fixtures no longer satisfy geometry | Medium | Tasks update the synthetic fixtures to exact lengths; all prior header messages/tests stay |
| Scope drift toward decompression | Medium | Spec states UNKNOWN decompressor as a hard non-goal; no probe work in tasks |

## Rollback Plan

Revert the `--tlist` / `--tshow` commits and the README bullet. `src/textures.rs` returns to header-only reporting and `--tshow` to its prior loud selector error. No data, no state, no config.

## Success Criteria

- [x] All four real containers pass geometry validation through one code path and list every slot with classes.
- [x] A one-byte length mismatch fails with the distinct geometry error, exit 1, and prints no listing.
- [x] `--tshow` rejects non-numeric and out-of-range indexes distinctly, and for a valid index names the class and the UNKNOWN decompressor, exit 1.
- [x] `cargo test` green on synthetic data only; `git status` shows only intended files.

## Capabilities

### New Capabilities

- `big-slots`: fixed-slot geometry validation, whole-slot classification, slot listing, and loud non-decodable slot reporting for the four SR1 `.BIG` containers.

### Modified Capabilities

- None at the spec level; the header contract is preserved.

## Next step

`spec.md` (requirements, message/exit contracts), then `design.md` and `tasks.md`. Apply follows as a single PR.

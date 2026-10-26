# Tasks: slot-name

Ordered work units. Each keeps its tests with its behavior and is independently
reviewable. `test_command` for apply: `cargo test`.

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~340 authored (`tex_hd_map.rs` ~150 new + ~70 test; `main.rs` ~60 add / ~10 del + ~40 test; README/help ~20) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (4 code/doc work-unit commits) |
| Delivery strategy | single-pr (cached) |
| Chain strategy | pending — not needed at this size |

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low
```

### Suggested Work Units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | Group + name codec | PR 1 | `cargo test tex_hd_map` | N/A — pure codec | `Group`, `Bank`, `TextureName` in `src/tex_hd_map.rs` |
| 2 | Map rows + name→slots scan | PR 1 | `cargo test tex_hd_map` | N/A — pure slices | `TextureMap` + `MapError` |
| 3 | `--map` on `--tlist` | PR 1 | `cargo test cli_tests` | `--textures <one real .BIG> --tlist --map <real TEX_HD.MAP>` (read-only) | `run_textures` list arm |
| 4 | Name selector + index suffix | PR 1 | `cargo test cli_tests` | `--tshow <unique name> --map` (user playtest, read-only) | `run_textures` show arm |
| 5 | README/help + playtest + legal gate | PR 1 | `cargo test` | N/A — docs | `README.md`, help text |

## Task 1 — `src/tex_hd_map.rs`: group + name codec (≤120 authored lines, tests included)

- [ ] 1.1 Create `src/tex_hd_map.rs`; register the module in `main.rs`. Add
  `MAP_BYTES = 384_000`, `ROW_BYTES = 64`, `ROW_CODES = 16`,
  `GROUP_ROWS = 2_000`, `SENTINEL = u32::MAX`.
- [ ] 1.2 Add `Group::{Textures, Movies, Bonus}` with
  `from_container_stem` (case-insensitive `TEXTURES`/`MOVIES`/`BONUS`),
  `base_row()` → 0 / 2,000 / 4,000, and `label()` → `TEXTURES`/`MOVIES`/`BONUS`.
- [ ] 1.3 Add `Bank::{Root, M, B}` and
  `TextureName { bank, id, sub }` with `display()`:
  `<id>_<sub>`, `M\<id>_<sub>`, `B\<id>_<sub>`.
- [ ] 1.4 Add `TextureName::from_code(v)`:
  sentinel → `None`; `v < 32000` → Root; `32000..64000` → M with `v - 32000`;
  `>= 64000` → B with `v - 64000`; integer divide by 16 and remainder.
- [ ] 1.5 Add `TextureName::parse(s)` for the spec grammar: optional
  case-insensitive `B\`/`M\`, then digits `_` digits; reject empty parts,
  non-digits, wrong separators, and extra text.
- [ ] 1.6 Unit tests (no files): boundaries `31999 → 1999_15`, `32000 → M\0_0`,
  `63999 → M\1999_15`, `64000 → B\0_0`; `0 → 0_0`; sentinel → `None`;
  parse/display round-trip for `100_10`, `M\9_2`, `B\262_1`; reject `abc`,
  `100`, `_10`, `100_`, `B/1_2`, `B\1_2_x`.
- [ ] 1.7 Verify: `cargo test tex_hd_map` green; `cargo build` warning-free.

## Task 2 — `src/tex_hd_map.rs`: map rows and name→slots (≤130 authored lines, tests included)

- [ ] 2.1 Add `MapError::{WrongLength { found, expected }, SlotOutOfCapacity { slot, group }}`
  with `Display`/`Error`.
- [ ] 2.2 Add `TextureMap<'a> { bytes: &'a [u8] }` and
  `parse(bytes)` enforcing exactly `MAP_BYTES`; nothing is padded or truncated.
- [ ] 2.3 Add `codes(group, slot) -> Result<[u32; 16], MapError>`: reject
  `slot >= GROUP_ROWS` with `SlotOutOfCapacity`; read
  `(group.base_row() + slot) * ROW_BYTES` as 16 little-endian `u32`.
- [ ] 2.4 Add `names(group, slot) -> Result<Vec<TextureName>, MapError>`:
  decode each code, skip sentinel, dedupe preserving first appearance.
- [ ] 2.5 Add `slots_for_name(group, slot_count, name) -> Vec<u16>`: scan
  `0..slot_count`, compare decoded `TextureName` exactly; return all matches in
  ascending order.
- [ ] 2.6 Unit tests (synthetic 384,000-byte buffers): wrong lengths 383,999 and
  384,001 → `WrongLength`; a crafted row decodes in order with dedupe; all
  sentinel → empty; base rows 0/2,000/4,000 hit different rows; slot 2,000 and
  6,000 → `SlotOutOfCapacity`; `slots_for_name` returns zero/one/two matches.
- [ ] 2.7 Verify: `cargo test tex_hd_map` green.

## Task 3 — `src/main.rs`: `--map` on `--tlist` (≤140 authored lines, tests included)

- [ ] 3.1 Extend `run_textures` arg handling to accept exactly the two shapes
  `--tlist --map <path>` (4 args) and `--tshow <sel> --map <path>` (5 args);
  every other shape keeps the current usage error. `--map` without a value is a
  usage error.
- [ ] 3.2 When `--map` is present: read the file (loud I/O error), parse with
  `TextureMap::parse` (loud length error), and derive `Group` from the
  container stem (loud accepted-stems error for anything but
  `TEXTURES`/`MOVIES`/`BONUS`). Do this after header/geometry validation.
- [ ] 3.3 In the `--tlist` arm with a map, append `; names <n1>,<n2>,...`
  (deduped, first-seen order) or `; names -` to each existing slot line. Do not
  change the summary, addressing line, class computation, or footer.
- [ ] 3.4 Keep the non-`--map` path byte-for-byte identical: same match arms,
  same order, same strings.
- [ ] 3.5 CLI tests on synthetic containers/maps: names suffix on a crafted row;
  `names -` on an all-sentinel row; bad map length; unknown stem (`E3.BIG`);
  `--tlist` without `--map` unchanged; `--map` shape errors.
- [ ] 3.6 Verify: `cargo test` green; `cargo run -- --textures <real .BIG>
  --tlist --map <real TEX_HD.MAP>` exits 0 and names resolve to files
  (read-only spot check).

## Task 4 — `src/main.rs`: name selector and index suffix (≤140 authored lines, tests included)

- [ ] 4.1 In the `--tshow` arm, keep the integer path first (unchanged). Without
  `--map`, a selector that parses as a valid texture name fails loud with the
  `require --map` message; any other non-integer selector keeps the existing
  `invalid slot selector` error.
- [ ] 4.2 With `--map` and a non-integer selector: parse with
  `TextureName::parse` (loud invalid-name error), then `slots_for_name`. Exactly
  one → use that index; zero → loud "not referenced"; more than one → loud
  ambiguity error listing up to 8 indices plus the total.
- [ ] 4.3 `--tshow <index> --map`: keep the existing decode/show path and append
  `; names ...` to the single stdout line. Without `--map`, the line is
  unchanged.
- [ ] 4.4 Keep all payload/class/render errors and exits unchanged; a resolution
  failure never opens a window.
- [ ] 4.5 CLI tests on synthetic bytes: unique name resolves and (helper-only)
  reaches the decode path; unknown name; ambiguous name lists candidates; name
  without map; invalid name; index mode line unchanged / suffixed with map.
- [ ] 4.6 Verify: `cargo test` green; `cargo run -- --textures <real .BIG>
  --tshow <unique real name> --map <real map>` shows the slot, exit 0 on close
  (user playtest, read-only).

## Task 5 — README + help + playtest + legal gate (docs only, ≤40 lines)

- [ ] 5.1 Update the `.BIG` bullet in `README.md`: `TEX_HD.MAP` names, the
  `--map` flag, the `--tlist` suffix, name resolution and its ambiguity rule,
  and E3 being out of scope. Point to `docs/notes/s8-big-slot-names.md`.
- [ ] 5.2 Update `--textures` help/usage text for the optional `--map` pair.
- [ ] 5.3 Record the playtest note (container, slot, name, what was seen) in the
  commit/PR body.
- [ ] 5.4 Legal: `git status --short` shows only `src/*.rs` and `README.md`; no
  game bytes, install paths, or content dumps in the diff.
- [ ] 5.5 Verify: `cargo test` green; whitelist clean.

## Apply gate

- [ ] All work units land as one PR from fresh `main`.
- [ ] `cargo test` green; `--tlist --map` on TEXTURES/MOVIES/BONUS exits 0 with
  names; `--tshow <unique name> --map` shows at least three different slots; a
  shared name fails loud with candidates.
- [ ] `--tlist` and `--tshow <index>` without `--map` are byte-for-byte
  unchanged; no new dependency; no game bytes.
- [ ] No-go gate: if the map is not 384,000 bytes or decoded names stop
  resolving at roughly the observed rate, stop, revert the claim, keep
  index-only output, and name the original PS1 page/room catalogue as the next
  candidate. Do not guess an alternative encoding.

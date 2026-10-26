# Proposal: slot-name

## Intent

Give `.BIG` slots the names the engine already uses. Today `--tlist` shows
`slot <i> offset <off> <class>` and `--tshow` accepts only an index, because the
container itself stores no names. The names live in the sibling
`AREA\TEX_HD.MAP` blob, encoded per slot, and this slice exposes them.

The investigation is decisive (evidence in `docs/notes/s8-big-slot-names.md`):

- `TEX_HD.MAP` is exactly **384,000 bytes = 6,000 rows × 64 bytes**, one row per
  container slot, **16 little-endian `u32` alias codes** per row.
- Three blocks of 2,000 rows: `TEXTURES.BIG` at base 0 (1,287 used),
  `MOVIES.BIG` at base 2,000 (29 used), `BONUS.BIG` at base 4,000 (268 used).
  `E3.BIG` has no block.
- A code `v` decodes to a loose-texture name:
  `v < 32000` → `<v/16>_<v%16>`; `32000..63999` → `M\...`; `>= 64000` → `B\...`.
  `0xFFFFFFFF` means "no reference".
- 99.6% of the 20,592 TEXTURES references decode to a `.DDS` that exists in the
  observed install. The names are the game's own numeric asset names
  (`100_10`, `M\9_2`, `B\262_1`), not descriptive words.

So this is a **FOUND (partial)** outcome: browse and select slots by their real
asset names. Something remains impossible: turning those names into descriptive
labels ("stone wall", "face") because no descriptive vocabulary exists in the
data. That is explicitly not this slice.

## No-go condition (explicit)

This slice is a **no-go** — and index-only stays — if any of these holds at
apply time:

1. On the target build, `TEX_HD.MAP` is not exactly 384,000 bytes, or the
   row/group model does not reproduce: decoded names stop resolving to real
   loose files at roughly the observed rate.
2. A slot row does not correspond to that container slot (the consumer trace is
   contradicted), so name-to-slot resolution cannot be trusted.

If either happens, stop, revert the claim, keep `--tlist`/`--tshow`
index-only, and name the next candidate in the verification report: mapping the
numeric alias families to original PS1 page/room catalogues (Soul Spiral / Only
Skin Deep domain). No alternative encoding may be guessed in code.

## Quick path

1. `cargo run -- --textures <TEXTURES.BIG> --tlist --map <AREA\TEX_HD.MAP>`
   lists every slot with its alias names.
2. `cargo run -- --textures <TEXTURES.BIG> --tshow <name> --map <AREA\TEX_HD.MAP>`
   resolves a name that belongs to exactly one slot and shows it. Shared names
   (common in this data) fail loud with the candidate indices.
3. `--tlist` and `--tshow <index>` **without** `--map` stay byte-for-byte
   unchanged.
4. `cargo test` covers the map decoder, group selection, name parsing, and
   ambiguity/error paths on synthetic bytes. The visible render is a manual
   playtest.

## Details

| Topic | Decision |
|---|---|
| Name source | The engine's own alias codes in `AREA\TEX_HD.MAP`; no invented labels |
| Map shape | 6,000 × 64-byte rows; 16 × `u32`; `0xFFFFFFFF` = none; no header, no strings |
| Group bases | TEXTURES 0, MOVIES 2,000, BONUS 4,000; selected by container file stem; E3 refused loud |
| Name encoding | Root `<id>_<sub>`, `M\...`, `B\...`; integer divide by 16 and remainder |
| New CLI surface | Optional trailing `--map <path>` on `--tlist` and `--tshow`; a non-integer `--tshow` selector is a name |
| Name display | Appended to the existing slot line (`; names a,b,c`), deduped, first-seen order; `; names -` when none |
| Ambiguity | A name owned by more than one slot is a loud error listing candidates; never pick the first silently |
| Existing contracts | No `--map` → every current message, line, and exit code unchanged |
| Tests | Synthetic 384,000-byte maps and synthetic containers in temp dirs; no game bytes |
| Dependencies | None new; the map is parsed from a byte slice |
| Legal | Format/behavior facts only; no map dumps, no addresses, no install paths |

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `src/tex_hd_map.rs` | New | Pure decoder: constants, length guard, group bases, row lookup, name encode/decode/parse, ambiguity scan |
| `src/main.rs` | Modified | `--map` argument handling; names on `--tlist`; name resolution in `--tshow` |
| `README.md` | Modified | `.BIG` bullet: slots have engine names via `TEX_HD.MAP`; `--map` flag |
| `docs/notes/s8-big-slot-names.md` | New | Evidence ledger and corrected map shape (from this plan) |
| `docs/changes/slot-name/*.md` | New | This planning bundle |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Another build ships a different map size or row layout | Low | Exact-size guard and loud failure; no-go path above |
| Alias meanings differ from the traced consumer | Low | Three independent confirmations (row counts, 99.6% file existence, pixel match) |
| Name ambiguity makes `--tshow <name>` frustrating | **High** (2,070 shared names) | Loud candidate list with indices; `--tlist --map` is the primary browsing path |
| E3 users expect names | Medium | Refused loud when `--map` is used; documented as out of scope |
| A renamed container defeats stem-based group selection | Low | Loud error naming the accepted stems; a `--group` override stays a future option |
| Scope drift into descriptive naming | Medium | Design lists it as out of scope; no heuristic labels |

## Rollback Plan

Revert the new module, the `--map` wiring, and the README bullet. `--tlist` and
`--tshow` return to index-only with unchanged output. No data, state, or config.

## Success Criteria

- [ ] `--tlist --map` prints every slot with its decoded names (or `names -` for
      all-sentinel rows).
- [ ] `--tshow <name> --map` resolves a unique name and shows that slot;
      zero-match and multi-match names are loud with exit 1 and no window.
- [ ] `--tlist` and `--tshow <index>` without `--map` are byte-for-byte
      unchanged.
- [ ] Map size, unknown container stem, invalid name syntax, and a row outside
      the group capacity are loud, distinct errors.
- [ ] `cargo test` green on synthetic bytes only; no game bytes in the diff.
- [ ] `docs/notes/s8-big-slot-names.md` records the shape, encoding, and the
      ruled-out hypotheses.

## Capabilities

### New Capabilities

- `big-slot-names`: decode `TEX_HD.MAP` alias rows and expose per-slot names,
  with name-to-slot resolution for unique names.

### Modified Capabilities

- `big-slots`: `--tlist` and `--tshow` gain an optional `--map` mode. Index-only
  behavior is untouched.

## Next step

`spec.md` (requirements, name grammar, message/exit contracts), then `design.md`
and `tasks.md`. Apply follows as a single PR.

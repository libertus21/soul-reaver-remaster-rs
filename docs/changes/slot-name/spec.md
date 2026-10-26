# Spec: slot-name

`--textures <container> --tlist --map <TEX_HD.MAP>` lists slots with the names
the engine derives from the map; `--tshow <sel> --map <TEX_HD.MAP>` accepts a
slot index or a texture name and shows that slot. Without `--map`, every
current `--tlist`/`--tshow` line, message, and exit code is unchanged. Names are
the game's numeric asset names (`<id>_<sub>`, `M\<id>_<sub>`,
`B\<id>_<sub>`), not descriptive labels.

## Quick path

1. `--tlist --map` on a real `TEXTURES.BIG`: exit 0, each slot line gains
   `; names <n1>,<n2>,...`.
2. `--tshow 4_0 --map` when the name belongs to exactly one slot: exit 0 after
   the window closes, image recognizable.
3. `--tshow <shared name> --map`: exit 1, candidate slot indices listed, no
   window.
4. `--tlist` and `--tshow 4` without `--map`: byte-for-byte prior output.
5. `cargo test`: decoder, group, name grammar, and error paths on synthetic
   bytes.

## Requirements

| ID | Requirement |
|---|---|
| R1 | Existing CLI shapes SHALL stay as they are: `--textures <path> --tlist` (2 args) and `--textures <path> --tshow <sel>` (3 args), same output and exit codes. A new optional trailing pair `--map <TEX_HD.MAP>` SHALL be accepted only as `--textures <p> --tlist --map <m>` (4 args) or `--textures <p> --tshow <sel> --map <m>` (5 args). Any other arrangement SHALL stay a loud usage error with exit 1 |
| R2 | The map SHALL be accepted only at exactly 384,000 bytes. Any other length SHALL be a distinct loud error naming the observed length and `384000 (6000 rows x 64 bytes)`; the map SHALL NOT be padded, truncated, or partially read |
| R3 | The map group SHALL be derived from the container file stem, case-insensitively: `TEXTURES.BIG` → base row 0; `MOVIES.BIG` → base row 2,000; `BONUS.BIG` → base row 4,000. Any other stem used with `--map` SHALL be a loud error naming the three accepted stems (`E3.BIG` included in the refusal), exit 1 |
| R4 | A slot row SHALL be read as 16 little-endian `u32` codes at byte offset `(base_row + slot) * 64`. The slot SHALL be < 2,000 (group capacity); an out-of-capacity row SHALL be a loud error, never a read into an adjacent block |
| R5 | A code SHALL decode as: `0xFFFFFFFF` → no reference; `v < 32000` → bank `Root`, id `v/16`, sub `v%16`; `32000 ≤ v < 64000` → bank `M`, id `(v-32000)/16`, sub `(v-32000)%16`; `v ≥ 64000` → bank `B`, id `(v-64000)/16`, sub `(v-64000)%16`; integer division and remainder. Display SHALL be `<id>_<sub>` for Root, `M\<id>_<sub>` for M, `B\<id>_<sub>` for B. `0` SHALL be a valid code, not a terminator |
| R6 | `--tlist --map` SHALL keep the existing summary, addressing line, per-slot line, and footer, and SHALL append `; names <n1>,<n2>,...` to each slot line. Names SHALL be deduplicated preserving first appearance. A row with only `0xFFFFFFFF` codes SHALL print `; names -` |
| R7 | With `--map`, a `--tshow` selector that is not a non-negative decimal integer SHALL be parsed as a texture name: optional case-insensitive bank prefix `B\` or `M\`, then `<digits>_<digits>`. Invalid syntax SHALL be a distinct loud error with exit 1 and no window |
| R8 | Name resolution SHALL scan the selected group's slot rows for a code whose display equals the requested name (case-insensitive bank). Exactly one match → show that slot. Zero → loud error. More than one → loud error listing up to 8 candidate slot indices and the total count, exit 1, no window. The first match SHALL never be chosen silently |
| R9 | Without `--map`, a selector that parses as a texture name SHALL be a loud error stating that texture names require `--map <TEX_HD.MAP>`, exit 1. A selector that is neither a non-negative decimal integer nor a valid texture name SHALL keep the existing `invalid slot selector` message and exit 1 |
| R10 | `--tshow <index> --map` SHALL keep the current decode/show behavior and stdout line, appending `; names <n1>,<n2>,...` (deduped) or `; names -`. `--tshow <index>` without `--map` SHALL keep the exact current line |
| R11 | Header parsing, geometry validation, selector/range errors, `empty`/`zero` payload errors, and every other CLI mode SHALL keep their exact prior messages and exit codes. Header/geometry errors SHALL take precedence over map and selector errors |
| R12 | No new dependency SHALL be added. Tests SHALL use synthetic 384,000-byte maps and synthetic containers in temp dirs; no game bytes, install paths, or content dumps in code, tests, outputs, or artifacts |
| R13 | The map SHALL be read only from the explicit `--map` path. No implicit discovery relative to the container or install directory SHALL occur |

## Name grammar

```text
name   := bank? digits "_" digits
bank   := "B\" | "M\"          (case-insensitive)
digits := 1 or more ASCII decimal digits
```

Examples: `100_10`, `4_0`, `M\9_2`, `B\262_1`, `B\2_1`.

Not names: `abc`, `100`, `100_`, `_10`, `x\1_2`, `B/1_2`.

## Decode contract

| Input | Result |
|---|---|
| `0xFFFFFFFF` | no reference (skipped) |
| `0` | `0_0` (Root) |
| `31999` | `1999_15` (Root) |
| `32000` | `M\0_0` |
| `63999` | `M\1999_15` |
| `64000` | `B\0_0` |
| Map length ≠ 384,000 | loud length error, exit 1 |
| `base_row + slot ≥ 6,000` or `slot ≥ 2,000` | loud row/capacity error, exit 1 |

## Message contracts

| Condition | Stream | Message (shape) | Exit |
|---|---|---|---|
| `--tlist --map`, row with names | stdout | `slot <i> offset <off> <class>; names <n1>,<n2>,...` | 0 |
| `--tlist --map`, all-sentinel row | stdout | `slot <i> offset <off> <class>; names -` | 0 |
| `--tshow <index> --map`, decode ok | stdout | existing line + `; names <n1>,<n2>,...` | 0 |
| Map wrong length | stderr | `TEX_HD.MAP is <n> bytes, expected 384000 (6000 rows x 64 bytes)` | 1 |
| Map unreadable | stderr | `cannot read TEX_HD.MAP: <io detail>` | 1 |
| Container stem unknown with `--map` | stderr | `cannot map names for '<file>': expected TEXTURES.BIG, MOVIES.BIG, or BONUS.BIG` | 1 |
| Name-shaped selector without `--map` | stderr | `texture names require --map <TEX_HD.MAP>` | 1 |
| Neither index nor valid name, without `--map` | stderr | unchanged: `invalid slot selector '<sel>': expected a slot index in 0..<count>` | 1 |
| Invalid name syntax | stderr | `invalid texture name '<s>': expected <id>_<sub>, M\<id>_<sub>, or B\<id>_<sub>` | 1 |
| Name matches no slot | stderr | `texture name '<n>' is not referenced by any slot in <container>` | 1 |
| Name matches several slots | stderr | `texture name '<n>' is referenced by <count> slots (e.g. <i1>, <i2>, ...); select one by index` | 1 |
| Row outside group capacity | stderr | `slot <i> has no name row in the <container> block (capacity 2000)` | 1 |
| Header/geometry error | stderr | unchanged; takes precedence over map/selector messages | 1 |
| Selector/range error (integer path) | stderr | unchanged | 1 |
| Empty/zero payload | stderr | unchanged | 1 |
| Window/render failure | stderr | existing `error: <text>` | 1 |

## Acceptance scenarios

### Mapping found (active path)

- [ ] **Names from a row:** a synthetic map with a TEXTURES row
      `[0, 32000, 64000, 0xFFFFFFFF, ...]` lists `names 0_0,M\0_0,B\0_0`.
- [ ] **Boundaries:** codes `31999`, `32000`, `63999`, `64000` decode to
      `1999_15`, `M\0_0`, `M\1999_15`, `B\0_0`.
- [ ] **Group bases:** identical row bytes under `TEXTURES.BIG`, `MOVIES.BIG`,
      and `BONUS.BIG` produce different slot names because the base rows differ.
- [ ] **Dedup:** repeated codes in a row are shown once, in first-seen order.
- [ ] **Sentinel row:** an all-`0xFFFFFFFF` row prints `names -`.
- [ ] **Index selector with map:** `--tshow <i> --map` still shows the slot and
      appends its names; the image path is unchanged.
- [ ] **Unique name:** `--tshow <name> --map` resolves a synthetic name present
      in exactly one row and shows that slot.
- [ ] **Unknown name:** a well-formed name absent from the group exits 1 with
      the "not referenced" message and no window.
- [ ] **Ambiguous name:** a name present in two rows exits 1, lists both slot
      indices, and opens no window.
- [ ] **Name without map:** `--tshow 4_0` without `--map` exits 1 with the
      `require --map` message, while `--tshow abc` without `--map` keeps the
      prior `invalid slot selector` message.
- [ ] **Bad map length:** 383,999 and 384,001 bytes both fail with the length
      message; nothing is clamped.
- [ ] **Unknown stem:** `E3.BIG --tlist --map` fails with the accepted-stems
      message; no slot lines are printed.
- [ ] **Capacity guard:** a synthetic container with a valid geometry but a slot
      index ≥ 2,000 in its group fails with the capacity message.
- [ ] **Preserved index mode:** `--tlist` and `--tshow 4` without `--map` are
      byte-for-byte unchanged.
- [ ] **Regressions:** single-file DDS, gallery, `--bigfile`, mesh, and launcher
      modes behave as before.
- [ ] **Manual playtest:** on the real install, `--tlist --map` names resolve to
      existing loose `.DDS` files for the sampled slots; several unique names
      show recognizable images.
- [ ] **Legal:** `git status --short` shows only intended files; no game bytes or
      install paths in artifacts.

### Documented no-go path (specified, not triggered)

- [ ] If the map is not 384,000 bytes, or decoded names stop resolving to real
      loose files at roughly the observed rate, the slice SHALL stop, revert the
      claim, keep index-only output, and record the no-go in the verification
      report with the next candidate (original PS1 page/room catalogues) named.
- [ ] If the consumer model is contradicted (row ≠ slot), the same stop applies;
      no alternative encoding SHALL be guessed in code.
- [ ] If a protection or anti-tamper layer appears on this path, stop and
      report; nothing SHALL attempt to defeat it.

## Next step

See `design.md` for the evidence ledger, the exact map shape, module/interface
shape, and out-of-scope list; `tasks.md` for work units and the review forecast.

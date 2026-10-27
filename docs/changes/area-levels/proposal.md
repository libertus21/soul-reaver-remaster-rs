# Proposal: area-levels

**NO-GO for geometry, object placement, and rendering.** A read-only probe of
the SR1 remaster `<install>/1/AREA` tree found the room data split across
three containers, but it did **not** decode vertex/face buffers, collision
meshes, or object transforms.

## Intent

Answer, with code and evidence, what the remaster room files are and expose the
smallest truthful slice. What the probe did establish is a read-only
`--level-info <PATH>` command that prints, per unit, the container record
count, the object-name table, and the portal/door links, plus the PMF/PNF
sidecar shape. That retires the "unknown level format" assumption with
evidence and gives the next change a decoded container to build on.

## Quick path

1. `cargo run -- --level-info "<install>/1/AREA/LAIR"` scans `BIN`, prints one
   block per `.PCM`: unit name, record count, object-name count, link count,
   each link as `<target> <index>`, and the sibling PMF/PNF counts.
2. `cargo test` proves the header model, the name-table rule, every refusal,
   and the exit-code matrix on synthetic bytes.
3. User acceptance: a linked room, a linkless room, a sidecar-present room,
   and a sidecar-absent room; record relative names, output, exit codes.

## What the probe found

| File | Observed | Verdict |
|---|---|---|
| `.PCM` (89) | Header `u32 N` + `N` ascending relative offsets (first `0`, second `4`), data base `4 + 4N`. Every file contains its own unit name; 368 link-like names, 346 (94%) resolve to a real `.PCM` unit; 11 files have none | Container + name table: GO |
| `.PMF` (23) | Magic `FMPa`, 16-bit id, 16-bit segment count, per-segment 24-byte headers with a size and a field that is 22050 or 11025 in all 23; smooth 16-bit LE payload | Audio-like (derived, strong), not decoded |
| `.PNF` (23) | Magic `FNPa`, 20-byte header + one `u16` per PMF segment; id and count match the PMF in 23/23 | Observed |
| Geometry / transforms | Vertex, face, collision and placement payloads exist inside `.PCM` records but were not decoded | **NO-GO** |

Same-size `.PMF` files are byte-identical except the id field (4 size
groups). That is recorded as a discovery, not as a format claim.

## Scope

**In:** new pure `src/level.rs` (PCM header + name table; PMF/PNF headers);
one `--level-info` arm in `main.rs`; synthetic-byte unit tests; README + note.

**Out:** geometry/collision decode, object positions, texture mapping,
rendering, audio playback, writing/extraction, SR2, Ghidra, export.

## Capabilities

**New:** `area-level-info` — read-only inventory and link graph for one room
directory or one `.PCM`.
**Modified:** None.

## Approach

Header and name-table only. No new crate, no render path, no game bytes in
the repo. Reuse `cli_out::outln`, the existing 0/1/2/3 exit split, and the
`sra-animation` module shape.

## Affected areas

| Area | Impact | Description |
|---|---|---|
| `src/level.rs` | New | PCM/PMF/PNF header models, name-table extraction, link parsing |
| `src/main.rs` | Modified | one `--level-info` arm, usage text, exit mapping |
| `README.md` | Modified | "What works / What's missing" |
| `docs/notes/area-levels.md` | New | findings note (written at plan time) |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Name-table rule is heuristic | Low | every PCM carries its own name (89/89); 94% of targets resolve; a file with no own name is refused |
| PMF audio reading is wrong | Medium | output exposes `segments=`/`rate_field=` only, never "audio"; doc marks it derived |
| Directory scan is slow on a large tree | Low | one pass, sorted, no recursion beyond one level |
| Over-reading geometry during apply | Medium | spec forbids it; tasks gate on it |

## Rollback

Delete `src/level.rs`, revert the `main.rs` arm and usage text, revert the two
doc hunks. No data is written, so no migration.

## Dependencies

None beyond the existing binary and `std`. No new crate, no external tool, no
Ghidra. The player's own install supplies the data at runtime.

## Success criteria

- [ ] `--level-info` prints a stable block for a linked room, a linkless room,
      and a sidecar-absent room; exit 0.
- [ ] 89/89 PCMs satisfy the header model; 23/23 PNFs match their PMF id and
      count.
- [ ] `cargo test` green; synthetic bytes only; no game bytes, absolute paths,
      coordinates, or hashes committed.
- [ ] The output never claims geometry, object placement, playback, or
      rendering.

## Explicit no-go condition

GO only while: (1) the PCM header model holds on 89/89; (2) all 89 files carry
their own name; (3) PNF id/count matches PMF on 23/23; (4) acceptance shows
readable output on four sample paths. If (1), (2), or (3) fails at apply,
STOP and publish a notes-only no-go naming the failed check. Do not
reinterpret the name table as geometry.

## Next step

`spec.md` for the CLI/exit contract; `design.md` for the probe ledger and
UNKNOWNs; `tasks.md` for the forecast.

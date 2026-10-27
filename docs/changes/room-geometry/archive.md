# Archive: room-geometry — DONE (block census shipped; geometry still NO-GO)

**Stage banner: DONE for the block census; NO-GO for geometry, placement, and transforms.**

`--level-records <PATH>` reads the SR1-remaster `.PCM` unit container and prints
a read-only per-unit block census: block boundaries come from the offset table
alone, every block gets exactly one shape class by size (`zero`, `small4`,
`fixed40`, `fixed48`, `other:<size>`), and the single largest block's ordinal and
size are reported. It opens no window, decodes no payload, places no object, and
never claims a transform or a render. Playtest accepted the output on one small
unit, one large unit, and one room directory. PR #44 is merged into `main` at
`52f88f2`.

## Quick path

1. `git checkout main && git pull` — HEAD is `52f88f2` (merge of PR #44).
2. `cargo run -- --level-records "<room directory>"` — for example the
   `AREA/LAIR` room; the real path is resolved at runtime from your own install.
3. Expect one `blocks <unit> total=<n>` line per unit plus `shape` and `largest`
   lines; 31 units, exit 0.
4. Point it at one unit file instead of a directory — one census block, exit 0.
5. `cargo test` — 231/231 green on synthetic bytes only.

## Delivered and working

| Item | Evidence |
|---|---|
| CLI | `--level-records` accepts exactly one `.PCM` file or one room directory; a directory resolves its `BIN` when present, else itself; units sort by name |
| Block boundaries | `u32 N` + `N` relative offsets + data base `4 + 4N`; exactly `N` blocks; block sizes sum to the data-region length |
| Shape classes | Mutually exclusive, all-zero wins over size, then `small4` (4 B), `fixed40` (40 B), `fixed48` (48 B), else `other:<size>`; labels describe shape ONLY |
| Largest block | One reported ordinal and size; lowest ordinal wins a tie; the block keeps its shape class |
| Output | One `blocks <unit> total=<n>`, one `  shape <label> <count>` line per present class in fixed order, one `largest <ordinal> <size>`; no payload, file offset, coordinate, or transform |
| Path/exit reach | Same path forms and exit codes as `--level-info` (0 = at least one unit printed, 1 = usage, 2 = malformed/I-O/no units, 3 = refused path); nothing prints on failure |
| Isolation | `src/level.rs` pure module plus one `main.rs` arm; `--level-info` output, ordering, and exit codes unchanged |
| Gates | 231/231 tests, zero warnings, legal clean, repository private |
| Authored lines | 150 production + 141 tests + 46 docs |

## Accepted playtest evidence (verbatim)

The user verified `--level-records` on these inputs:

| Input | Accepted observation |
|---|---|
| One small unit (`LAIR5`) | 62 blocks: zero 23, small4 14, fixed40 10, fixed48 11, other:28 ×2, other:3300, other:3616; largest 61/3616 |
| One large unit (`MOVIE3`) | 2256 blocks; largest 549/143872 |
| One room directory (`LAIR`) | 31 units, exit 0 |

Shape of the accepted small-unit output (counts as accepted above):

```text
blocks <unit> total=62
  shape zero 23
  shape small4 14
  shape fixed40 10
  shape fixed48 11
  shape other:28 2
  shape other:3300 1
  shape other:3616 1
largest 61 3616
```

## Census (read-only, counts only)

- 46,124 blocks across 89 `.PCM` units; median 281 blocks per unit.
- Three sizes carry ~92.7% of all blocks (4-byte 36.0%, 48-byte 28.9%, 40-byte
  27.8%); 8.2% of blocks are entirely zero.
- One dominant block per unit (3,616–143,872 B); it is the last block in only
  1 of 89 units, so it is a resident payload, not a trailing append.
- Class labels are shape-only: no block carries a type tag, so no semantic type
  name is printed anywhere.

## Correction — records 0/1 are NOT a directory (supersedes the earlier reading)

This archive records the correction explicitly, because the earlier probe's
reading was wrong:

- The earlier probe read records 0 and 1 as if they formed a directory.
  **That reading is WRONG and is superseded here.**
- Record 0 is **4 zero bytes in 89/89** units.
- Record 1 is **all zero across 89/89** units: 8 bytes in 21 units, 80 bytes in
  67, and 120 bytes in 1.
- The **offset table is the only directory** in a `.PCM`. Records 0 and 1 are
  constant zero padding, and no payload block carries a type tag.
- Consequence: any future probe must derive structure from the offset table and
  from payload statistics, never from a record-0/1 directory.

## Later observation — 40/48-byte block internals (AFTER the playtest)

Recorded on 2026-10-27 from a follow-up inline probe that ran **after** the
accepted playtest. Evidence class: **CONFIRMED-observed**. Semantics: **UNKNOWN**.
Nothing here changes the shipped census, and nothing here is a geometry claim.

| Observation | Detail |
|---|---|
| Not parallel arrays | The 40-byte and 48-byte blocks are NOT three parallel per-object arrays. The count correlation `fixed48 = fixed40 + 1` holds in 26/31 `LAIR` units, but block content disproves the parallel-array reading |
| Group stride | Content reads as 12-byte groups: `[u32 id-ish][f32 value][u32 flags]` |
| 48-byte block | Exactly 4 groups (4 × 12 B) |
| 40-byte block | 3 groups (36 B) plus 4 unexplained tail bytes |
| f32 field | 106 non-zero values in these blocks; 44 with `|v| < 5000` (for example −0.45, −460.35, −14.39, −14730.5, −471377.1), observed repeating in runs of the same value |
| Leading u32 | High byte `0x34` with readable low bytes (`48 48 44` = `HHD`, `43 43 43` = `CCC`) — a possible id/tag family, semantics UNKNOWN |

Why this matters for sequencing: the next slice targets **placement and
transforms**, not geometry. A 12-byte `[u32 id-ish][f32 value][u32 flags]` group
is a plausible transform/placement shape, while the dominant block's 12-byte row
signal is still statistical only.

## Deliberate non-goals (NO-GO)

| Area | Status |
|---|---|
| Room geometry (vertices, faces, topology) | **NO-GO**; still unproven — no layout is decoded |
| Object placement and transforms | **NO-GO** in this change; the later observation is a lead, not a decode |
| Rendering | **NO-GO**; the mode opens no window |
| Block type taxonomy | **NO-GO**; no type tag exists, so classes stay shape-only |
| PMF audio decode and playback | Separate audio scope; untouched here |

## Details

| Topic | Outcome |
|---|---|
| PR | [#44 — feat(room-geometry): add --level-records block shape census](https://github.com/libertus21/soul-reaver-remaster-rs/pull/44), state **MERGED** (`mergedAt` `2026-10-27T16:50:16Z`), base `main`, head `feature/room-geometry`, 8 files, +906/−12 |
| Commits on `main` | `b47f54a` plan the PCM block census and record the geometry no-go → `95f556e` count PCM blocks by shape class → `4c6fe49` add `--level-records` census arm → `1359bde` document the census and the geometry no-go → `52f88f2` merge of PR #44 |
| Workload | Single PR, no chain: production 150 and tests 141 both under the 330 stop rule; docs 46 |
| Process win | The census was derived from the offset table only, so the slice stayed falsifiable on synthetic bytes and needed no payload decoding to ship |
| Warning (recorded) | `--level-records` counts blocks by SHAPE class. A shape count is not a type count, and a dominant block is not a mesh. Do not relabel a class as geometry |
| Warning (recorded) | The 12-byte group reading is a LATER observation (evidence confirmed, semantics unknown). It must not be cited as a format proof |
| Legal | Format and behavior facts only. No game bytes, no absolute game paths, no coordinates, no hashes. Tests use synthetic bytes; the real tree was a read-only playtest input |

## Branch cleanup report

Containment was checked with `git branch --merged main` plus a per-branch
`git cherry`/tree check. For a branch that is not an ancestor, the check
confirms every commit it carries is already applied on `main`.

| Branch | State | Action |
|---|---|---|
| `feature/room-geometry` | ancestor of `main` (PR #44 head) | Deleted local + remote |
| `feature/room-geometry-plan` | not an ancestor; its single commit (`020a28c`) is patch-equivalent to `main`'s `b47f54a`, so nothing is unique | Deleted local + remote |
| `docs/archive-area` | ancestor of `main` (PR #43 merge) | Deleted local + remote |
| `dev`, `dev-2-0`, `dev-3.0` | unrelated long-lived branches | Left untouched |

## Checklist

- [x] PR #44 MERGED and `main` contains the change (HEAD `52f88f2`)
- [x] Playtest evidence recorded verbatim (small unit, large unit, room directory)
- [x] Census recorded with counts, sizes, and shares only
- [x] Directory-reading correction stated explicitly (records 0/1 are all zero; offset table is the only directory)
- [x] Later 12-byte group observation recorded with its evidence class and UNKNOWN semantics
- [x] Non-goals stated: geometry, placement/transforms, rendering, type taxonomy are NO-GO
- [x] Commit list, PR URL, and MERGED state recorded
- [x] Contained branches deleted after containment checks; unrelated branches kept
- [x] README "What works" carries the census and "What's missing" points at placement/transforms
- [x] No game bytes, absolute game paths, coordinates, or hashes added by this archive

## Next step

Probe **placement and transforms** next, starting from the 12-byte group
hypothesis `[u32 id-ish][f32 value][u32 flags]` and its `0x34`-high-byte tag
families. Room geometry stays **UNPROVEN**: the dominant block's 12-byte row
signal is still statistical, no vertex or face count is claimed, and the PMF
codec remains a separate audio slice.

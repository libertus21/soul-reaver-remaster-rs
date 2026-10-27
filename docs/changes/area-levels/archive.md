# Archive: area-levels — DONE (level graph inventoried; geometry still NO-GO)

**Stage banner: DONE for the container/name table; NO-GO for geometry, placement, and audio.**

`--level-info <PATH>` reads the SR1-remaster `.PCM` unit container and prints a
read-only level graph: one `level` line per unit (record count, accepted name
count, cross-link count, PMF/PNF sidecar counts) plus one `  link <target>
<index>` line per cross-link name. It opens no window, decodes no geometry,
places no objects, and never claims playback. Playtest accepted the output on a
linked room (31 units) and a single-unit room; malformed input and empty rooms
fail loud. PR #42 is merged into `main` at `a22db12`.

## Quick path

1. `git checkout main && git pull` — HEAD is `a22db12` (merge of PR #42).
2. `cargo run -- --level-info "<room directory>"` — for example the `AREA/LAIR`
   room; the real path is resolved at runtime.
3. Expect 31 `level` lines plus `  link <target> <index>` lines; exit 0.
4. Point it at the `AREA/NIGHTB` room — it holds no `.PCM`, so it exits 2 with
   `error: no .PCM units in the directory`.
5. `cargo test` — 226/226 green on synthetic bytes only.

## Delivered and working

| Item | Evidence |
|---|---|
| CLI | `--level-info` accepts exactly one `.pcm` file or one room directory; a directory resolves its `BIN` when present, else itself; units sort by name |
| PCM header model | `u32 record_count`, then `record_count` `u32` relative offsets; `offset[0] = 0`, `offset[1] = 4`, strictly ascending; data base `4 + 4N`; last record start inside the file. Holds 89/89 |
| Name table | Printable runs of 3..=20 bytes matching `^[a-z][a-z0-9]*(,[0-9]{1,2})?$`; every file must contain its own stem (case-insensitive); duplicates preserved, file order kept. Own-name guard 89/89 |
| Cross-links | `<target>,<index>` names split into `Link { target, index }`; target existence is not required (special and absent targets are real data) |
| Sidecars | `X.PMF` + `X.PNF` looked up case-insensitively next to the unit; `FMPa`/`FNPa` magic, segment count, rate field, PNF length rule, and id/count agreement; a lone or mismatched pair is a hard error |
| Exit matrix | 0 = at least one unit printed; 1 = usage; 2 = malformed container / I/O / no units / bad or incomplete sidecar; 3 = refused path; nothing prints on failure |
| Isolation | Header and name table only; `src/level.rs` is a pure module plus one `main.rs` arm; `render.rs`, `srm.rs`, `mesh_entry.rs`, `tex_hd_map.rs`, `bigfile.rs` untouched |
| Gates | 226/226 tests, zero warnings, legal clean, repository private |
| Authored lines | 324 production + 311 tests (under the 330 stop rule) + 36 docs |

## Accepted playtest evidence (verbatim)

Linked room (`AREA/LAIR`) — 31 unit lines with cross-links:

```text
level lair1 pcm_records=1655 names=126 links=4 pmf=absent segments=0 rate_field=0 pnf=0
level lair10 pcm_records=1720 names=101 links=6 pmf=present segments=3 rate_field=22050 pnf=3
```

with link lines including:

```text
  link lair29 66
  link lair3 51
```

Single-unit room (`AREA/ALUKA`) — one unit:

```text
level aluka13 pcm_records=553 names=15 links=0 pmf=present segments=5 rate_field=22050 pnf=5
```

Invocation and empty-room refusals:

| Run | Observation |
|---|---|
| no arguments | `error: expected --level-info <path>`, exit 1 |
| `AREA/NIGHTB` (room with no `.PCM`) | `error: no .PCM units in the directory`, exit 2 |

## Census (read-only, counts only)

- 89 `.PCM` units across 11 rooms; 89/89 pass the header model and the own-name guard.
- 2,106 accepted names; 368 cross-links with 346 resolving (94.0%); 11 linkless units.
- Sidecars present in 23 units, absent in 66.
- Segment counts: 1 × 11, 2 × 2, 3 × 7, 4 × 2, 5 × 1.
- Rate field: 22,050 × 18, 11,025 × 5.
- PNF entries equal the PMF segment count in 23/23.

## Deliberate non-goals (NO-GO)

| Area | Status |
|---|---|
| Room geometry and collision | **NO-GO** in this change; record payloads are not interpreted |
| Object placement and transforms | **NO-GO**; no instance/transform table was located |
| Rendering | **NO-GO**; the mode opens no window |
| PMF audio decode and playback | **NO-GO**; the tool reports sidecar numbers only and never claims playback |
| Link semantics | **INFERRED** (portals/doors), not proven; only 94.0% target resolution is evidence |

## Details

| Topic | Outcome |
|---|---|
| PR | [#42 — feat(level): inventory .PCM headers, name tables, and sidecars](https://github.com/libertus21/soul-reaver-remaster-rs/pull/42), state **MERGED** (`mergedAt` `2026-10-27T05:23:50Z`), base `main`, head `feature/area-levels`, 8 files, +1332/−0 |
| Commits on `main` | `1d673a1` plan the SR1 room container probe → `5c39bba` inventory `.PCM` headers, name tables, sidecars → `47334ac` record the apply census and README slice → `a22db12` merge of PR #42 |
| Workload | Single PR, no chain: production 324 and tests 311 both under the 330 stop rule; docs 36 |
| Process win | Tests were budgeted as a first-class line (~1.5× the first instinct) and the single-PR forecast held, so no mid-apply split was needed |
| Warning (recorded) | The census calls `.PCM` "the unit container" from header and name evidence only; the record-payload meaning stays UNKNOWN and must not be relabeled as geometry |
| Legal | Format and behavior facts only. No game bytes, no absolute game paths, no coordinates, no hashes. Tests use synthetic bytes; the real tree was a read-only playtest input |

## Branch cleanup report

Containment was checked with `git merge-base --is-ancestor <ref> main`. For a
branch that is not an ancestor, the paths it touched were diffed against `main`;
only main-side content (ticked checkboxes, encoding normalization, or a later
archive file) was present, so the branch had nothing `main` lacks.

| Branch | State | Action |
|---|---|---|
| `feature/area-levels` | ancestor of `main` (tip `47334ac`) | Deleted local + remote |
| `feature/area-levels-plan` | not an ancestor (tip `6144ec7`); diff is 22 main-side lines in `docs/notes/area-levels.md` | Deleted local + remote |
| `feature/anim-binding-plan` | not an ancestor (tip `6ec05c2`); all planning files identical in `main` | Deleted local + remote |
| `feature/sra-animation-plan` | not an ancestor (tip `d4f145c`); all planning files identical in `main` | Deleted local + remote |
| `docs/animation-investigation` | ancestor of `main` (tip `fd02c97`) | Deleted local + remote |
| `docs/archive-animation` | ancestor of `main` (tip `a75c42e`) | Deleted local + remote |
| `docs/archive-audio` | ancestor of `main` (tip `92c9f72`) | Deleted local + remote |
| `feature/big-header-plan` | not an ancestor (tip `da1d49e`); diff is checkbox/encoding churn in `docs/changes/big-header/tasks.md` | Deleted local |
| `feature/big-slots-plan` | not an ancestor (tip `912ef5e`); diff is checkbox/encoding churn plus the `archive.md` that `main` added | Deleted local + remote |
| `feature/slot-name-plan` | not an ancestor (tip `de2afa3`); diff is checkbox/encoding churn in `docs/changes/slot-name/tasks.md` | Deleted local + remote |
| `feature/f4-launcher-cli-plan` | remote-only, not an ancestor (tip `26a43f5`); all planning files identical in `main` | Deleted remote |
| `dev`, `dev-2-0`, `dev-3.0` | unrelated long-lived branches | Left untouched |

## Checklist

- [x] PR #42 MERGED and `main` contains the change (HEAD `a22db12`)
- [x] Playtest evidence recorded verbatim (linked room, single-unit room, two refusals)
- [x] Census recorded with counts only
- [x] Non-goals stated: geometry, placement/transforms, rendering, PMF playback are NO-GO
- [x] Link semantics marked INFERRED, not proven
- [x] Commit list, PR URL, and MERGED state recorded
- [x] Contained branches deleted after containment checks; unrelated branches kept
- [x] README "What works" carries the level graph and "What's missing" names room geometry
- [x] No game bytes, absolute game paths, coordinates, or hashes added by this archive

## Next step

Probe room geometry **inside the `.PCM` records**: classify records 0/1 as a
directory, then map record kinds to terrain, collision, and objects before any
decode claim. Object placement and transforms come only after that; the PMF
codec is a separate audio slice.

# Design: s4-run-internals

Call-site order first, identity second: walk `run`'s CALL sites in address
order, then anchor each stage with strings/imports at depth 1-2. Output is a
behavior map with a class per row — never a listing, never a struct guess.

## Quick path

`survey + S1/S2/S3 notes` -> read-only TEMP project (re-pin `run` by export
name) -> ordered call-site walk -> depth-1/2 anchors -> stage table + entry
points + state surfaces -> hygiene gate.

## Method (bounded, time-boxed)

| Step | Bound | Tool |
|---|---|---|
| Project | Reuse the TEMP engine project read-only, `-noanalysis`. If its marker file is missing, re-import the engine library into a fresh TEMP project (read-only input, ~2 min analysis) — plan-time recon hit exactly this. Never the repo or game dir. | `analyzeHeadless` 12.1.4 |
| Re-pin | Resolve the `run` export by export name (never address); resolve the default archive name and stage strings by case-insensitive defined-string search + xrefs. | Symbol table + string/xref |
| Ordered walk | Enumerate CALL instructions inside `run`'s body in ascending address order; record call-site ordinal + resolved callee. Only source of stage order. | Instruction iterator |
| Stage facts | Per callee: instruction count, callee count, defined-string refs (short printable quotes), thunk-resolved external names. | Function body walk |
| Depth 2 | One more level per stage candidate: strings + thunk-resolved imports, capped per stage. | Same walk, bounded |
| Box | 2 probe passes, 60-minute wall box. Pass 1 = ordered walk + depth-1; pass 2 = depth-2 anchors + state-surface sweep. Expiry leaves UNKNOWN rows. | Record passes + wall time |
| Note shape | New `docs/notes/s4-run-internals-behavior.md`: stage table, entry points, surfaces, follow-ups, not-established list, re-run steps, legal block. | Paraphrase + short string quotes |

## Evidence classes

| Class | Definition | Applies to |
|---|---|---|
| CONFIRMED (edge) | A CALL site inside `run` (or a named stage's body) resolves to the function. | Invocation, order |
| CONFIRMED (anchor) | A defined string or thunk-resolved import inside the function anchors the purpose. | Identity, entry points, surfaces |
| INFERRED | Only function shape or naming supports the purpose; no anchor. | Provisional stage names |
| UNKNOWN | No call edge and/or no anchor inside the box. | Device init, runtime order, field layouts |

A row needs CONFIRMED (edge) for existence and order; its NAME may stay
INFERRED. Anchors upgrade the name; absence keeps it INFERRED.

## Stage table skeleton (seed; apply re-derives and may demote)

Order is UNKNOWN until the ordered walk runs; these unordered seeds carry their
anchoring evidence.

| Stage (provisional) | Invocation | Anchor evidence | Class |
|---|---|---|---|
| Default-archive handoff | `run` body itself | default archive name; S3 L4 | CONFIRMED (anchor) |
| Boot/archive orchestrator | 1 of 7 direct callees | archive name, logo movie names, `KAININT`, reaches load-failure half, memory-fit diagnostics | CONFIRMED (edge+anchor); name INFERRED |
| HD texture-map loader | direct callee | `AREA\TEX_HD.MAP`; its callee references `AREA\FX_TEX_MAP.TXT` | CONFIRMED (edge+anchor) |
| Localization / string tables | direct callee | `STRINGS`, `COMMON`, `English`, `PC%s\` | CONFIRMED (edge+anchor) |
| Legal splash + SFX bank | direct callee | `SFX.WAS`, `LEGAL_1..4` | CONFIRMED (edge+anchor); "splash" INFERRED |
| Common-strings init | direct callee (small) | `COMMON` | CONFIRMED (edge); purpose INFERRED |
| Bonus content init | direct callee (small) | reaches `BONUS`, `MAP_LORE_PURE` at depth 2 | CONFIRMED (edge); purpose INFERRED |
| Splash / menu bootstrap | direct callee (largest) | `SPLASH`, glyph/menu formatters, reaches bonus init | CONFIRMED (edge+anchor); role INFERRED |
| Graphics / window / audio init | NOT OBSERVED | none reachable; imports are KERNEL32 only | UNKNOWN |

## Subsystem entry points

| Surface | Entry-point candidate | Structure evidence |
|---|---|---|
| Archive boot | boot orchestrator | four-name archive pointer table (documented survey layout fact) |
| Texture overrides | HD texture-map loader + its map-table callee | map-file path strings; table layout UNKNOWN |
| Text/localization | localization init | string-table family names only |
| Audio bank | legal-splash/SFX callee | `SFX.WAS`; no device API reachable statically |
| Front end | splash/menu bootstrap | menu glyph formatters, splash string |
| Bonus | bonus init | `BONUS` / `MAP_LORE_PURE`; survey-documented bonus strings |
| Movies | boot orchestrator (logo names) | movie-name literals; playback API UNKNOWN |

## Structures: "built first" discipline

A stage may be described as touching a surface when a string xref ties them
(archive-name pointer table, movie slots, bonus-state strings). Field order,
size, and population sequence are never asserted: the bounded method does not
read writes to initialized data, so write order is UNKNOWN and listed as such.

## Tooling gotchas (carry into apply)

- `getCalledFunctions()` returns an UNORDERED set; only ascending CALL-site
  addresses produce a sequence.
- Import reachability needs thunk resolution plus instruction-operand
  reference ranges. Plan-time recon reported zero imports for a set that must
  contain them: that zero is a probe artifact, not evidence — fix or record
  UNKNOWN.
- A TEMP project can lose its marker file: failed open means import fresh.
  Counts and `FUN@` names shift; re-pin every time.

## Follow-up slices (by rewrite value)

| ID | Slice | Why now |
|---|---|---|
| S5 | HD texture override table (`TEX_HD.MAP`, `FX_TEX_MAP.TXT`) | Feeds the texture pipeline the rewrite already has |
| S6 | Archive selection + secondary `.big` surfaces | Completes the asset layer after S1/S2 |
| S7 | Localization / string-table family | Cheap, high reuse for UI text |
| S8 | Front-end + bonus init | Menus, gallery, bonus content |
| S9 | Movie/logo playback path | Only startup visual until gameplay |
| D1 | Dynamic trace (deferred, needs execution) | Only route to runtime order under C8 |

## Deliberately NOT built

Runtime execution or tracing, body decompilation, struct/field reconstruction,
engine code changes, loader error rows (S1/S2), launcher CLI (F4), GUI Ghidra
sessions, committed game data, box extensions without user approval.

## Checklist

- [ ] Reviewer can state why order requires call-site addresses, not a callee set.
- [ ] Reviewer can trace each CONFIRMED row to a call edge plus an anchor string.
- [ ] Reviewer can state what stays UNKNOWN (device init, layouts, runtime order).
- [ ] No retail addresses, listings, or game bytes; box and pass budget stated.

## Next step

Implement in the order given in `tasks.md` as a SINGLE PR (forecast there).

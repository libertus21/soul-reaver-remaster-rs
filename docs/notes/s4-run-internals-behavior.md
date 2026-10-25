# Behavior notes: s4-run-internals - engine startup inside `run` (apply-time)
Tasks 1-3 of `docs/changes/s4-run-internals/tasks.md`. Behavior paraphrases only:
no disassembly, no decompiled code, no Ghidra DBs, no addresses, `FUN_` names,
RVAs, paths, hashes, or game bytes. Quoted strings are the evidence; every label
was assigned fresh at apply time and is re-derivable from them (see Re-run).
## Method (bounded; 2 analysis passes + verification re-runs; ~5 min of 60)
- Ghidra `12.1.4` headless, engine library read-only, TEMP project outside repo
  and game dir (the plan project's marker was lost: re-imported fresh, read-only).
- Pass 1: re-pin `run` by export name; walk CALL sites in ascending call-site
  address order; per callee record instruction count, call-site count, callee
  count, defined-string refs and thunk-resolved imports (operand-range read fixed).
- Pass 2: depth-2 anchors per stage plus a depth<=2 external-import sweep;
  verification re-runs reproduced the order, counts and instruction metrics.
- DRM / anti-cheat / packing: none encountered on this line.
## Re-pin (fresh counts)
- Exports: 3; `run` found once (also ordinal 1). Body: 56 instructions, 10 CALL
  sites, 7 distinct direct callees, 0 direct imports, 1 direct string (the default
  archive name). 7 sites are unconditional and resolved; 3 are computed calls with
  no operand reference and no resolvable target - they stay UNKNOWN.
- Depth<=2 import sweep (operand-range read fixed): 0 external functions reached
  across 121 functions. That zero is a bounded depth result, not proof of absence.
Static order is not runtime order: the sequence below is ascending CALL-site
address order inside `run`, not measured execution order (nothing was executed).
## Step zero - default-archive handoff (CONFIRMED anchor)
`run` itself references the default archive name `bigfile.dat` in its body (no
call involved; S3 L4). The same name is referenced by the boot orchestrator
(#9), which reaches the archive load-failure message at depth 2 (the S1 open path).
## Ordered stage table (call-site order)
Call sites 1, 6, 7 are computed calls with no resolvable callee: NOT stage rows (UNKNOWN).
| # | Stage (provisional) | Edge | Anchor evidence | Instr/calls/callees | Class |
|---|---|---|---|---|---|
| 2 | HD texture-map loader | direct | `AREA\TEX_HD.MAP`; d2 `AREA\FX_TEX_MAP.TXT` | 194/34/10 | CONFIRMED edge+anchor |
| 3 | Common-strings init | direct | `COMMON` | 9/1/1 | CONFIRMED edge; purpose INFERRED |
| 4 | Legal splash + SFX bank | direct | `SFX.WAS`, `LEGAL_1..4` | 111/20/9 | CONFIRMED edge+anchor; "splash" INFERRED |
| 5 | Splash / menu bootstrap | direct | `SPLASH`, menu/joy glyph formatters; d2 bonus init | 635/64/12 | CONFIRMED edge+anchor; role INFERRED |
| 8 | Localization / string tables | direct | `STRINGS`, `COMMON`, `English`, `PC%s\` | 122/13/9 | CONFIRMED edge+anchor |
| 9 | Boot/archive orchestrator | direct | `bigfile.dat`, `publogo.bik`, `crylogo.bik`, `KAININT`; d2 `Can't load big file named "%s"` | 496/120/88 | CONFIRMED edge+anchor; name INFERRED |
| 10 | Bonus content init | direct | `BONUS`, `MAP_LORE_PURE` (d2) | 35/2/1 | CONFIRMED edge; purpose INFERRED |
## Entry points and state surfaces
| Surface | Owning function | Anchoring evidence | Class |
|---|---|---|---|
| Archive selection / movies | Boot/archive orchestrator (#9) | archive name, movie-name literals | CONFIRMED edge; layout UNKNOWN |
| HD texture overrides | HD loader (#2) + its map-table callee | `TEX_HD.MAP`, `FX_TEX_MAP.TXT` | CONFIRMED; table layout UNKNOWN |
| Text / localization | Localization init (#8) | `STRINGS`, `COMMON`, `English`, `PC%s\` | CONFIRMED; family layout UNKNOWN |
| Audio bank | Legal splash + SFX (#4) | `SFX.WAS`; no audio API at depth<=2 | CONFIRMED edge; device UNKNOWN |
| Front end | Splash / menu bootstrap (#5) | `SPLASH`, glyph formatters | CONFIRMED; role INFERRED |
| Bonus | Bonus init (#10) | `BONUS`, `MAP_LORE_PURE` | CONFIRMED edge; purpose INFERRED |
| Graphics / window / audio device | none | no device import or string at depth<=2 | UNKNOWN |
## Follow-up slices
- S5 HD texture override table (feeds the rewrite's texture pipeline); S6 archive
  selection + secondary `.big` surfaces (S1/S2 seam); S7 localization/string
  family (cheap UI reuse); S8 front-end + bonus init; S9 movie/logo playback;
  D1 dynamic trace (only route to runtime order).
## Deliberately not established
Runtime order (C8), the 3 computed call targets, device/window/audio creation, surface
layouts/write order, orchestrator internals beyond depth 2 - UNKNOWN, none inferred.
## Re-run (second pair of eyes)
Repeat Pass 1-2 read-only: the 7-row order, 3 computed sites, counters above and
the depth<=2 import zero must reproduce; anything that does not is demoted.
## Legal
Binaries read-only; Ghidra projects under `%TEMP%`, outside repo and game dir. No
bytes, paths, hashes, addresses, `FUN_` names, or listings recorded here.
## Box
2 analysis passes + 3 verification re-runs (~5 min of 60). No row was demoted on
expiry; every UNKNOWN above is evidence-bounded, not time-bounded.

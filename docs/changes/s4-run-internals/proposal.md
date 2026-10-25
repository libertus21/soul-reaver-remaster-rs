# Proposal: s4-run-internals

Document what the engine's `run` entry does after the launcher hands it the
default archive: ordered startup stages, the function owning each subsystem,
and the core state surfaces built before gameplay. S3 proved the handoff
(`run` + default archive name, L4 CONFIRMED) but left the startup contract
UNKNOWN; S4 turns that gap into a bounded BEHAVIOR map. Docs-only.

## Quick path

1. Reopen the TEMP engine project read-only (`-noanalysis`) and re-pin `run`
   and the default archive name by string xref — never by address.
2. Walk `run`'s CALL sites in address order for the stage sequence, then
   collect one level of callee evidence (strings, thunk-resolved imports).
3. Write `docs/notes/s4-run-internals-behavior.md`: ordered stage table with an
   evidence class per claim, entry points, first-built surfaces, UNKNOWN list.

## Decisions

| Topic | Decision |
|---|---|
| Slice scope | S4 only: startup sequence inside `run`, per-stage owning functions, first-built state surfaces. Deliverable is a BEHAVIOR map — no code, no struct layouts, no listings. Loader error rows (S1/S2), launcher CLI (F4), dynamic tracing stay out. SR1 only. |
| Order rule | Stage ORDER comes from CALL-site address order inside `run`. An unordered callee set is never presented as a sequence. |
| Evidence rule | Every row carries CONFIRMED (call edge + string/import xref), INFERRED (naming only), or UNKNOWN. No stage invented from plausibility. |
| Structure rule | Surfaces are named only where a string xref or documented layout fact anchors them (archive-name pointer table, movie slots, bonus state). Field layouts and write order stay UNKNOWN unless observed. |
| Re-pin rule | `run` and every string are re-resolved at apply time. Survey RVAs and `FUN@` names are hints only, never in notes or code. |
| Box | 2 probe passes / 60-minute wall box. Expiry leaves rows UNKNOWN with their no-evidence result. |
| Budget guard | Single PR <=400 authored lines (plan + behavior note, docs-only). |
| Tests | `cargo test` is a no-op baseline (no code touched). |
| Stop condition | DRM / anti-cheat / packing on this line: STOP that line and report. Never defeat anything. |

## Seed evidence (plan-time recon, own 2-pass box, read-only)

- `run` has a small body, 7 direct callees, no direct imports, and one string:
  the default archive name.
- One callee is the heavyweight boot orchestrator (archive name, logo movie
  names, a `KAININT` literal, reaches the load-failure half and memory-fit
  diagnostics). Others anchor an HD texture-map loader (`AREA\TEX_HD.MAP` then
  `AREA\FX_TEX_MAP.TXT`), localization (`STRINGS`, `COMMON`, `English`), a
  legal-splash plus SFX bank (`SFX.WAS`, `LEGAL_1..4`), a small `COMMON` init,
  bonus init (`BONUS`, `MAP_LORE_PURE`), and splash/menu bootstrap.
- No graphics/window/audio device stage appears at depth <=2, and the library
  imports KERNEL32 only — device creation is dynamic or outside this module
  (UNKNOWN, never guessed).

Seeds carry classes, not results: apply re-pins them fresh and may demote any
row the re-run does not reproduce.

## Checklist

- [ ] Every stage row cites a call site (order) plus an anchor (identity).
- [ ] Unresolved rows are UNKNOWN with an explicit no-evidence record.
- [ ] Entry points and first-built surfaces named or marked UNKNOWN.
- [ ] Single PR <=400 lines; `cargo test` green; `git status` shows only intended files.

## Next step

Spec (`spec.md`) fixes the claim set and UNKNOWN rules; design (`design.md`)
fixes the ordered call-site method and time box; `tasks.md` lists the atomic
apply units with the workload forecast.

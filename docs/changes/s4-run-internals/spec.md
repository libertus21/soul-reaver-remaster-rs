# Spec: s4-run-internals

Document the engine startup sequence inside the `run` export of `sr1.dll`:
ordered stages, subsystem entry points, and the core state surfaces built
before gameplay. S3 closed the handoff (L4 CONFIRMED) and left the contract
UNKNOWN; S4 must supply ordered call-site evidence. Behavior notes only.

## Quick path

1. Apply re-pins `run` by export name and the default archive name by string
   xref, then walks `run`'s call sites in address order (Task 1).
2. Stage names get an identity class from anchors; table, entry points, and
   surfaces land in behavior notes (Tasks 2-3). `cargo test` is the no-op gate.

## Claims

| # | Claim | Evidence required | Verdict |
|---|---|---|---|
| C1 | `run` invokes N startup steps in a specific order | CALL sites inside `run` enumerated in address order, each mapped to its callee | Proven per step only; total N re-derived at apply |
| C2 | Each named stage is really invoked by `run` (directly) | A call edge from `run` to the stage function | CONFIRMED when the edge exists; otherwise the stage is not a `run` stage |
| C3 | Each stage's purpose (what subsystem it starts) | Defined-string refs and/or thunk-resolved imports inside the stage function or its immediate callees | CONFIRMED only when a string/import xref anchors the purpose; naming alone = INFERRED |
| C4 | The default archive handoff is step zero | `run` references `bigfile.dat` and its first-ordered call site reaches the archive open path | CONFIRMED (S3 L4 + re-pin) |
| C5 | Named subsystem entry points | Stage function that owns the subsystem's first call edge, plus the anchoring strings | CONFIRMED edge; ownership INFERRED unless a second independent signal agrees |
| C6 | Core state surfaces built first (archive selection, movie slots, bonus state, save-type area) | A stage call site plus a string xref to the surface's pointer table or message family | CONFIRMED (surface touched) / UNKNOWN (field layout, write order) |
| C7 | Graphics, window, and audio device creation happen inside `run`'s reachable set | Import or string xref naming a device/window/audio API reachable from the stage set | Expected UNKNOWN: `sr1.dll` imports KERNEL32 only, so device init is dynamic or outside the module |
| C8 | Runtime execution order equals static call-site order | Runtime observation | UNKNOWN by construction (static only; nothing executed) |

## Requirements

- **R1 — Re-pin only.** Every symbol, string, and function is resolved at apply
  time by export name or string xref; no survey RVA, `FUN@` name, or retail
  offset appears in notes or code.
- **R2 — Order needs call sites.** The stage sequence comes from CALL
  instructions inside `run` in address order; an unordered callee set is never
  presented as a sequence.
- **R3 — Classed rows.** Every row carries CONFIRMED (edge plus anchor),
  INFERRED (naming/shape only), or UNKNOWN; no call edge means no stage row.
- **R4 — Structures named, not invented.** Surfaces are limited to survey
  layout facts or fresh string/table xrefs; no field, offset, size, or
  population order without direct observation, else UNKNOWN.
- **R5 — Paraphrase only.** No disassembly, decompiled listings, Ghidra DBs,
  game bytes, paths, or hashes in the repo. Short string quotes already in
  committed notes are allowed evidence.
- **R6 — Stop on protection.** DRM / anti-cheat / packing: STOP that line,
  record it, never attempt to defeat it.
- **R7 — Docs-only.** Zero production-code changes; `cargo test` stays green.
- **R8 — Box discipline.** 2 passes / 60 minutes; on expiry record the partial
  map with UNKNOWN rows — scope is never widened or absorbed elsewhere.

## UNKNOWN rules

- No CALL site inside `run` = not a stage; such a function may appear as
  "reached indirectly" but never in the ordered table.
- Call edge but no anchor = CONFIRMED-invoked / INFERRED-purpose, with reason.
- No import or string xref = UNKNOWN. A KERNEL32-only import table is evidence
  of absence, not of device init.
- Static order is not runtime order: state the difference once, claim no
  observed execution.

## Acceptance scenarios

- [ ] **Ordered map:** the stage table lists each `run` call site in address
  order with callee role label, evidence class, and anchoring strings.
- [ ] **Handoff first:** the default archive name row is CONFIRMED and ordered
  first, and it reaches the archive open path.
- [ ] **Subsystem entry points:** each subsystem named in the table has one
  owning function and its anchoring evidence; the rest are UNKNOWN.
- [ ] **Structures honesty:** state surfaces list only xref-anchored tables and
  message families; field layouts remain UNKNOWN.
- [ ] **Docs-only:** `cargo test` green, `git status --short` shows only plan
  docs plus the behavior note, no listings/DBs/game data.
- [ ] **Box:** probe pass count and wall time recorded; every unresolved row has
  its no-evidence result.

## Next step

Design (`design.md`) fixes the ordered call-site walk, the depth budget, and
the note shape; `tasks.md` makes apply atomic with the workload forecast.

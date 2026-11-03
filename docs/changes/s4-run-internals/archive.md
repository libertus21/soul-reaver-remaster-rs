# Archive: s4-run-internals — DONE (behavior map)

**Stage banner: DONE — ordered startup stages inside `run`, behavior only.**

`run` is 56 instructions with 10 call sites, 7 distinct direct callees, zero
direct imports and one direct string (the default archive name). Seven call sites
give an ordered stage map: HD texture-map loader, common-strings init, legal
splash + SFX bank, splash/menu bootstrap, localization/string tables, boot/archive
orchestrator and bonus-content init. Three computed calls have no resolvable
target and stay UNKNOWN. No graphics/window/audio device appears at depth ≤ 2.
Evidence: `docs/notes/s4-run-internals-behavior.md`.

## What landed
- PR #23 (`feature/s4-run-internals`, merge `3f3960b`, commits `c465079`,
  `45b3cfb`) — proposal/spec/design/tasks plus the behavior note.
- Docs only; no `src/` change; `cargo test` baseline green.

## What stayed UNKNOWN
- Runtime order (the map is call-site address order; nothing was executed).
- The 3 computed call targets and device/window/audio creation.
- Surface layouts, write order, and orchestrator internals beyond depth 2.

## Next step
Dynamic tracing is the only route to runtime order; the HD texture override table
(S5) and archive selection (S6) are the cheapest follow-ups.

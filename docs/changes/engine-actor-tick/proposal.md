# Proposal: engine-actor-tick

## Intent

Make the existing actor machinery run inside the fixed frame loop without
inventing AI behaviour. The carrier (`ActorRuntime`), the profile data and the
numeric relation/action fields already exist. This slice connects them to the
real per-object stage and makes them part of the canonical state.

## What is plumbing

- **Dispatch.** Stage 12 asks the class table for the tick hook and calls it in
  live order. Only `raziel__` and `monster_` install a non-null tick; the other
  four classes make zero calls. This is the documented null-hook opt-out.
- **State carriage.** A live object owns `Option<ActorRuntime>`. It is populated
  from the loaded model's actor block only, never from a name or a class. The
  bound player receives its profile during the shared boot pass.
- **Transition entry point.** `ActorRuntime::advance` is the single documented
  way the carrier may change. It returns
  `ActorTransition::NotInterpreted`: no relation change, no action id selected.
- **Behaviour-id carriage.** The eight trios and the behaviour-entry ids stay
  observable data on the profile. Ticking reads none of them.
- **Determinism.** The canonical fold gains, per live object in live order, an
  actor-presence byte plus the relation and state bytes. Non-actors fold
  `0/0/0`. The world, frame-state and headless goldens are re-pinned in the same
  PR; the spec change is recorded.

## What is deliberately NOT interpreted

`docs/notes/actor-ai.md` and `docs/notes/actor-behaviour-data.md` mark these
UNKNOWN, so this slice does not fill them:

- The named state enum and the meaning of any state/action id. The ids are
  model-relative bytes; no enum is invented.
- The meaning of the `0..3` relation codes beyond the documented numeric space.
- Perception, engagement radius, melee gate, turn-to-face, disengage.
- The model byte that selects an initial/later behaviour entry set.
- Any mapping from a trio row to a named state or an animation clip.
- Damage, health, hit records and death.

`advance` selects nothing rather than returning a plausible-looking default.

## Non-goals

No animation playback, no collision or contact, no script units, no damage, no
pathfinding, no audio, no streaming. The player interaction tick and the monster
locomotion thunk are not implemented; both class rows share one carriage body
only because the decision and locomotion bodies are out of scope.

## Acceptance

- Only the two tick classes receive the per-object hook; the four other classes
  receive none.
- The same actor world replays the same per-tick actor state.
- The trios and behaviour ids are observable and unchanged by ticking.
- A non-tick class never advances a carrier even when one is attached.
- The canonical digest includes relation and state, and dropping `state` fails
  the gate.

## Legal

No game bytes, decompiled listings or absolute install paths are committed. The
real-data check runs read-only against the user's own corpus; nothing from it is
stored in the repository.

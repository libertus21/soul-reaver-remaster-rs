# Live self-test: a real-runtime gate for probes

A probe is not shippable until its agent runs inside the real Frida runtime.
Mocked self-tests cannot catch API-shape bugs because the mock is written from
the same assumption that produced the bug; three probes in this line shipped
broken for exactly that reason. This change adds a live harness mode to the
staging-split probe, fixes the crash the harness exposed, and turns the gate into
a standing rule for every future probe.

The probe itself stays in the session scratch directory because it carries
module-relative anchors and engine field offsets. This note is the committed
artifact and deliberately contains no offsets, addresses, game bytes, or install
paths.

## Quick path

1. Run the live gate: `python sr1_staging_split_probe.py --live-selftest`.
2. Run the proof that the gate works: `python sr1_staging_split_probe.py --live-selftest-regress`.
3. Both must print `PASS` before any capture session.

The live gate spawns a throwaway system process, never the game. It builds
synthetic memory inside that process and drives the agent with the real Frida
bindings.

## The bug class

Every failure in this class is a wrong assumption about the Frida API surface.
Because the historical self-tests mocked that surface in Python or Node, the mock
implemented the same wrong assumption and the test passed. The runtime was the
only component that could disagree, and it was not in the test.

| Instance | Assumption in the probe | What the real runtime does |
|---|---|---|
| Pointer conversion | `NativePointer.toNumber()` | No such method; only the 64-bit integer types convert that way |
| Backtrace API | global `BacktraceType` | The Frida 17 global is `Backtracer` |
| Register read | register names live on the callback argument | The argument is `InvocationArguments`; registers live on `this.context` |

The third instance produced the reported failure. `Interceptor` calls
`onEnter(args)` and binds the invocation context to `this`. `args` is an exotic
object whose numeric-index getter parses the property name as an argument index.
Reading a register name such as `args.rcx` makes that getter reject the name and
throw `RangeError: invalid array index`, on every invocation. The probe's
`try/catch` swallowed it and reported `SPLIT ... reason=unobserved`, so the run
looked like "no evidence" instead of "wrong API".

The same callback contract applies to `onLeave`, which receives the return value
as its argument and the invocation context as `this`.

Why a mock cannot catch this class:

- A mock defines the object the test author believes exists. If the author
  believes `args.rcx` is a register, the mock returns a register.
- A mock never runs the exotic argument object, the real `NativePointer`,
  `Memory`, `Process`, `DebugSymbol`, `Interceptor`, `NativeCallback`, or
  `NativeFunction` implementations.
- A mocked test can therefore prove internal arithmetic and nothing about
  whether the probe can execute against a real agent.

## Harness design

`--live-selftest` runs five scenarios in order and fails loudly on any throw.

| Layer | What it does |
|---|---|
| Process | Spawns a throwaway system process and kills it at the end. The game is never attached or launched |
| Synthetic memory | One zeroed region shaped like the agent's records: a node with channel count, track head, and a channel array; a track-pool pointer with one entry; a staging array of kind words. Shape only, never game bytes |
| Disarmed hooks | The agent lifecycle never installs its channel hook in live mode. The harness sets the module range and staging base itself, then calls the internals directly |
| Direct invocation | Calls `snapshotGeometry`, `readModes`, `objectKey`, `hashId`, and `slotsSummary`, then drives the enter/leave callbacks with fake contexts |
| Faithful fake context | `this` carries `threadId`, `returnAddress`, and `context` with `rcx`/`rsp` as real `NativePointer`s. The first callback argument is a proxy that resolves numeric names and throws the same `RangeError` for every other name, mirroring the real `InvocationArguments` |
| Real Interceptor round-trip | Hooks a `NativeCallback`, calls it with the synthetic node, and lets the agent callbacks receive the actual `InvocationArguments` and `InvocationContext` from Frida |
| End-to-end assertion | A synthetic packed shape yields `SPLIT packed=62`; a synthetic mode shape yields `packed=50 mode=1`; an empty shape yields `unobserved`; the real round-trip yields no enter errors and at least one record |

The fake context intentionally does not put `rcx` directly on `this`: the real
runtime does not, and a harness that invents that shortcut would recreate the
same failure mode it exists to prevent.

### Error reporting

Any caught exception is reported with three fields:
- the stage tag naming the function that was executing;
- the message;
- the first three stack frames.

Identical failures are counted rather than printed again, so a permanently
broken callback cannot flood the log with the same bare message hundreds of
times. The final `SPLIT` remains truthful: a run with no records still says
`unobserved`.

## Negative regression cases

`--live-selftest-regress` builds three deliberately broken copies of the agent
and requires the live gate to reject each one:

| Case | Injected defect | Expected runtime failure |
|---|---|---|
| `tonumber` | pointer conversion through the removed method | `TypeError` from the real `NativePointer` |
| `backtracetype` | reference to the pre-Frida-17 backtrace global | `ReferenceError` from the real runtime |
| `arrayindex` | register read on the callback argument | `RangeError: invalid array index` from the real `InvocationArguments` |

A case passes only when the defect is detected, the identifying message is
present, a stack is captured, and the runtime-surface scenario still passes in
the same process. That last condition proves the harness itself was healthy when
it rejected the broken agent.

## The rule

Every probe in this project must ship with a `--live-selftest` mode that runs its
agent against synthetic memory in a real Frida runtime, and a probe is not
shippable until that mode passes. A mocked self-test may remain as a fast check
of pure classification logic, but it is never the acceptance gate for the agent.
Once a probe has negative regression cases, `--live-selftest-regress` must stay
green as well, so the gate is proven rather than assumed.

## Reviewer checklist

- [ ] The committed diff contains no offsets, addresses, disassembly, game
      bytes, or absolute install paths.
- [ ] The live gate passes against the current probe.
- [ ] The regression gate detects all three historical defects.
- [ ] The mocked self-test still passes and remains secondary.

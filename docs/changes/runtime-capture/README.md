# runtime-capture - what is committed and what stays local

This change is the **preparation** for a runtime capture of the SR1 spline
system. It commits the plan and this note. It does **not** commit the
instrumentation, because the instrumentation necessarily contains module offsets
and engine field offsets, which the repository rules forbid.

## The split

| Artifact | Location | Committed | Why |
|---|---|---|---|
| Capture plan | `docs/changes/runtime-capture/capture-plan.md` | yes | Roles, mechanism choice, session procedure, outcome table. No numbers |
| Live self-test note | `docs/changes/runtime-capture/live-selftest.md` | yes | Why a mocked self-test misses API-shape bugs, and the live-runtime gate every probe must pass |
| This note | `docs/changes/runtime-capture/README.md` | yes | Explains the boundary so the next reader does not go looking for a missing script |
| Probe script | scratch directory, outside the repo | **no** | Carries module-relative offsets, engine field offsets and host paths |
| Capture logs | wherever the operator saves them | **no** | Runtime data from a shipped binary; never repo material |

The scratch directory for this project is the session temp directory
(`%TEMP%\opencode`). The file is `sr1_spline_capture.py` there. It is a single
Python file with an embedded agent, run as
`python sr1_spline_capture.py --attach SRX.exe`.

## Why not commit the script

Two of the repository's hard rules point the same way:

1. No retail offsets or addresses in the repository.
2. No game bytes and no decompiled output in the repository.

The probe is only useful if it names exact module-relative entry points, and
those are exactly the numbers that must not be recorded here. A committed script
with the numbers removed would not run; a committed script with the numbers kept
would break the rule. Keeping it local and describing the roles in the plan is
the only consistent option.

## Re-pinning without the script

If the script is lost, the role table in the plan is enough to rebuild it: each
row names a stable identity (a diagnostic literal, a unique call relationship, or
a unique write set) rather than a number. Re-derive the offsets from that
identity and put them straight back into a local script.

## What a reviewer should check

- [ ] No module offsets, raw field offsets, addresses, disassembly or install
      paths appear in the committed diff.
- [ ] The plan names every role it relies on and marks the one role it could not
      pin.
- [ ] The outcome table has an explicit negative branch, not only a happy path.
- [ ] `cargo test` stays at its baseline; this change touches no production code
      and moves no golden.

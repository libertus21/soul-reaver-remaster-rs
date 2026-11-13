# angle-source-plan - who produces the constructor's angle triples? (2026-11-12)

This is a **preparation** slice for the body-animation investigation. The two
sibling orientation constructors are already understood down to a confirmed
rotation model. What is still open is the **source** of the three scalar angle
floats they read. This plan states why a bounded caller-chain capture answers
"who" more cheaply than a write-watch, defines the aggregation and its tagged
verdicts, and ships the instrumentation as a local scratch file. Nothing was
attached, launched or instrumented against the game for this document, and no
engine behaviour changed. The committed diff is this plan only.

## Quick path

1. Read "The question" - a caller is named, not proven to be a source.
2. Read "Why the caller chain, not a write-watch" - the cost argument.
3. Read "Aggregation design" and "The tagged verdicts" - the deliverable.
4. Read "How the result is used downstream" - the static follow-up.
5. Run the 4-step human procedure when a session is available.
6. Read "What this cannot establish" before writing any conclusion.

## What is already settled

These are **inputs** to this experiment, not things it re-derives:

```text
ABI: float* ctor(const float3* src /* rcx */, float* slot /* rdx */) -> slot

Two sibling constructors differ only in rotation order.
Each reads three consecutive 32-bit floats through src and writes the
slot's nine-float rotation block.

Pose slot: 13 f32, stride 52:
  [0..8] rotation row-major, [9] scale, [10..12] translation.

Confirmed rotation model: M = Rz(c*K) * Ry(b*K) * Rx(a*K), K = 2*pi/4096.
Verified on every sampled row (three-axis, two-axis and single-axis).
The src pointer is often a shared scratch buffer (the same address recurs
across slots) and sometimes a per-slot buffer. The values animate.

The body pose is COMPUTED procedurally, not copied from any file.
```

The constructors compute from three scalars; they do not read a matrix from
disk. The remaining unknown is where those three scalars come from, per slot,
per frame. Naming the caller narrows the search to one function.

## The question

> Who produces the three angle floats that the constructor reads, and where do
> those values live?

The probe answers the **who** by capturing the call stack of the constructor and
aggregating it into a small set of in-module call sites. It also reports the
class of the `src` pointer (module data versus an anonymous writable region) and
whether that pointer repeats a page across calls. Those two facts distinguish a
static table from a per-frame scratch buffer.

It deliberately does **not** claim to answer **where the values are computed**. A
caller is not a source; it is the next address to inspect.

## Why the caller chain, not a write-watch

A write-watch is the wrong first tool here.

| Aspect | Write-watch | Caller-chain capture |
|---|---|---|
| Fault volume | The source scratch is rewritten per slot per frame. A page guard faults on every store, or the watch must be re-armed constantly. | No faults. One backtrace per constructor call, for the first N calls only. |
| Re-arm safety | A second `MemoryAccessMonitor.enable()` was measured not to report on this build. The one-shot design is safe but only samples the first access per page. | No page protection changes. Plain interceptor reads the stack. |
| What it names | The first instruction that touches a page - which may be a reader, and may be one frame above or below the real producer. | The immediate in-module caller of the constructor, resolved to a function entry where possible. |
| Cost | Page-granular and one-shot; a hot scratch defeats it. | Bounded: N calls, capped frames, aggregated output, hard line cap. |

The backtrace is cheap and direct: one bounded sample at the exact moment the
constructor receives the triplet. If the callers collapse to a small stable set,
the site is named immediately. If they do not, the optional one-shot write-watch
is available as a fallback, never as the default.

## Method

| Aspect | Design |
|---|---|
| Hook sites | both sibling constructor **entries**, addressed module-relative |
| Capture point | `onEnter`, before the constructor consumes the triplet |
| Stack walk | `Thread.backtrace(context, BacktraceType.ACCURATE)` |
| Capture bound | the first N attempts **per constructor** (default 120); after that the hook returns without walking |
| Frame budget | at most a fixed number of kept frames per backtrace (default 16) |
| Frame resolution | frames outside the engine module are counted `foreign`, never dumped; frames inside are projected to module-relative offsets |
| Self frames | the currently-executing frame near the hooked entry is stripped |
| Entry snap | each frame is snapped to the containing function's entry with the debug-symbol resolver; when no symbol resolves, the raw frame offset is reported and the site is flagged `raw` |
| Site key | the immediate in-module caller (the first kept frame); the rest of the kept frames form the chain |
| Source class | per site, the `src` range is classified as module data, file-mapped, anonymous writable, anonymous other, or unmapped |
| Source locality | per site, `same_page_frac` is the fraction of captured calls whose `src` sits on the same page as the previous call for that site |
| Output | aggregated only; no per-call text |
| Safety | never writes game memory, never calls game code, never patches bytes |

### Why aggregate instead of dumping

Dumping stacks per call is both unreadable and a latency source. The probe keeps
counters only and emits **one line per site** at summary time. This is the
anti-lag rule: capture is bounded, the hot path emits nothing, and the text is
produced from an in-memory aggregate.

## Aggregation design

One site is created per distinct immediate in-module caller. For each site the
probe stores: total hits, a small map of chain variants, the source-class
histogram, the distinct-page count, the same-page count, and a `raw` flag.

```text
ANGLE CALLERSET captured=<n> calls=<n> foreign_frames=<n> self_skipped=<n>
               inmodule_sites=<n> raw_frames=<n>

ANGLE CALLER site=<i> hits=<h> depth=<k> chain=<a,b,c,...>
             src=<class>=<n>,... same_page_frac=<f> raw=<0|1>

ANGLE VERDICT CALLERS_STABLE|CALLERS_SPREAD|UNOBSERVED reason=<...>
```

- `site=<i>` is the rank after sorting by hits, so the dominant site is always
  first. The chain's first element is that site's module-relative entry offset;
  the rest is the caller's caller chain.
- `depth` is the number of kept in-module frames in the chain.
- `chain` elements are module-relative function entries where a symbol resolves,
  otherwise raw frame offsets (the `raw=1` flag and a `STAT-NOTE` say so).
- `src` is the source-class histogram: module data/file means a static table;
  anonymous writable means a scratch or heap buffer.
- `same_page_frac` near 1 means the pointer repeats a page across calls (a
  scratch buffer); near 0 means fresh buffers per call.
- Only the top `--top` sites are printed, sorted by hits.

## The tagged verdicts

The verdict is a pure function exercised directly by `--selftest`.

| Verdict | Rule | Meaning |
|---|---|---|
| `CALLERS_STABLE` | at most 3 distinct in-module sites cover at least 90% of captured calls | the source is a small, inspectable set of callers; go read them statically |
| `CALLERS_SPREAD` | many in-module sites, or fewer than 90% covered by the top 3 | no single producer; a shared transform site or several call paths are in play; escalate to the write-watch |
| `UNOBSERVED` | no capture at all, or capture with no resolvable in-module frame | a **missing observation**, never "no caller" |

`UNOBSERVED` is load-bearing. A run with zero captured stacks, or one whose
backtraces produce only foreign frames, proves nothing about the existence of a
caller. It is reported as missing evidence so no downstream conclusion can
mistake silence for absence.

## How the result is used downstream

The probe names a site; it does not prove a source. The follow-up is **static**:

1. Take the top `ANGLE CALLER` site(s) and disassemble that function in the
   engine module.
2. Follow the data flow of the three floats it passes to the constructor. They
   may be loaded from a table, computed by arithmetic, or forwarded from another
   function.
3. If the caller only forwards, repeat the chain one level up using the remaining
   `chain` elements. The probe already prints the caller's caller chain for
   exactly this reason.
4. Cross-check the `src` class: a module-data pointer points at a static table;
   an anonymous writable pointer with a high `same_page_frac` points at a
   per-frame scratch producer.

The static step proves where the triples are computed. The runtime step only
tells the analyst which function to open.

## The 4-step human procedure

1. **Validate without attaching.** From the session scratch directory run
   `python sr1_angle_source_probe.py --selftest` (aggregation and verdicts),
   `--dry-run` (prints the plan, attaches nothing), and
   `--print-agent` piped to `node --check` (agent syntax). Confirm the agent
   source uses pointer-safe conversions and contains no pointer `toNumber`.
2. **Launch and attach.** Start the game, then run
   `python sr1_angle_source_probe.py --attach SRX.exe --wait 300 --calls 120
   --top 10 --max-lines 400 --log <scratch>\angle_source.log
   --summary <scratch>\angle_source_summary.txt`. Let the character walk so the
   constructors run across several slots and frames.
3. **Read the aggregate.** Inspect the `ANGLE CALLERSET`, the ranked
   `ANGLE CALLER` lines, the source-class note, and the `ANGLE VERDICT`.
   `CALLERS_STABLE` gives an inspectable caller; `CALLERS_SPREAD` says the
   motion is produced in more than one place.
4. **Escalate only if needed.** If the caller chain is inconclusive, rerun with
   `--watch-src` to arm one one-shot watch on up to a few source pages and read
   the `WATCH WRITE` lines. Then perform the static analysis from step 1 of the
   downstream section.

## What this cannot establish

- **A caller is not a source.** The chain names who hands the triplet over, not
  where the numbers were computed.
- **The angles may be computed one level further up.** The immediate caller can
  be a thin wrapper. The printed chain is a hint to keep walking.
- **Inlined code hides frames.** A producer inlined into the caller may not
  appear as its own frame, so the chain can under-report.
- **Unresolved symbols yield raw offsets.** Without symbols, the chain elements
  are frame offsets, not function entries; the `raw` flag marks this.
- **The optional watch is one-shot.** A page whose first access is a read cannot
  be re-armed; an unobserved write is missing evidence, not absence.
- **The accurate unwinder can miss frames** across trampolines or hand-written
  assembly. A short chain is a lower bound on depth.
- **One session is one sample.** A `CALLERS_STABLE` result from one window should
  be reproduced before it is treated as the whole story.

## Repository rules and verification

- The plan names roles only. It contains no module offsets, raw field offsets,
  addresses, disassembly, game bytes or absolute install paths.
- The probe stays in the session scratch directory because it necessarily
  carries module-relative entry offsets that the repository forbids.
- This change touches no production code and moves no golden. `cargo test` must
  remain at its baseline of 618 passing.
- Reviewer checklist:
  - [ ] No addresses, RVAs, disassembly or install paths in the committed diff.
  - [ ] The verdict table has an explicit missing-observation branch.
  - [ ] The downstream section says plainly that a caller is not a source.
  - [ ] The 4-step procedure validates without attaching before any session.

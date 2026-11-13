# staging-split-plan - decide whether the 110 STAGING slots are packed or
# mode-applied (2026-11-12)

This change prepares the decisive runtime probe for the last open item in the
body-animation investigation. The census left the 110 non-aim slots in a named
superclass, STAGING, because file data cannot separate the packed-channel writer
from the mode applier. The probe hooks the per-object animation channel loop,
reads the staging kind word, and aggregates per object, so one short capture
returns a per-slot verdict. This document is the **plan only**. The
instrumentation stays in the session scratch directory because it must carry
module-relative entry offsets; no game was launched, attached or instrumented
for this slice.

## Quick path

1. Validate offline: `--selftest`, `--dry-run`, and the agent syntax check.
2. Attach once, walk the character through several animations, stop after a
   bounded window.
3. Read the per-object `CHANNEL` lines and the final `SPLIT`, then join by slot
   index with the census.

## The hyper-test

Two signals are read per slot, both from the runtime animation record that the
sampler consumes, plus one direct mode signal.

- **Signal A - producer of a channel.** A channel record covers a contiguous
  slot range. If it carries a keyed frame table whose frame array is non-null,
  the quaternion sampler runs for that range; otherwise the packed decoder
  runs.
- **Signal B - staging kind word.** The staging record carries one word that
  names the orientation form: zero means angle form, non-zero means quaternion
  form.
- **Signal C - direct mode evidence.** A track entry names one target slot and
  one mode byte. The mode applier runs after the channel pass, so a targeted
  slot is owned by the mode applier regardless of channel coverage.

The split is a pure function of those three observations. Both possible
outcomes close the question, and neither is favoured:

| Observation | Reading | Consequence |
|---|---|---|
| Every observed STAGING slot lies in a packed channel, every covered kind word is zero, and no track entry targets those slots. `SPLIT packed=<all> mode=0 unknown=0`. | **PACKED_CHANNEL wins.** | The 110 slots come from channel data. Offline decode needs the packed payload, palette, frame divisor and blend - not the aim/mode rules. |
| Track entries target some of those slots, and/or a kind word is non-zero on a slot no channel covers. `SPLIT packed=<x> mode=<y> unknown=0`. | **MODE_APPLIED wins, or the set is mixed.** | The 110 are not all packed. The per-slot map is the deliverable; the mode-applied subset is computed from aim/mode rather than decoded from channel data. |
| A packed-covered slot carries a non-zero kind word, or a keyed-covered slot carries zero, or no object is observed at all. `unknown>0` or `reason=unobserved`. | **INCONSISTENT or UNOBSERVED.** | The simple rule is falsified or the observation is missing. Treat this as evidence to escalate, never as a vote for packed. |

`UNOBSERVED` is load-bearing: zero captured objects, a failed hook or an
unreadable staging area must never be reported as "zero mode-applied slots".

### What the field semantics add

The static reading behind this plan refines how the second branch is read:

- the packed decoder writes the kind word to zero;
- the keyed sampler writes it to a non-zero marker;
- the mode applier writes the orientation and translation block but does not
  change the kind word.

So the kind word is a **producer fingerprint for the channel pass**, not a
per-frame proof that a slot was mode-applied. The direct mode evidence is the
track target (Signal C). A non-zero word on a slot that no channel covers can
also be **stale**, because the staging area is global scratch shared by every
object. The probe reports that case as `flag_uncovered` in the `SPLIT` reason
and never folds it into the packed count.

## What the probe reads

All offsets are byte offsets in the memory-form runtime records. They are roles,
not file offsets, and no module-relative number appears here.

| Record | Field | Role and value implication |
|---|---|---|
| node | channel count at +0 | Number of channel records sampled for this object. Zero means the node contributed no coverage. |
| node | track head at +2 | Index of the first track entry; zero means no per-slot mode overrides. |
| node | channel array base at +56 | Base of the contiguous channel array. |
| channel | first slot at +2, slot count at +3 | The contiguous staging/pose slot range this channel covers. This is the coverage map the file side cannot supply. |
| channel | keyed table at +72, frame array at +16 of that table | Producer switch. Non-null table with non-null frame array means quaternion sampler; otherwise packed decoder. |
| channel | format byte at +0 | Channel format flags, used only to describe the channel; not part of the split. |
| staging | kind word at +44 | Zero = angle form, non-zero = quaternion form. Read for the covered window after the channel pass. |
| staging | orientation at +0 through +12, scale at +16 through +24, translation at +32 through +40 | The record the slot builder consumes. Reported only in the fallback, as the first words of the record. |
| track entry | next at +0, mode at +2, target at +3, flags at +16 | A target equal to a slot is direct evidence the mode applier owns it. Flags mark disabled entries; the probe counts them. |
| object | no semantic name | The object is labelled by a short hash of its pointer when the channel loop is reached from the object update path. A leading `n` marks a node-only record that has no confirmed object base. |

The object label is deliberately not an actor name. The operator joins it to
the census by slot count, coverage range and the existing descriptor dump.

## Aggregation design

- **Per invocation, not per call text.** On channel-loop entry the probe reads
  the node's channel count and channel geometry, and walks the track chain. On
  return it reads the kind word for a bounded slot window. Nothing is printed
  from the hot path.
- **Per object.** Results are stored in one record per object: coverage ranges,
  keyed-channel count, kind histogram, non-zero kind slots, mode targets and a
  mode histogram. Repeated frames update the same record and increment an
  invocation counter; they do not create a new line.
- **Per object per dump interval.** At most one line is emitted per object per
  interval, and only when its snapshot changed:

  ```text
  CHANNEL obj=<id> slots=<n> channels=<n> keyed_tables=<n> covered=<a..b> flags=<histogram>
  ```

  `slots` is the number of distinct observed slots; `covered` is the channel
  coverage range; `flags` is the kind-word histogram over covered slots.
- **Final split.** Exactly one line, always reserved even when the text cap is
  reached:

  ```text
  SPLIT packed=<n> mode=<n> unknown=<n> reason=<...>
  ```

  The reason carries the object count, observed-slot count, quaternion count,
  mismatch count, `flag_uncovered` count, keyed-channel count and `unobserved`
  when no object was seen.
- **Per-object summation.** Slot indices are model-local, so two objects may use
  the same index for different joints. The split sums per object and never
  unions across objects.
- **Aux exclusion.** Channel-loop calls that are not on the object update path
  are recorded as node-only and excluded from the split unless explicitly
  included. This keeps attachment sub-nodes out of the body count.
- **Selftest.** The pure classifier is exercised with a packed case, a mode
  case, an overlap case where the mode applier wins, a mismatch case, an
  uncovered non-zero case, an unobserved case, per-object summation, aux
  exclusion, quaternion surfacing, the fallback row and summary, and the hard
  line cap.

### Cost bounds

| Bound | Default | Why |
|---|---:|---|
| Objects tracked | 12 | One line per object per interval; excess objects are counted and dropped. |
| Channel records per node | 32 | Bounds the geometry read on a malformed or oversized node. |
| Track-chain entries per node | 64 | Cycle-guarded; overflow is counted. |
| Slot window per object | 255 | One bounded scan of the staging records per invocation. |
| Dump interval | 1500 ms | At most one line per object per interval. |
| Capture duration | 20 s | Auto-stop; the operator can also stop with Ctrl+C. |
| Emitted lines | 200 | Hard cap; the final `SPLIT` is always reserved. |

The hot path allocates only the per-object record on first sight and sends
nothing per call. The probe never writes game memory, never calls game code and
never patches bytes.

## The 3-step human procedure

1. **Validate without attaching.** From the scratch directory run
   `python sr1_staging_split_probe.py --selftest`,
   `--dry-run` (prints the plan, imports no Frida and attaches nothing), and
   `--print-agent` into a UTF-8 file checked with `node --check`. Confirm the
   agent has the capability gate, no pointer `toNumber`, and no wrong Frida 17
   backtrace API.
2. **Launch and attach.** Start the game and let the character walk, turn and
   play at least two animations. Run
   `python sr1_staging_split_probe.py --attach SRX.exe --wait 300
   --duration 20 --interval 1500 --max-lines 200
   --log <scratch>\staging_split.log
   --summary <scratch>\staging_split_summary.txt`.
   Then stop with Ctrl+C or let the duration expire.
3. **Read the result.** Inspect the `CHANNEL` line for each object and the final
   `SPLIT`. Join the per-slot classification to the census by slot index. If the
   reason contains `unobserved`, the run proves nothing: rerun, and if the hook
   cannot be armed, use the fallback below.

## The fallback: `--dump-staging`

If the channel loop cannot be hooked reliably, the same probe can run in a
staging-only mode. It attaches, does not hook any engine routine, and samples
the global staging area at a fixed interval. For each bounded slot it reports:

```text
STAGING slot=<i> kind=<0|non-zero> w=<first four record words> changed=<0|1>
```

and closes with `STAGING SUMMARY slots=<n> kind0=<n> kind_nonzero=<n>
changed=<n>`. The `changed` flag marks slots whose record moved between
samples, which separates live slots from untouched scratch.

This is inference from data alone, and it is weaker than the channel hook: the
staging area is global, so an uncovered slot can hold another object's values or
a previous frame's values. The summary says so instead of pretending otherwise.

## What it cannot establish

- **Actor identity.** The object label is a hash, not a name. The join to
  raziel/skinner/sluagh/morlock is by slot count, coverage range and the
  descriptor dump.
- **Slots that are never covered.** A slot with no channel, no track target and
  no non-zero word is a missing observation, not a packed slot.
- **Whether the final pose consumes the staging record.** One of the per-object
  builder paths computes the pose directly and does not read the body staging
  record. The split classifies the producer of the record, not the last writer
  of every pose.
- **Whether the packed payload is authored or a default.** The probe shows that
  the packed decoder ran; it does not decode the payload or prove it carries
  authored motion.
- **Per-frame stability.** A channel's keyed table can change with the active
  animation. The probe keeps the last observed snapshot and counts invocations;
  it does not prove the producer is constant over the whole session.
- **The exact census set.** The probe reports the slots it observed. The
  operator intersects that map with the 110 STAGING slots from the census; the
  probe does not read the file-side aim offsets.
- **The cause of a mismatch.** A packed-covered slot with a non-zero word, or a
  keyed-covered slot with a zero word, is reported as unknown. Naming the third
  writer is follow-up work, not a result of this probe.
- **A verdict from silence.** A hook failure, a missing object or an unreadable
  area yields `unobserved`, never "zero mode-applied".

## Falsification ledger

| Claim | Observation that would refute it |
|---|---|
| The kind word follows the channel producer | a packed-covered slot with a non-zero word, or a keyed-covered slot with zero |
| A track target means the mode applier owns the slot | a targeted slot whose record is provably written only by the channel pass |
| The 110 are one producer | a non-trivial `mode` count in the split, or a non-trivial `flag_uncovered` count |
| The per-object aggregate is stable | the same object emitting different coverage across intervals without an animation change |

## Repository rules and verification

- This plan contains no addresses, module-relative numbers, disassembly, game
  bytes or absolute install paths. Field offsets are stated in decimal as
  roles; they are not file offsets.
- The probe stays in the session scratch directory. The committed diff is this
  plan and nothing else.
- This change touches no production code and moves no golden. `cargo test` must
  remain at its baseline of 618 passing.
- Reviewer checklist:
  - [ ] No addresses, module-relative numbers, disassembly or install paths in
        the committed diff.
  - [ ] The hyper-test lists both outcomes and neither is favoured.
  - [ ] The aggregation is per object and the fallback is documented.
  - [ ] `UNOBSERVED` is an explicit branch, not a silent zero.
  - [ ] The three-step procedure validates without attaching before any session.

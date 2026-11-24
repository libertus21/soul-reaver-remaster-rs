# campath-consumer - method for the runtime reader of a campath `+0x38` spline (2026-11-24)

Read-only follow-up to `campath-format` (the position-spline note; delivered by
its own change) and to `spline-animation`. This note answers one question: which
code reads a placed `campath` record's `+0x38` spline block at runtime, and what
does it drive. It adds a scratch measurement probe and an offline cross-check
against the shipped archive. It does not change production code, does not add a
CLI mode, and does not move a digest.

**Status: steady-state live run done; reader identity still open.** The probe is
built and both of its self-tests pass. The archive scan below is real. A
steady-state run in `oracle15` found 76 resident blocks and 0 reads in 60 s, so
the runtime does not sample these blocks per frame; the load-time reader's
identity was not captured. The offline placement test in `campath-route-test`
refutes the NPC-route reading of the spline. See the UNKNOWN list for what
remains.

## Verdict first

| # | Question | Answer | Class |
|---|---|---|---|
| Q1 | Is the `+0x38` block a spline-set descriptor (channel slot table + one position-channel descriptor + source keys)? | Yes: 0x30 bytes of 4-byte channel slots, only slot 0 populated, followed by the 8-byte channel descriptor, then `numkeys * 0x20` source keys | CONFIRMED (offline, structural) |
| Q2 | Can the descriptor at block `+0x30` hold a 64-bit pointer? | No. Bytes `+0x34..+0x37` are the u16 count and the stride/behaviour bytes, so the pointer field is 4 bytes wide by construction. Its value must be a container-relative offset. | CONFIRMED (field packing) |
| Q3 | Does the scanner locate and join the shipped blocks offline? | Yes: 2,106 key-valid blocks, 1,976 campath records, 1,943 joins over 231 units | CONFIRMED (measured) |
| Q4 | Which code reads `+0x38` in the running game? | Steady-state `oracle15`: 76 blocks resident, 0 reads in 60 s (no per-frame sampler). Reader identity not captured | **UNKNOWN** |
| Q5 | What does the reader drive? | Prediction: a load-time consumer of this descriptor (the spline-set builder is the confirmed consumer of this shape), feeding the object-transform sampler. Refutable by the probe. | **PREDICTION** |

Q1-Q3 are the new evidence. They narrow Q4 from "some consumer" to "a reader of
a standard spline-set descriptor", which is the strongest structural statement the
files can make. Q4 still needs the running game.

## 1. What this slice adds

`campath-format` decoded the `+0x38` block and left the consumer open.
`spline-animation` independently established that a load-time builder reads a
file-side **channel descriptor** of this exact shape and expands it into runtime
keys played by an object-transform sampler:

| File-side descriptor (`spline-animation` 1.2) | Campath block (`campath-format` 4) |
|---|---|
| `+0x00` offset to source key array | block `+0x30` offset to keys at block `+0x38` |
| `+0x04` u16 `numkeys` | block `+0x34` u16 `numkeys` |
| `+0x06` u8 stride flag (`1` = rotation, `0` = position) | block `+0x36` u8 stride flag (`0`) |
| `+0x07` u8 behaviour flags | block `+0x37` u8 behaviour (`1`) |

The field order, widths and roles are identical. The campath block is not a
campath-specific structure: it is the shared descriptor, preceded by a slot
table.

## 2. The block is a one-channel spline set (Q1)

`campath-format` measured the prefix: block `+0x00` points at block `+0x30`, and
bytes `+0x04..+0x2F` are zero on every non-empty campath block. Read as a slot table,
that is twelve 4-byte channel slots with only slot 0 populated. Slot 0 is the
position channel: stride flag `0`, behaviour `1`, source keys of `0x20` bytes
(`i16 delta`, 3 x `i16` position, 3 x `i32` out-tangent, 3 x `i32` in-tangent).

So one `+0x38` block describes **one object's position over time**, in the same
encoding the animation system already loads. Nothing in the block names an
actor, a joint, a platform or a camera.

## 3. The pointer slot is 4 bytes, so the block stays file-form (Q2)

This is a hard structural consequence, not an inference. In an 8-byte slot at
block `+0x30`, bytes `+0x34..+0x37` would be the high half of the pointer. Those
same bytes are the `numkeys`/stride/behaviour fields on every shipped block. A
64-bit pointer there would overwrite the count and destroy the block. The only
possible encoding is the 4-byte value the files already carry.

Practical consequence for a live probe: the block cannot be located by chasing
an absolute pointer. It is found by scanning for the header shape, validating
every key, and reconstructing the container base as
`block + 0x38 - *(block + 0x30)`. The same rule works in a mapped archive and in
a heap copy of a unit, without the archive index.

## 4. The probe

Scratch only, not committed: it carries module names, field offsets and runtime
output. One probe file with four self-checked entry points:

| Mode | What it does | Game needed |
|---|---|---|
| `--selftest` | 39 offline checks: header parse, extent, key bounds, pointer packing, record parse, join, zone match, chunk dedupe, output formats | no |
| `--live-selftest` | spawns a throwaway `PING.EXE`, plants a synthetic region, runs the same discovery, arms the monitor, spawns a native reader thread and verifies hit delivery, re-arm and stack attach | no (never the game) |
| `--scan-file` | scans a `BIGFILE.DAT` read-only with the same scanner and prints the same discovery line | no |
| `--attach` | attaches to `SRX.exe`, runs the discovery gate, arms the monitor on the chosen blocks, reports readers and stacks | yes |

Discovery gate, printed before anything is armed:

```text
CAMTRACE discover records=<n> valid=<n> zone=<guess> blocks=<n> forms=<...>
          moving=<n> joined=<n> contiguous=<n> zone_source=<...> ...
```

- `records` - campath records found by NUL-padded name (case-insensitive) plus
  `flags == 0x10000`.
- `valid` - records whose `+0x38` resolves to a block whose header and **every**
  key validate.
- `blocks` - all key-valid blocks, campath and non-campath.
- `zone` - named only when a live record matches one shipped zone on authored
  id, placed position, key count **and** a position signature over every key;
  otherwise `multi` (many units present), `unknown`, or the operator's
  `--zone` label.
- `pos_delta_max` - the largest first-key-to-placed-position offset among joined
  records; this is the join's tolerance being measured, not assumed.

Tracing uses guard pages (`MemoryAccessMonitor`), not a guessed function. The
faulting instruction address is reported as `reader=<module>+<offset>`; the page
holding the keys as `kmod=<module>+<offset>`. A stack is attached to the first
readers and reported as `CAMTRACE stack`.

Two modes test the two live hypotheses:

- **steady** (default): stand still in the zone. A hit here means a per-frame
  consumer, and the stack names it.
- **reload** (`--reload`): trigger a level load or zone transition while armed.
  A hit here is the load-time builder, and the stack names the spawn path.

## 5. Offline validation of the scanner (Q3)

The archive was scanned read-only with the same `scan_buffer` the live gate uses,
over all 85,923,840 bytes. No game byte is reproduced here.

| Measure | Value |
|---|---|
| Bytes scanned | 85,923,840 (one pass, 4 MiB chunks with 512 KiB overlap) |
| Key-valid blocks | 2,106 - the exact sum of the `campath-format` census (1,948 campath blocks + 158 non-campath blocks) |
| Encodings seen | `offset4` 2,106 / 2,106 (no pointer form, as Q2 requires) |
| Campath records | 1,976 - exactly the `campath-format` record count |
| Joined records | 1,943, resolving to 1,903 distinct blocks |
| Unjoined blocks | 203 (2,106 blocks minus the 1,903 that a record names) |
| Units with a joined block | 231 |
| Blocks followed exactly by another block header | 1,833 |
| Largest joined first-key offset | 200 units |
| Elapsed | about 1 s |

The block and record counts reproduce the established corpus exactly, which is
the strongest available offline proof that the scanner is not inventing blocks.

Thirty-three of the 1,976 records do not join, and the reasons are counted, not
guessed: 28 have no usable key block at all (25 empty blocks and 3 zero `+0x38`,
per `campath-format`), and 5 have a real block whose first key sits farther from
the placed position than the proximity rule allows (`aluka27` two keys at 4,360
units, `chrono19` two keys at 18,398, `train5` one key at 320). Forty-five of the
1,948 campath blocks are unjoined: those five, plus 40 copies of placements that
exist byte-identically in several units and are named once.

Two corpus facts the probe had to absorb:

- One record's name is `Campath`, capital C. Name matching is case-insensitive;
  a case-sensitive scan loses one record and one block.
- Twenty-four block offsets are shared by two or more units holding
  byte-identical campath placements (40 extra joins), for example the same
  eight placements in `city6`, `clfpil3`, `huba8` and `stone15`. A live record
  can therefore name more than one unit; the probe reports `multi` unless
  `--zone` pins it.

## 6. The prediction, stated so it can fail

`spline-animation` 1.2 is a CONFIRMED result: a load-time builder reads exactly
this descriptor shape and expands the `0x20` source keys into `0x30` runtime
keys. Combined with Q1/Q2, the shortest account that fits every measurement is:

1. At spawn/load, an object builder walks a placed record, sees the spline-set
   descriptor at `+0x38`, allocates the runtime key array, and expands the keys.
   The confirmed builder of this descriptor shape is reached from an object
   animation record; a placed record is a different access path, so the reader
   may be a sibling function rather than the same one.
2. The per-object sampler then plays the runtime `0x30` keys and writes the
   object's own transform (position, rotation, scale).
3. Therefore `+0x38` itself is read **at load**, not per frame, and the campath
   spline animates an object transform. It is not an NPC locomotion planner.

What that leaves open is the interesting part: `object-behaviour` established
that a `campath` record never becomes an actor, so the object whose transform is
animated is an invisible marker. Whether any other system (the always-spawned
`paths` system object, a camera anchor, a patrol consumer) reads that animated
transform is a **second-order** question this probe does not answer.

## 7. Pre-registered reading of the live output

| Live observation | Reading |
|---|---|
| One burst at level load, many blocks, reader inside the object/spawn path | Load-time builder confirmed. Consumer is the object-transform animator; campath is not NPC locomotion. |
| Steady-state hits at frame rate | Load-time-only is refuted. Classify by the first stack frame: NPC velocity/transform write, platform update, camera matrix, or other. |
| Hits only on non-campath blocks | Campath splines are unread at runtime; report it as a negative and revisit the join. |
| No hits in steady state **and** none on reload | The runtime likely rewrites or frees the file-form block. Report the `discover` line and stop; do not infer a consumer. |
| Reader outside `sr1.dll` (copy function) | Recapture with `--stack-scope any` and classify by the first `sr1.dll` frame. |

## 8. How to run it (the live step)

The probe and the zone index live in the session scratch directory. The zone
index is built offline from the archive; without it the probe still works but
reports the operator's `--zone` label as unverified.

```text
python <scratch>/sr1_campath_trace_probe.py --live-selftest
python <scratch>/sr1_campath_trace_probe.py --scan-file <BIGFILE.DAT> \
    --zones-json <scratch>/campath_zones.json
```

Both must pass before a game run. Then, with SR1 loaded and the player standing
in a zone that has a long campath route (`oracle15` has two 81-key routes and
live enemies; `city2` and `oracle9` also carry long routes):

```text
python <scratch>/sr1_campath_trace_probe.py --attach SRX.exe \
    --zones-json <scratch>/campath_zones.json --zone oracle15 \
    --seconds 45 --scan-mb 768
```

Stand still for the whole window. If the result is `CAMTRACE idle`, run the
reload variant and trigger a level load or zone transition while it is armed:

```text
python <scratch>/sr1_campath_trace_probe.py --attach SRX.exe \
    --zones-json <scratch>/campath_zones.json --zone oracle15 \
    --reload --seconds 60
```

Send back every `CAMTRACE` line: `discover`, `gate`, `armed`, `hit`, `stack`,
`summary`, and any `error`/`idle`. If a reader inside `sr1.dll` is reported
without a stack, the probe prints a rerun command with `--readers`; repeat the
same in-game action with it. The probe sets `PAGE_GUARD` on the monitored pages
and clears it on each hit; it never writes game data.

## 9. UNKNOWN (explicit)

1. **Which code reads `+0x38` in the running game.** Method defined; the live
   steady-state run in `oracle15` (76 blocks resident, monitor on 7, **0 reads in
   60 s**) shows the runtime does not sample these blocks per frame, but no stack
   was captured. Still awaiting a reader identity.
2. **Whether the read is load-time only.** The steady-state 0-read result points
   to a load-time reader, but it is not observed directly.
3. **Which system consumes the animated object transform** (camera, the `paths`
   system object, NPC patrol, or nothing). The offline placement test in
   `campath-route-test` now **REFUTES the NPC-route reading**: moving splines do
   not start on placed enemies (only 10/520 within 500 units, none within 250,
   median 3,193 units), static splines behave the same (2.41% within 500), and
   the campath's own placed position gives the same distances. The remaining
   candidates are camera, platform, door or another non-NPC consumer, or an NPC
   patrol authored independently of enemy spawn points. Still UNKNOWN which.
4. **Whether the file-form blocks stay resident at runtime.** The live
   `oracle15` steady-state run found 76 blocks resident with 0 reads, so the
   file-form blocks were resident but not sampled per frame.
5. **The five far-first-key records (and their blocks).** They are real data; whether their first
   key is deliberately offset or whether the placed position is a separate
   anchor is not known.
6. **Coverage.** One shipped build. Other builds or regional variants are
   unobserved.

Evidence that would settle 1/2: the live `hit` and `stack` lines, read through
the table in section 7. Evidence for 3: a second trace that starts at the marker
object's animated transform and follows it to the next reader.

## 10. Reproduction of the offline numbers

- Open the archive read-only and run the probe's `--scan-file` mode with the
  zone index; expect the discovery line in section 5 to start with
  `records=1976 valid=1943 ... blocks=2106 forms=offset4:2106`.
- `--selftest` and `--live-selftest` must both print `PASS` before any live run.
- The probe, the zone index and the scratch parser that built the index are not
  committed; only this note is.

## Legal

Read-only format note. No game bytes, no decompiled text, no listings, no
addresses, no RVAs and no absolute install paths are recorded. Only field
offsets, counts, roles and corpus measurements appear. The archive and the
running module were never written.

## Related notes

- `campath-format` - the `+0x38` position-spline decode this note extends
  (delivered by its own change; this note stands alone so either can land first).
- `spline-animation` - the load-time builder and `0x20` source / `0x30` runtime
  key shapes that Q1/Q2 match field for field.
- `object-behaviour` - `campath` never becomes an actor; why the animated
  object's identity is still open.
- `object-rotation-field` - the no-rotation finding that `campath-format`
  resolves.
- `level-geometry` - the `0x4C` placed record and the object array.



# realm-diff - a differential memory probe for the live realm state

**Status:** research note and a scratch probe. No engine change, no realm logic,
no decoded offset, **no realm flag claimed**. A controlled run has now been
performed and its result is recorded below: the realm change is clearly larger
than same-realm churn, but every repeated-structure shape the detector found is
a per-frame counter (a timer), not a realm bit. A **revert** test was added to
separate a reversible state byte from a one-way timer; the three existing
snapshots cannot exercise it honestly and a real four-capture run is still
required. The realm state address, stride, field and meaning remain **UNKNOWN**
and are not claimed here.

The placed-record census (`docs/notes/placed-flags.md`) closed one door: no bit
of the authored record encodes realm. 165 of the 166 realm-shift `portal`
records are `flags=0`, and `warpg` is `0`. The running game still has to know
which realm it is in, so the state exists somewhere in memory at runtime even
though the file does not name it.

## Why this shape

The rotation model was not decoded by reading more file data and guessing. An
atomic runtime capture paired the inputs of one call with its output, and a
closed search over candidate models returned exactly one. Two states, aligned,
diffed, no guessing.

This slice applies the same method to the realm switch:

1. capture the process in the material realm,
2. shift realm in game without moving,
3. capture again in the same process,
4. align the two snapshots by address and diff.

Whatever the engine writes to flip realm is in the difference. The probe does
not assume what it looks like; it reports the changed bytes and, separately, a
detector for a field that flips consistently inside a repeated structure.

## Why the first run was inconclusive

The first live run took the two captures roughly 25 s apart with the game still
running. Between the captures the process keeps mutating on its own: timers
advance, counters tick, allocator and render state drift. The raw A/B diff was
**noise-dominated**: 475 of 9045 shared chunks changed and the differing bytes
totalled about **11.7 million**. That is far too much to be a realm flag, and
the reported shapes were consistent with time, not realm:

- a repeated table of 32-bit values whose byte at a fixed offset simply
  decremented by one between the captures;
- a long list of `(offset, old, new)` tuples, many of them a single bit moving
  from clear to set, each repeated many times.

A detector that is working correctly still cannot separate a realm flag from a
frame counter without a **control**. The problem was the experiment, not the
detector: two live captures seconds apart with the game live are not a
controlled comparison, and without a same-realm baseline the diff cannot
conclude anything about realm. No finding is drawn from that run beyond "noise
dominated".

## The control

A controlled comparison needs a third capture. `--diff A B --control C` takes
`A` and `C` in the **same realm, a few seconds apart**, and `B` in the other
realm. Then, for every byte that differs between `A` and `B`:

- if the byte **also differs between `A` and `C`**, it is churn that moves with
  time and not with realm; it is **noise** and is dropped;
- if `A` and `C` **agree** on the byte, the change is stable across a same-realm
  standstill pair; it is **signal** and is kept.

The strided pattern detector and the tuple ranking then run over the **signal
bytes only**, so the output is bounded by realm change and not by runtime
churn. `noise_bytes + signal_bytes` equals the raw A/B differing bytes, and
`reduction` is the fraction of the raw diff that the control removed.

A chunk that carries **both** signal and noise cannot be trusted, so it is
reported `kind=noisy` with both counts instead of being silently dropped. The
operator can then see exactly which regions are unusable.

## The controlled run (measured)

One controlled run was performed with `--diff A B --control C`, taking `A` and
`C` in the material realm a few seconds apart and `B` in spectral, all in the
same process. The control line it produced was:

```text
REALMDIFF control label=<c> noise_bytes=1298977 signal_bytes=10302523 chunks_noise=279 chunks_signal=236 reduction=11.2%
REALMDIFF chunks changed=515 of 9150 only_a=3 only_b=3 diffs=11601500
```

Read honestly, the experiment worked:

- the realm change is about **eight times** the same-realm churn: roughly
  10.3 MB of the raw A/B difference survived the control, against 1.3 MB of
  noise removed;
- the memory map stabilised between captures: the chunks present in only one
  snapshot fell from hundreds in the first run to **`only_a=3 only_b=3`**, so
  the two captures align;
- among the 515 changed chunks, 236 were clean `kind=signal` and 279 were
  contaminated `kind=noisy`.

The detector then reported **eight** `0x4C`-strided patterns. They are **not**
a realm flag. Their records are 32-bit values around `0x00fff6xx` that advance
between captures, inside the live object table: the differing field in each
record is a **per-frame counter**, i.e. a timer that moves with time rather than
with realm. A same-realm control does not remove a timer whose byte happens to
agree at both material captures, so the detector still sees it as a candidate.
That is exactly the ambiguity the revert test is meant to cut.

Exactly **one** chunk was reported `kind=signal` (no noise bytes at all, a small
cluster of differing bytes). It is a **candidate, not a conclusion**; nothing
here claims it is the realm flag. Its address is withheld with the other runtime
addresses; see "What a result can and cannot show".

## The revert

The control cannot tell a reversible realm flag from a monotonic timer: both
survive it. The decisive discriminator is **reversibility**. A reversible state
byte flips when the realm changes and flips **back** when the realm returns; a
timer never returns. That asymmetry is what `--revert` tests.

`--diff A B --revert D` takes a fourth capture `D` taken **after returning to
A's realm**. For every byte that differs between `A` and `B`:

- if the byte **returns to its A value in D**, it is **reversible** and is kept;
- if it **does not return**, it is **`one-way`** - a timer or one-shot - and is
  counted and summarised, but **not detailed**;
- `--revert` **implies the control subtraction** when `--control C` is also
  given: bytes that differ between `A` and `C` are removed first, then the
  revert test runs on the surviving signal bytes only.

The summary line is:

```text
REALMDIFF revert label=<d> reversible=<n> one_way=<n> of=<n> reduction=<x>%
```

- `of` - how many bytes entered the revert test: the control signal when
  `--control` is given, otherwise the raw A/B diff;
- `reversible + one_way == of`;
- `reduction` - `one_way / of` as a percentage, i.e. the fraction the
  revert test removed.

The strided pattern detector and the tuple ranking then run over the
**reversible set only**. For each reversible pattern the probe additionally
prints how the value restores, for example:

```text
REALMDIFF restore off=0x<o> a=0x<v> b=0x<v> d=0x<v>
```

where `d` is the value in the revert capture at the same field of the same
record the pattern line reports. For a truly reversible field `d == a`, and the
`b -> a` restore is the observation; a timer would print `d != a` (or would
already have been classified `one-way` and never appear as a pattern). One-way
bytes and control noise are masked out of the buffers the detector and the tuple
ranking see, exactly as the control masks noise.

Without `--revert`, the diff behaves exactly as before: same lines, same
detector, same tuple ranking, no revert line. `--revert` is opt-in.

## The probe (scratch)

`sr1_realm_diff_probe.py` lives in scratch, not in this repository. It carries
module names and field offsets and prints runtime addresses, all of which the
repository forbids; only this note is committed. It follows the conventions of
`sr1_campath_trace_probe.py` and `sr1_warp_trace_probe.py`: attach to the game
process, module `sr1.dll`, a discovery gate before acting, bounded
tag-prefixed output, `--selftest` and `--live-selftest`.

Three phases:

### `--capture <label> [--scan-mb N]`

Walks the process's committed, readable, non-executable regions and writes one
snapshot file:

- every region is recorded as a base+size pair, so two captures of the same
  process align by virtual address;
- memory is read in 1 MiB blocks and divided into 64 KiB chunks;
- an all-zero chunk is skipped entirely (process memory is sparse);
- every other chunk is zlib-compressed and stored with its virtual address;
- the manifest lists the regions and every kept chunk (address, raw length,
  compressed length, file offset).

The file is `MAGIC + payload chunks + manifest + footer`. The payload is
streamed while the snapshot is taken, so a multi-GiB capture never sits on the
heap.

### `--diff <labelA> <labelB> [--control <labelC>] [--revert <labelD>]`

Aligns by virtual address and reports, bounded:

- `REALMDIFF chunks changed=<n> of <n>` over the chunks present in both
  captures, plus the counts present in only one snapshot;
- per changed chunk, the differing byte offsets and their old/new values
  (`--min-changed-bytes`, `--max-hits-per-chunk`, `--max-chunks`);
- the repeated-structure detector (`--stride`, default `0x4C`; `--min-hits`,
  default `8`; `--max-pairs`, default `2`): changed bytes inside a stride-sized
  record grid share one residue modulo `stride`; a run of at least `min_hits`
  consecutive records whose field byte differs between A and B with at most
  `max_pairs` distinct value pairs is a pattern. It prints
  `REALMDIFF pattern base=<hex> stride=<hex> off=<hex> a=<v> b=<v> hits=<n>`
  and a short hexdump of a couple of records from each side;
- the most frequent `(offset, old, new)` tuples (`--top-tuples`, default 20).
  The offset is measured modulo `--tuple-mod`; `0` (the default) follows
  `--stride`, so a repeated field is visible even when the run is interrupted.

With `--control <labelC>`, where `C` is the **same realm as `A`** a few seconds
later, the per-chunk detail line gains `kind=signal|noisy` and the controlled
summary line is printed:

```text
REALMDIFF control label=<c> noise_bytes=<n> signal_bytes=<n> chunks_noise=<n> chunks_signal=<n> reduction=<x>%
```

Read it as:

- `noise_bytes` - among the raw A/B differing bytes, how many also differ
  between A and C (temporal churn, dropped);
- `signal_bytes` - how many differ between A and B but are identical in A and C
  (the realm candidate set that survives);
- `chunks_noise` - chunks contaminated by churn (reported `kind=noisy`;
  unusable even if they also carry signal);
- `chunks_signal` - chunks whose A/B change survived the control cleanly
  (`kind=signal`); `chunks_noise` and `chunks_signal` partition the changed
  chunks;
- `reduction` - `noise_bytes / (noise_bytes + signal_bytes)` as a percentage:
  how much of the raw diff the control removed.

Without `--control`, the diff behaves exactly as before: same lines, same
detector, same tuple ranking, no control summary. The control is opt-in.

With `--revert <labelD>`, where `D` is a capture taken **after returning to
`A`'s realm** (optionally together with `--control`), the per-chunk line gains
`reversible=<n> one_way=<n>` and reports only the reversible offsets; the
revert summary line described above is printed, and the detector, the tuple
ranking and the `restore` lines run over the reversible set only. One-way bytes
are counted, not detailed.

### `--report`

Prints a readable summary of the last diff, persisted beside the snapshots.

### Self-tests

`--selftest` is offline and needs no game. It builds a synthetic buffer pair
containing a planted `0x4C`-strided array of 64 records whose byte at `+0x2C`
flips `0 -> 1`, random noise that also differs between the two buffers, and an
all-zero 64 KiB chunk. It verifies the capture skips the zero chunk, the diff
finds the one changed chunk, and the detector returns **exactly one** pattern at
the planted base with `stride 0x4c`, `off 0x2c`, `a=0`, `b=1`, `hits=64`. It
also checks that raising `--min-hits` past the planted length silences the
detector, that an identical diff is empty, and that range filtering drops
executable regions.

The same `--selftest` now has a **controlled** case. It builds three synthetic
buffers over one region plus an all-zero chunk:

- a **realm** field at `+0x2C` that flips `0 -> 1` from A to B but stays `0` in
  C (A and C agree; 64 bytes);
- a **noise** field at `+0x30` that flips `0 -> 1` from A to B *and* differs
  between A and C (A and C disagree; 64 bytes);
- a constant non-zero marker so A's data chunk is not itself all-zero and is
  actually captured.

It asserts the control removes exactly the noise and keeps exactly the realm
flip: `noise_bytes=64`, `signal_bytes=64`, `diffs=128`, `reduction=50.0%`,
`chunks_noise=1`, `chunks_signal=0`, exactly one pattern at `off=0x2C`,
`a=0`, `b=1`, `hits=64`, no pattern at `off=0x30`, and a `+0x2C` tuple present
with no `+0x30` tuple.

The `--selftest` also has a **revert** case: a synthetic diamond `A -> B` with a
same-realm control `C` and a return capture `D`, over one region plus an
all-zero chunk. Three `0x4C`-strided fields, 64 records each:

- `+0x2C` flips `0 -> 1` from A to B and **returns to `0` in D** (reversible);
- `+0x30` flips `0 -> 1` from A to B and also differs between A and C (noise,
  removed by the control);
- `+0x34` advances `0 -> 1 -> 2` from A to B to D and never returns (one-way).

It asserts the exact counts: `reversible=64`, `one_way=64`, `of=128`
(`reversible + one_way == of`), `reduction=50.0%`, a single pattern at `off=0x2C`
with `a=0 b=1 hits=64`, **no** pattern at `off=0x34`, one tuple at `+0x2C` with
no `+0x30`/`+0x34` tuple, and a `restore off=0x2c a=0x00 b=0x01 d=0x00` line.
Without a control the same diamond tests over the raw diff (`of=192`,
`reversible=64`, `one_way=128`). Extra checks pin the byte lookup at an
unaligned region base. The offline suite is **51 checks**.

`--live-selftest` attaches to a throwaway process (never the game), allocates a
tiny region, captures it twice with no change, pokes its own buffer, and shows
a small diff with the planted stride.

## Operator protocol

Read this exactly. The control must be the **same realm as the first capture**,
and the revert capture must be the **same realm as the first capture after
returning to it**. The four captures are what make the run meaningful.

Preconditions: the game is running; do **not** restart it. All **four** captures
must come from the **same process and session**, or the addresses will not align
and both the control and the revert are invalid. A large scan takes a while;
4096 MiB is not instant.

1. Stand still in a zone, in the **material** realm.
   `python sr1_realm_diff_probe.py --capture material_a --scan-mb 4096`
2. Wait about 20 s doing nothing (no movement, no realm shift).
   `python sr1_realm_diff_probe.py --capture material_b --scan-mb 4096`
   This is the **control**: same realm as step 1.
3. Shift to the **spectral** realm in game, then stand still.
   `python sr1_realm_diff_probe.py --capture spectral_a --scan-mb 4096`
4. Shift **back** to the **material** realm, then stand still.
   `python sr1_realm_diff_probe.py --capture material_c --scan-mb 4096`
   This is the **revert** capture: same realm as step 1.
5. `python sr1_realm_diff_probe.py --diff material_a spectral_a --control material_b --revert material_c --stride 0x4C`

Do not restart the game between any of these steps, do not take the control in a
different realm from `material_a`, and do not take the revert capture before the
return to `material_a`'s realm. A control in spectral, a revert capture still in
spectral, or a restart between steps silently invalidates the comparison.

Notes for the run:

- The probe attaches to `SRX.exe` and requires module `sr1.dll`; it prints a
  gate line before it scans. If the module is not loaded, it stops.
- If the free-space check refuses, either free space, pass `--out-dir` on a
  drive with room, or lower `--scan-mb`. The check is deliberate; see below.
- `--report` summarises the last diff without re-reading the snapshots.

## Expected output shape

Shapes only; addresses are placeholders and no real address is recorded here.

```text
REALMDIFF gate ok module=sr1.dll pid=<n> ranges=<n> readable_mb=<n> scan_mb=4096
REALMDIFF capture-plan label=material_a scan_mb=4096 sample_mb=128 expected_mb=<n> free_mb=<n> zero=<n>% ratio=<n>
REALMDIFF capture label=material_a chunks=<n> kept=<n> kept_mb=<n> bytes=<n> raw_mb=<n> ratio=<n> file_mb=<n> dropped=<n> regions=<n> elapsed=<s>
REALMDIFF chunks changed=<n> of <n> only_a=<n> only_b=<n> diffs=<n>
REALMDIFF chunk addr=<hex> kind=signal|noisy|mixed|one-way changed=<n> reversible=<n> one_way=<n> [noise=<n>] +0x<hex>:0x<v>>0x<v> ...
REALMDIFF control label=<c> noise_bytes=<n> signal_bytes=<n> chunks_noise=<n> chunks_signal=<n> reduction=<x>%
REALMDIFF revert label=<d> reversible=<n> one_way=<n> of=<n> reduction=<x>%
REALMDIFF pattern base=<hex> stride=0x4c off=<hex> a=<v> b=<v> hits=<n>
REALMDIFF record base=<hex> a=<hexbytes> b=<hexbytes>
REALMDIFF restore off=<hex> a=0x<v> b=0x<v> d=0x<v>
REALMDIFF tuple off=+0x<hex> old=0x<v> new=0x<v> count=<n>
REALMDIFF diff-summary labels=material_a->spectral_a changed=<n> of <n> only_a=<n> only_b=<n> diffs=<n> patterns=<n>
```

What to paste back: the four `capture` lines, the `control` line, the `revert`
line, the `chunks` line, every `pattern`, `record` and `restore` line, and the
top `tuple` lines. The `control` line says whether the subtraction was usable;
the `revert` line says how much of the surviving set is one-way.

## Snapshot size and disk hygiene

The worst-case size of a capture is its scan budget: incompressible data
compresses to roughly its own size. In practice process memory is sparse and
mostly zero, and zero chunks are not written, so the file is usually much
smaller than the budget.

Before scanning, `--capture` samples the first `--sample-mb` (default 128 MiB),
measures the zero ratio and the compressed ratio, and prints an `expected_mb`
estimate for the whole scan. It refuses to start when free space on the output
drive is below that estimate plus `--free-margin-mb` (default 256 MiB). The
snapshots and the last-diff summary live in the scratch output directory; they
are never committed.

## What a result can and cannot show

- A changed chunk proves the realm switch wrote there. It does not name the
  meaning of the bytes.
- A stride pattern (`a=0 b=1` or similar, many consecutive records) that
  survives the same-realm control is a candidate: a per-record field that moves
  with realm and not with time. It is **not** a conclusion. The control rules
  out churn; it does not by itself separate a reversible flag from a timer, and
  the controlled run above is the proof: every surviving pattern was a per-frame
  counter.
- Only a stride pattern that also **returns in the revert capture** (`--revert`,
  `d == a`) is the strong shape: a per-record field that flips on the shift and
  flips back on the return, which a timer cannot do. Even then it is a
  candidate, not a conclusion, until a second independent round trip agrees.
- A changed chunk with no stride pattern means the change is not a uniform
  per-record field; read the tuple list for a repeated `(offset, old, new)`.
- A one-way proportion near zero (or a `diff-note` saying every surviving byte
  was restored) with no independent one-way control means the revert test could
  not discriminate: check that the revert capture really is the return realm and
  is a different capture from the control.
- No change in the scanned window means the state lived outside it: raise
  `--scan-mb`, or the captures did not come from the same session. Report that
  as UNKNOWN, not as absence.
- A raw A/B diff **without** a same-realm control cannot separate a realm change
  from runtime churn. Treat such a diff as inconclusive however large the
  detector output looks; that is exactly what the first live run showed.

## Partial analysis of the three existing snapshots

The three snapshots already on disk are `material_a`, `material_b` (the
control) and `spectral_a`; there is **no** true revert capture. Running the new
`--revert` against them is therefore only a partial run, and it is reported here
honestly:

- `--diff material_a spectral_a --control material_b --revert material_b`
  reports `reversible=10302523 one_way=0 of=10302523 reduction=0.0%`. This is
  **degenerate**: the revert capture `D` is the same file as the control `C`, so
  every signal byte trivially satisfies `A == D`. It proves only that the revert
  test is consistent with the control (the split equals the control split); it
  says **nothing** about which bytes are reversible.
- `--diff material_a spectral_a --revert material_b` (no control) reports
  `of=11601500 reversible=10302523 one_way=1298977 reduction=11.2%`, i.e. the
  **one-way proportion over the raw diff is 11.2%** (88.8% of bytes reappear in
  `material_b`). But this is numerically identical to the control split for the
  same reason, because `material_b` doubles as the control. It is a consistency
  check, not a realm result, and it is still churn-dominated.

So the existing snapshots can state a one-way proportion (11.2% over the raw
diff) but **cannot** decide reversibility: `material_b` is the control, not a
capture taken after returning to the material realm. A real revert capture
(`material_c`, step 4 of the protocol) is still needed, and only the full
four-capture run can answer the question.

## What remains UNKNOWN

- The address, stride, field offset and value meaning of any realm flag are
  **UNKNOWN**. Nothing here claims one.
- Whether the realm state is a single byte, a wider field, an enum, or a
  pointer swap is **UNKNOWN**; the diff reports shape, not semantics.
- Whether the one `kind=signal` chunk of the controlled run is the realm state,
  an effect of the realm switch, or something else is **UNKNOWN**; it is a
  candidate only.
- Whether a material/spectral difference is cause or effect (render bit vs
  gameplay state) is **UNKNOWN**; the four-capture run is the next step.
- Whether the eight `0x4C`-strided patterns are all timers beyond the observed
  per-frame counters is **UNKNOWN**; the revert test is designed to settle it.

## Result

**No realm flag is claimed.** A controlled three-capture run was performed and
its measured result is recorded above: the realm change is about eight times the
same-realm churn, the memory map stabilised, and the eight surviving strided
patterns are per-frame counters (timers), not a realm bit. Exactly one chunk was
`kind=signal`; it is a candidate, not a conclusion. The revert discriminator
(`--revert`) was designed and implemented in the scratch probe, with a
self-tested synthetic diamond and a four-capture protocol. The existing three
snapshots were analysed as a partial run and only yield a one-way proportion,
because there is no true revert capture yet. The realm state remains
**UNKNOWN** until the full four-capture run is executed and reported.

## Digest check

No engine path changed, so the documented digests are unchanged:

| run | digest |
|---|---|
| `under1` 60 ticks | `ede39c540a10ee39` |
| `cathy28` 60 ticks | `3e8293c0e38afac1` |

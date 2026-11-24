# realm-diff - a differential memory probe for the live realm state

**Status:** research plan and a scratch probe. No engine change, no realm logic,
no decoded offset. This note documents the method and the operator protocol;
the live result is **pending** and is not claimed here.

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

### `--diff <labelA> <labelB>`

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

`--live-selftest` attaches to a throwaway process (never the game), allocates a
tiny region, captures it twice with no change, pokes its own buffer, and shows
a small diff with the planted stride.

## Operator protocol

Preconditions: the game is running; do not restart it between captures. The two
captures must come from the **same process and session** or the addresses will
not align. A large scan takes a while; 4096 MiB is not instant.

1. Stand still in a zone. Avoid animated crowds if possible.
2. `python sr1_realm_diff_probe.py --capture material --scan-mb 4096`
3. Shift to the spectral realm in game. Do not move.
4. `python sr1_realm_diff_probe.py --capture spectral --scan-mb 4096`
5. `python sr1_realm_diff_probe.py --diff material spectral --stride 0x4C`

Notes for the run:

- The probe attaches to `SRX.exe` and requires module `sr1.dll`; it prints a
  gate line before it scans. If the module is not loaded, it stops.
- If the free-space check refuses, either free space, pass `--out-dir` on a
  drive with room, or lower `--scan-mb`. The check is deliberate; see below.
- `--report` summarises the last diff without re-reading both snapshots.

## Expected output shape

Shapes only; addresses are placeholders and no real address is recorded here.

```text
REALMDIFF gate ok module=sr1.dll pid=<n> ranges=<n> readable_mb=<n> scan_mb=4096
REALMDIFF capture-plan label=material scan_mb=4096 sample_mb=128 expected_mb=<n> free_mb=<n> zero=<n>% ratio=<n>
REALMDIFF capture label=material chunks=<n> kept=<n> kept_mb=<n> bytes=<n> raw_mb=<n> ratio=<n> file_mb=<n> dropped=<n> regions=<n> elapsed=<s>
REALMDIFF chunks changed=<n> of <n> only_a=<n> only_b=<n> diffs=<n>
REALMDIFF chunk addr=<hex> changed=<n> +0x<hex>:0x<v>>0x<v> ...
REALMDIFF pattern base=<hex> stride=0x4c off=<hex> a=<v> b=<v> hits=<n>
REALMDIFF record base=<hex> a=<hexbytes> b=<hexbytes>
REALMDIFF tuple off=+0x<hex> old=0x<v> new=0x<v> count=<n>
REALMDIFF diff-summary labels=material->spectral changed=<n> of <n> only_a=<n> only_b=<n> diffs=<n> patterns=<n>
```

What to paste back: the `capture` lines (both labels), the `chunks` line, every
`pattern` and `record` line, and the top `tuple` lines.

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
- A stride pattern (`a=0 b=1` or similar, many consecutive records) is the
  strongest shape: a per-record field that flips with realm. It is a candidate,
  not a conclusion; confirm it against a second shift (spectral back to
  material) and against a standstill capture in one realm.
- A changed chunk with no stride pattern means the change is not a uniform
  per-record field; read the tuple list for a repeated `(offset, old, new)`.
- No change in the scanned window means the state lived outside it: raise
  `--scan-mb`, or the captures did not come from the same session. Report that
  as UNKNOWN, not as absence.

## What remains UNKNOWN

- The address, stride, field offset and value meaning of any realm flag are
  **UNKNOWN**. Nothing here claims one.
- Whether the realm state is a single byte, a wider field, an enum, or a
  pointer swap is **UNKNOWN**; the diff reports shape, not semantics.
- Whether a material/spectral difference is cause or effect (render bit vs
  gameplay state) is **UNKNOWN**; a second reversible shift is needed.

## Result

**Pending a live run.** No live capture has been performed for this note. There
is no decoded realm offset, no claimed flag, and no fabricated finding. The
deliverable is the method, the probe and the protocol above.

## Digest check

No engine path changed, so the documented digests are unchanged:

| run | digest |
|---|---|
| `under1` 60 ticks | `ede39c540a10ee39` |
| `cathy28` 60 ticks | `3e8293c0e38afac1` |

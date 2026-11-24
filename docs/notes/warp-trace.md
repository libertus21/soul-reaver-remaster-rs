# warp-trace - which runtime reader consumes a unit link table, and at which entry

Method note for a read-only Frida probe that watches a unit's 40-byte link table
in the running game and reports the code that reads it when a portal is crossed.
Behaviour notes only: role names, short level names, counts and field offsets. No
game bytes, no addresses, no RVAs, no absolute install paths. The probe lives in
a session scratch directory and is not committed.

Date: 2026-11-24. Evidence class per claim: **CONFIRMED** (two independent
observations agree), **INFERRED** (one side only), **UNKNOWN**.

## Verdict

**Method defined, awaiting live run.**

The probe is built and its offline and synthetic-live checks pass. The
portal-to-link binding is still **UNKNOWN**: no run of the real game was made in
this slice, so no reader and no entry index were captured yet. This note records
the exact method, the validated signature, and the one command that turns the
UNKNOWN into a measurement. It does not claim a result it did not observe.

## The question

`docs/notes/level-transitions.md` proved that a `portal` placed record carries no
target, and that a separate 40-byte link table resolves 855 real unit edges but
cannot be attached to a specific portal from authored data. The missing link is
the **index** the engine uses at the moment of a transition. If the engine reads
entry `n` of the current unit's link table, then `n` is the binding between that
portal and one of the 855 decoded edges. This slice asks the data to answer it:

- Which code reads the link table, and is that read per frame or only at warp
  time?
- Which entry index does it use, and which decoded unit edge does that entry
  name?

## What is already established

- `docs/notes/level-transitions.md`: 166 `portal` objects in 136 units; no field
  carries a target; 855 real edges resolve, 96.8% pooled; the portal-to-link join
  is at chance level.
- `docs/notes/warp-level-selection.md`: the warp gate is a separate, hardcoded
  14-name cycle. An ordinary portal's destination is the **link record's target
  name string** plus a neighbouring level id, written into the pending
  level-change state. That is the read this probe is built to catch.

## The decoded link record (confirmed layout)

One record is 40 bytes, `[5 x i32 prelude][12-byte name][2 x i32 tail]`:

| offset | field | constraint |
|---:|---|---|
| `+0x00` | `prelude[0]` | always `1` |
| `+0x04` | `prelude[1]` | paired connection id; equals `+0x0C` |
| `+0x08` | `prelude[2]` | always `18` |
| `+0x0C` | `prelude[3]` | equals `prelude[1]` |
| `+0x10` | `prelude[4]` | stable target id |
| `+0x14` | name (12 bytes) | `<identifier>,<digits>`, either case |
| `+0x20` | tail (8 bytes) | uninterpreted |

The decoder proper (`src/level.rs`) additionally bounds the table with header
slot `0x35`. The probe deliberately does **not** enforce that bound: it is a
locator, not the decoder. The prelude invariant plus the name grammar is the
signature.

## Method

1. **Discovery gate (mandatory, before any tracing).** Scan readable,
   non-executable memory for the anchor `i32 18` at record offset `+0x08`, grow
   every hit into the maximal run of consecutive 40-byte records that all satisfy
   the prelude invariant, and keep the runs. Print:

   `WARPTRACE discover entries=<n> valid=<n> zone=<guess> scanned=<mb> ...`

   A run counts as validated when either it holds at least eight records
   (`big`) or every record decodes as a name and the run's target family matches
   the requested zone (`family`). The corpus is lopsided - most units carry one
   or two links - so the family rule is what makes a single-record table in a
   named zone a valid gate pass. Zero validated runs means the resident form is
   not the file form: report it and stop, do not trace guesses.

2. **Arm `MemoryAccessMonitor`** on the pages holding the chosen runs, re-arm on
   every hit, and emit one bounded line per hit:

   `WARPTRACE hit reader=<mod>+<off> kmod=<mod>+<off> op=read addr=<hex>
   pageoff=<n> entry=<n|-> field=+0x<hex>`

   `reader` is the instruction pointer that performed the access, reported
   module-relative; `pageoff` is the byte offset inside the run; `entry` is
   `pageoff / 40` when the address falls inside a record, else `-`.

3. **Correlate.** The target name is resident in the same record, so the hit
   yields the edge directly:

   `WARPTRACE edge entry=<n> from=<zone> to=<unit> tid=<id> index=<name-digits>`

4. **Classify the access pattern.** `WARPTRACE pattern hits=<n> span=<s>
   rate=<r> class=continuous|burst|sparse|idle`. `continuous` means a per-frame
   lookup; `burst` means the read clustered in a short window, which is what a
   warp-time handler looks like.

5. **Aggregate noisy streams.** `--readers` suppresses the per-hit lines and
   prints one `WARPTRACE readers <mod>+<off>:<count>,...` summary plus the
   pattern and stack lines, so a steady per-frame reader can still be named.

A short stack is captured automatically for the first distinct readers inside
the engine module, and can be widened to copy-function callers with
`--stack-scope any`.

## Offline evidence: the signature is right on the shipped archive

A read-only `--scan-file` pass over the SR1 archive (81 MiB) reports:

```
WARPTRACE discover entries=20 valid=2 zone=cathy scanned=81 runs=383
          all_entries=903 named=903 big=2 dups=7 max=11 zone_source=target-family
WARPTRACE gate ok valid=2 need=1 runs=383 zone=cathy
```

- **903** record-shaped entries across **383** contiguous runs; every one of the
  903 names decodes. The file-form signature is therefore not the limiting
  factor.
- **2** runs reach the eight-record threshold (`cathy15` 11, `cathy70` 9).
- **7** runs are exact duplicate copies of another run's target list, which
  accounts for part of the gap to the **883** records the decoder counts across
  **371** units. The rest is the skipped `0x35` end bound: a trailing
  record-shaped entry beyond a declared table end is counted by the raw scan and
  dropped by the decoder. The probe is a locator; the decoder stays the source of
  truth for counts.
- With `--zone aluka41` the family rule accepts **45** runs / **132** entries,
  showing the gate scales down to normal one- and two-link units.

Cross-check against the decoder (entry 115, the opening unit):

- Decoder `--level-links --entry 115`:
  `links under1 count=1` / `link under2 2 target_id=-9233
  prelude=1,1,18,1,-9233`.
- Probe's raw scan of the same bytes: one 1-record run, `entry=0`,
  `target=under2`, name digits `2`, `prelude=[1,1,18,1,-9233]`, `tid=-9233`.

The entry arithmetic and the entry-to-edge conversion agree with the decoder on
real data.

## Synthetic-live evidence

`--selftest` (offline) passes 52 checks: record parse, every name-grammar
rejection, prelude gates, run expansion and merging, offset-to-entry arithmetic
at the record boundaries, entry-to-edge conversion, zone guessing, page math,
chunk-overlap deduplication and every output line contract.

`--live-selftest` (throwaway system process, never the game) passes 16 checks: it
writes a synthetic table, runs the discovery gate, arms `MemoryAccessMonitor`,
spawns a native reader, and observes hits that carry `entry=0` and the decoded
edge `to=aluka2 tid=-4579`. The monitor, the re-arm loop and the correlation are
exercised end to end.

## Live run: the exact steps

The probe stays in the session scratch directory. Run it from that directory.

### Zone and portal

Prefer a unit with **one portal and one real link**, so the index cannot be
ambiguous: crossing the only portal must read the only link. Shipped examples and
the decoded edge the probe should print:

| zone | portal | decoded edge (entry 0) |
|---|---|---|
| `cathy46` | the only portal in the room | `to=cathy45 tid=-4754` |
| `city4` | the only portal in the room | `to=city3 tid=3407` |
| `cathy60` | the only portal in the room | `to=cathy76 tid=-4723` |
| `skinnr9` | the only portal in the room | `to=skinnr12 tid=21004` |
| `oracle22` | the only portal in the room | `to=oracle21 tid=-26855` |

Use whichever of these you can reach from an existing save. `cathy46` is the
recommended first attempt.

### Primary command

```text
python sr1_warp_trace_probe.py --attach SRX.exe --zone cathy46 --seconds 90
```

- Start the game, load a save that can reach `cathy46`, and stand in that room.
- Start the command. Wait for the `WARPTRACE armed ...` line.
- Walk through the room's single portal once, about ten seconds in, then stand
  still. Let the command run to the end of the window.

### Fallback with a wider scan

If the gate reports zero valid runs, the resident table is outside the default
budget or is not in the 40-byte form. Retry wider before concluding absence (the
campath work needed a 4 GiB window to see its blocks):

```text
python sr1_warp_trace_probe.py --attach SRX.exe --zone cathy46 --scan-mb 4096 --seconds 120
```

If that still reports `valid=0`, paste the `discover` and `gate` lines back
unchanged. That is itself the answer: the resident form differs from the file
form.

### Aggregate mode for a noisy stream

If the per-hit lines scroll too fast, aggregate instead:

```text
python sr1_warp_trace_probe.py --attach SRX.exe --zone cathy46 --readers --seconds 90
```

### What to paste back

Paste, verbatim:

1. the `WARPTRACE discover ...` and `WARPTRACE gate ...` lines;
2. every `WARPTRACE hit ...` and `WARPTRACE edge ...` line;
3. the `WARPTRACE pattern ...`, `WARPTRACE readers ...` and
   `WARPTRACE summary ...` lines;
4. if nothing fired, the `WARPTRACE idle ...` line.

## How to read the result

- A hit with `entry=0` and `to=cathy45` (for `cathy46`) **closes the binding**:
  the portal reads entry 0 of the current unit's link table, and entry 0 is the
  decoded `cathy46 -> cathy45` edge.
- `class=burst` with the hit at the moment of crossing means a warp-time
  handler; `class=continuous` with many hits means a per-frame lookup (for
  example a proximity or prompt scan) rather than the transition itself.
- The stack frames name the handler; the reader offset is repeated on every line
  so a second pass can pre-attach it.

## UNKNOWN (explicit)

1. **No live capture yet.** The reader offsets, the entry index used by a real
   portal, and whether the lookup is per frame or warp-time are all UNKNOWN until
   the command above runs.
2. **The resident form.** The gate assumes 40-byte records with an in-place name.
   If the engine expands the table or stores names as pointers, the gate reports
   zero runs; that outcome is recorded, not guessed around.
3. **Which portal is "the portal" in a multi-portal room.** The one-portal rooms
   above remove that ambiguity for the first run.
4. **Whether the one-portal / one-link rooms actually pair that portal with that
   link.** The authored data cannot prove it; the live run can.
5. **The `warpgate` entries.** The 28 `warpgate` pseudo-targets are the
   hardcoded 14-cycle, not ordinary portals. The probe reports them as edges when
   they are read; it does not model the cycle.

## Evidence ledger

| # | Claim | Class |
|---|---|---|
| T1 | A 40-byte record with prelude `[1,n,18,n,id]` and a `<ident>,<digits>` name is the decoded link record. | CONFIRMED (decoder + bytes) |
| T2 | The raw signature finds 903 record-shaped entries / 383 runs in the shipped archive, all names decoding. | CONFIRMED (measurement) |
| T3 | The probe's entry arithmetic and entry-to-edge conversion match the decoder on `under1`. | CONFIRMED |
| T4 | The monitor, re-arm and correlation run end to end on a synthetic table. | CONFIRMED (live selftest) |
| T5 | The index a real portal uses is the `n` in the probe's `edge entry=<n>` line. | UNKNOWN (awaiting live run) |
| T6 | The portal->link binding is available at runtime and not in authored data. | UNKNOWN |

## Method box and legal

One offline pass: the signature and entry maths validated against the shipped
archive read-only, plus `--selftest` and `--live-selftest` on synthetic data and
a throwaway system process. Behaviour notes only: counts, short level names,
field offsets and module-relative reader tokens. No game bytes, no listings, no
addresses or RVAs, no absolute install paths entered this note. The probe stayed
in a session scratch directory; the game archive was opened read-only and the
game was never launched by this slice.

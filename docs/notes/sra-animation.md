# `.SRA` decode ledger - container confirmed, animation NOT recoverable (2026-11-09)

Read-only probe of the SR1 remaster `OBJECT` folder (227 `.SRA` files). Nothing
under the install was written, no game bytes were copied into the repository, and
scratch lived only in the session temp directory. This note records counts,
structural shapes and small format constants only: no absolute offsets, no
addresses, no dumps, no install paths.

This supersedes the 2026-10-27 census in this same file. That pass found the
header and the float runs but treated the payload as undifferentiated. This pass
resolves the payload into a **record directory plus typed records** and runs the
decisive cross-check against the actor clip-set trios; the verdict is still
NO-GO for playback, now for a sharper reason.

A third pass tested the runtime slot-pose hypothesis directly (payload =
`header + frames x slots x 52`, 13 floats per slot) against the live per-object
slot counts: `docs/notes/sra-slot-pose.md`. That layout is falsified; the
runtime slot count is the model bone count, not an `.SRA` field.

## Verdict

| Question | Answer | Confidence |
|---|---|---|
| 1. Container kind | **Two-level offset-indexed record container** (not a `.SRM`-style chunk table, not a flat record array) | CONFIRMED (227/227) |
| 2. What it contains | **Variable-length typed sub-resource records**: a directory plus records tagged `0x6464640n`, `0x0FACE0FF` and others, carrying dense `f32` runs | CONFIRMED (shape) / UNKNOWN (meaning) |
| 3. 24-clip cross-check | **FAILS**: record count is below the maximum clip-set id for all 21 actor-family files; only one file (a non-player) has exactly 24 records | CONFIRMED |
| 4. Bone binding | **NONE**: no name table, no index array, no per-record joint field | CONFIRMED (absence) |
| 5. Keyframe encoding / time base | **NOT ESTABLISHED**: no unit-quaternion runs, values exceed `360`, no duration or frame-rate field | CONFIRMED (negative) |
| 6. Pose from files alone? | **NO** | CONFIRMED |

The decisive new result is the cross-check. The actor block stores **8 rows x 3
one-byte clip-set ids** (24 values in `0..66`,
`docs/notes/actor-behaviour-data.md`). If a model's `.SRA` were the clip table
those ids index, its record count would have to exceed the largest id it is
indexed by. It never does. That is a systematic falsification, not a
not-found-yet.

A later pass searched the payload directly for the confirmed spline key shapes
instead of comparing counts: `docs/notes/sra-key-search.md`. Its detector is
calibrated on the one known positive (`font`) and reconfirms the negative from
the key-pattern side.

## Quick path

1. Read section 1 for the container (what is confirmed).
2. Read section 3 for the cross-check table (the decisive evidence).
3. Read section 6 for the answer to "can we animate from files alone".
4. Treat everything under UNKNOWN as open; do not fill it by inference.

## 1. Container structure

`.SRA` is a **two-level offset container**. Both levels are plain ascending
`u32` offset lists; there is no text magic anywhere in the 227 files.

```text
[ preamble ]                         file start
  u32  preamble_count
  u32  0
  u32  offsets[preamble_count - 1]   ascending, names record starts
  u32  0                             terminator
[ zero padding to a 2 KiB boundary ] (larger for oversized preamble tables)
[ payload directory ]                at the 2 KiB boundary
  u32  offsets[K]                    ascending, relative to the boundary
  u32  0xFFFFFFFF                    terminator
[ K records ]                        variable length, last runs to EOF
```

| Part | Shape | Confidence |
|---|---|---|
| Preamble count | `u32` at file start; observed `2..1054` | observed |
| Preamble table | `preamble_count - 1` ascending `u32` offsets, closed by `0` | observed (227/227) |
| Padding | payload starts at `max(2 KiB, next 2 KiB >= preamble size)` | derived (strong) |
| Payload directory | `K` ascending `u32` offsets relative to the boundary, closed by `0xFFFFFFFF` | CONFIRMED (227/227) |
| Record count `K` | number of payload-directory offsets; range `1..230`, median `2` | CONFIRMED (227/227) |
| Records | `K` variable-length spans; last ends at EOF | CONFIRMED (227/227) |

Two facts make the payload directory load-bearing:

- **227/227 files parse** with the invariant "ascending offsets, `0xFFFFFFFF`
  terminator, every offset inside the file". The earlier census parsed the
  preamble only; this directory is a new, stronger structural fact.
- Both the preamble and the payload directory name the **same record starts**,
  but under different bases. The preamble also carries extra leading entries and
  a constant base bias relative to the payload directory. Why the two indexes
  disagree on base and why the preamble has extra entries is **UNKNOWN**. Use
  the payload directory as the authoritative record list.

Record-count distribution across the 227 files: `1` record in 83 files, `2` in
35, `3` in 20, `4-5` in 17, `6-10` in 9, `11-20` in 48, `21-30` in 8, `31-60`
in 6, and one file above 60. The set is dominated by tiny prop/effect files;
large counts occur only on character/prop animation-ish assets.

## 2. Record shape (best effort)

Records are a **tagged union**. Two tag families are confirmed by sampling many
files:

| Record head | Byte pattern | Files | Confidence |
|---|---|---|---|
| `0x64646401` / `0x64646402` / `0x64646403` | ASCII-ish tag, low byte 1..3 | 204 / 227 | CONFIRMED (tag present) |
| `0x0FACE0FF` | engine resource marker | 43 / 227 | CONFIRMED (tag present) |

40 files carry both families; 20 carry neither (other tags, not decoded). The
`0x0FACE0FF` marker is the same tag the engine's generic `0x32` resource-table
post-processor expects (`docs/notes/anim-binding.md`, A7), so at least some
`.SRA` files are shaped like a resource table rather than an animation track.

Best-effort `0x6464640n` record head, from a small but consistent sample:

| Field | Observation | Confidence |
|---|---|---|
| tag | `0x6464640n`, `n` in `1..3` | observed |
| next three words | an identical `u32` triple (values vary per record) | observed (sample) |
| following word | `0` | observed (sample) |
| next word | an integer `B` (e.g. `36`, `56`, `96`, `308`, `480`) | observed (sample) |
| rest of head | variable; sometimes a `0xCD…` uninitialized fill, sometimes a packed sub-block | UNKNOWN |
| payload | dense little-endian `f32` in many records | observed |

The `0x0FACE0FF` family sampled differently: a 12-byte record of
`[0x0FACE0FF][id][constant]`, where `id` increments across adjacent records.
Some `.SRA` files (e.g. one wraith sibling) are 26 such 12-byte records back to
back. The meaning of `id` and the constant is **UNKNOWN**.

Payload statistics over sampled records (f32 interpreted over the whole record):

| File (record | floats in range | min | max | unit-quaternion windows |
|---|---|---|---|
| door prop, record 0 | 46 | -2040.7 | 2049.4 | 0 / 52 |
| door prop, record 4 | 661 | -1265.5 | 1188.1 | 6 / 764 |
| statue prop, records 0-1 | 2178 | -1230.6 | 1242.8 | 0 / 2205 |
| character, record 0 | 684 | -511.5 | 422.6 | 0 / 753 |
| character, record 0 | 213 | -807.3 | 5.5e5 | 4 / 249 |
| character (large), first span | 3821 | -1907.5 | 5656 | 10 / 4477 |

Two negative results follow directly:

- **No quaternion track.** A 4-float window is unit-norm in 0-1.6% of sampled
  windows, i.e. chance. There is no contiguous normalized-quaternion array.
- **Not degrees-bounded.** Values span `±~2000` and occasionally far higher.
  The 2026-10-27 note inferred "rotation angles in degrees" from runs bounded
  to `0..360`; that bound does not hold across the corpus. The earlier
  inference is **not supported** and is retracted here.

## 3. Clip-count cross-check (decisive)

The actor block's 8x3 trios are 24 one-byte **clip-set ids** in `0..66`
(`docs/notes/actor-behaviour-data.md` section 5.1). The falsifiable prediction
is: if a model's `.SRA` is the clip table those ids index, the `.SRA` record
count `K` must be greater than the largest id used by that actor.

| Actor | `.SRA` records `K` | max trio id | distinct ids | `K >= max id` |
|---|---:|---:|---:|:---:|
| aluka | 55 | 66 | 19 | no |
| alukabss | 12 | 29 | 14 | no |
| hunter | 30 | 39 | 20 | no |
| kain | 21 | 36 | 16 | no |
| morboss | 14 | 48 | 18 | no |
| morlock | 41 | 51 | 18 | no |
| priests | 17 | 29 | 16 | no |
| ronin | 44 | 54 | 18 | no |
| roninbss | 20 | 36 | 15 | no |
| skinbos | 19 | 43 | 18 | no |
| skinner | 42 | 55 | 18 | no |
| sluagh | 23 | 25 | 16 | no |
| soul | 1 | 20 | 4 | no |
| vlgra | 26 | 36 | 19 | no |
| vlgrb | 26 | 35 | 19 | no |
| vlgrc | 26 | 35 | 19 | no |
| vwraith | 24 | 35 | 17 | no |
| walbosb | 9 | 20 | 6 | no |
| walboss | 20 | 28 | 12 | no |
| wallcr | 53 | 62 | 19 | no |
| wrshp | 39 | 62 | 19 | no |

`K >= max id` is **no for all 21**. The `.SRA` record list cannot be the
per-actor clip table indexed by the trio ids. Additional falsifiers:

- Only **one** of the 227 files has exactly 24 records (a wraith, and its max
  trio id is 35, so even that coincidence fails the index test). A per-actor
  "24 clips" signature does **not** appear.
- The only file with more than 66 records is the **player** `raziel` (230
  records), which has no trio table at all. It is not a shared clip library.
- The trio ids overlap between actors (`2d`/`2e`, `30`-`33`, max `66`), which the
  behaviour note reads as a **shared global id space**, not a per-actor one.

Conclusion: the trios most likely index a **global animation/action list that
does not live in the per-model `.SRA` files**, or the ids are not animation-clip
indices at all. Either way, `.SRA` is not the per-actor clip catalog.

## 4. Bone binding

There is no binding mechanism in `.SRA`:

- No name table (apparent ASCII runs are float-bit noise or the short
  `0x0FACE0FF` constant, not bone names).
- No contiguous small-index array: longest `u16` run in `1..=127` is 8 and
  longest `u32` run in `1..=255` is 15 across the census (prior pass,
  reproduced in shape by the tag/record scan here).
- No per-record joint field; records carry `f32` runs and `0x0FACE0FF` nodes,
  not `(joint, value)` pairs.

The model side is a flat 128-slot joint pool with **no hierarchy**
(`docs/notes/srm-bone-block.md`), so even a hypothetical `.SRA` channel would
have to name a joint slot. Nothing in the payload does.

Corroboration from the binary: the only consumer of the `.SRA` buffer on the
loader path is a `RET 0` stub, and the animation runtime is a separate spline
system fed from elsewhere (`docs/notes/anim-binding.md`, A1-A11). Even the
`0x0FACE0FF` post-processor (A7) builds a resource table, not a player.

## 5. Time base and keyframe encoding

Not established:

- **No duration or frame-rate field** survives a cross-file check. Files have a
  record count and per-record lengths, but no time unit.
- **No rotation representation** is confirmed: not unit quaternions (section 2),
  not `0..360` degrees (values exceed that), and no `3x3` matrix run was found.
- **No sample-rate or key-count field** was identified. Candidate head integers
  (`B` in the `0x6464640n` head) do not correlate with payload float counts in
  the samples (e.g. `B = 96` with ~740 floats, `B = 56` with ~47 floats).

A "reconstruct at time t" step therefore has no time axis to evaluate against.

## 6. Can a pose be reconstructed from file data alone?

**NO.** The minimal algorithm would be: pick a clip, get its duration and sample
rate, evaluate each joint's rotation/translation at `t`, and blend with the
`.SRM` per-vertex weights. Every input that algorithm needs is missing or
unbound:

| Needed | Status |
|---|---|
| Clip identity / duration / sample rate | absent (section 3, 5) |
| Joint or bone reference per channel | absent (section 4) |
| Rotation encoding (quaternion/euler/matrix) | not established; not quaternions, not degrees (section 2) |
| Hierarchy (parent/child) for a full-body pose | absent in `.SRM`; absent in `.SRA` (section 4) |
| Time axis `t` | absent (section 5) |

This is a **documented no-go**, not gate-able by playtesting: there is no
honest mapping from a `.SRA` record to a joint rotation. Claiming one would be
fabrication. Full-body posing needs a different asset (the per-object spline key
table, or `.SRL`), not a larger reading of `.SRA`.

## What stays UNKNOWN

1. The meaning of the preamble's extra entries and its constant base bias
   relative to the payload directory.
2. The `0x6464640n` head fields (the identical triple, `B`, the `0xCD…` word).
3. The `0x0FACE0FF` record's `id` and constant word, and whether these records
   are resource nodes or key data.
4. The meaning of the `f32` payload: coordinate/UV/profile vs. any time series.
   The ranges (`±~2000`) and symmetric shapes suggest spatial data, but this is
   INFERRED, not proven.
5. The 20 files with neither known tag, and any other tag family.
6. The exact relationship between the preamble count (`2..1054`) and `K`.
7. `.SRL` record semantics (60 files, `SRL.` magic, ~1200-byte stride) and
   whether it carries the global clip list the trios may index.
8. Whether `0x0FACE0FF` records are the "child tables" the `0x32`
   post-processor copies; sampled large files do not carry the tag, so the
   processor's coverage is partial.

## Falsification notes

This verdict is a no-go for *skeletal playback from `.SRA`*. It would be
overturned only by evidence of:

1. A per-record joint index paired with a rotation, repeated over a time axis,
   in a file whose record count exceeds that actor's max trio id.
2. A global `.SRA` (or `.SRL`) table with ≥67 entries that the trio ids index
   and that carries joint-bound channels.
3. A head field that reproducibly equals a key/frame count and a sample rate
   across several actors.

The refuted claims from the earlier pass are: "rotation angles in degrees"
(contradicted by values far outside `0..360`) and "undifferentiated payload"
(contradicted by the description, directory and tags in section 1-2).

## Sibling formats and community tools (unchanged from the 2026-10-27 census)

- `.SRL` (60 files): ASCII `SRL.` magic, a count, ~1200-byte stride; no name
  overlap with `.SRA`/`.SRM`. Still the most likely place for a global list.
- `.PCM` (29 loose files): the object-model container
  (`docs/notes/object-models.md`); not animation.
- `.SRM` (349 files): flat 128-slot joint pool, no hierarchy, no clips
  (`docs/notes/srm-bone-block.md`).
- Community tools (io_scene_xrm, ModelEx, Recombobulator, AsunderNoMore) do not
  read remaster `.SRA`; the 2026-10-27 note records the exact URLs and licenses.
  No public remaster `.SRA` parser was found.

## Method and legal

- Three read-only passes: full 227-file record-directory parse; per-record tag
  and payload statistics on sampled files of each size class; the trio/record
  cross-check against `docs/notes/actor-behaviour-data.md`.
- Tags `0x6464640n` and `0x0FACE0FF`, the `0xFFFFFFFF` directory terminator, and
  the 2 KiB alignment are format constants (the same class of fact as the
  `0x80000000` `.SRM` sentinel), not data dumps.
- Counts, shapes and small ids only. No addresses, no absolute offsets, no game
  bytes, no install paths, no disassembly. The install was read-only; scratch
  was in `%TEMP%` and not committed.
- No production code changed; no CLI shipped. The record interior is not
  confirmed enough ("zero contradictions") to justify a `--sra-info` command,
  so the earlier proposal's diagnostic remains intentionally unshipped.

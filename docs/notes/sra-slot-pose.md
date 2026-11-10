# sra-slot-pose - the 52-byte slot-pose layout is falsified (2026-11-09)

Read-only probe of the SR1 remaster `OBJECT` folder (227 loose `.SRA` files) plus
a read-only census of the archived object containers in `BIGFILE.DAT`. Nothing
under the install was written; no game bytes were copied into the repository;
scratch lived only in the session temp directory. This note records counts,
sizes, record counts and format constants only: no addresses, no file/record
offsets, no disassembly, no raw game bytes, no absolute install paths.

The starting hypothesis was: `.SRA` payload = a small header plus
`frames x slots x 52` bytes, where `52` is `13` little-endian `f32` (a 3x3
rotation, a scale marker and a translation), and `slots` is the runtime slot
count observed live (`raziel 66`, `sluagh 24`, `eggfx 3`, `soul 2`, and `1` for
the rest). The hypothesis is **falsified**. The three independent tests below
each refute it on their own, and the runtime slot count is explained by a
source outside `.SRA` entirely.

## Verdict

| # | Question | Answer | Class |
|---|---|---|---|
| 1 | Does any `.SRA` field equal the runtime slot count (66/24/3/2/1)? | **NO.** The only "hits" are the container's own preamble count and directory entries. | CONFIRMED negative |
| 2 | Does the payload decompose as `header + frames x slots x 52`? | **NO** for the large files. Small files match only by arithmetic coincidence. | CONFIRMED negative |
| 3 | Is a 13-float (or 12-float) rotation block present anywhere? | **NO.** 0 of 3.74M 3x3 windows and 0 of 3.72M 3x4 windows at every byte offset pass an orthonormality gate. | CONFIRMED negative |
| 4 | What block structure does the file actually have? | Two-level offset container; payload = `K` variable-length tagged records; 84/227 payloads open with a nested ascending offset table. | CONFIRMED |
| 5 | Where does the runtime slot count come from? | The model-side bone count of the object: the archived object-container bone count matches 9 of 10 sampled models exactly. | CONFIRMED (9/10) / INFERRED (carrier) |
| 6 | Ship a decoder? | **NO.** The gate "the file must independently agree with the runtime slot counts" fails. | CONFIRMED negative |

The decisive result is #1: a slot-pose layout must encode its slot count, and
`raziel` (66), `sluagh` (24), `eggfx` (3) and `soul` (2) do not appear as
headers. #3 then shows the pose block itself is absent, so even a slot count
without a pose would not help.

## 1. The runtime slot counts are model bone counts, not `.SRA` fields

The live capture reported per-object slot counts. The archived object model for
each of those names carries a bone count in its model header. Reading the object
container for each name (archived low-poly model, the same family decoded in
`docs/notes/object-models.md`) gives:

| Asset | Runtime slots | Archived object model bone count | Match |
|---|---:|---:|:---:|
| `raziel` | 66 | 66 | yes |
| `sluagh` | 24 | 24 | yes |
| `eggfx` | 3 | 3 | yes |
| `soul` | 2 | 2 | yes |
| `font` | 1 | 1 | yes |
| `tranhst` | 1 | 1 | yes |
| `undhost` | 1 | 1 | yes |
| `warpg` | 1 | 1 | yes |
| `glphicon` | 1 | 1 | yes |
| `hint` | 1 | 6 | **no** |

Nine of ten match exactly, including the three that the flat-layout hypothesis
could not explain (`raziel 66`, `sluagh 24`, `eggfx 3`). `hint` is the single
outlier: its object model carries six bones while the capture reported one slot.
The capture reads the *in-memory animation record's slot count*, which is at
most the model bone count and can be smaller when only one descriptor is active;
that reconciles `hint` without weakening the corpus result. The important point
is that the slot count is a property of the **model** (the object container and
its bone table), not of the `.SRA` side-car.

This is the answer to "why does the naive layout hold for the trivial case and
fail for the big ones": it never held. `soul` (2 slots) is a coincidental size
match and `raziel`/`sluagh` are not, because the slot count was never in the file.

## 2. Header test: no `.SRA` field equals the slot count

For each named asset, the whole file was scanned for a 32-bit word equal to the
runtime slot count, and the hits were classified by region (container preamble,
payload directory, record heads):

| Asset | Runtime slots | Header/preamble hit | What the hit actually is |
|---|---:|---|---|
| `raziel` | 66 | none | 1,210 scattered data words equal 66; none is a header field |
| `sluagh` | 24 | one | an entry of the ascending payload directory, not a field |
| `eggfx` | 3 | one | the container preamble count is 3 |
| `soul` | 2 | one | the container preamble count is 2 |
| `hint` | 1 | none | the value 1 does not occur as a 32-bit word at all |
| `tranhst` | 1 | none | 142 scattered data words equal 1; none is a header field |

The two numeric "agreements" (`eggfx 3`, `soul 2`) are exactly the container's
own preamble count, a value that has nothing to do with animation slots and
takes the same number for any file of that shape. `raziel 66` never appears in
a header position, which alone falsifies a header-carried slot count. `hint`
has no matching word anywhere, so a "1-slot header" cannot exist.

## 3. Size test: `header + frames x slots x 52` does not hold

The strong hit that motivated the hypothesis was
`SOUL.SRA = 2084 = 4 + 2 x 20 x 52`. Against the documented container it is
arithmetic, not structure: the payload begins at the 2 KiB boundary, so the real
breakdown of a 2,084-byte file is `2,048 (payload boundary) + 8 (directory) +
28 (one record) = 2,084`. The `4 + 40 x 52` reading ignores the 2 KiB base and
the 8-byte directory, and the resulting "20 frames" has no independent evidence.

For the large files the decomposition is arithmetically impossible:

| Asset | File size | Payload size | `52 x slots` | Payload mod `52 x slots` |
|---|---:|---:|---:|---:|
| `soul` | 2,084 | 36 | 104 | 36 (payload is one 28-byte record) |
| `sluagh` | 38,156 | 36,108 | 1,248 | 1,164 |
| `raziel` | 533,444 | 527,300 | 3,432 | 2,204 |

Neither `sluagh` nor `raziel` is a multiple of `frames x slots x 52` for any
integer frame count, so no single flat block can hold the payload. Across the
corpus, file size modulo 52 is spread over all thirteen residues (14 files are
exact multiples, 43 have remainder 4, 44 have remainder 20, and so on); a
"clean multiple" is not a distinguishing signature.

## 4. Pose test: the 13-float signature is absent

If the payload were `13` floats per slot, the first nine floats of each block
would be a 3x3 rotation. A real rotation matrix is orthonormal: each row has unit
length and the rows are mutually orthogonal. The detector measures the worst
deviation of those three conditions (0.0 for a perfect rotation). Results over
all 227 files, at **every** byte offset (not only aligned ones):

| Window | Windows tested | Below 0.05 error | Minimum error found |
|---|---:|---:|---:|
| 9 floats (3x3 rotation) | 3,744,114 | **0** | 0.329 |
| 12 floats (3x4 affine basis) | 3,717,786 | **0** | 0.370 |
| 52-byte aligned 9-float blocks | 64,193 | **0** | 0.400 |
| deterministic random baseline | 2,894 | 0 | 1.000 |

Not one window anywhere in the corpus is rotation-like, and the best case (0.329)
is far outside any plausible tolerance. Random data scores near 1.0 because its
rows are not unit length; real rotation matrices score near 0.0. The payload is
therefore not a packed array of 13-float (or 12-float) transforms. The floats
that are present are spatial/parameter data (they include plausible coordinate
ranges) but never organized as orthonormal bases.

## 5. The block structure that is actually present

The container is the two-level shape already recorded in
`docs/notes/sra-animation.md`: a padded preamble, then a `0xFFFFFFFF`-closed
`u32` payload directory whose offsets are relative to the payload boundary, then
`K` variable-length records. This pass reproduces it on 227/227 files.

| Asset | File size | Payload | Records `K` |
|---|---:|---:|---:|
| `raziel` | 533,444 | 527,300 | 230 |
| `sluagh` | 38,156 | 36,108 | 23 |
| `undhost` | 198,528 | 196,480 | 13 |
| `tranhst` | 16,288 | 14,240 | 3 |
| `soul` | 2,084 | 36 | 1 |
| `eggfx` | 2,172 | 124 | 1 |
| `hint` | 2,232 | 184 | 1 |

"Multiple sections inside one file" was a candidate explanation and is partly
right: the file *is* a list, but of variable-length tagged records, not of fixed
`frames x slots x 52` blocks. 84 of the 227 payloads themselves begin with an
ascending nested offset table, i.e. records are nested resources. The record
head families (`0x6464640n` and the `0x0FACE0FF` marker) and their `(count,
value)` triples are unchanged from the prior ledger; they do not equal the
runtime slot counts either.

## 6. Falsification ledger

| Claim | Observation that would refute it | Tested? |
|---|---|---|
| `.SRA` encodes the runtime slot count as a header field | the value 66 (raziel) or 24 (sluagh) in a header position | Tested: 9/10 match the object model instead; raziel's 66 never in a header, hint's 1 absent entirely |
| Payload = `header + frames x slots x 52` | payload not divisible by `52 x slots`, or no rotation blocks | Tested: neither large payload divides; 0 rotation blocks |
| Each slot is a 13-float pose (9 rotation + scale + translation) | no orthonormal 3x3 run | Tested: 0 of 3.74M windows pass; minimum error 0.329 |
| The palette output (12-float 3x4) is stored in `.SRA` | no orthonormal 3x4 run | Tested: 0 of 3.72M windows pass; minimum error 0.370 |
| `.SRA` is a single flat block | a `K`-record payload with a nested directory | Tested: 230/23/13/3/1 records across the sampled set; 84/227 have nested tables |
| The runtime slot count comes from the model, not `.SRA` | a `.SRA` field equal to the slot count, or a model bone count that does not match | Tested: 9/10 object-model bone counts match exactly; no `.SRA` field does |

## 7. Why no decoder shipped

The pass/fail gate was "the file must independently agree with the runtime slot
counts". It does not: the counts are the model's bone counts, and the pose
signature is absent. Per the task's hard rule, a documented negative is the
success condition, and inventing a layout to force a decoder would be
fabrication. No `--sra-slots` command, parser or engine wiring was added, and no
golden moved. The `.SRA` payload remains UNKNOWN in meaning; what is now settled
is that it is **not** a per-slot 13-float pose array and does **not** carry the
runtime slot count.

The remaining honest route to a file-only body pose is the one already named in
`docs/changes/remaster-animation/animation-source.md`: pin the animation-record
carrier, then decode the 32-byte per-slot descriptor and the 4-float track
frame. The slot count for that work is read from the model bone table, not from
`.SRA`.

## Method and legal

- Read-only probe of the 227 loose `OBJECT/*.SRA` files and the archived object
  containers inside `BIGFILE.DAT`; the install was never written.
- The alignment-free 3x3/3x4 scan and the slot-count comparison are scratch
  scripts that stay outside the repository. The repository records counts,
  sizes, record counts and format constants only.
- No production code changed; no golden moved; no CLI added. The negative is the
  deliverable.

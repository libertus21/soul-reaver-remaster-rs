# procedural-slot-pose-gate - the corrected gate still finds no per-slot descriptor array in the archived object container (2026-11-09)

Read-only byte probe of the single-model object containers inside the shipped
SR1 `BIGFILE.DAT`, using the engine's own object parser as the bone-count oracle.
Scratch lived only in the session temp directory; the archive was opened
read-only and never copied into the repository. This note records counts, sizes,
strides and signed-16 distributions only: no addresses, no file/record offsets,
no disassembly, no raw game bytes, no absolute install paths.

Date: 2026-11-09. Evidence class per claim: **CONFIRMED** (bytes plus a
falsifiable test), **INFERRED** (one side only), **UNKNOWN**.

## Question

`docs/notes/animation-loader-trace.md` corrected the earlier `.PCM` negative:
the per-slot descriptor array is described as an **indirect, base-relative,
signed-16-marker** record region (32-byte stride, count equal to the model bone
count), not the flat `bones x 32` block with a 0/1 byte marker that
`docs/notes/pcm-animation-record.md` had tested. It left "Next probe #1" open:
**re-scan the object container with the corrected gate and confirm or refute the
descriptor array at byte level.** This slice runs exactly that probe.

Gate models (bone-count oracle, unchanged): `raziel` 66, `sluagh` 24,
`skinner` 35, `morlock` 37, `eggfx` 3, `soul` 2.

## Verdict (agreement rate first)

**The corrected gate does not locate a per-slot descriptor array. Agreement on
the decisive gate models is 0/3: `raziel` 0, `sluagh` 0, `skinner` 0.** The
corpus-wide "hits" are false-positive dominated by the smallest bone counts, in
which a count-equal window is trivial.

| Gate | Corpus agreement | raziel (66) | skinner (35) | sluagh (24) | Hit location by bone count |
|---|---:|---:|---:|---:|---|
| **A** - indirect pointer/slot table of `slotCount` 32-byte-stride records | 219 / 394 | **0** | **0** | **0** | 100% at bone count 1 (201) and 2 (18); **none** at bone count >= 3 |
| **B** - contiguous `slotCount x 32` window whose signed-16 marker active = (value != -1) exactly equals the ranged-bone pattern | 78 / 394 | **0** | **0** | 1 (non-marker) | concentrated at bone count <= 8; single hits at 21, 24, 40 |
| **C** - contiguous `slotCount x 32` window with a marker-like signed-16 lane (values in {-1, 0, 1}, empty/filled split present) | 163 / 394 | **0** | **0** | 1 (same lane) | **all** at bone count <= 8 |

Corpus: 394 single-model containers, ~2.0 MB payload, every byte offset and
every signed-16 lane of every candidate window tested.

The decisive rows are the three gate models. `raziel` (66) and `skinner` (35)
produce **zero** candidates under every gate; `sluagh`'s single Gate B/C hit is
not a marker split (16 of its 24 values are outside {-1, 0, 1}, and it lands on
the trailing lane, not an authored marker).

## 1. Gate A - the "indirect pointer array" is the object auxiliary pointer list

For a window to be the corrected pointer array it must hold exactly `slotCount`
base-relative `u32` offsets and the targets must be the `slotCount` records of a
32-byte-stride block.

- The only structural candidates are runs that coincide with the object header's
  **auxiliary pointer list** (the list `docs/notes/object-models.md` already
  maps: ascending data-base-relative positions of auxiliary records). Their
  targets are **variable-length** auxiliary records, not 32 bytes apart, so they
  fail the strict 32-byte-stride requirement in every gate model.
- That list is exactly the class of "known array" the earlier negative predicted
  would masquerade as a pointer table; the pointer values look offset-like
  because they *are* offsets into a different structure.
- Requiring the strict 32-byte-stride run leaves **0 hits at bone count >= 3**;
  the entire 219/394 corpus agreement is bone count 1 (a single valid offset is
  trivial) and bone count 2 (two offsets 32 apart is trivial).
- Across all candidates the pointed records have **no coherent signed-16 marker
  lane** (no lane with an -1/off-versus-live split), which is the independent
  structural refutation.

## 2. Gate B - exact ranged-pattern equality never fires on the gate models

The corrected marker is a signed 16-bit value with -1 meaning "off". Testing
active = (value != -1) against the model's ranged-bone pattern:

- `raziel`: **0** windows/lanes anywhere in the payload reproduce the 58-of-66
  ranged pattern.
- `skinner`: **0** windows reproduce the 32-of-35 pattern.
- `sluagh`: exactly **1** window/lane matches, but its values are 1 x -1, 7 x 0,
  and 16 values outside {-1, 0, 1}; that is a coincidental numeric match, not a
  {-1, live} marker split.
- Corpus hits are concentrated at bone count 2-8, where a short pattern matches
  on some lane by chance; there is no repeatable per-model record shape.

## 3. Gate C and best-case marker statistics

Loosening the gate to "any marker-like lane" still fails on the gate models:

| Model | best "-1" fraction over all windows/lanes | reading |
|---|---:|---|
| `raziel` (66) | 12.1% | nowhere near a marker; the only fully-constant lane is a padding fill |
| `skinner` (35) | 68.6% at one window/lane | multiple-comparison artefact over ~0.9M windows; not a slot-coherent split |
| `sluagh` (24) | 45.8% at one window/lane | same artefact |

The only perfectly constant lanes found are constant **padding fills**
(`0x8000`, `0x8080`, `0x0000`) in the trailing lane of the record, i.e. filler
bytes, not an authored active marker. A genuine marker would show a bimodal
off/live split tied to the slots; none does on the gate models.

## 4. What this means

Combined with `docs/notes/pcm-animation-record.md`, the honest status of the
"data is in the archived object container" claim is now:

- **The archived object container does not carry a materialised per-slot
  descriptor array** - not as a flat 0/1-marked block (earlier negative), and
  not as the corrected indirect, base-relative, signed-16-marker block (this
  scan). The gate models are uniformly negative.
- The static module trace (`animation-loader-trace.md`) correctly identifies the
  *carrier kind* as relocatable object-container records, but the on-disk
  object containers this project already parses **are not where those records
  physically sit**, or they are produced by a load step this byte probe cannot
  see (the loader's decompress-plus-relocate pass and the exact entry/placement
  remain UNKNOWN, as that note states).
- **PROCEDURAL SLOT POSE Phase 2 is not implemented.** There is no descriptor
  array to parse at the byte level, so there is no evidence-backed
  `SlotDescriptor` layout, no aim vector to read, and no composition rule to
  ship. Inventing a 32-byte layout that "looks plausible" but disagrees with the
  bone-count gate would be fabrication. Per the hard rule, this documented
  negative is the success condition.
- The `src/pose.rs` path is unchanged; it remains the only shipped pose math
  (one-bone `.SRM` rotation about a joint head, render/orientation only, never
  in the headless digest). No engine behaviour, digest or golden moved.

## What stays UNKNOWN

1. **The exact carrier** of the animation record's per-slot descriptor array and
   track table. The static trace binds it to the object-container loader path,
   but no archived object container holds it under the corrected gate.
2. **The load-time relocation product.** The descriptor array may only exist
   after the loader's decompress-plus-relocate pass, in an in-memory form not
   equal to any archived byte window. Reproducing it file-only requires the
   object-container decompression, still UNKNOWN.
3. **Whether the records live in a named dependency object** rather than the
   character's own container (`animation-loader-trace.md` UNKNOWN #2). This scan
   only covers the character mesh containers and other single-model containers in
   the same archive; it does not enumerate dependency objects.
4. **The descriptor fields** beyond the roles the static trace reads (mode bits,
   signed-16 marker, three-short aim vector): no byte-level field is established
   by this scan because the array is not located.

## Next probes (ordered by cost)

1. **Capture the descriptor array in memory at load time** (the prepared runtime
   probe path): dump the per-object descriptor array after the object-container
   loader runs, and compare its bytes to any archived window. This distinguishes
   "produced by relocation" from "stored flat somewhere we did not scan".
2. **Pin whether the records belong to a named dependency object**, by following
   the tagged name resolution from the model definition.
3. **Decode the object-container decompression** only if a file-only replay is
   required; otherwise leave it as the documented blocker.
4. Do not re-run a raw byte scan of the same containers for the same gate; two
   independent gates (0/1 byte and signed-16 indirect) are now negative on the
   gate models.

## Falsification ledger

| Claim | Observation that would refute it | Status |
|---|---|---|
| The archived container carries a `slotCount`-record 32-byte descriptor array (corrected gate) | any gate model with a 32-byte-stride pointer array or a slot-coherent signed-16 marker lane | Tested: 0/0/0 on `raziel`/`skinner`/`sluagh` |
| The corrected pointer-array candidates are real descriptor arrays | candidates whose targets are 32-byte-strided with a marker lane | Tested: targets are the variable-length auxiliary records; no marker lane |
| The correction (signed-16 marker, not 0/1) rescues the placement | a slot-coherent -1/live split on a gate model | Tested: `raziel` max -1 fraction 12.1%; `skinner`/`sluagh` only single-window artefacts |
| The earlier negative stands | a corrected-gate hit on `raziel` or `skinner` | Not refuted: 0 under all three gates |

## Method and legal

- Read-only: a standalone scratch scanner over the user's own SR1 archive
  (`1\BIGFILE.DAT`), replicating the repository's index and object-container
  parse (shapes only) and the engine bone-count oracle.
- Corpus: 394 single-model containers, ~2.0 MB payload; every byte offset and
  every signed-16 lane per candidate window for three gate definitions (indirect
  pointer table, exact ranged pattern, marker-like lane).
- No production code changed; no CLI added; no golden moved; the engine loop and
  digest are untouched.
- Counts, sizes, strides and signed-16 distributions only. No addresses, file or
  record offsets, disassembly, raw game bytes or absolute install paths are
  recorded here.

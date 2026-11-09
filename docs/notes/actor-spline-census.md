# actor-spline-census - do any actor containers carry spline descriptors?

Read-only census of the shipped SR1 object corpus. Counts, container names and
format facts only. No binary addresses, no container/file offsets, no
disassembly listings, no raw game bytes, no absolute install paths. Scratch
work lived in the session temp directory and was removed; the archive and the
loose `.PCM` files were opened read-only.

This note closes item 2 of `docs/notes/spline-animation.md` section 8 ("corpus
census of actor-block animation records") and constrains the actor-field
UNKNOWN in that note. It does not change production code and does not move a
golden.

## Verdict first

| # | Question | Answer | Class |
|---|---|---|---|
| 1 | Does any of the 21 actor containers carry a usable, non-empty spline channel descriptor that decodes plausibly? | **NO.** Zero valid rotation descriptors and zero valid position descriptors across all 21 actor containers. | CONFIRMED (corpus-wide scan) |
| 2 | Does the player container fare differently? | **NO.** `raziel` carries no valid descriptor either. | CONFIRMED |
| 3 | Does any container in the corpus carry one? | Only `font` (one 3-key rotation channel) and a marginal 2-key position fragment in `particle`; both are non-actors. | CONFIRMED |
| 4 | Can an actor object-transform animation be reconstructed from files? | **NOT from the actor container under the documented descriptor shape.** The actor-side carrier remains unpinned; building a decoder now would invent a layout with no shipped example. | CONFIRMED negative / UNKNOWN carrier |

The decisive numbers are in section 5.

## 1. Scope

- 21 actor containers: the 19 actor-family containers in `BIGFILE.DAT`
  (`aluka`, `alukabss`, `hunter`, `kain`, `morlock`, `ronin`, `roninbss`,
  `skinbos`, `skinner`, `sluagh`, `soul`, `vlgra`, `vlgrb`, `vlgrc`, `vwraith`,
  `wallcr`, `walbosb`, `walboss`, `wrshp`) plus the two loose actor `.PCM`
  files (`morboss`, `priests`). This is the same actor set as
  `actor-behaviour-data.md`.
- `raziel` (player family) was scanned separately for completeness.
- Contrast: every one of the 397 object containers in `BIGFILE.DAT` was
  scanned with the same rules, so "no actor hit" is measured against the whole
  corpus, not a sample.

## 2. Descriptor and key shapes used

The scan uses the file-side shape already recorded in
`docs/notes/spline-animation.md` section 1.2 and 2:

| Record | Field | Width |
|---|---|---|
| channel descriptor | key-array offset (data-base-relative) | `u32` |
| | key count | `u16` |
| | stride flag | `u8` |
| | behaviour flags | `u8` |

- stride flag `1` -> rotation keys, source stride `0x0A`
  (`i16` delta + 4 x `i16` quaternion).
- stride flag `0` -> position keys, source stride `0x20`
  (`i16` delta + 3 x `i16` position + 3 x `i32` out-tangent + 3 x `i32`
  in-tangent).
- fixed-point scale 4096 (`>>12`).

A candidate "plausible" channel must satisfy all of:

- the referenced key array lies inside the container data;
- every frame delta is in `0..=4096` and the total span is greater than zero;
- rotation: every quaternion component is within `+/-8192` and the normalized
  length is within `0.10` of `1.0` after `>>12`;
- position: every tangent is bounded (`|v| <= 1_000_000`), positions are the
  stored `i16` values.

The task brief abbreviates the stride flag as `+0x0A`; that is the runtime
definition field. The file descriptor places the same flag at `+0x06`. The
scan used the file shape. An alternate 16-byte record with the count at `+0x08`
and the stride flag at `+0x0A` was also scanned, to cover the runtime-definition
mirror; it produced zero actor hits as well.

## 3. Method

1. Read the `BIGFILE.DAT` index, resolve each object container's data base and
   name, and classify the block at object-header `+0x1C` by signature
   (`0xACE00065` actor, `0x00BB0630` player).
2. For each actor container, probe every structural candidate the authority
   notes name:
   - actor-block sub-block pointers at `+0x04` and `+0x08`;
   - the actor-block pointer table at `+0x30` (up to 32 entries);
   - the model-header `+0x2C` carrier.
   Each candidate was interpreted both as a single 8-byte descriptor and as a
   four-descriptor set.
3. Independently, brute-force every even data-relative offset in the container
   as a descriptor start, requiring the four channel roles
   (position / rotation / position / any), and validate the referenced keys.
4. Calibrate the scan on the one end-to-end example already validated in
   `spline-animation.md`: `font` must produce exactly one 3-key rotation
   channel with deltas `75, 75, 0` and unit quaternions. It does.
5. Run a layout-free scan for runs of rotation-shaped keys (positive deltas
   plus four near-unit 12-bit quaternions) as a backstop: if a rotation key
   array exists anywhere, even without a descriptor, this catches it.

## 4. Per-model census

`ptr` is the number of in-range actor-block pointer-table targets probed as
candidate descriptor sets. "Valid" counts descriptor-shaped records whose
referenced keys pass the section 2 checks.

| Model | Source | Actor block | ptr | Valid descriptors | Non-empty | Plausible decode |
|---|---|---|---:|---:|---:|---|
| `aluka` | archive | yes | 17 | 0 | 0 | no |
| `alukabss` | archive | yes | 13 | 0 | 0 | no |
| `hunter` | archive | yes | 16 | 0 | 0 | no |
| `kain` | archive | yes | 12 | 0 | 0 | no |
| `morlock` | archive | yes | 17 | 0 | 0 | no |
| `ronin` | archive | yes | 16 | 0 | 0 | no |
| `roninbss` | archive | yes | 12 | 0 | 0 | no |
| `skinbos` | archive | yes | 13 | 0 | 0 | no |
| `skinner` | archive | yes | 16 | 0 | 0 | no |
| `sluagh` | archive | yes | 12 | 0 | 0 | no |
| `soul` | archive | yes (degenerate) | 8 | 0 | 0 | no |
| `vlgra` | archive | yes | 13 | 0 | 0 | no |
| `vlgrb` | archive | yes | 13 | 0 | 0 | no |
| `vlgrc` | archive | yes | 12 | 0 | 0 | no |
| `vwraith` | archive | yes | 12 | 0 | 0 | no |
| `walbosb` | archive | yes | 16 | 0 | 0 | no |
| `walboss` | archive | yes | 14 | 0 | 0 | no |
| `wallcr` | archive | yes | 16 | 0 | 0 | no |
| `wrshp` | archive | yes | 16 | 0 | 0 | no |
| `morboss` | loose `.PCM` | yes | 13 | 0 | 0 | no |
| `priests` | loose `.PCM` | yes | 11 | 0 | 0 | no |
| `raziel` | archive (player) | yes (player family) | 3 | 0 | 0 | no |

All 22 actor/player rows: **0 valid descriptors, 0 non-empty, 0 plausible
decodes**.

No actor row has a non-zero model-header `+0x2C` carrier. The one shipped
container with a non-zero `+0x2C` is `font`, a non-actor.

## 5. Corpus contrast and decisive numbers

| Container | Family | Valid rotation | Valid position | Reading |
|---|---|---:|---:|---|
| `font` | non-actor | 1 (3 keys) | 0 | Calibration: deltas `75, 75, 0`, unit quaternions. The only end-to-end validated channel in the corpus. |
| `particle` | non-actor | 0 | 1 (2 keys) | Marginal byte coincidence: deltas `244, 0`, positions and tangents do not form a coherent clip. Not usable. |
| 375 other non-actors | non-actor | 0 | 0 | No descriptor-shaped data. |

Decisive counts:

- actor containers scanned: **21**
- actor containers with a valid non-empty descriptor: **0**
- valid rotation descriptors in all actor data: **0**
- valid position descriptors in all actor data: **0**
- valid alternate-shape (16-byte) descriptors in all actor data: **0**
- object containers in the corpus: **397**
- object containers with any valid descriptor: **2** (`font`, `particle`), both
  non-actors
- actor containers whose model-header `+0x2C` carrier is non-zero: **0**

The layout-free backstop found only isolated 2-3 key rotation-shaped fragments
in actor data. None is referenced by a descriptor, none has a coherent frame
span and authored tangents, and the same pattern appears in binaries and mesh
arrays where no animation exists. They are treated as coincidental byte
patterns, not clips. `font` is the only descriptor-backed rotation channel in
the corpus.

## 6. What this settles and what it does not

Settled:

- The actor containers do **not** carry a usable spline descriptor in the
  documented file shape. This is a corpus-wide negative with numbers, not a
  not-found-yet.
- The `font` example in `spline-animation.md` remains valid, but it is an
  isolated non-actor case. It does not generalise to actors.
- A decoder for actor object-transform animation cannot be written from the
  actor container data as it stands. Writing one would have to invent the
  carrier and the clip resolution, which the project rules forbid.

Still UNKNOWN:

- The exact actor/file field that carries the actor spline set (the note's
  UNKNOWN #1). The actor-block pointer table points at behaviour and
  animation-parameter sub-tables, not at a four-channel descriptor set.
- Whether the actor spline set is built from a global table, from another
  archive family, or from a runtime-only source. The corpus negative is
  consistent with any of the three.
- Clip identity: how a `(name, id)` pair or a trio clip byte selects a spline
  slot. UNKNOWN, unchanged.
- Behaviour-flag bit semantics, scale-channel encoding, and the twelve further
  qwords copied into the runtime set. UNKNOWN, unchanged.
- Whether any non-actor family other than `font` uses the same system. The
  corpus scan says no, but `particle` shows how easily a 2-key fragment can
  look descriptor-shaped by chance.

## 7. Why phase 2 was not implemented

The task conditioned the decoder on Phase 1 finding at least one plausible
non-empty descriptor **in an actor container**. Phase 1 found zero. A decoder
built on the `font` rotation channel alone would target a non-actor clip and
would still not reproduce actor object-transform animation, because the actor
carrier, the clip selection and the scale channel are all unpinned. That would
be an invented layout, so the honest outcome is this documented negative.

No production code was changed. No CLI was added. `cargo test` and `cargo
build` were left at their baseline.

## 8. Falsification ledger

| Claim | Observation that would refute it | Tested? | Result |
|---|---|---|---|
| No actor container carries a valid rotation descriptor | one actor container with a descriptor whose keys decode as near-unit quaternions with positive deltas | Full strict scan of all 21 actors at every even offset, plus structural candidates, plus the alternate 16-byte shape | Not refuted: 0 hits |
| No actor container carries a valid position descriptor | one actor container with a descriptor whose position keys have a positive span and bounded tangents | Same scan | Not refuted: 0 hits |
| The file descriptor is 8 bytes with stride at `+0x06` | a valid descriptor that only parses with a different field order | `font` calibrates the 8-byte shape and its 3 keys exactly; the alternate shape yields 0 actor hits | Consistent with 8-byte shape |
| The actor spline set is not in the actor container | a four-descriptor set at a known actor carrier | Actor-block `+0x04`, `+0x08`, pointer table and model `+0x2C` probed; all fail the descriptor bounds/role test | Refuted the "in-container" hypothesis |
| The corpus negative is not a sampling artefact | a valid actor descriptor outside the scanned offsets | Every even data-relative offset was scanned, not a sample | Not refuted |

## 9. Reproducibility

The scan is deterministic and needs only the player's own `BIGFILE.DAT` plus
the loose `OBJECT/*.PCM` files:

1. Resolve each object container exactly as `object.rs` does (relocation count,
   `4 * roundup512(N)` data base, 12-byte name at object-header `+0x4C`).
2. Classify the `+0x1C` block signature.
3. For the 21 actor containers, probe the structural carriers and brute-force
   every even data-relative offset with the section 2 rules.
4. Expect: 21 actor containers, 0 valid descriptors; 397 object containers,
   2 non-actor hits (`font`, `particle`).

A future reader can add this as a read-only CLI capability if a reproducible
in-repo probe is wanted; it is not needed to settle the question, and none was
added in this slice.

## 10. Legal and method

- Read-only byte probe of `BIGFILE.DAT` and the loose `OBJECT/*.PCM` files.
- Scratch scripts lived in the session temp directory and were removed.
- Counts, container names and format constants only. No binary addresses, no
  container offsets, no disassembly, no raw game bytes, no install paths.
- No game process was launched or instrumented. No production code changed; no
  golden moved.

# SR1 engine — decompiled-pseudocode findings

Read-only decompilation session over `1\sr1.dll` (Ghidra 12.1.4 headless, 9 targets).
Output was written to a scratch directory outside the repository and is **not**
committed. Everything below is a behavior description, not reproduced code.

Claim classes: **CONFIRMED** = visible in control flow and/or verified against
real bytes on disk; **INFERRED** = role/naming only; **UNKNOWN** = not established.

## 1. Host service table (CONFIRMED)

The exported entry point takes one argument: a pointer to a **table of function
pointers** supplied by the launcher. Every platform service is reached through
that table — file path construction, open, read, clock, input polling, callback
registration.

Consequence for the rewrite: the engine is host-agnostic by construction. The
Rust equivalent is a `Host` trait (open/read/clock/input/callbacks) passed into
the engine, which also makes the loader testable without touching the real
install.

## 2. Engine entry flow (CONFIRMED)

Ordered, static:

1. store the host table in a global
2. archive scan/open routine (see §8)
3. three startup calls (roles UNKNOWN)
4. a guard call: if it returns 0, the engine returns immediately
5. open `bigfile.dat` through host slot 0
6. install four callbacks into host-table slots (indices kept in the analysis log)
7. a readiness check
8. **main state machine** (§3)
9. a final check that decides loop-or-return

The whole block sits inside an unconditional loop, so the engine re-enters the
same flow (e.g. returning to the menu) without re-executing process startup.

## 3. Main state machine (states CONFIRMED, roles INFERRED)

A single state word drives everything; observed cases:

| State | Role (inferred) |
|---|---|
| 1 | intro / attract loop; waits for input, plays videos |
| 2 | intro hand-off trigger (references `KAININT`) |
| 3 | post-intro menu bootstrap |
| 5 | transition / confirm step |
| 7 | logo + legal sequence (`publogo.bik` family) |
| 8 | short path (single call) |
| 9 | re-init: copies a saved name into a buffer, clears 7 bytes, then state 2/10 |
| 10 | main game init (long init chain) |

States observed in the `switch`; ordering of execution is runtime-dependent and
therefore **UNKNOWN** from static analysis alone.

## 4. `BIGFILE.DAT` index — the 4th field is a name tag (CONFIRMED)

Measured directly on the file:

- `u32` entry count at offset 0: **1325**
- entries follow immediately, **16 bytes** each:
  `[name_hash: u32][length: u32][offset: u32][tag: 4 ASCII chars]`

Field order verified against real bytes: entry 0 has `length = 491540`,
`offset = 22528`; entry 1 has `offset = 516096` — non-overlapping and
sequential, which rules out the swapped reading.

The tag is **not** an unknown: it is a 4-character code. Distribution over all
1325 entries includes `DOOR` (15), `HOST` (9), `GEAR` (6), `PIL2`/`UNB1`/`OUT4`/
`OUT3`/`PIL3` (6 each), `UBB1`/`PIPE`/`UNA1`/`PIL1` (4 each), plus `DING` and
`INGF`. These read as the leading 4 characters of the asset name, uppercased.

The engine's lookup compares **both** the name hash and the tag, i.e. the real
lookup key is `(hash, tag)`. Our reader currently discards the tag as
"unknown"; it should parse and expose it (hash-only selectors keep working, the
tag adds fidelity).

## 5. Entry payloads are game data, not DDS (CONFIRMED)

Sampled entry heads: `10 00 00 00 02 00 00 00 …` on model-like entries and the
ASCII magic `FNPa` on entries 7, 9, 11, 13 (audio-like). No `DDS ` magic
anywhere in the archive — consistent with the earlier full scan of all 1325
entries.

So `--show` refusing with "not a DDS texture" is correct behavior, not a defect.

## 6. The four `.BIG` files share one header (CONFIRMED)

`TEXTURES.BIG`, `MOVIES.BIG`, `BONUS.BIG` and `E3.BIG` all start with two
`u16` fields:

| File | u16[0] | u16[1] (count) | size |
|---|---|---|---|
| TEXTURES.BIG | 512 | 1287 | 168,693,760 |
| MOVIES.BIG | 512 | 29 | 3,805,184 |
| BONUS.BIG | 512 | 268 | 35,131,392 |
| E3.BIG | 512 | 178 | 23,334,912 |

`512` reads as the header size; the second field as the entry count, and all
four counts are plausible for their content. Payload after the header is
91–96 % non-padding, so these are not stubs.

The engine reaches these files through a **different** path than `bigfile.dat`
and keeps a fixed 48-slot table for them. Where the per-file index table lives
is **UNKNOWN** — bounded probes at offsets 512 and 4096 did not find it.

## 7. Texture side tables (CONFIRMED shapes)

- `AREA\TEX_HD.MAP` — read as a fixed 384,000-byte blob in one path; another
  path treats the first 4096 `u16` values as an offset table and the remainder
  as payload. This matches the earlier byte probe (zeros, then a fill region,
  then data).
- `AREA\FX_TEX_MAP.TXT` — plain text, `#` starts a comment, numeric fields per
  line; builds a table of 6144 entries.

## 8. Async load queue (CONFIRMED shape)

A ring buffer of **64 slots × 120 bytes**. A request records target buffer,
size, callback, parameter, flag, and the file name (up to ~52 characters).

Lookup path: compute name hash + 4-char tag → scan the bigfile index →
(a) load the entry's byte range from the archive, or (b) fall back to a real
file on disk when the name is not in the index. Paths under `\kain2\area`
bypass the archive when a flag is set, with the `\kain2\` prefix stripped first.

Failure messages observed: `Load Queue is full or corrupted`,
`Can't locate file %s`, `Can't read from bigfile: %s`.

## 9. Consequences for the Rust rewrite

- Model host services as a trait — it mirrors the original design and keeps the
  loader testable with synthetic inputs.
- Add the 4-char tag to `BigfileEntry` and to the lookup path.
- Implement the `.BIG` header (`u16 512` + `u16 count`) as an incremental step;
  leave the per-file index for a dedicated slice.
- The state machine gives a skeleton for `Engine::run`: intro → menu → game.
- Do not expect textures inside `BIGFILE.DAT`.

## 10. Open questions

- `.BIG` per-file index location (the blocker for reading `TEXTURES.BIG`).
- Full enumeration of the host table slots (only the touched ones are known).
- Runtime ordering of the state machine (needs dynamic tracing, not static).

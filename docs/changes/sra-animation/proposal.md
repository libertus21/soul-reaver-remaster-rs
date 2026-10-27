# Proposal: sra-animation

**NO-GO for hierarchy, and NO-GO for a clip catalog.** A read-only probe of
all 227 `.SRA` files in the SR1 remaster `OBJECT` folder found animation-like
float curves but **no parent array, no bone/bone-index table, no clip names,
and no proven time base**. There is no honest way to bind a `.SRA` curve to a
`.SRM` joint, so this proposal does **not** deliver posing or playback.

What it does deliver is the smallest visible, truthful slice: a read-only
`--sra-info <FILE.SRA>` census command that decodes the container enough to
prove, per file, what is and is not there, and prints one curve as a text
sparkline. That keeps the probe reproducible in code instead of living only
in a notes file, and it makes the no-go falsifiable.

## Quick path

1. `cargo run -- --sra-info "<one .SRA>"` prints entry count, header and
   payload sizes, table shape, `hierarchy=none`, `clip_table=none`, curve
   segment count, and a 40-column sparkline of the longest curve.
2. `cargo test` proves the header model, every refusal, and the curve
   detector on synthetic bytes only.
3. User acceptance: run it on one large character file, one prop file, and
   one stub; record relative names, output lines, and exit code. No window,
   no GPU, no playback.

## Details

| Topic | Decision |
|---|---|
| Intent | Answer, with code and evidence, whether `.SRA` can animate the decoded `.SRM`: does it carry a hierarchy, clips, or neither |
| Format verdict (read-only census) | **Neither.** 227 `/` 227 files expose a small declaration count, a zero second word, an ascending offset table terminated by `0`, a header with a 2 KiB floor, and a payload of packed integer records plus smooth float ramps bounded to `0..360`. No parent list, no name table, no duration, no frame rate |
| Why hierarchy is excluded | Longest run of `u16` values in `1..=127` across all payloads is 8; longest run of `u32` values in `1..=255` is 15. A 128-slot parent array would appear as a contiguous run of at least 128 small indices. It is not there |
| Why clips are excluded as a catalog | No text magic, no printable name table (apparent ASCII runs are float-bit noise), no duration or frame-count field that survives a cross-file check, and no per-file clip boundary record |
| What the curves are | Derived (strong), not observed: monotone float runs with small positive steps inside `0..360` in 156 of 227 files. Consistent with rotation angles in degrees. The channel-to-joint mapping is **UNKNOWN** |
| Minimal slice | NEW pure `src/sra.rs` (header + table + classification) and one `--sra-info` arm in `main.rs`. No `render.rs`, no `pose.rs`, no new crate, no window |
| Reuse rule | `srm.rs`, `pose.rs`, `render.rs`, `bigfile.rs`, `textures.rs`, `dds.rs` untouched. `.SRA` is decoded only through the new module |
| Input contract | Path arrives at runtime only; no game bytes, absolute install paths, coordinates, or dumps committed; tests use synthetic buffers |
| Failure policy | LOUD: missing arg, extra arg, non-`.SRA` suffix, file shorter than the 2 KiB floor, declaration count above the sanity ceiling, truncated table, table not ascending, missing `0` terminator → clear stderr line + exit 1 or 2, never a panic |
| Non-goals | Pose, playback, skeleton overlay, hierarchy inference, clip segmentation claims, `.SRL`/`.PCM`, SR2 side, Ghidra, export |
| Slice boundary | Diagnostic only. A layout surprise at apply is a documented no-go naming the failed check, never a reinterpretation of a curve as a bone rotation |

## Explicit no-go condition

This change is GO only while all four hold:

1. The header model (count word, ascending table, `0` terminator, 2 KiB
   floor) holds on at least 225 of 227 census files.
2. No contiguous parent/index array of 128 small values exists in any
   payload.
3. No printable name, duration, or frame-rate field is found by the
   classifier on the census.
4. User acceptance shows a stable, readable `--sra-info` line on a large
   character file, a prop file, and a stub, with the documented exit code.

If (1), (2), or (3) fails at apply, STOP: the format is richer than the
probe found, and the change becomes a notes-only no-go naming the failed
check and `.SRL` plus Ghidra as the next candidates.

## Why this is not a wasted slice

The repo previously treated `.SRA` as "the animation candidate". That
assumption is why one-bone posing stopped where it did. Encoding the probe
as a command (a) retires the assumption with evidence, (b) gives the next
animation change a decoded container to build on, and (c) costs one small
module instead of an open-ended animation effort.

## Checklist

- [ ] Reviewer can state the verdict without re-reading (no hierarchy, no
      clip catalog; curves exist but are unbound).
- [ ] Reviewer can state what the command prints and what it refuses.
- [ ] Reviewer can state what remains UNKNOWN (channel binding, time base,
      semantics of the packed header words, `N`).
- [ ] Legal guardrails hold: whitelist `.gitignore` intact, no game bytes,
      relative names only, synthetic test bytes only, community sources
      cited with licenses and no code copied.

## Next step

Read `spec.md` for the CLI contract and exit codes, `design.md` for the
probe ledger and evidence classes, `tasks.md` for the forecast.

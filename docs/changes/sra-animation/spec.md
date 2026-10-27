# Spec: sra-animation

`cargo run -- --sra-info <FILE.SRA>` parses one SR1-remaster `.SRA`
container, prints a single structural summary line plus one text sparkline,
and exits 0. It never opens a window, never poses, and never claims to
animate. Malformed or truncated input fails LOUD with exit 2; bad
invocation fails with exit 1; a non-`.SRA` path fails with exit 3. Every
existing CLI form (`--mesh`, `--pose`, `--bigfile`, `--textures`, audio,
file, directory) keeps its exact prior behavior.

## Quick path

1. Run `cargo run -- --sra-info "<install>\1\OBJECT\<NAME>.SRA"`
   (relative name in docs; the real path is runtime-only).
2. stdout shows the summary line, then `curve: <40-column sparkline>` or
   `curve: none`.
3. User acceptance records three relative names (one large character file,
   one prop file, one stub), the summary lines, and the exit code.

## Requirements

| ID | Requirement |
|---|---|
| R1 | CLI shape: exactly two args, `--sra-info` then one path. Zero, one, three or more args, an unknown flag, or `--sra-info` absent the path are a LOUD usage error + exit 1. Existing modes are unchanged |
| R2 | Extension gate: a path whose extension is not `.sra` (case-insensitive) is refused with a clear message + exit 3, before any parse |
| R3 | Container model: read `u32 declaration_count`; the header length is `max(2048, 8 + 4 * (declaration_count + 1))`. The next `declaration_count` words are the offset table in ascending order; the following word MUST be `0` (the terminator observed in 227/227 census files). The payload is `[header_len, EOF)` |
| R4 | Bounds and refusals: a file shorter than the 2 KiB floor and 16 bytes, a `declaration_count` above `MAX_DECLARATIONS` (a fixed sanity ceiling), a header longer than the file, a table that is not strictly ascending, or a missing `0` terminator are each `Err` + exit 2. No panic, no unbounded allocation |
| R5 | Payload classification (heuristic, MUST be labelled): scan aligned `f32` values; a *curve segment* is a run of at least 8 consecutive finite values in `0.0..=360.0` with strictly positive steps below `30.0`. Report the segment count. The output word is `curves=` and the doc says heuristic |
| R6 | Hierarchy probe: report `hierarchy=none` when the payload has no run of at least 128 consecutive `u16` values in `1..=127`. Report `hierarchy=candidate` otherwise. The check is per file, bounded to one pass |
| R7 | Clip-table probe: report `clip_table=none`. No name, duration, or frame-rate field is decoded, so the value is a fixed statement of what this parser establishes, not a format claim. Documented as such |
| R8 | Output: first line exactly `sra entries=<N> header=<H> payload=<P> table=ascending terminator=yes hierarchy=<none\|candidate> clip_table=none curves=<K>`, then `curve: <sparkline>` using the 8 glyphs `.:-=+*#@` mapped over the longest segment's value range, or `curve: none` when `K == 0`. Counts and a rendered curve only: no coordinates, no hex, no file bytes |
| R9 | Non-goals: pose, playback, keyframe sampling, joint binding, skeleton overlay, `.SRL`/`.PCM`, `extra > 0` matrices, export, SR2 side, Ghidra. No module outside `src/sra.rs` and the new `main.rs` arm may change |
| R10 | Legal/tests: tests use SYNTHETIC byte buffers only; no game file is read by a test; no game bytes, absolute paths, coordinates, hashes, or dumps committed; whitelist `.gitignore` stays |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Parsed and reported (including `curves=0`, which is a valid result) |
| 1 | Usage error: wrong argument shape, unknown flag, missing path |
| 2 | Malformed container: too short, header past EOF, count above ceiling, table not ascending, terminator missing |
| 3 | Refused input: extension is not `.sra` |

## Acceptance scenarios

- [ ] **Large character file:** `--sra-info "<large character>.SRA"` exits 0;
      the summary reports `entries` in the census range, `table=ascending
      terminator=yes`, `hierarchy=none`, `curves=` greater than zero; the
      sparkline renders. No window opens.
- [ ] **Prop file:** the same command on a small prop exits 0 and prints a
      well-formed line; `curves=0` is accepted and prints `curve: none`.
- [ ] **Stub file:** the 2,084-byte placeholder class exits 0 with
      `payload` around 36 bytes and `curve: none`.
- [ ] **Hierarchy probe honest:** a synthetic payload containing 128
      consecutive `u16` values in `1..=127` yields `hierarchy=candidate`;
      the same payload with a single value of `200` inserted halfway yields
      `hierarchy=none`.
- [ ] **Curve detector:** a synthetic ascending ramp of 20 floats in
      `0..100` yields `curves=1`; the same values with one step above `30.0`
      split into two shorter runs and yield the documented count.
- [ ] **Refusals:** missing arg, two paths, `--sra-info` alone, a `.SRM`
      path, a 1 KiB buffer, a count above the ceiling, a descending table,
      and a non-zero terminator each produce the documented exit code and a
      one-line stderr message; none panics.
- [ ] **No regression:** `--mesh`, `--pose 7:0`, `--bigfile`, `--textures`
      and audio forms print exactly what they printed before this change.
- [ ] **Unit level:** `cargo test` covers the header length rule, the 2 KiB
      floor, table ascending/terminator, count ceiling, truncation at every
      boundary, hierarchy probe both arms, curve detector both arms,
      sparkline mapping, and the four exit codes, without touching a game
      file.
- [ ] **Legal:** `git status --short` shows only intended source/doc files;
      no extracted bytes, no absolute game paths, no coordinates.

## Next step

See `design.md` for the probe ledger, evidence classes, and UNKNOWNs;
`tasks.md` for the forecast and the single-PR plan.

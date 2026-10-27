# Tasks: audio-snd

Ordered work units. Each keeps its tests with its behavior and is independently
reviewable. `test_command` for apply: `cargo test`.

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~380 authored (`snd.rs` ~110 code + ~85 test; `wav.rs` ~45 code + ~35 test; `main.rs` ~50 code + ~45 test; README ~12) |
| 400-line budget risk | Medium (fits, but with little slack) |
| Chained PRs recommended | No |
| Suggested split | Single PR (4 work-unit commits) |
| Delivery strategy | single-pr (cached) |
| Chain strategy | pending - not needed at this size |

```text
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Medium
```

This planning bundle (proposal + spec + design + tasks + the findings note) is
docs-only and ships on its own branch without applying anything.

### Suggested Work Units

| Unit | Goal | Likely PR | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|---|
| 1 | Container parse and validation | PR 1 | `cargo test snd` | N/A - pure slices | `Bank`, `SndError`, tables in `src/snd.rs` |
| 2 | ADPCM decode plus WAV writer | PR 1 | `cargo test snd` / `cargo test wav` | N/A - pure buffers | `decode_frame`, `WaveInfo`, `src/wav.rs` |
| 3 | `--audio` CLI and output guard | PR 1 | `cargo test audio` | `--audio <one real .SND> --alist` (read-only) | `run_audio` in `src/main.rs` |
| 4 | README, playtest, legal gate | PR 1 | `cargo test` | `--wave <n> --out <%TEMP% file>`, listen (user playtest) | `README.md` |

## Task 1 - `src/snd.rs`: container parse and validation (<=140 authored lines, tests included)

- [ ] 1.1 Create `src/snd.rs`; register the module in `main.rs`. Add constants
  `HEADER_BYTES = 40`, `PROGRAM_BYTES = 8`, `ZONE_BYTES = 16`,
  `FRAME_BYTES = 16`, `SAMPLES_PER_FRAME = 28`, `PLAYBACK_RATE = 44_100`.
- [ ] 1.2 Add `SndError` with one variant per spec message row (magic, header
  size, truncation, zone sum, zone wave range, key range, wave offsets, bank
  magic, bank size, span bounds, wave index, filter, output path, io) and
  `Display`/`Error`.
- [ ] 1.3 Add `Program` and `Zone` structs and parse the three tables in the
  documented order, bounds-checking every read with `checked_add` against the
  slice length. Reject magic other than `DNSa` and header size other than 40.
- [ ] 1.4 Validate before exposing anything: `sum(program.zones) == zones` (only
  windows that fit), `zone.program < programs`, `zone.wave < waves`,
  `zone.lowest_key <= zone.highest_key`.
- [ ] 1.5 Add `spans(body_len) -> Vec<WaveSpan>`: strictly increasing, first
  zero, all multiples of 16, last span ends exactly at `body_len`; reject
  anything else.
- [ ] 1.6 Add `zones_for_wave(wave) -> usize` and `waves()` for the report.
- [ ] 1.7 Unit tests on synthetic buffers: happy path with 2 programs / 3
  zones / 3 waves; wrong magic; header size 28; truncated program table; zone
  sum off by one; `wave == waves`; `lowestKey > highestKey`; offset 24;
  offsets out of order; last span short of / past the body.
- [ ] 1.8 Verify: `cargo test snd` green; `cargo build` warning-free.

## Task 2 - `src/snd.rs` decode and `src/wav.rs` writer (<=150 authored lines, tests included)

- [ ] 2.1 Add `WaveInfo { span, frames, samples, stopped, loop_start,
  loop_end }` and `info(body, index)`: scan byte 1 of each frame for flags
  6 and 3 (sample indexes) and for 7 (stop); compute
  `samples = frames * 28 - (28 if stopped)`.
- [ ] 2.2 Add `decode_frame(frame, prev, out) -> Result<u8, SndError>`: read
  shift/filter from byte 0, reject filter > 4, decode bytes 2..15 as 28
  nibbles (low first, sign-extended, 12-bit left shift, arithmetic shift,
  filter prediction divided by 64, clamp to `i16`), return the flag byte.
- [ ] 2.3 Add `decode(body, index) -> Vec<i16>`: honour flag 7 by not decoding
  that frame and stopping; decode all other flags normally.
- [ ] 2.4 Unit tests: filter 0 with nibbles `0`, `7`, `-8` (`0`, `28672`,
  `-32768`); filters 1-4 against hand-computed two/three-frame sequences; a
  stop frame ending the wave one frame early; a loop span reported as
  `Some(start)/Some(end)`; sample count formula for one-shot and looped.
- [ ] 2.5 Create `src/wav.rs`: `mono_pcm16(samples, rate) -> Vec<u8>` emitting
  the canonical 44-byte header (`RIFF`, size, `WAVE`, `fmt `/16/PCM 1/1 ch/
  rate/rate*2/2/16, `data`, `samples*2`) followed by little-endian samples.
- [ ] 2.6 Add `write_mono_pcm16(path, samples, rate)` that writes the whole
  buffer in one call, and `ensure_outside_git_tree(path)` that walks the
  parent chain and refuses when any ancestor contains a `.git` entry.
- [ ] 2.7 Unit tests: header field-by-field byte compare for 2 samples; total
  size == `44 + samples*2`; git guard accepts a temp dir and refuses one that
  contains a `.git` directory and one whose parent does.
- [ ] 2.8 Verify: `cargo test snd` and `cargo test wav` green; no `unsafe`, no
  new dependency.

## Task 3 - `src/main.rs`: `--audio` arm and output guard (<=120 authored lines, tests included)

- [ ] 3.1 Add the `--audio` first-argument arm accepting exactly
  `--audio <snd> --alist` (3 arguments) and
  `--audio <snd> --wave <n> --out <wav>` (5 arguments); every other shape is a
  usage error with exit 1.
- [ ] 3.2 `run_audio`: read the `.SND` (loud I/O error), `Bank::parse`, derive
  and read the sibling `.SMP` by extension (loud missing-bank error naming the
  expected path), check magic and `declared == file - 8` and `% 16 == 0`.
- [ ] 3.3 `--alist`: print the track summary line and one line per wave using
  `spans` + `info`, in index order, with `loop <start>..<end>` or `loop -`.
  Exit 0.
- [ ] 3.4 `--wave <n> --out <path>`: range-check, `info`, `decode`,
  `ensure_outside_git_tree`, parent-exists check, `write_mono_pcm16`, then the
  report line with peak, RMS and non-zero percentage. Exit 0.
- [ ] 3.5 Add the two usage lines to the no-argument usage block; do not touch
  existing lines.
- [ ] 3.6 CLI tests on synthetic pairs in temp dirs: `--alist` shape and
  counts; decode report and WAV bytes; wave 999; bad magic; header size 28;
  missing `.SMP`; bank size off by one; in-git output path; missing output
  parent; `--wave 1` without `--out`; `--alist extra`.
- [ ] 3.7 Verify: `cargo test` green; `cargo run -- --audio <one real .SND>
  --alist` exits 0 and prints the wave count of that track (read-only check).

## Task 4 - README, playtest, legal gate (docs only, <=40 authored lines)

- [ ] 4.1 Update the README audio bullet: the pair is readable, `--alist` lists
  waves, `--wave/--out` writes one scratch WAV, 44100 is chosen, sequences and
  music playback are still missing. Point to `docs/notes/s9-audio-snd.md`.
- [ ] 4.2 Run the playtest: `--alist` on a real track, then
  `--wave <n> --out <%TEMP%\<name>.wav>`, open it in a player. Record track,
  wave index, sample count, duration and what was heard in the PR body.
- [ ] 4.3 Delete the scratch WAV after listening; confirm it never entered the
  repository.
- [ ] 4.4 Legal: `git status --short` shows only `src/*.rs`, `README.md`; no
  `.wav`, no game bytes, no install path in the diff.
- [ ] 4.5 Verify: `cargo test` green.

## Forecast lesson (post-apply)

The forecast above (~380 authored lines) landed at ~1,491 authored code lines,
the second consecutive large miss, against an 800-line review budget. Two
changes to future forecasts:

- Budget tests as their own line. Tests were folded into each unit's estimate
  and then grew with the error surface (one case per invariant and per refusal
  path), which is where most of the overrun came from.
- If the honest total exceeds the review budget, chain the PRs from the start.
  This slice discovered the overrun at the end, when splitting was no longer
  cheap, so it shipped as one large PR.

## Apply gate

- [ ] All work units land as one PR from fresh `main`.
- [ ] `cargo test` green; `--audio <track> --alist` exits 0 on at least three
      different tracks and reports the same wave counts the probe found
      (`ALMA` 23, `PLTR` 3, `MLSP` 40).
- [ ] `--wave <n> --out <%TEMP% file>` writes a playable, non-silent WAV for at
      least one one-shot wave and one looped wave; the reported sample count
      matches `frames * 28` theory including the stop-frame case.
- [ ] All six refusal paths fail loud with distinct messages and exit 1, and
      none leaves a partial WAV behind.
- [ ] Every existing mode is unchanged; no new dependency; no game bytes.
- [ ] No-go gate: if the header size or counts do not reconcile, or every wave
      decodes to silence, stop, revert the claim, keep audio unread, and name
      the `.OGG` fallback. Do not guess an alternative encoding.

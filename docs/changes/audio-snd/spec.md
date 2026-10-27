# Spec: audio-snd

`--audio <FILE.SND> --alist` enumerates the waves of one Crystal Dynamics
sound bank. `--audio <FILE.SND> --wave <n> --out <FILE.wav>` decodes exactly one
wave from the paired `.SMP` sample bank to 16-bit mono PCM and writes a WAV to
the path the user chose. Nothing else in the tool changes. A music sequence is
never rendered: only one instrument sample is made audible.

## Quick path

1. `--audio <MUSIC\ALMA\ALMA.SND> --alist`: exit 0, one line per wave, plus a
   track summary line.
2. `--audio <MUSIC\ALMA\ALMA.SND> --wave 17 --out <D:\tmp\alma17.wav>`: exit 0,
   one report line, a WAV on disk outside the repo.
3. Open the WAV in any player: audible sample, correct length.
4. `--wave 999`: exit 1, out-of-range message, no file written.
5. `--out <repo>\x.wav`: exit 1, refused as inside a git working tree.
6. `cargo test`: parser, decoder, WAV writer and every refusal on synthetic
   bytes.

## Requirements

| ID | Requirement |
|---|---|
| R1 | The CLI SHALL keep every current mode exactly as it is. `--audio` SHALL be a new first argument accepting exactly two shapes: `--audio <snd> --alist` (3 arguments) and `--audio <snd> --wave <n> --out <wav>` (5 arguments). Any other shape SHALL be a loud usage error with exit 1 |
| R2 | The `.SND` file SHALL be accepted only with magic `DNSa` (bytes `44 4E 53 61`) and header size 40. Any other magic SHALL report `not a .SND container`; any other header size SHALL report `unsupported .SND header size` naming the observed value and `40`. Nothing SHALL be guessed or partially parsed |
| R3 | The body SHALL be read in the documented order: program table at `header_size` (8 bytes per program), zone table (16 bytes per zone), wave offset table (`u32` per wave), sequence offset table (`u32` per sequence), label offset table (`u32` per label), then sequences. Every read SHALL be bounds-checked against the file length; a short file SHALL be a loud truncation error naming required and actual bytes |
| R4 | Counts SHALL be validated before use: `sum(program.num_zones) == header.zones` (summed over programs whose zone window fits), every `zone.parent_program < programs`, every `zone.wave_index < waves`, and `zone.lowest_key <= zone.highest_key`. Any violation SHALL be a distinct loud error |
| R5 | Wave offsets SHALL be read as `u32` and SHALL be strictly increasing, with the first equal to 0, and every value a multiple of 16. Any violation SHALL be a loud error naming the offending index and value. No offset SHALL be clamped |
| R6 | The sample bank SHALL be the sibling `.SMP` of the `.SND` (same directory and stem, extension replaced case-insensitively). If absent, the error SHALL name the expected path. The `.SMP` SHALL have magic `PMSa` and a declared body size equal to `file_length - 8`; the body size SHALL be a multiple of 16. Any mismatch SHALL be a distinct loud error |
| R7 | A wave SHALL span `[offset_i, offset_i+1)` in the body, and the last wave `[offset_last, body_size)`. A span SHALL be a non-zero multiple of 16. A span reaching past the body SHALL be a loud error |
| R8 | Each 16-byte frame SHALL be decoded as: byte 0 = shift (low nibble) and filter (high nibble), byte 1 = flag, bytes 2..15 = 28 4-bit nibbles, low nibble first, sign-extended, shifted and filtered per the published PSX SPU ADPCM rules with filter coefficients `(0,0)`, `(60,0)`, `(115,-52)`, `(98,-55)`, `(122,-60)` divided by 64, accumulated with the two previous output samples and clamped to `i16` |
| R9 | A frame whose flag is 7 (stop) SHALL NOT be decoded; decoding of that wave SHALL stop there. Flags 0, 1, 2, 3, 4 and 6 SHALL decode normally; loop flags SHALL be recorded as metadata only. A filter value above 4 SHALL be a loud error naming the frame index (the published spec defines filters 0-4 only) |
| R10 | `--alist` SHALL print one summary line and one line per wave, in index order, containing the wave index, body offset, byte length, frame count, decoded sample count, duration in seconds at 44100 Hz, and the loop span as `<start>..<end>` in samples, or `-` when the wave has no loop markers. `start` SHALL be the first sample of the flag-6 frame and `end` the first sample of the flag-3 frame, so the looped region is `[start, end)`. Exit 0 |
| R11 | `--wave <n> --out <path>` SHALL decode wave `n`, report one line containing the track name, wave index, frames, samples, rate (labelled as chosen), duration, peak absolute value, RMS and non-zero sample percentage, then write the WAV. Exit 0 |
| R12 | The WAV SHALL be a canonical 44-byte RIFF/WAVE file: `RIFF`, size, `WAVE`, `fmt ` (16, PCM = 1, 1 channel, 44100, byte rate 88200, block align 2, bits 16), `data`, size = `samples * 2`, then little-endian `i16` samples. It SHALL be written in full or not at all (no partial file left behind on failure) |
| R13 | The output path SHALL be refused, loud, when: its parent directory does not exist; or it, or any ancestor directory, contains a `.git` entry (file or directory) - i.e. the path is inside a git working tree, which includes this repository. The refusal message SHALL state that game audio must stay outside the repository |
| R14 | No new dependency SHALL be added. Tests SHALL use synthetic SND/SMP bytes and temp dirs; no game bytes, install paths or decoded audio SHALL appear in code, tests, outputs or artifacts |
| R15 | The sample rate SHALL be 44100 Hz in the WAV and in every printed duration, and every message that prints it SHALL mark it as chosen rather than parsed, because the container stores no rate |

## Frame decode contract

| Frame byte(s) | Meaning | Handling |
|---|---|---|
| 0 low nibble | shift, 0..12 | `sample >>= shift` after the 12-bit left shift |
| 0 high nibble | filter, 0..4 | prediction from the two previous samples; >4 is a loud error |
| 1 | flag | `7` stops before decoding this frame; `0,1,2,3,4,6` decode |
| 2..15 | ADPCM | 14 bytes x 2 nibbles = 28 samples, low nibble first |

Sample arithmetic per nibble `n` (sign-extended to -8..7):

```text
s   = (n << 12) >> shift
s  += (prev1 * c1 + prev2 * c2) >> 6      // c = (0,0) (60,0) (115,-52) (98,-55) (122,-60) by filter
out = clamp(s, -32768, 32767); prev2 = prev1; prev1 = out
```

Decoded sample count for a wave is `frames * 28`, minus `28` for a wave whose
last decoded position was a stop frame. Both cases occur in the observed data:

| Kind | Frame flags present | Sample count |
|---|---|---|
| One-shot | exactly `{0, 1, 4, 7}` | `(frames - 1) * 28` |
| Looped | exactly `{0, 2, 3, 6}` | `frames * 28` |

## Message contracts

| Condition | Stream | Message (shape) | Exit |
|---|---|---|---|
| `--alist` summary | stdout | `track <name> programs <p> zones <z> waves <w> sequences <s> bank <v> sample-rate 44100 (chosen)` | 0 |
| `--alist` wave line | stdout | `wave <i> offset <off> bytes <n> frames <f> samples <n> duration <sec>s loop <start>..<end>` | 0 |
| `--alist` wave line, no loop | stdout | `... loop -` | 0 |
| Decode report | stdout | `wrote <path>: <name> wave <i>, <n> samples @ 44100 Hz (chosen), <sec>s, peak <p>, rms <r>, nonzero <pct>%` | 0 |
| Wrong magic | stderr | `audio: not a .SND container (magic '<m>', expected DNSa)` | 1 |
| Wrong header size | stderr | `audio: unsupported .SND header size <n> (expected 40, soul-reaver revision)` | 1 |
| Truncated tables | stderr | `audio: truncated .SND: needs <n> bytes, has <m>` | 1 |
| Zone count mismatch | stderr | `audio: inconsistent counts: programs declare <a> zones, header says <b>` | 1 |
| Zone out of range | stderr | `audio: zone <i> references wave <w>, but only <n> waves exist` | 1 |
| Bad key range | stderr | `audio: zone <i> has lowest key <a> above highest key <b>` | 1 |
| Wave offsets bad | stderr | `audio: wave offset <i> is <v>, expected a multiple of 16 above <prev>` | 1 |
| Missing bank | stderr | `audio: sibling sample bank not found: expected <path>` | 1 |
| Wrong bank magic | stderr | `audio: not an .SMP sample bank (magic '<m>', expected PMSa)` | 1 |
| Bank size mismatch | stderr | `audio: sample bank size mismatch: header says <a> bytes, file has <b>` | 1 |
| Span past body | stderr | `audio: wave <i> span 0x<o>+<len> exceeds sample bank body <n>` | 1 |
| Wave index out of range | stderr | `audio: wave <n> out of range (0..<count>)` | 1 |
| Unsupported filter | stderr | `audio: wave <i> frame <f> uses filter <v> (supported 0-4)` | 1 |
| Output parent missing | stderr | `audio: output directory does not exist: <dir>` | 1 |
| Output inside a git tree | stderr | `audio: refusing to write inside a git working tree (<dir>); game audio must stay outside the repository` | 1 |
| Write failure | stderr | `audio: cannot write <path>: <io detail>` | 1 |
| Usage error | stderr | `usage: ... --audio <FILE.SND> --alist` / `--audio <FILE.SND> --wave <n> --out <FILE.wav>` | 1 |

## Acceptance scenarios

### Decodable path (active)

- [ ] **List:** a synthetic track with 3 waves lists `wave 0..2` with the
      expected offsets, byte lengths, frame counts, sample counts and durations.
- [ ] **Sample count:** a 77-frame one-shot wave reports `samples 2128`
      (`(77 - 1) * 28`); a 474-frame looped wave reports `samples 13272`
      (`474 * 28`). Both are the values the real `ALMA` bank shows for its
      wave 3 and `PLTR` wave 1.
- [ ] **Loop span:** a wave with flag 6 at frame 1 and flag 3 at the last frame
      reports `loop 28..<last>`; a wave with no such flags reports `loop -`.
- [ ] **Decode:** `--wave 0 --out <temp>/a.wav` writes a file whose header is
      `PCM16`, 1 channel, 44100 Hz, and whose `data` size is `samples * 2`.
- [ ] **Audible:** the written WAV is non-silent (peak > 0) and openable in a
      player; on the real install the sampled waves report 50-99.9% non-zero
      samples and peaks near full scale.
- [ ] **Stop flag:** a synthetic wave whose last frame has flag 7 decodes to
      one frame fewer and the stop frame's nibbles never reach the output.
- [ ] **Filters:** synthetic frames for filters 0-4 reproduce hand-computed
      sample sequences, including the `(115,-52)` and `(122,-60)` cases.
- [ ] **Falsifiable guard:** a synthetic wave offset of 24 (not a multiple of
      16) is refused; offsets out of order are refused; the first offset must
      be 0.

### Corrupt or unsupported (loud)

- [ ] `DNSb` magic, header size 28, a truncated table, `sum(zones)` off by one,
      `zone.waveIndex == waves`, `lowestKey > highestKey`, `PMSa` absent, body
      size off by one, a span past the body, wave index 999, and filter 5 each
      produce their own message and exit 1.
- [ ] Every refusal happens before any file is created; a refused run leaves no
      partial WAV behind.
- [ ] `--audio x.SND --wave 1` (missing `--out`) and `--audio x.SND --alist
      extra` are usage errors with exit 1.

### Legal and preserved modes

- [ ] An output path under a directory containing `.git` is refused with the
      git-working-tree message; the same path outside any git tree is accepted.
- [ ] Output parent that does not exist is refused with the missing-directory
      message.
- [ ] `git status --short` after the documented playtest shows no new game
      bytes, no `.wav`, and no install path.
- [ ] Every existing mode is unchanged: single DDS, gallery, `--bigfile`,
      `--textures` (with and without `--map`), `--mesh`, `--mesh-textured`,
      `--mesh-all`, launcher. No existing message or exit code changes.
- [ ] No new dependency appears in `Cargo.toml`.

### Documented no-go path (specified, not triggered)

- [ ] If the header size or counts do not reconcile on the target build, the
      slice SHALL stop, revert the claim, keep audio unread, and record the
      no-go with the `.OGG` fallback named as the next candidate.
- [ ] If every wave decodes to silence, the same stop applies; no alternative
      encoding or half-guessed filter set SHALL be shipped.
- [ ] If any protection, encryption or anti-tamper layer appears on this path,
      stop and report; nothing SHALL attempt to defeat it.

## Next step

See `design.md` for the probe ledger, exact field shapes, the encoding and
audio-output decisions, and the out-of-scope list; `tasks.md` for work units
and the review forecast.

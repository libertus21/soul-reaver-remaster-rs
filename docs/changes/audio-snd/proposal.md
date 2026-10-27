# Proposal: audio-snd

## Intent

Prove the paired Crystal Dynamics `.SND` + `.SMP` audio container is readable
end to end, and make exactly **one wave** from **one track** audible. The 31
pairs under `1\MUSIC\<track>\` have been unread since the project started
(README "What's missing", audio row).

This is deliberately not a music player. The container holds an instrument
bank (programs, zones, waves) plus 155 music sequences (`QSMa`). Rendering a
*sequence* needs a MIDI-like interpreter, SPU ADSR envelopes and pitch
resampling - a much larger slice. This slice stops at the sample bank: decode
one 4-bit ADPCM wave to PCM and hand it to the user as a WAV file at a path
they choose.

The investigation is decisive (evidence in `docs/notes/s9-audio-snd.md`):

- **31/31 pairs parse identically.** Magic `DNSa` / `PMSa`, header size 40,
  and a fixed body order: programs, zones, wave offsets, sequence offsets,
  label offsets, then sequences.
- **Every structural invariant holds with zero exceptions:** the program table
  sums to the declared zone count, every zone's wave index is in range, every
  wave offset is 16-aligned and strictly increasing from 0, every `.SMP` body
  is a whole number of 16-byte frames, and all 155 sequence offsets land
  exactly on a `QSMa` magic.
- **The sample bank is plain Sony 4-bit ADPCM.** 841 waves, 8,620,808 samples
  (195.5 s at 44.1 kHz). A standard PSX SPU decoder produces non-silent audio
  from every wave sampled (26 of 26, peak near full scale).
- **The format does not store a sample rate.** The community converter
  (unLoKable `desnd`, MIT) hard-codes 44100 when it wraps a wave in a VAG
  header. This slice does the same and says so in its output.

Verdict: **FOUND**. A single wave is decodable to PCM with documented,
reproducible steps.

## No-go condition (explicit)

This slice is a **no-go** - and audio stays unread - if any of these holds at
apply time on the target build:

1. A `.SND` file does not present header size 40 and the `soul-reaver` body
   order, or the counts do not reconcile (`sum(program zones) != zones`,
   `max(zone waveIndex) >= waves`).
2. Wave offsets are not 16-aligned and strictly increasing from 0, or the
   `.SMP` body length disagrees with the declared body size or with the last
   wave span.
3. Decoding the selected wave yields silence on every wave tried (peak 0 over
   the whole sample).

If any happens, stop, revert the claim, keep audio unread, and record the
result with the next candidate named: the remaster's own `.OGG` audio
(`EXTRA\MUSIC\*.OGG`, 79 files, and `TRACKS\<lang>\*.OGG`, 3,870 files). That
path is a different format, smaller to implement, and legitimate to name as
the fallback for "one track audible" - but it is not this change. No
alternative `SND` encoding may be guessed in code.

## Quick path

1. `cargo run -- --audio <MUSIC\ALMA\ALMA.SND> --alist` lists the track's 23
   waves with offset, byte length, frame count, sample count, duration at
   44.1 kHz and loop span. Exit 0.
2. `cargo run -- --audio <MUSIC\ALMA\ALMA.SND> --wave 17 --out <D:\tmp\alma17.wav>`
   decodes that wave, writes a 16-bit mono WAV outside the repo, and prints
   the sample count, rate (labelled as chosen), duration, peak and RMS.
3. Opening the WAV in any player is the audible check. A single instrument
   sample is short (0.05-0.7 s in this install), not a song.
4. `cargo test` covers container parsing, the ADPCM decoder, the WAV header and
   every refusal path on synthetic bytes. The audible result is a playtest.

## Details

| Topic | Decision |
|---|---|
| Container | `.SND` header (40 B) + programs + zones + wave/sequence/label offset tables + `QSMa` sequences |
| Sample bank | `.SMP`: `PMSa` + `u32` body size + body of 16-byte ADPCM frames |
| Slice target | One wave addressed by index; no sequence playback, no mixing |
| Encoding | Sony 4-bit ADPCM (28 samples per 16-byte frame, filters 0-4) |
| Sample rate | Not stored; 44100 Hz chosen by community convention, labelled "(chosen)" in output |
| Frame flags | `7` = stop (frame not decoded); `1`, `2`, `3`, `4`, `6` decode normally |
| Audio output | WAV file written to a user-supplied path; no audio device, no new dependency |
| Legal guard | Writer refuses an output path inside any git working tree, so the repo cannot receive game audio |
| New CLI surface | `--audio <x.SND> --alist` and `--audio <x.SND> --wave <n> --out <x.wav>` |
| Sibling bank | `.SMP` derived from the `.SND` path by extension; missing is a loud error naming the expected path |
| Existing contracts | No other mode changes: only a new first-argument arm is added |
| Tests | Synthetic SND/SMP bytes in temp dirs; no game bytes anywhere |
| Dependencies | None new |

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `src/snd.rs` | New | Container parse + validation, wave spans, ADPCM decode, flag handling |
| `src/wav.rs` | New | Canonical 44-byte RIFF/WAVE PCM16 mono writer |
| `src/main.rs` | Modified | `--audio` dispatch arm, usage text, git-tree output guard, CLI tests |
| `README.md` | Modified | Audio bullet: what is readable now, what is still missing |
| `docs/notes/s9-audio-snd.md` | New | Evidence ledger: shapes, invariants, flag census, sources |
| `docs/changes/audio-snd/*.md` | New | This planning bundle |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Sample rate is a guess, so pitch may be wrong | **High** (not stored in the format) | Label the rate "(chosen)" in output and record 44100 as the community convention; 22050 is the documented alternative if a playtest sounds an octave high |
| Looped waves render with a seam at the loop point | Medium (322/841 waves are looped) | Report the loop span in `--alist`; one-shot render is documented; loop-aware playback is out of scope |
| Author line count drifts past 400 | Medium | Work units capped; docs/tests split inside the same PR rather than a chain |
| A different build ships a different revision | Low | Header size must be 40 and counts must reconcile, else loud refusal; no-go path above |
| WAV export is mistaken for asset extraction | Medium (repo doctrine is "no extraction to disk") | The writer refuses paths inside a git tree, the path is mandatory and user-chosen, and the README states it is a scratch playtest artifact, never a distribution path |
| Reviewer expects a music player | Medium | Proposal and spec state explicitly that only one wave is audible, not a sequence |

## Rollback Plan

Revert the two new modules, the `--audio` arm in `main.rs`, and the README
bullet. Every other mode is untouched by construction (new first-argument arm
only). No data, state, or configuration is written anywhere except the WAV file
the user explicitly requests.

## Success Criteria

- [ ] `--audio <track> --alist` lists every wave with offset, bytes, frames,
      samples, duration and loop span; exit 0.
- [ ] `--audio <track> --wave <n> --out <path>` writes a WAV whose header
      declares PCM16/mono/44100 and whose data length is `samples * 2`.
- [ ] The written WAV is non-silent (peak > 0) and its sample count equals
      `frames * 28` minus stopped frames.
- [ ] Bad magic, wrong header size, inconsistent counts, out-of-range wave,
      missing `.SMP`, size mismatch and in-repo output path are six distinct
      loud errors with exit 1.
- [ ] Every pre-existing mode is byte-for-byte unchanged; `cargo test` green on
      synthetic bytes only.
- [ ] No game audio, decoded PCM, WAV or install path appears in the diff.

## Capabilities

### New Capabilities

- `audio-snd`: parse the Crystal Dynamics `.SND`/`.SMP` pair, enumerate waves,
  decode one 4-bit ADPCM wave to PCM, and emit a playable WAV.

### Modified Capabilities

- None. No existing capability changes behavior.

## Next step

`spec.md` (requirements, decode contract, message/exit contracts), then
`design.md` and `tasks.md`. Apply follows as a single PR.

# Archive: audio-snd — DONE (the bank is readable and one wave is audibly correct)

The paired `.SND` + `.SMP` instrument banks under `MUSIC/<track>/` are now
parsed and partially decoded. `--audio <FILE.SND> --alist` validates the 40-byte
`DNSa` header, the program/zone tables and the `u32` offset tables, then lists
every wave with its offset, bytes, frames, decoded samples, duration and loop
span. `--audio <FILE.SND> --wave <n> --out <FILE.wav>` decodes exactly one wave
from the sibling sample bank as Sony 4-bit ADPCM and writes one scratch mono
PCM16 file outside any git working tree. The user listened to the output and
confirmed it sounds natural at 44100 Hz. PR #35 is merged into `main`; the change
touched `src/snd.rs`, `src/wav.rs`, `src/main.rs`, `README.md` and the change
docs only.

## Quick path

1. `git checkout main && git pull` — HEAD is `f93be86` (merge of PR #35).
2. `cargo test` — 187/187 green on synthetic bytes only.
3. `cargo run -- --audio <ALMA.SND> --alist` — exit 0, 23 waves with the chosen
   rate printed as `(chosen)`.
4. `cargo run -- --audio <ALMA.SND> --wave 3 --out <%TEMP%\<name>.wav>` — exit 0,
   one-shot wave written outside the repo; delete it after listening.

## Accepted playtest evidence (verbatim)

| Item | Value |
|---|---|
| Audible verdict | PASS — the user listened to the generated WAVs and confirmed the tone sounds natural. 44100 Hz (the documented community convention, not stored in the file) is correct; the 22050 alternative was not needed. |
| `--alist` (`ALMA`) | 23 waves, exit 0; summary `programs 24 zones 54 waves 23 sequences 7 bank 262 sample-rate 44100 (chosen)`. |
| Wave 17 (looped) | 1109 frames / 31052 samples / 0.704 s / WAV 62,148 B / peak 23,940 / rms 5,933 / 99.9% nonzero. |
| Wave 3 (one-shot) | 77 frames / 2,128 samples / WAV 4,300 B / peak 31,586. |
| WAV header | Independently verified: `RIFF`/`WAVE`, PCM, mono, 44100, 16-bit. |
| Git-tree guard | Verified for both the root and the nested output path: exit 1, no file created. |
| Gates | 187/187 tests, zero warnings on an isolated cold build, legal clean, repository private. |

The `RMS`/`peak`/non-zero figures come from the decoder's own report; the audible
pass is what establishes that the chosen 44100 Hz rate is right by ear. The
scratch WAV stayed in `%TEMP%` and was never committed.

## Details

| Topic | Outcome |
|---|---|
| PR | [#35 — feat(audio): read the SND/SMP bank and decode one wave](https://github.com/libertus21/soul-reaver-remaster-rs/pull/35), state MERGED (`mergedAt` 2026-10-27T02:47:13Z), base `main`, head `feature/audio-snd`, 9 files, +2,571/−8, 5 commits. |
| Commits on `main` | `75f6d31` probe the `.SND`/`.SMP` pair and plan the slice (`docs/changes/audio-snd/*`, `docs/notes/s9-audio-snd.md`) → `fa2833c` Sony 4-bit ADPCM frame decoder → `9bca680` parse the container and wave tables → `091b6cf` `--audio` listing and one-wave WAV decode → `a576221` README → `f93be86` PR #35 merge. |
| Format invariants proven (31/31 tracks) | 40-byte `DNSa` header (`header_size == 40`); programs 8 B, zones 16 B, `u32` offset tables; `QSMa` sequences. 2,486/2,486 zone wave indices in range; 841 wave offsets 16-aligned and strictly increasing; 155/155 sequences land on a magic. Frame = shift/filter + flag + 14 nibble bytes = 28 samples; flag 7 = stop (that frame is not decoded); flag sets are exactly `{0,1,4,7}` one-shot (519 waves) or `{0,2,3,6}` looped (322 waves). |
| Sample rate | Not stored anywhere in `.SND`, the zone table or the `.SMP` header. 44100 Hz is the documented community convention; every message prints it as `(chosen)`. The audible playtest accepted it. |
| CLI behavior | `--audio <snd> --alist` accepts exactly 3 arguments and lists per-wave offset/bytes/frames/samples/duration/loop span. `--audio <snd> --wave <n> --out <wav>` accepts exactly 5, range-checks the index, resolves the sibling `.SMP` by extension, validates bank magic and `declared == file - 8` and `% 16 == 0`, then writes one 44-byte-header PCM16 mono file. Missing parent and any path inside a git working tree are refused before writing. Every existing mode is unchanged. |
| Deliberately not built | Sequence playback, mixing, loop-aware rendering, multi-track playback, other audio banks, and any new dependency. The wave is treated as a complete VAD stream on its own; sequences need an interpreter (see below). |
| Process warnings (recorded) | (1) ~1,491 authored code lines against a ~380 forecast and the 800-line review budget — the second consecutive large forecast miss; the user reviewed and merged it as a single PR. (2) `--alist` requires the sibling `.SMP` even for listing: a documented refusal, not a bug, because the bank is opened before any wave is described. (3) 322 of 841 waves are looped, but rendering is one-shot; the loop span is reported only. |
| Branch cleanup | Deleted only branches whose work is fully in `main`; see the report below. |
| Follow-ups | (1) Loop-aware rendering for the 322 looped waves. (2) Sample rate was decided by ear; a stored or derivable rate would remove the `(chosen)` caveat. (3) Multi-track playback and mixing. (4) The other audio banks and the remaster's own `.OGG` libraries are a separate scope. (5) The 155 `QSMa` sequences need a MIDI-like interpreter, SPU ADSR envelopes and pitch resampling. |
| Legal | Format and behavior facts only. No game audio, no PCM, no WAV bytes, no absolute game paths, no offsets or hashes from the install. Tests use synthetic bytes in temp directories; the real pair is a read-only playtest input. |

## Branch cleanup report

Containment was checked with `git merge-base --is-ancestor <ref> main`, and for
rebased duplicates with `git diff <tip> main -- <paths its commit touched>` (an
empty diff means the same content already landed).

| Branch | State | Action |
|---|---|---|
| `feature/audio-snd` | ancestor of `main` | Deleted local + remote. |
| `feature/audio-snd-plan` | not an ancestor (rebased copy), content identical to `main` | Deleted local + remote. |
| `docs/archive-slot-name` | ancestor of `main` | Deleted local + remote. |
| `feature/texture-gallery-plan` | not an ancestor, content identical to `main` | Deleted local + remote. |
| `feature/big-header-plan` | content differs from `main` (`tasks.md`) | Kept. |
| `feature/big-slots-plan` | content differs from `main` | Kept. |
| `feature/slot-name-plan` | content differs from `main` | Kept. |
| `origin/feature/f4-launcher-cli-plan` | not contained | Kept. |
| `origin/dev`, `origin/dev-2-0`, `origin/dev-3.0` | long-lived development branches (`dev` and `dev-3.0` are contained) | Kept deliberately. |

## Checklist

- [x] PR #35 MERGED and `main` contains all slice commits (HEAD `f93be86`)
- [x] Audible playtest evidence recorded verbatim (PASS, chosen 44100 Hz, both probe waves, header, guard, gates)
- [x] Format invariants stated as proven across all 31 tracks
- [x] Process warnings recorded without softening (forecast miss, `.SMP` requirement for `--alist`, one-shot rendering of looped waves)
- [x] Contained branches deleted after containment checks; uncontained branches kept with the reason stated
- [x] README "What works" carries the playtest-accepted audio line and "What's missing" points at animation next
- [x] No game audio, PCM, WAV bytes, absolute game paths, offsets or hashes added by this archive

## Next step

Scope animation for the existing mesh path: the loose `.SRM` slice parses one
mesh as a flat-shaded solid, and skeleton/animation is the next open area. Audio
has no blocker at this level; loop-aware rendering and the sequence interpreter
are separate, currently unscoped slices.

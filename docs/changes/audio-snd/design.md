# Design: audio-snd

Read the Crystal Dynamics `.SND` + `.SMP` pair far enough to enumerate its
waves and decode one of them to PCM, then write that PCM to a WAV outside the
repository. The container is a fixed-order instrument bank: a 40-byte header,
a program table, a zone table, three `u32` offset tables, then `QSMa`
sequences. The sample bank is an unheadered array of Sony 4-bit ADPCM frames,
sliced by the `.SND` wave offsets.

Nothing here renders a sequence. That is the whole point of the slice boundary.

## Technical approach

`src/snd.rs` owns the container format only:
- parse the header and the three offset tables from one byte slice;
- validate the structural invariants before exposing anything;
- turn wave offsets into spans against the `.SMP` body length;
- scan a span's frame flags for stop and loop markers (no decoding needed);
- decode one span to `Vec<i16>` with the PlayStation SPU ADPCM rules.

It takes bytes and returns values; no file I/O, no `std::fs`, no paths.

`src/wav.rs` owns bytes out:
- build the canonical 44-byte RIFF/WAVE PCM16 mono image;
- write it in one call;
- refuse an output path inside a git working tree.

`src/main.rs` owns I/O and surfaces:
- one new first-argument arm, `--audio`, with the two shapes in `spec.md`;
- derive the sibling `.SMP` from the `.SND` path;
- print the `--alist` table or the decode report;
- keep every existing arm untouched.

## Investigation box

Three passes, one sitting, ~90 minutes of agent work, no install mutation:

| Pass | What | Budget spent |
|---|---|---|
| 1 | Inventory + byte probe of `1\MUSIC` | 62 files enumerated, 31 pairs fully parsed, 5 headers hex-dumped |
| 2 | Community spec (unLoKable wiki + `desnd` source) | 3 pages/source files read, MIT confirmed |
| 3 | Feasibility verdict + decode trial | 26 waves decoded across 2 tracks, 841 waves flag-censused |

Read-only throughout: every probe opened files with read access only, all
scratch output went to `%TEMP%`, and no byte of game data was copied into the
repository.

## Probe ledger

| # | Target | Range read | Bytes | Question answered |
|---|---|---|---|---|
| P1 | `1\MUSIC` tree | names, sizes only | 62 files | Pair count, naming, per-track layout |
| P2 | `ALMA.SND` | `0x000..0x0BF` | 192 | Header field order, magic, table start |
| P3 | `ALMA.SMP` | `0x000..0x05F` | 96 | `PMSa` + declared body size; body origin |
| P4 | `PLTR.SND` | `0x000..0x0BF` | 192 | Same header shape on a 5.6 KB track |
| P5 | `PLTR.SMP` | `0x000..0x05F` | 96 | Same bank header on a small bank |
| P6 | `UWTR.SND` | `0x000..0x0BF` | 192 | Whether the 784-byte outlier shares the prefix |
| P7 | `ALMA.SND` + `.SMP` | whole files | 24,824 + 176,264 | Full structural parse, spans, sequence magic |
| P8 | All 31 `.SND` (+8 B of each `.SMP`, then full `.SMP` for body size) | whole | 657,084 + 4,934,728 | Invariants across the install |
| P9 | `PLTR` + `ALMA` bodies | wave spans only | 18,608 + 176,256 | ADPCM decode sanity: peak, RMS, non-zero % |
| P10 | All 31 bodies | byte 1 of every 16-byte frame | 308,405 frames | Flag census, loop spans, stop behaviour |
| P11 | unLoKable wiki + `desnd/src/main.rs` | text | - | Documented field names, codec, sample rate convention |

P6 answered a real question: `UWTR.SND` is 784 bytes and `PLTR.SND` is 5,632
bytes, yet both carry the same 40-byte header and the same table arithmetic
(4 programs, 6 zones, 3 waves, 2 sequences, 3 labels). Short tracks are short
in the sequence area, not in the tables. That is why the slice can address a
wave without touching sequences.

## Exact `.SND` layout (soul-reaver revision)

Offsets are from the start of the file. All integers are little-endian.

| Offset | Type | Field | Observed |
|---|---|---|---|
| `0x00` | ASCII x4 | Magic | `DNSa` in 31/31 |
| `0x04` | `u32` | Header size | `40` in 31/31 |
| `0x08` | `u32` | Bank version | `0x0106` (262) in 31/31; encoding not interpreted |
| `0x0C` | `u32` | Programs | 4..56 |
| `0x10` | `u32` | Zones | 6..156 |
| `0x14` | `u32` | Waves | 3..40 |
| `0x18` | `u32` | Sequences | 2..8 |
| `0x1C` | `u32` | Labels | 3..58 |
| `0x20` | `u32` | Reverb mode | `4` on `ALMA`; semantics UNVERIFIED, unused |
| `0x24` | `u32` | Reverb depth | `6000` on `ALMA`; semantics UNVERIFIED, unused |

Body order, starting at `header_size`:

| Table | Entry | Size | Content |
|---|---|---|---|
| Programs | `programs` | 8 B | `u16` zone count, `u16` first zone, `u8` volume, `u8` pan, `u16` unused |
| Zones | `zones` | 16 B | 10 x `u8` (`priority`, `parent program`, `volume`, `pan`, `root key`, `pitch finetune`, `lowest key`, `highest key`, `mode`, `max bend`), `u16` ADSR1, `u16` ADSR2, `u16` wave index |
| Wave offsets | `waves` | 4 B | `u32`, relative to the `.SMP` body |
| Sequence offsets | `sequences` | 4 B | `u32`, relative to the first sequence |
| Label offsets | `labels` | 4 B | `u32`, base UNKNOWN, unused by this slice |
| Sequences | - | - | `QSMa` (155/155 in this install); no `QESa` observed |

The upstream wiki notes that the header size may need rounding up to a multiple
of 4 before it is used as the body offset. At 40 it is already aligned, so this
slice rejects any other value instead of implementing the rounding for
revisions it cannot test.

## Exact `.SMP` layout

| Offset | Type | Field |
|---|---|---|
| `0x00` | ASCII x4 | `PMSa` (present in 31/31) |
| `0x04` | `u32` | Body size == `file_length - 8` in 31/31 |
| `0x08` | - | Body: whole 16-byte ADPCM frames |

Body length is a multiple of 16 in 31/31, so the body is a frame array with no
tail. A wave is a contiguous run of frames:

```text
span(i)      = [offset(i), offset(i+1))            // last wave: [offset(last), body_size)
wave_bytes   = 16 * frames
wave_samples = 28 * frames - (28 if the span contains a stop frame else 0)
```

## ADPCM frame

The codec is Sony's `SONY_4BIT_ADPCM` (VAG), the same one used by PS1 banks.

| Byte | Meaning |
|---|---|
| 0 | low nibble = shift (0..12), high nibble = filter (0..4) |
| 1 | flag: 0 none, 1 end, 2 loop, 3 loop end, 4 start, 6 loop start, 7 stop |
| 2..15 | 14 bytes of ADPCM = 28 nibbles, low nibble first |

```text
v  = sign_extend_4bit(nibble)          // -8..7
s  = (v << 12) >> shift
s += (prev1 * c1 + prev2 * c2) >> 6    // per filter, see table
out = clamp(s, -32768, 32767)
```

| Filter | `c1` | `c2` |
|---|---|---|
| 0 | 0 | 0 |
| 1 | 60 | 0 |
| 2 | 115 | -52 |
| 3 | 98 | -55 |
| 4 | 122 | -60 |

Flag `7` is honoured exactly as documented: that frame is not decoded and the
wave ends there. Flags `1`, `2`, `3`, `4`, `6` decode normally; `2/3/6` are
reported as loop metadata only, because this slice renders a single shot.

## Verified invariants (31/31 pairs, zero exceptions)

| Invariant | Result |
|---|---|
| Magic `DNSa`, header size 40 | 31/31 |
| `sum(program.num_zones) == header.zones`, no window overflow | 31/31, 1,057 programs / 2,486 zones |
| `zone.parent_program < programs` | 2,486/2,486 |
| `zone.lowest_key <= zone.highest_key` | 2,486/2,486 |
| `max(zone.wave_index) < waves` | 31/31, 841 waves |
| Wave offsets strictly increasing, first == 0, all `% 16 == 0` | 841/841 |
| Last wave span ends exactly at the body end | 31/31 |
| `.SMP` magic `PMSa`, declared body == `file - 8`, `% 16 == 0` | 31/31 |
| Sequence offsets land on `QSMa` | 155/155 |
| Wave flag sets are exactly `{0,1,4,7}` or `{0,2,3,6}` | 841/841 (519 / 322) |
| Loop start precedes loop end | 322/322 |

## Sample-encoding decision

**4-bit ADPCM with filters 0-4, 28 samples per 16-byte frame.** Not PCM16.

Reasons, in order of weight:

1. The byte pattern is the documented Sony `SONY_4BIT_ADPCM` stream: 1 shift/
   filter byte, 1 flag byte, 14 data bytes, 28 samples.
2. All 841 waves decode to non-silent audio with the standard coefficient set;
   the 26 waves sampled in detail show peaks between 21,577 and 31,586 and
   50.6-99.9% non-zero samples - the profile of real instrument samples, not
   noise or misaligned data.
3. The flag census is deterministic: two patterns only, 519 one-shot and 322
   looped, no anomalies. Misinterpreted stream layout does not produce that.
4. A PCM16 alternative (as `MUS`/`SAM` uses on PC) would show no 16-byte frame
   periodicity, no shift/filter bytes and no in-band flags.

## Sample-rate decision

**44100 Hz, marked as chosen.** The container stores no rate: the header has no
candidate field, the zone table has only a root key and a finetune byte, and
the `.SMP` header is just magic + length.

The community converter writes `0x0000AC44` (44100) into every VAG header it
emits for these banks (`desnd/src/main.rs`, MIT), i.e. the community treats
44100 as the canonical playback rate rather than deriving it. This slice does
the same and prints `@ 44100 Hz (chosen)` everywhere so nobody mistakes it for
parsed metadata. Duration therefore always means "duration at 44100".

If a playtest sounds exactly an octave low, 22050 is the documented alternative
and a one-line change; that is recorded as an open question, not implemented.

## Audio-output decision

**Write a WAV file to a path the user supplies. No audio device, no new
dependency.**

The user asked for the smallest thing that makes one sample audible. On
Windows, a 44-byte RIFF header plus raw `i16` samples is 60 lines of code and
is readable by every player on the machine. Adding an audio backend instead
means pulling `cpal` (device enumeration, callback, ring buffer) or `rodio`
(which drags `cpal` plus `symphonia`), both of which fail in headless CI, need
a device at runtime, and cannot be tested on synthetic bytes.

| Option | Adds | License | Testable on synthetic bytes | Fails without an audio device |
|---|---|---|---|---|
| WAV file (chosen) | 0 crates, ~60 lines | none (own code) | Yes | No |
| `cpal` | ~20 crates | Apache-2.0 | No (needs a device) | Yes |
| `rodio` | ~40 crates | MIT OR Apache-2.0 | No | Yes |
| `wgpu` abuse / visual waveform | none new | - | No | No, but it is not audible |

The legal cost of the WAV option is handled by construction, not by policy:

- the output path is mandatory, never derived, never defaulted;
- the writer refuses any path whose parent chain contains a `.git` entry, so
  this repository cannot receive game audio even by accident;
- the README calls it a scratch playtest artifact, not an export feature;
- nothing else in the repo gains a "save" path.

This is a deliberate, documented exception to the project's "no export" stance,
scoped to one scratch file outside the tree because the user asked for it
explicitly and because "audible" cannot be verified any other way.

## Module shape

```rust
// src/snd.rs
pub const HEADER_BYTES: usize = 40;
pub const PROGRAM_BYTES: usize = 8;
pub const ZONE_BYTES: usize = 16;
pub const FRAME_BYTES: usize = 16;
pub const SAMPLES_PER_FRAME: usize = 28;
pub const PLAYBACK_RATE: u32 = 44_100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program { pub zones: u16, pub first_zone: u16, pub volume: u8, pub pan: u8 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    pub priority: u8, pub program: u8, pub volume: u8, pub pan: u8,
    pub root_key: u8, pub finetune: u8, pub lowest_key: u8, pub highest_key: u8,
    pub mode: u8, pub max_bend: u8, pub adsr1: u16, pub adsr2: u16, pub wave: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveSpan { pub index: usize, pub offset: u32, pub len: usize }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveInfo {
    pub span: WaveSpan,
    pub frames: usize,
    pub samples: usize,
    pub stopped: bool,
    pub loop_start: Option<u32>,   // first sample of the flag-6 frame
    pub loop_end: Option<u32>,     // first sample of the flag-3 frame; region is [start, end)
}

pub struct Bank<'a> { /* snd bytes + parsed tables */ }

impl<'a> Bank<'a> {
    pub fn parse(snd: &'a [u8]) -> Result<Bank<'a>, SndError>;
    pub fn waves(&self) -> usize;
    pub fn zones_for_wave(&self, wave: u16) -> usize;
    pub fn spans(&self, body_len: usize) -> Result<Vec<WaveSpan>, SndError>;
    pub fn info(&self, body: &[u8], index: usize) -> Result<WaveInfo, SndError>;
    pub fn decode(&self, body: &[u8], index: usize) -> Result<Vec<i16>, SndError>;
}

pub fn decode_frame(frame: &[u8], prev: (i32, i32), out: &mut Vec<i16>) -> Result<u8, SndError>;

#[derive(Debug)]
pub enum SndError { /* one variant per message contract row */ }
```

```rust
// src/wav.rs
pub const WAV_HEADER_BYTES: usize = 44;

pub fn mono_pcm16(samples: &[i16], rate: u32) -> Vec<u8>;
pub fn write_mono_pcm16(path: &Path, samples: &[i16], rate: u32) -> io::Result<()>;
/// Refuses a path whose parent chain contains `.git` (file or directory).
pub fn ensure_outside_git_tree(path: &Path) -> Result<(), String>;
```

`Bank` borrows the caller's bytes so CLI I/O stays in `main.rs`. `decode_frame`
exists so filter arithmetic is unit-testable without a whole file.

## Data flow

```text
--audio <x.SND> --alist:
  read .SND -> Bank::parse (magic, header size, tables, count checks)
            -> read sibling .SMP header (8 bytes) -> body_len + size check
            -> Bank::spans(body_len) (monotonic, aligned, in range)
            -> per wave: info(body) -> frames, samples, loop span (flags only)
            -> summary line + one line per wave -> exit 0

--audio <x.SND> --wave <n> --out <path>:
  Bank::parse -> .SMP header + full body read -> spans
  -> index in range -> info(body) -> decode(body, n) (stop flag honoured)
  -> wav::ensure_outside_git_tree(path), parent exists
  -> wav::write_mono_pcm16(path, &pcm, 44100)
  -> report line (samples, rate chosen, duration, peak, rms, non-zero %) -> exit 0
```

## Evidence ledger

| # | Hypothesis | Verdict | Evidence that decided it |
|---|---|---|---|
| 1 | `.SND` is a header plus fixed-order tables | SUPPORTED | 31/31 parse with the same order; 155/155 sequence offsets land on `QSMa` |
| 2 | The header counts are authoritative | SUPPORTED | Program zone counts sum to the declared zone count in 31/31 |
| 3 | Wave offsets are `.SMP` body offsets | SUPPORTED | First is 0, strictly increasing, last span ends exactly at the body end in 31/31 |
| 4 | Wave offsets might be absolute SPU addresses (wiki caveat) | RULED OUT here | Every bank starts at 0 and lands inside its own body; no absolute-address residue |
| 5 | `.SMP` body is raw Sony 4-bit ADPCM | SUPPORTED | Standard decoder yields peaks 21k-31k and 50-99% non-zero on 26/26 waves |
| 6 | A frame is shift/filter + 14 data bytes starting at index 1 | RULED OUT | That layout hides the flag byte; moving data to 2..15 exposes exactly two flag patterns |
| 7 | The sample rate is stored somewhere | RULED OUT | No field fits; the community tool hard-codes 44100 instead of reading one |
| 8 | Zone wave indices stay in range | SUPPORTED | 2,486/2,486, max index is `waves - 1` in every bank |
| 9 | Sequences exist and are reachable | SUPPORTED | 155 `QSMa` hits, one per declared sequence, zero misses |
| 10 | `QESa` (single-track) sequences appear in this install | NONE FOUND | Zero hits across all 31 files |
| 11 | Label offsets are needed to locate waves | RULED OUT | Waves come from the wave table alone; labels are unused and their base is UNKNOWN |
| 12 | The waves are encrypted or obfuscated | NONE FOUND | Plain integers parse, and a public codec decodes the body to plausible PCM |
| 13 | Program entries describe contiguous zone windows | SUPPORTED | `first_zone + zones` never exceeds the zone table; sums match exactly |
| 14 | Reverb fields at `0x20`/`0x24` are understood | UNKNOWN | Read as 4 and 6000 on `ALMA`; not needed, not interpreted |

## Falsifiability

The model makes five independent predictions, all checked:

1. Table arithmetic must reconcile: `sum(program zones) == zones` and
   `max(zone wave) < waves`. It does, 31/31.
2. Wave spans must tile the `.SMP` body exactly. Last span == body length, 31/31.
3. Sequence offsets must land on a sequence magic. 155/155 do.
4. Decoding must produce audio, not noise. Peaks near full scale, 26/26 sampled.
5. Frame flags must form a small deterministic set. Exactly two patterns,
   841/841.

If an apply-time probe contradicts (1), (2) or (5), the no-go path in
`proposal.md` applies and no guessing is added.

## Contract preservation boundary

| Surface | Status |
|---|---|
| Single DDS, gallery, `--bigfile`, `--mesh*`, launcher modes | Unchanged; new first-argument arm only |
| `--textures --tlist/--tshow`, with or without `--map` | Unchanged |
| Usage text | One added pair of lines; existing lines untouched |
| `Cargo.toml` | Unchanged |
| Files written by the tool | None before this slice; exactly one user-specified WAV after it |

## Decisions

| Decision | Choice | Alternatives | Rationale |
|---|---|---|---|
| Wave selector | Explicit `--wave <n>` | Auto-pick longest or loudest | No hidden policy; `--alist` makes the choice informed |
| Bank lookup | Sibling `.SMP` by extension | `--smp <path>` flag | The pair is the format; one fewer flag; missing bank names the expected path |
| Output | WAV file, path mandatory | Live playback via `cpal`/`rodio` | Zero dependencies, testable, works headless, license-free |
| Output guard | Refuse any path in a git tree | Advisory README only | Enforceable and testable; the repo cannot receive game audio |
| Rate | 44100, labelled chosen | 22050, or derive from root key | Matches the community tool; the label prevents a false claim of parsing |
| Stop flag | Honour it (drop that frame) | Decode all frames | Documented behaviour; keeps sample counts exact |
| Loop flags | Metadata only | Loop-aware render | A single shot is the slice; loops need playback |
| Header size | Must be 40 | Implement the 4-byte rounding for other revisions | Only one revision is observable; untestable paths are refused, not guessed |
| Zones | Parsed and validated, not played | Skip zone table entirely | Validation is what proves the container model; it costs ~15 lines |
| Labels | Skipped | Parse the label table | Base is UNKNOWN and nothing needs it |
| Encoder/decoder library | Own 60-line ADPCM decoder | A VAG crate | Filter set is 5 coefficients; a dependency would be larger than the code |

## Out of scope

- Sequence playback (`QSMa`), tempo, track muting, MIDI export.
- ADSR envelopes, pitch/finetune resampling, volume/pan mixing, reverb.
- Loop-aware playback and looping WAV (`smpl` chunk).
- VAB/SF2/DLS export, or any conversion other than the one scratch WAV.
- `.MUS`/`.SAM` (SR2/Defiance), `.MUL`, Dreamcast `.SMP` codecs.
- The remaster's `.OGG` audio.
- Batch export, playlists, or any UI.
- SR2 data, protection, DRM.

## Testing strategy

| Layer | What | Approach |
|---|---|---|
| Unit (parse) | Magic, header size, truncation, count reconciliation, key ranges, offset monotonicity/alignment, span bounds | Synthetic `.SND`/`.SMP` buffers built in memory |
| Unit (decode) | Filters 0-4 vectors, sign extension, shift, clamp, stop flag, loop markers, sample count formula | 1-3 hand-built 16-byte frames |
| Unit (wav) | 44-byte header fields, sizes, little-endian samples, odd-length guard | Pure function, byte compare |
| CLI | `--alist` shape, decode report, six refusal paths, usage shapes, git guard, missing parent | `run_audio` on synthetic pairs in `tempfile` dirs |
| Regression | Every existing mode | Existing suite stays green |
| Manual | One real track: `--alist`, then `--wave <n> --out <%TEMP% file>`, listen | Playtest note with track, wave, samples, duration, what was heard |
| Legal | `git status --short`, no `.wav`, no game bytes, no install path | Pre-commit check |

## Threat matrix

| Boundary | Risk | Handling |
|---|---|---|
| File write | Writing into the repository | Refused when any ancestor has `.git`; path is mandatory and user-supplied |
| File write | Clobbering an unrelated file | Parent must exist; the path is explicit; no defaults, no expansion |
| Parsing | Attacker-shaped counts causing huge allocation | Counts validated against table bounds and file length before any `Vec` is sized from them |
| Parsing | Integer overflow on offsets | `u32` offsets widened to `usize` and checked with `checked_add` against the body length |
| Format strings / shell | None | No `format!` with user data as a format, no subprocesses |
| Network | None | No network code |

## Migration / rollout

No data migration, no state, no config. One new CLI arm and two new modules.
Reverting the commits restores an audio-free tool with every existing mode
byte-for-byte intact.

## Open questions

- [ ] Is 44100 the true playback rate? The format does not say; a playtest can
      only tell us if it is grossly wrong (octave). Recorded, not guessed.
- [ ] What is the label table for? Base and semantics UNKNOWN; unused.
- [ ] Do the reverb fields at `0x20`/`0x24` mean what the wiki says? Unused.
- [ ] Does any retail build ship the `prototype` or `gex` revision here? Header
      size 40 in 31/31; anything else is refused loudly rather than guessed.
- [ ] Do looped waves need a seamless render for musical use? Out of scope; the
      loop span is reported so a future slice can act on it.

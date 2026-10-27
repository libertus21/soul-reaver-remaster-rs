# s9 - `.SND` / `.SMP` audio format probe (2026-10-26)

Read-only probe of `1\MUSIC\<track>\*.SND` + `*.SMP` in the observed SR1
install, done to scope the `audio-snd` slice. Nothing was extracted, copied or
modified; scratch output stayed in `%TEMP%`. Format and behaviour facts only -
no addresses, no listings, no game data in this note.

Sources used (nothing vendored, nothing copied):

| Source | URL | License | What it gave |
|---|---|---|---|
| unLoKable (SalsaGal) | https://github.com/SalsaGal/unLoKable | MIT (`LICENSE.txt`) | Suite for Crystal Dynamics audio; `desnd` reads `.SND`+`.SMP` |
| Wiki: SND and SMP | https://github.com/SalsaGal/unLoKable/wiki/File-Format:-SND-and-SMP | MIT (same repo) | Header field names/order, program/zone/wave/sequence/label layout, offset bases |
| Wiki: VAG | https://github.com/SalsaGal/unLoKable/wiki/File-Format:-VAG | MIT (same repo) | 16-byte frame layout, flag values, 28 samples per frame |
| `desnd/src/main.rs` | https://github.com/SalsaGal/unLoKable/blob/master/desnd/src/main.rs | MIT | VAG header it emits: magic `VAGp`, version 3, body length = wave span, sample rate hard-coded `0x0000AC44` = 44100 |

## Inventory

- 30 directories, 31 pairs, 62 files, 5,591,812 bytes total
  (657,084 `.SND` + 4,934,728 `.SMP`).
- Pair naming: `<TRACK>\<TRACK>.SND` + `<TRACK>\<TRACK>.SMP`; stem always matches
  the directory name, upper-case.
- Two directories hold the odd sizes: `PLTR` (`.SND` 5,632) and `UWTR`
  (`.SND` 784) - both with the same 4 program / 6 zone / 3 wave / 2 sequence /
  3 label shape.
- No `.OGG` in `MUSIC`. The remaster's own audio lives elsewhere in the install
  (`EXTRA\MUSIC\*.OGG` 79 files; `TRACKS\<lang>\*.OGG` 3,870 files across 8
  languages). Those are a different format and are not part of this probe.
- No protection or container wrapper: both file types parse as plain
  little-endian structures.

| Track | SND | SMP | p | z | w | s | l |
|---|---:|---:|---:|---:|---:|---:|---:|
| ALMA | 24,824 | 176,264 | 24 | 54 | 23 | 7 | 50 |
| ALSP | 21,292 | 179,544 | 56 | 98 | 26 | 6 | 42 |
| CHMA | 26,504 | 147,128 | 14 | 38 | 29 | 4 | 34 |
| CHSP | 24,716 | 145,960 | 49 | 70 | 21 | 5 | 35 |
| CTMA | 22,544 | 178,856 | 18 | 72 | 23 | 6 | 56 |
| CTSP | 24,288 | 146,888 | 54 | 86 | 20 | 6 | 58 |
| KNMA | 26,596 | 173,816 | 14 | 38 | 31 | 4 | 34 |
| KNSP | 24,724 | 145,960 | 49 | 70 | 21 | 5 | 35 |
| LIMBO1 | 17,908 | 108,792 | 32 | 42 | 22 | 2 | 43 |
| MLMA | 25,508 | 155,832 | 26 | 83 | 33 | 7 | 37 |
| MLSP | 29,960 | 184,008 | 54 | 156 | 40 | 6 | 48 |
| PLMA | 14,804 | 180,472 | 25 | 61 | 31 | 4 | 42 |
| PLSP | 19,788 | 184,008 | 54 | 156 | 40 | 5 | 40 |
| PLTR | 5,632 | 18,616 | 4 | 6 | 3 | 2 | 3 |
| ROMA | 16,372 | 163,800 | 19 | 47 | 26 | 5 | 40 |
| ROSP | 15,520 | 184,008 | 54 | 156 | 40 | 4 | 40 |
| SKMA | 24,732 | 174,248 | 15 | 58 | 34 | 8 | 45 |
| SKSP | 25,188 | 180,648 | 54 | 89 | 21 | 6 | 43 |
| SNMA | 16,372 | 163,800 | 19 | 47 | 26 | 5 | 40 |
| SNSP | 15,520 | 184,008 | 54 | 156 | 40 | 4 | 40 |
| STMA | 21,880 | 174,248 | 15 | 58 | 34 | 7 | 42 |
| STSP | 20,844 | 180,648 | 54 | 89 | 21 | 5 | 41 |
| UCMA | 33,328 | 176,392 | 24 | 86 | 30 | 5 | 33 |
| UCSP | 31,020 | 172,456 | 54 | 89 | 19 | 4 | 31 |
| URMA | 21,880 | 174,248 | 15 | 58 | 34 | 7 | 42 |
| URSP | 20,844 | 180,648 | 54 | 89 | 21 | 5 | 41 |
| UWMA | 16,384 | 163,800 | 19 | 47 | 26 | 5 | 39 |
| UWSP | 15,532 | 184,008 | 54 | 156 | 40 | 4 | 40 |
| UWTR | 784 | 18,616 | 4 | 6 | 3 | 2 | 3 |
| WCMA | 25,828 | 149,000 | 22 | 69 | 23 | 5 | 40 |
| WCSP | 25,968 | 184,008 | 54 | 156 | 40 | 5 | 37 |

Totals: 1,057 programs, 2,486 zones, 841 waves, 155 sequences.

**Sample banks are deduplicated.** Six SHA-256 groups repeat, so 31 tracks
reference only 18 distinct `.SMP` bodies (all 31 `.SND` files are unique):

| Shared body | Tracks |
|---|---|
| one 184,008-byte bank | MLSP, PLSP, ROSP, SNSP, UWSP, WCSP |
| one 163,800-byte bank | ROMA, SNMA, UWMA |
| one 174,248-byte bank | SKMA, STMA, URMA |
| one 180,648-byte bank | SKSP, STSP, URSP |
| one 145,960-byte bank | CHSP, KNSP |
| one 18,616-byte bank | PLTR, UWTR |

So wave offsets are per-`.SND`, while the bytes they point at are shared. Any
future caching must key on the `.SMP` digest, not the track name.

## `.SND` header (40 bytes, `DNSa`)

| Offset | Type | Field | Observed |
|---|---|---|---|
| `0x00` | ASCII x4 | magic | `DNSa` (31/31) |
| `0x04` | `u32` | header size | 40 (31/31) |
| `0x08` | `u32` | bank version | `0x0106` = 262 (31/31); encoding not interpreted |
| `0x0C` | `u32` | programs | 4..56 |
| `0x10` | `u32` | zones | 6..156 |
| `0x14` | `u32` | waves | 3..40 |
| `0x18` | `u32` | sequences | 2..8 |
| `0x1C` | `u32` | labels | 3..58 |
| `0x20` | `u32` | reverb mode | `4` on ALMA; semantics UNVERIFIED |
| `0x24` | `u32` | reverb depth | `6000` on ALMA; semantics UNVERIFIED |

Body order at `header_size`: programs (8 B each), zones (16 B each), wave
offsets (`u32` x waves), sequence offsets (`u32` x sequences), label offsets
(`u32` x labels), then sequences.

- Program: `u16` zone count, `u16` first zone, `u8` volume (120 or 127 seen),
  `u8` pan (64 seen), `u16` unused.
- Zone: `u8` priority, parent program, volume, pan, root key, finetune, lowest
  key, highest key, mode, max bend; `u16` ADSR1, `u16` ADSR2; `u16` wave index.
- The wiki's note that header size may need rounding up to a multiple of 4 is a
  no-op here: 40 is already aligned.

## `.SMP` (`PMSa`)

`PMSa` + `u32` body size == `file_length - 8` (31/31), then the body: a flat
array of 16-byte ADPCM frames, length a multiple of 16 (31/31).

## Verified invariants

| Invariant | Result |
|---|---|
| `sum(program.num_zones) == header.zones`, no window overflow | 31/31 |
| `zone.parent_program < programs` | 2,486/2,486 |
| `zone.lowest_key <= zone.highest_key` | 2,486/2,486 |
| `max(zone.wave_index) < waves` | 31/31 |
| Wave offsets strictly increasing, first == 0, all `% 16 == 0` | 841/841 |
| Last wave span ends exactly at the body end | 31/31 |
| Sequence offset + first-sequence base lands on `QSMa` | 155/155 |
| `QESa` sequence magic present | 0 of 155 |
| Frame flag sets | 841/841 split exactly into `{0,1,4,7}` or `{0,2,3,6}` |

## Sample bank decode

Codec: Sony 4-bit ADPCM (VAG). Frame = shift/filter byte, flag byte, 14 ADPCM
bytes = 28 samples. Filters 0-4 with coefficients `(0,0) (60,0) (115,-52)
(98,-55) (122,-60)` / 64.

Flag census over all 308,405 frames / 841 waves:

| Kind | Flags | Waves | Samples |
|---|---|---|---|
| One-shot | `{0, 1, 4, 7}` | 519 | `(frames - 1) * 28` - the flag-7 frame is not decoded |
| Looped | `{0, 2, 3, 6}` | 322 | `frames * 28` |

No third pattern, no malformed loop order. Total sample material:
8,620,808 samples = 195.5 s at 44.1 kHz.

Decode spot checks (standard decoder, 44.1 kHz):

| Track | Wave | Kind | Bytes | Frames | Samples | Duration | Peak | RMS | Non-zero |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ALMA | 17 | looped | 17,744 | 1,109 | 31,052 | 0.704 s | 23,940 | 5,933 | 99.9% |
| ALMA | 0 | looped | 10,464 | 654 | 18,312 | 0.415 s | 24,576 | 3,320 | 93.5% |
| ALMA | 3 | one-shot | 1,232 | 77 | 2,128 | 0.048 s | 31,586 | 4,015 | 97.2% |
| PLTR | 1 | looped | 7,584 | 474 | 13,272 | 0.301 s | 29,041 | 8,893 | 99.4% |
| PLTR | 0 | one-shot | 2,784 | 174 | 4,844 | 0.110 s | 28,672 | 3,732 | 80.3% |

26 of 26 waves decoded across `ALMA` and `PLTR` were non-silent. `ALMA` wave 0
opens with 32 zero bytes (two silent frames) before audio, which is sample
content, not a header.

## The one thing the format does not tell us: sample rate

There is no rate field in `.SND`, in the zone table, or in the `.SMP` header.
The upstream converter does not derive one either: it writes `0x0000AC44`
(44100) into every VAG header it generates. 44100 is therefore the documented
community convention, not a parsed fact, and 22050 remains a plausible
alternative (it would double every duration in the table above). Any slice that
prints a duration must label the rate as chosen.

## Ruled out

| Hypothesis | Verdict | Why |
|---|---|---|
| `.SMP` body is PCM16 like PC `SAM` banks | RULED OUT | 16-byte periodicity with shift/filter bytes and in-band flags; PCM16 would show neither |
| Wave offsets are absolute SPU addresses | RULED OUT here | Every bank starts at 0 and lands inside its own body |
| A frame is shift/filter at byte 0 and data at bytes 1..14 | RULED OUT | Hides the flag byte; moving data to 2..15 makes the flag set collapse to exactly two patterns |
| Sample rate is stored in the header | RULED OUT | No candidate field; community tool hard-codes it |
| Label offsets are needed to find waves | RULED OUT | Waves come from the wave table; label base is UNKNOWN and unused |
| Sequences are needed for a first audible result | RULED OUT | A wave is a complete VAG stream on its own; sequences need an interpreter |
| The banks are encrypted or packed | NONE FOUND | Plain integers parse; a public codec decodes to plausible PCM |
| `MUSIC` tracks are duplicated across areas | PARTLY | `.SMP` bodies repeat (18 distinct of 31); `.SND` descriptors never do |

## Still unknown

- Label offset table base and semantics (50 entries on `ALMA`, small values).
- Reverb mode/depth semantics (`4` / `6000` on `ALMA`).
- Whether 44100 or 22050 is the intended playback rate.
- Zone `mode`, `max bend`, ADSR1/ADSR2 encoding beyond "SPU format" per the
  wiki - not needed to decode a sample, needed for sequence playback.
- Whether any other build ships the `prototype` or `gex` header revisions.

## Legal

Read-only probing of an owned single-player install. Names, sizes, field
layouts and statistical decode results only - no audio, no sample data, no
extracted files in this note or in the repository. The planned slice writes at
most one scratch WAV to a user-chosen path outside any git working tree.

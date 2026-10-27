//! Crystal Dynamics `.SND` + `.SMP` audio bank (SR1).
//!
//! The `.SND` container is a 40-byte `DNSa` header, a program table
//! (8 B each), a zone table (16 B each), then `u32` wave, sequence and
//! label offset tables, then `QSMa` sequences. The paired `.SMP` is a
//! `PMSa` header plus a flat array of 16-byte Sony ADPCM frames sliced
//! by the wave offsets. Only waves are addressed here; sequences are
//! counted, never interpreted.
//!
//! Clean-room shape facts only; see `docs/notes/s9-audio-snd.md` and
//! `docs/changes/audio-snd/design.md`. Tests use synthetic bytes only.
//!
//! A frame is 16 bytes: byte 0 = shift (low nibble) + filter (high
//! nibble), byte 1 = flag, bytes 2..15 = 28 4-bit samples (low nibble
//! first). Flag 7 is a stop marker: that frame is not decoded.

use std::fmt;

pub const HEADER_BYTES: usize = 40;
pub const PROGRAM_BYTES: usize = 8;
pub const ZONE_BYTES: usize = 16;
pub const FRAME_BYTES: usize = 16;
pub const SAMPLES_PER_FRAME: usize = 28;

/// The container stores no sample rate; 44100 Hz is the community
/// convention (labelled as chosen in every printed duration).
pub const PLAYBACK_RATE: u32 = 44_100;

pub const SND_MAGIC: &[u8; 4] = b"DNSa";
pub const SMP_MAGIC: &[u8; 4] = b"PMSa";

/// PSX SPU prediction coefficients, divided by 64 during decode.
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

/// Program table entry: a window of zones inside the zone table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // every field is parsed; only the window is consumed here
pub struct Program {
    pub zones: u16,
    pub first_zone: u16,
    pub volume: u8,
    pub pan: u8,
}

/// Zone table entry. Parsed and validated as container evidence; the
/// voice fields are not surfaced by this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Zone {
    pub priority: u8,
    pub program: u8,
    pub volume: u8,
    pub pan: u8,
    pub root_key: u8,
    pub finetune: u8,
    pub lowest_key: u8,
    pub highest_key: u8,
    pub mode: u8,
    pub max_bend: u8,
    pub adsr1: u16,
    pub adsr2: u16,
    pub wave: u16,
}

/// One wave's byte range inside the `.SMP` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveSpan {
    pub index: usize,
    pub offset: u32,
    pub len: usize,
}

/// Flag-derived facts about one wave. `samples` excludes the stop frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveInfo {
    pub span: WaveSpan,
    pub frames: usize,
    pub samples: usize,
    pub stopped: bool,
    pub loop_start: Option<u32>,
    pub loop_end: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SndError {
    NotSnd {
        magic: [u8; 4],
    },
    HeaderSize(u32),
    Truncated {
        what: &'static str,
        need: usize,
        has: usize,
    },
    ZoneSum {
        summed: u32,
        declared: u32,
    },
    ZoneProgram {
        zone: usize,
        program: u8,
        programs: usize,
    },
    ZoneWave {
        zone: usize,
        wave: u16,
        waves: usize,
    },
    ZoneKeys {
        zone: usize,
        lowest: u8,
        highest: u8,
    },
    WaveOffset {
        index: usize,
        value: u32,
        previous: u32,
    },
    NotBank {
        magic: [u8; 4],
    },
    BankSize {
        declared: u32,
        actual: usize,
    },
    BankNotFramed {
        bytes: usize,
    },
    SpanPastBody {
        wave: usize,
        offset: u32,
        len: usize,
        body: usize,
    },
    WaveOutOfRange {
        wave: usize,
        waves: usize,
    },
    UnsupportedFilter {
        wave: Option<usize>,
        frame: Option<usize>,
        filter: u8,
    },
}

impl fmt::Display for SndError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SndError::NotSnd { magic } => write!(
                f,
                "not a .SND container (magic '{}', expected DNSa)",
                magic_text(magic)
            ),
            SndError::HeaderSize(size) => write!(
                f,
                "unsupported .SND header size {size} (expected 40, soul-reaver revision)"
            ),
            SndError::Truncated { what, need, has } => {
                write!(f, "truncated {what}: needs {need} bytes, has {has}")
            }
            SndError::ZoneSum { summed, declared } => write!(
                f,
                "inconsistent counts: programs declare {summed} zones, header says {declared}"
            ),
            SndError::ZoneProgram {
                zone,
                program,
                programs,
            } => write!(
                f,
                "zone {zone} references program {program}, but only {programs} programs exist"
            ),
            SndError::ZoneWave { zone, wave, waves } => write!(
                f,
                "zone {zone} references wave {wave}, but only {waves} waves exist"
            ),
            SndError::ZoneKeys {
                zone,
                lowest,
                highest,
            } => write!(
                f,
                "zone {zone} has lowest key {lowest} above highest key {highest}"
            ),
            SndError::WaveOffset {
                index,
                value,
                previous,
            } => write!(
                f,
                "wave offset {index} is {value}, expected a multiple of 16 above {previous}"
            ),
            SndError::NotBank { magic } => write!(
                f,
                "not an .SMP sample bank (magic '{}', expected PMSa)",
                magic_text(magic)
            ),
            SndError::BankSize { declared, actual } => write!(
                f,
                "sample bank size mismatch: header says {declared} bytes, file has {actual}"
            ),
            SndError::BankNotFramed { bytes } => {
                write!(f, "sample bank body of {bytes} bytes is not a multiple of 16")
            }
            SndError::SpanPastBody {
                wave,
                offset,
                len,
                body,
            } => write!(
                f,
                "wave {wave} span 0x{offset:X}+{len} exceeds sample bank body {body}"
            ),
            SndError::WaveOutOfRange { wave, waves } => {
                write!(f, "wave {wave} out of range (0..{waves})")
            }
            SndError::UnsupportedFilter { wave, frame, filter } => match (wave, frame) {
                (Some(w), Some(i)) => write!(
                    f,
                    "wave {w} frame {i} uses filter {filter} (supported 0-4)"
                ),
                _ => write!(f, "frame uses filter {filter} (supported 0-4)"),
            },
        }
    }
}

impl std::error::Error for SndError {}

/// A parsed `.SND` container. Owns the validated tables; the `.SMP`
/// body stays with the caller and is passed to every wave operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank {
    bank_version: u32,
    programs: Vec<Program>,
    zones: Vec<Zone>,
    wave_offsets: Vec<u32>,
    sequence_count: usize,
}

impl Bank {
    /// Parse and validate one `.SND` image. Nothing is guessed: any
    /// shape outside the soul-reaver revision is a loud error.
    pub fn parse(snd: &[u8]) -> Result<Bank, SndError> {
        let len = snd.len();
        if len < SND_MAGIC.len() {
            return Err(SndError::Truncated {
                what: ".SND",
                need: SND_MAGIC.len(),
                has: len,
            });
        }
        let magic: [u8; 4] = snd[0..4].try_into().expect("checked length");
        if &magic != SND_MAGIC {
            return Err(SndError::NotSnd { magic });
        }
        if len < HEADER_BYTES {
            return Err(SndError::Truncated {
                what: ".SND",
                need: HEADER_BYTES,
                has: len,
            });
        }
        let header_size = le_u32(snd, 4);
        if header_size != HEADER_BYTES as u32 {
            return Err(SndError::HeaderSize(header_size));
        }
        let bank_version = le_u32(snd, 8);
        let program_count = le_u32(snd, 0x0C) as usize;
        let zone_count = le_u32(snd, 0x10) as usize;
        let wave_count = le_u32(snd, 0x14) as usize;
        let sequence_count = le_u32(snd, 0x18) as usize;
        let label_count = le_u32(snd, 0x1C) as usize;

        let programs_at = HEADER_BYTES;
        let zones_at = table_end(programs_at, program_count, PROGRAM_BYTES, len)?;
        let waves_at = table_end(zones_at, zone_count, ZONE_BYTES, len)?;
        let sequences_at = table_end(waves_at, wave_count, 4, len)?;
        let labels_at = table_end(sequences_at, sequence_count, 4, len)?;
        table_end(labels_at, label_count, 4, len)?;

        let programs: Vec<Program> = (0..program_count)
            .map(|index| {
                let at = programs_at + index * PROGRAM_BYTES;
                Program {
                    zones: le_u16(snd, at),
                    first_zone: le_u16(snd, at + 2),
                    volume: snd[at + 4],
                    pan: snd[at + 5],
                }
            })
            .collect();

        let zones: Vec<Zone> = (0..zone_count)
            .map(|index| {
                let at = zones_at + index * ZONE_BYTES;
                Zone {
                    priority: snd[at],
                    program: snd[at + 1],
                    volume: snd[at + 2],
                    pan: snd[at + 3],
                    root_key: snd[at + 4],
                    finetune: snd[at + 5],
                    lowest_key: snd[at + 6],
                    highest_key: snd[at + 7],
                    mode: snd[at + 8],
                    max_bend: snd[at + 9],
                    adsr1: le_u16(snd, at + 10),
                    adsr2: le_u16(snd, at + 12),
                    wave: le_u16(snd, at + 14),
                }
            })
            .collect();

        let mut summed = 0u32;
        for program in &programs {
            let first = usize::from(program.first_zone);
            let count = usize::from(program.zones);
            let fits = first.checked_add(count).is_some_and(|end| end <= zone_count);
            if !fits {
                return Err(SndError::ZoneSum {
                    summed,
                    declared: zone_count as u32,
                });
            }
            summed += u32::from(program.zones);
        }
        if summed as usize != zone_count {
            return Err(SndError::ZoneSum {
                summed,
                declared: zone_count as u32,
            });
        }

        for (index, zone) in zones.iter().enumerate() {
            if usize::from(zone.program) >= program_count {
                return Err(SndError::ZoneProgram {
                    zone: index,
                    program: zone.program,
                    programs: program_count,
                });
            }
            if usize::from(zone.wave) >= wave_count {
                return Err(SndError::ZoneWave {
                    zone: index,
                    wave: zone.wave,
                    waves: wave_count,
                });
            }
            if zone.lowest_key > zone.highest_key {
                return Err(SndError::ZoneKeys {
                    zone: index,
                    lowest: zone.lowest_key,
                    highest: zone.highest_key,
                });
            }
        }

        let wave_offsets: Vec<u32> = (0..wave_count)
            .map(|index| le_u32(snd, waves_at + index * 4))
            .collect();
        let mut previous = 0u32;
        for (index, &value) in wave_offsets.iter().enumerate() {
            let ordered = if index == 0 {
                value == 0
            } else {
                value % FRAME_BYTES as u32 == 0 && value > previous
            };
            if !ordered {
                return Err(SndError::WaveOffset {
                    index,
                    value,
                    previous,
                });
            }
            previous = value;
        }

        Ok(Bank {
            bank_version,
            programs,
            zones,
            wave_offsets,
            sequence_count,
        })
    }

    pub fn programs(&self) -> usize {
        self.programs.len()
    }

    pub fn zones(&self) -> usize {
        self.zones.len()
    }

    pub fn waves(&self) -> usize {
        self.wave_offsets.len()
    }

    pub fn sequences(&self) -> usize {
        self.sequence_count
    }

    pub fn bank_version(&self) -> u32 {
        self.bank_version
    }

    /// Turn the wave offsets into byte spans against one `.SMP` body.
    /// Offsets must tile the body: strictly increasing, 16-aligned, and
    /// the last span must end exactly at `body_len`.
    pub fn spans(&self, body_len: usize) -> Result<Vec<WaveSpan>, SndError> {
        let mut spans = Vec::with_capacity(self.wave_offsets.len());
        for (index, &offset) in self.wave_offsets.iter().enumerate() {
            let end = match self.wave_offsets.get(index + 1) {
                Some(&next) => next as usize,
                None => body_len,
            };
            let start = offset as usize;
            let len = end.saturating_sub(start);
            if end <= start || end > body_len || len % FRAME_BYTES != 0 {
                return Err(SndError::SpanPastBody {
                    wave: index,
                    offset,
                    len,
                    body: body_len,
                });
            }
            spans.push(WaveSpan { index, offset, len });
        }
        Ok(spans)
    }

    /// Scan a wave's frame flags for stop and loop markers. No decoding.
    pub fn info(&self, body: &[u8], index: usize) -> Result<WaveInfo, SndError> {
        let spans = self.spans(body.len())?;
        let span = *spans.get(index).ok_or(SndError::WaveOutOfRange {
            wave: index,
            waves: spans.len(),
        })?;
        let start = span.offset as usize;
        let frames = span.len / FRAME_BYTES;
        let mut stopped = false;
        let mut decoded = 0usize;
        let mut loop_start = None;
        let mut loop_end = None;
        for frame in 0..frames {
            let flag = body[start + frame * FRAME_BYTES + 1];
            let sample_at = (frame * SAMPLES_PER_FRAME) as u32;
            match flag {
                6 if loop_start.is_none() => loop_start = Some(sample_at),
                3 if loop_end.is_none() => loop_end = Some(sample_at),
                7 => {
                    stopped = true;
                    break;
                }
                _ => {}
            }
            decoded += 1;
        }
        Ok(WaveInfo {
            span,
            frames,
            samples: decoded * SAMPLES_PER_FRAME,
            stopped,
            loop_start,
            loop_end,
        })
    }

    /// Decode one wave to PCM. Flag 7 stops the wave before its samples.
    pub fn decode(&self, body: &[u8], index: usize) -> Result<Vec<i16>, SndError> {
        let info = self.info(body, index)?;
        let start = info.span.offset as usize;
        let mut out = Vec::with_capacity(info.samples);
        let mut previous = (0i32, 0i32);
        for frame in 0..info.frames {
            let at = start + frame * FRAME_BYTES;
            let flag = decode_frame(&body[at..at + FRAME_BYTES], previous, &mut out)
                .map_err(|error| attach_wave(error, index, frame))?;
            if flag == 7 {
                break;
            }
            let last = out.len();
            previous = (i32::from(out[last - 1]), i32::from(out[last - 2]));
        }
        Ok(out)
    }
}

/// Validate a `.SMP` header and return its frame body.
pub fn bank_body(smp: &[u8]) -> Result<&[u8], SndError> {
    if smp.len() < 8 {
        return Err(SndError::Truncated {
            what: ".SMP",
            need: 8,
            has: smp.len(),
        });
    }
    let magic: [u8; 4] = smp[0..4].try_into().expect("checked length");
    if &magic != SMP_MAGIC {
        return Err(SndError::NotBank { magic });
    }
    let declared = le_u32(smp, 4);
    let body = &smp[8..];
    if declared as usize != body.len() {
        return Err(SndError::BankSize {
            declared,
            actual: body.len(),
        });
    }
    if body.len() % FRAME_BYTES != 0 {
        return Err(SndError::BankNotFramed { bytes: body.len() });
    }
    Ok(body)
}

fn attach_wave(error: SndError, wave: usize, frame: usize) -> SndError {
    match error {
        SndError::UnsupportedFilter { filter, .. } => SndError::UnsupportedFilter {
            wave: Some(wave),
            frame: Some(frame),
            filter,
        },
        other => other,
    }
}

fn table_end(start: usize, count: usize, stride: usize, len: usize) -> Result<usize, SndError> {
    let size = count.checked_mul(stride).ok_or(SndError::Truncated {
        what: ".SND",
        need: usize::MAX,
        has: len,
    })?;
    let end = start.checked_add(size).ok_or(SndError::Truncated {
        what: ".SND",
        need: usize::MAX,
        has: len,
    })?;
    if end > len {
        return Err(SndError::Truncated {
            what: ".SND",
            need: end,
            has: len,
        });
    }
    Ok(end)
}

fn magic_text(magic: &[u8; 4]) -> String {
    magic
        .iter()
        .map(|&byte| {
            if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            }
        })
        .collect()
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("checked table bounds"))
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("checked table bounds"))
}

/// Decode one 16-byte ADPCM frame into `out`.
///
/// Returns the frame's flag byte. A flag-7 frame is a stop marker and is
/// never decoded, so `out` stays untouched. `prev` is the last decoded
/// sample pair `(previous, previous previous)` used by filters 1-4.
pub fn decode_frame(
    frame: &[u8],
    prev: (i32, i32),
    out: &mut Vec<i16>,
) -> Result<u8, SndError> {
    if frame.len() < FRAME_BYTES {
        return Err(SndError::Truncated {
            what: "ADPCM frame",
            need: FRAME_BYTES,
            has: frame.len(),
        });
    }
    let flag = frame[1];
    if flag == 7 {
        return Ok(7);
    }
    let shift = i32::from(frame[0] & 0x0F);
    let filter = usize::from(frame[0] >> 4);
    let (c1, c2) = *FILTERS.get(filter).ok_or(SndError::UnsupportedFilter {
        wave: None,
        frame: None,
        filter: filter as u8,
    })?;
    let (mut prev1, mut prev2) = prev;
    for index in 0..SAMPLES_PER_FRAME {
        let byte = frame[2 + index / 2];
        let nibble = if index % 2 == 0 { byte & 0x0F } else { byte >> 4 };
        let value = if nibble < 8 {
            i32::from(nibble)
        } else {
            i32::from(nibble) - 16
        };
        let mut sample = (value << 12) >> shift;
        sample += (prev1 * c1 + prev2 * c2) >> 6;
        let clamped = sample.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        out.push(clamped);
        prev2 = prev1;
        prev1 = i32::from(clamped);
    }
    Ok(flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(filter: u8, shift: u8, flag: u8, nibbles: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; FRAME_BYTES];
        bytes[0] = (filter << 4) | (shift & 0x0F);
        bytes[1] = flag;
        for (index, nibble) in nibbles.iter().enumerate() {
            let byte = 2 + index / 2;
            if index % 2 == 0 {
                bytes[byte] = (bytes[byte] & 0xF0) | (nibble & 0x0F);
            } else {
                bytes[byte] = (bytes[byte] & 0x0F) | ((nibble & 0x0F) << 4);
            }
        }
        bytes
    }

    fn decode(bytes: &[u8]) -> Vec<i16> {
        let mut out = Vec::new();
        decode_frame(bytes, (0, 0), &mut out).expect("synthetic frame must decode");
        out
    }

    #[test]
    fn filter_zero_maps_nibbles_with_sign_extension() {
        assert_eq!(&decode(&frame(0, 0, 0, &[0, 7, 8]))[..3], [0, 28_672, -32_768]);
    }

    #[test]
    fn shift_divides_the_twelve_bit_value() {
        assert_eq!(&decode(&frame(0, 12, 0, &[7, 8]))[..2], [7, -8]);
    }

    #[test]
    fn accumulator_clamps_to_i16_range() {
        assert_eq!(&decode(&frame(1, 0, 0, &[7, 7]))[..2], [28_672, 32_767]);
        assert_eq!(&decode(&frame(1, 0, 0, &[8, 8]))[..2], [-32_768, -32_768]);
    }

    #[test]
    fn filters_one_to_four_use_their_published_coefficients() {
        assert_eq!(&decode(&frame(1, 0, 0, &[1, 0, 0]))[..3], [4_096, 3_840, 3_600]);
        assert_eq!(&decode(&frame(2, 0, 0, &[1, 0, 0]))[..3], [4_096, 7_360, 9_897]);
        assert_eq!(&decode(&frame(3, 0, 0, &[1, 0, 0]))[..3], [4_096, 6_272, 6_084]);
        assert_eq!(&decode(&frame(4, 0, 0, &[1, 0, 0]))[..3], [4_096, 7_808, 11_044]);
    }

    #[test]
    fn previous_samples_feed_the_next_filtered_frame() {
        let mut out = Vec::new();
        decode_frame(&frame(1, 0, 0, &[1, 0, 0]), (0, 0), &mut out).expect("frame one");
        let prev = (i32::from(out[2]), i32::from(out[1]));
        decode_frame(&frame(1, 0, 0, &[0, 0, 0]), prev, &mut out).expect("frame two");
        assert_eq!(out[3], 3_375);
    }

    #[test]
    fn stop_flag_is_reported_and_never_decoded() {
        let mut out = Vec::new();
        let flag = decode_frame(&frame(0, 0, 7, &[7, 7]), (0, 0), &mut out)
            .expect("stop frame is not an error");
        assert_eq!(flag, 7);
        assert!(out.is_empty());
    }

    #[test]
    fn filter_above_four_is_a_loud_error() {
        assert_eq!(
            decode_frame(&frame(5, 0, 0, &[0]), (0, 0), &mut Vec::new()),
            Err(SndError::UnsupportedFilter {
                wave: None,
                frame: None,
                filter: 5
            })
        );
    }

    /// Synthetic `.SND`: 40-byte header, program/zone tables, wave and
    /// sequence offsets, no labels. Values are invented.
    fn snd_bytes(
        programs: &[(u16, u16)],
        zones: &[(u8, u16, u8, u8)],
        waves: &[u32],
        sequences: usize,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SND_MAGIC);
        bytes.extend_from_slice(&(HEADER_BYTES as u32).to_le_bytes());
        bytes.extend_from_slice(&0x0106u32.to_le_bytes());
        bytes.extend_from_slice(&(programs.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(zones.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(waves.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(sequences as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for &(count, first) in programs {
            bytes.extend_from_slice(&count.to_le_bytes());
            bytes.extend_from_slice(&first.to_le_bytes());
            bytes.extend_from_slice(&[0u8; 4]);
        }
        for &(program, wave, lowest, highest) in zones {
            let mut zone = [0u8; ZONE_BYTES];
            zone[1] = program;
            zone[6] = lowest;
            zone[7] = highest;
            zone[14..16].copy_from_slice(&wave.to_le_bytes());
            bytes.extend_from_slice(&zone);
        }
        for &offset in waves {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        for _ in 0..sequences {
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
        bytes
    }

    /// A frame whose 28 samples are all nibble 7 with filter 0, shift 0.
    fn tone(flag: u8) -> Vec<u8> {
        frame(0, 0, flag, &[7; SAMPLES_PER_FRAME])
    }

    fn bank(programs: &[(u16, u16)], zones: &[(u8, u16, u8, u8)], waves: &[u32]) -> Bank {
        Bank::parse(&snd_bytes(programs, zones, waves, 0)).expect("synthetic bank must parse")
    }

    #[test]
    fn parse_reads_tables_counts_and_spans() {
        let snd = snd_bytes(
            &[(1, 0), (1, 1)],
            &[(0, 0, 0, 127), (1, 1, 0, 127)],
            &[0, 16, 32],
            1,
        );
        let parsed = Bank::parse(&snd).expect("synthetic container must parse");
        assert_eq!(
            (
                parsed.programs(),
                parsed.zones(),
                parsed.waves(),
                parsed.sequences()
            ),
            (2, 2, 3, 1)
        );
        assert_eq!(parsed.bank_version(), 0x0106);
        assert_eq!(
            parsed.spans(48).expect("spans must tile the body"),
            vec![
                WaveSpan { index: 0, offset: 0, len: 16 },
                WaveSpan { index: 1, offset: 16, len: 16 },
                WaveSpan { index: 2, offset: 32, len: 16 },
            ]
        );
    }

    #[test]
    fn parse_refuses_a_foreign_magic_and_header_size() {
        let mut snd = snd_bytes(&[(1, 0)], &[(0, 0, 0, 127)], &[0], 0);
        snd[0..4].copy_from_slice(b"DNSb");
        assert_eq!(
            Bank::parse(&snd),
            Err(SndError::NotSnd { magic: *b"DNSb" })
        );

        let mut snd = snd_bytes(&[(1, 0)], &[(0, 0, 0, 127)], &[0], 0);
        snd[4..8].copy_from_slice(&28u32.to_le_bytes());
        assert_eq!(Bank::parse(&snd), Err(SndError::HeaderSize(28)));
    }

    #[test]
    fn parse_refuses_a_truncated_table() {
        let mut snd = snd_bytes(&[(1, 0)], &[(0, 0, 0, 127)], &[0], 1);
        snd.truncate(44);
        assert!(matches!(Bank::parse(&snd), Err(SndError::Truncated { .. })));
    }

    #[test]
    fn parse_refuses_counts_that_do_not_reconcile() {
        let snd = snd_bytes(
            &[(1, 0), (1, 1)],
            &[(0, 0, 0, 127), (1, 1, 0, 127), (0, 0, 0, 127)],
            &[0],
            0,
        );
        assert_eq!(
            Bank::parse(&snd),
            Err(SndError::ZoneSum {
                summed: 2,
                declared: 3
            })
        );
    }

    #[test]
    fn parse_refuses_zone_program_wave_and_key_ranges() {
        assert_eq!(
            Bank::parse(&snd_bytes(&[(1, 0)], &[(2, 0, 0, 127)], &[0], 0)),
            Err(SndError::ZoneProgram {
                zone: 0,
                program: 2,
                programs: 1
            })
        );
        assert_eq!(
            Bank::parse(&snd_bytes(&[(1, 0)], &[(0, 3, 0, 127)], &[0, 16, 32], 0)),
            Err(SndError::ZoneWave {
                zone: 0,
                wave: 3,
                waves: 3
            })
        );
        assert_eq!(
            Bank::parse(&snd_bytes(&[(1, 0)], &[(0, 0, 127, 0)], &[0], 0)),
            Err(SndError::ZoneKeys {
                zone: 0,
                lowest: 127,
                highest: 0
            })
        );
    }

    #[test]
    fn parse_refuses_unaligned_unordered_or_nonzero_wave_offsets() {
        assert_eq!(
            Bank::parse(&snd_bytes(&[(1, 0)], &[(0, 0, 0, 127)], &[0, 24, 32], 0)),
            Err(SndError::WaveOffset {
                index: 1,
                value: 24,
                previous: 0
            })
        );
        assert_eq!(
            Bank::parse(&snd_bytes(&[(1, 0)], &[(0, 0, 0, 127)], &[0, 32, 16], 0)),
            Err(SndError::WaveOffset {
                index: 2,
                value: 16,
                previous: 32
            })
        );
        assert_eq!(
            Bank::parse(&snd_bytes(&[(1, 0)], &[(0, 0, 0, 127)], &[16, 32], 0)),
            Err(SndError::WaveOffset {
                index: 0,
                value: 16,
                previous: 0
            })
        );
    }

    #[test]
    fn spans_must_tile_the_body_exactly() {
        let bank = bank(&[(1, 0)], &[(0, 0, 0, 127)], &[0, 16, 32]);
        assert_eq!(
            bank.spans(16),
            Err(SndError::SpanPastBody {
                wave: 1,
                offset: 16,
                len: 16,
                body: 16
            })
        );
        assert_eq!(
            bank.spans(40),
            Err(SndError::SpanPastBody {
                wave: 2,
                offset: 32,
                len: 8,
                body: 40
            })
        );
    }

    #[test]
    fn info_reports_loop_span_and_sample_counts() {
        let bank = bank(&[(1, 0)], &[(0, 0, 0, 127)], &[0]);
        let looped = [tone(6), tone(0), tone(3)].concat();
        let info = bank.info(&looped, 0).expect("looped wave must scan");
        assert_eq!(
            (info.frames, info.samples, info.stopped),
            (3, 84, false)
        );
        assert_eq!((info.loop_start, info.loop_end), (Some(0), Some(56)));

        let one_shot = [tone(0), tone(7)].concat();
        let info = bank.info(&one_shot, 0).expect("one-shot wave must scan");
        assert_eq!(
            (info.frames, info.samples, info.stopped),
            (2, 28, true)
        );
        assert_eq!((info.loop_start, info.loop_end), (None, None));
    }

    #[test]
    fn decode_honours_the_stop_frame() {
        let bank = bank(&[(1, 0)], &[(0, 0, 0, 127)], &[0]);
        let body = [tone(0), tone(7)].concat();
        let pcm = bank.decode(&body, 0).expect("one-shot wave must decode");
        assert_eq!(pcm.len(), SAMPLES_PER_FRAME);
        assert!(pcm.iter().all(|&sample| sample == 28_672));
    }

    #[test]
    fn decode_names_the_wave_and_frame_on_a_bad_filter() {
        let bank = bank(&[(1, 0)], &[(0, 0, 0, 127)], &[0]);
        let mut bad = tone(0);
        bad[0] = 0x50;
        assert_eq!(
            bank.decode(&bad, 0),
            Err(SndError::UnsupportedFilter {
                wave: Some(0),
                frame: Some(0),
                filter: 5
            })
        );
    }

    #[test]
    fn wave_operations_reject_an_out_of_range_index() {
        let bank = bank(&[(1, 0)], &[(0, 0, 0, 127)], &[0]);
        let body = tone(0);
        assert_eq!(
            bank.info(&body, 3),
            Err(SndError::WaveOutOfRange { wave: 3, waves: 1 })
        );
    }

    #[test]
    fn bank_body_validates_magic_size_and_frame_alignment() {
        let mut smp = Vec::new();
        smp.extend_from_slice(SMP_MAGIC);
        smp.extend_from_slice(&32u32.to_le_bytes());
        smp.extend_from_slice(&[0u8; 32]);
        assert_eq!(bank_body(&smp).expect("synthetic bank must parse").len(), 32);

        let mut wrong_magic = smp.clone();
        wrong_magic[0..4].copy_from_slice(b"PMSb");
        assert_eq!(
            bank_body(&wrong_magic),
            Err(SndError::NotBank { magic: *b"PMSb" })
        );

        let mut wrong_size = smp.clone();
        wrong_size[4..8].copy_from_slice(&31u32.to_le_bytes());
        assert_eq!(
            bank_body(&wrong_size),
            Err(SndError::BankSize {
                declared: 31,
                actual: 32
            })
        );

        let mut unaligned = smp;
        unaligned[4..8].copy_from_slice(&24u32.to_le_bytes());
        unaligned.truncate(8 + 24);
        assert_eq!(bank_body(&unaligned), Err(SndError::BankNotFramed { bytes: 24 }));
    }
}

//! Crystal Dynamics `.SND` + `.SMP` audio bank: Sony 4-bit ADPCM decode.
//!
//! Clean-room shape facts only; see `docs/notes/s9-audio-snd.md` and
//! `docs/changes/audio-snd/design.md`. Tests use synthetic bytes only.
//!
//! A frame is 16 bytes: byte 0 = shift (low nibble) + filter (high
//! nibble), byte 1 = flag, bytes 2..15 = 28 4-bit samples (low nibble
//! first). Flag 7 is a stop marker: that frame is not decoded.

use std::fmt;

pub const FRAME_BYTES: usize = 16;
pub const SAMPLES_PER_FRAME: usize = 28;

/// PSX SPU prediction coefficients, divided by 64 during decode.
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

#[derive(Debug, PartialEq, Eq)]
pub enum SndError {
    Truncated {
        what: &'static str,
        need: usize,
        has: usize,
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
            SndError::Truncated { what, need, has } => {
                write!(f, "truncated {what}: needs {need} bytes, has {has}")
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
}

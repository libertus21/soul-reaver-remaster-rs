//! Fixed-slot payload pixel format (`big-decompressor`): raw, headerless
//! 256 x 256 little-endian A1R5G5B5. Verified against sampled slots in all
//! four `.BIG` containers (`docs/notes/s7-big-slot-payload-format.md`):
//! 131072 bytes = 256 * 256 * 2, bit 15 alpha, bits 14-10 red, 9-5 green,
//! 4-0 blue, row-major top-down with no padding. The payload is NOT
//! compressed, so this is a pixel-format converter, not a decompressor.
//! Pure: no I/O, no dependencies. Synthetic bytes in tests only.

use std::fmt;

/// Verified slot width in pixels.
pub const WIDTH: usize = 256;

/// Verified slot height in pixels.
pub const HEIGHT: usize = 256;

/// Bytes per pixel: one little-endian `u16`.
pub const PIXEL_BYTES: usize = 2;

/// Exact payload length of one slot: 131072 bytes, no header.
pub const SLOT_BYTES: usize = WIDTH * HEIGHT * PIXEL_BYTES;

/// Why a payload could not be decoded. Length is the only failure: the
/// format has no header or version field to reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotDecodeError {
    WrongLength { found: usize, expected: usize },
}

impl fmt::Display for SlotDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotDecodeError::WrongLength { found, expected } => write!(
                f,
                "payload length {found} does not match {WIDTH}x{HEIGHT} A1R5G5B5 ({expected} bytes)"
            ),
        }
    }
}

impl std::error::Error for SlotDecodeError {}

/// 5-bit channel -> 8-bit by bit replication (exact at 0 and 31).
pub fn expand5(v: u8) -> u8 {
    (v << 3) | (v >> 2)
}

/// Pure: little-endian A1R5G5B5, row-major, top-down, no padding.
/// Returns `WIDTH * HEIGHT * 4` RGBA8 bytes; never pads or truncates.
pub fn decode_a1r5g5b5(bytes: &[u8]) -> Result<Vec<u8>, SlotDecodeError> {
    if bytes.len() != SLOT_BYTES {
        return Err(SlotDecodeError::WrongLength {
            found: bytes.len(),
            expected: SLOT_BYTES,
        });
    }
    let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * 4);
    for pixel in bytes.chunks_exact(PIXEL_BYTES) {
        let v = u16::from_le_bytes([pixel[0], pixel[1]]);
        rgba.push(expand5(((v >> 10) & 0x1F) as u8));
        rgba.push(expand5(((v >> 5) & 0x1F) as u8));
        rgba.push(expand5((v & 0x1F) as u8));
        rgba.push(if v & 0x8000 != 0 { 255 } else { 0 });
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole 256 x 256 slot filled with one repeated pixel value.
    fn slot_of(pixel: u16) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SLOT_BYTES);
        for _ in 0..WIDTH * HEIGHT {
            bytes.extend_from_slice(&pixel.to_le_bytes());
        }
        bytes
    }

    fn rgba_at(rgba: &[u8], pixel: usize) -> [u8; 4] {
        [
            rgba[pixel * 4],
            rgba[pixel * 4 + 1],
            rgba[pixel * 4 + 2],
            rgba[pixel * 4 + 3],
        ]
    }

    #[test]
    fn geometry_constants_are_the_verified_ones() {
        assert_eq!(WIDTH, 256);
        assert_eq!(HEIGHT, 256);
        assert_eq!(PIXEL_BYTES, 2);
        assert_eq!(WIDTH * HEIGHT, 65536);
        assert_eq!(SLOT_BYTES, 131072);
    }

    #[test]
    fn expand5_hits_both_endpoints_and_a_midpoint() {
        assert_eq!(expand5(0), 0);
        assert_eq!(expand5(31), 255);
        assert_eq!(expand5(16), 132);
        for v in 0..=31u8 {
            assert_eq!(expand5(v), (v << 3) | (v >> 2), "v={v}");
        }
    }

    #[test]
    fn decodes_channel_isolation_and_alpha_per_pixel() {
        // One pixel repeated slot-wide; values are invented bit patterns.
        for (pixel, expected) in [
            (0xFC00u16, [255u8, 0, 0, 255]), // alpha set, red only
            (0x83E0, [0, 255, 0, 255]),      // alpha set, green only
            (0x801F, [0, 0, 255, 255]),      // alpha set, blue only
            (0xFFFF, [255, 255, 255, 255]),  // every bit set
            (0x7FFF, [255, 255, 255, 0]),    // every bit but alpha
        ] {
            let rgba = decode_a1r5g5b5(&slot_of(pixel)).expect("full slot must decode");
            assert_eq!(rgba.len(), WIDTH * HEIGHT * 4);
            assert_eq!(rgba_at(&rgba, 0), expected, "pixel {pixel:#06x}");
            assert_eq!(rgba_at(&rgba, WIDTH * HEIGHT - 1), expected);
        }
    }

    #[test]
    fn decodes_all_zero_and_all_ones_slots() {
        let zero = decode_a1r5g5b5(&slot_of(0x0000)).expect("full slot must decode");
        assert!(zero.chunks_exact(4).all(|px| px == [0, 0, 0, 0]));
        let ones = decode_a1r5g5b5(&slot_of(0xFFFF)).expect("full slot must decode");
        assert!(ones.chunks_exact(4).all(|px| px == [255, 255, 255, 255]));
    }

    #[test]
    fn decode_is_row_major_top_down_with_no_padding() {
        let mut bytes = vec![0u8; SLOT_BYTES];
        bytes[0..2].copy_from_slice(&0xFC00u16.to_le_bytes()); // pixel 0
        let first_pixel_of_row_1 = WIDTH * PIXEL_BYTES; // pixel 256
        bytes[first_pixel_of_row_1..first_pixel_of_row_1 + 2]
            .copy_from_slice(&0x801Fu16.to_le_bytes());
        let rgba = decode_a1r5g5b5(&bytes).expect("full slot must decode");
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert!(rgba[4..WIDTH * 4].iter().all(|b| *b == 0));
        assert_eq!(&rgba[WIDTH * 4..WIDTH * 4 + 4], &[0, 0, 255, 255]);
    }

    #[test]
    fn rejects_any_length_but_exactly_one_slot() {
        for len in [0usize, 2, SLOT_BYTES - 2, SLOT_BYTES - 1, SLOT_BYTES + 1] {
            let err = decode_a1r5g5b5(&vec![0u8; len]).expect_err("wrong length must fail");
            assert_eq!(
                err,
                SlotDecodeError::WrongLength {
                    found: len,
                    expected: SLOT_BYTES,
                }
            );
            let msg = err.to_string();
            assert!(
                msg.contains(&len.to_string()) && msg.contains(&SLOT_BYTES.to_string()),
                "got: {msg}"
            );
        }
    }
}

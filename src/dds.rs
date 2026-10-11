//! DDS parsing + decoding for slice 1 (`dds-texture-viewer`).
//!
//! One job: read a `.DDS` file, validate the `DDS ` magic, extract the
//! DX10 extended-header `dxgiFormat` (when present), and decode the base
//! mip level to RGBA8 via the `image` crate.
//!
//! Failure policy (LOUD, never guess): bad magic is [`ViewerError::NotDds`];
//! ANY decode failure on a DX10-extended file is
//! [`ViewerError::UnsupportedFormat`] naming the `dxgiFormat` value — a
//! corrupt-but-supported file is indistinguishable at this level and still
//! fails loud, which is the required policy. Non-DX10 decode failures are
//! [`ViewerError::Decode`]. Every variant renders a clear stderr message
//! (see [`ViewerError`]'s `Display`) and the caller exits nonzero.

use std::fmt;
use std::path::Path;

/// DDS magic: every valid DDS file starts with these 4 bytes.
pub const DDS_MAGIC: &[u8; 4] = b"DDS ";

/// FourCC marking a DX10 extended header right after the 124-byte header.
pub const FOURCC_DX10: &[u8; 4] = b"DX10";

/// Byte offset of the DX10 `dxgiFormat` field (124-byte header + 4 magic).
pub const DXGI_FORMAT_OFFSET: usize = 128;

/// Byte range of the pixel-format FourCC inside the DDS header.
pub const FOURCC_OFFSET: usize = 84;

/// Decoded base mip level, always RGBA8.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

/// Every way loading a texture can fail. Each variant maps to a clear
/// stderr message + nonzero exit in `main`; see `Display` below.
#[derive(Debug)]
pub enum ViewerError {
    /// File could not be read (missing path, permissions, ...).
    Io(std::io::Error),
    /// First 4 bytes are not `DDS `.
    NotDds,
    /// DX10 file the `image` crate cannot decode; carries the raw
    /// `dxgiFormat` value so the user can report it.
    UnsupportedFormat(u32),
    /// Non-DX10 file the `image` crate cannot decode; carries its message.
    Decode(String),
}

impl fmt::Display for ViewerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewerError::Io(e) => write!(f, "cannot read file: {e}"),
            ViewerError::NotDds => write!(f, "not a DDS file: bad magic (expected `DDS `)"),
            ViewerError::UnsupportedFormat(fmt_id) => write!(
                f,
                "unsupported DDS format (dxgiFormat {fmt_id}); cannot decode, refusing to guess"
            ),
            ViewerError::Decode(msg) => write!(f, "failed to decode DDS: {msg}"),
        }
    }
}

impl std::error::Error for ViewerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ViewerError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// True when `bytes` start with the `DDS ` magic.
pub fn has_dds_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == *DDS_MAGIC
}

/// Pixel-format FourCC at header offset 84 (`None` when truncated).
pub fn fourcc_of(bytes: &[u8]) -> Option<[u8; 4]> {
    bytes
        .get(FOURCC_OFFSET..FOURCC_OFFSET + 4)
        .map(|s| [s[0], s[1], s[2], s[3]])
}

/// DX10 `dxgiFormat` (`Some` only for `DX10`-FourCC files with a full
/// extended header present).
pub fn dxgi_format_of(bytes: &[u8]) -> Option<u32> {
    if fourcc_of(bytes)? != *FOURCC_DX10 {
        return None;
    }
    bytes
        .get(DXGI_FORMAT_OFFSET..DXGI_FORMAT_OFFSET + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Load `path`, validate it, decode the base mip to RGBA8.
///
/// Game paths arrive via CLI arg at runtime only; nothing here ever
/// embeds a game path, and tests use synthetic bytes, never game files.
pub fn load_rgba8(path: impl AsRef<Path>) -> Result<DecodedImage, ViewerError> {
    let bytes = std::fs::read(path.as_ref()).map_err(ViewerError::Io)?;
    if !has_dds_magic(&bytes) {
        return Err(ViewerError::NotDds);
    }
    let dxgi = dxgi_format_of(&bytes);
    match image::load_from_memory(&bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = (rgba.width(), rgba.height());
            Ok(DecodedImage {
                width,
                height,
                rgba8: rgba.into_raw(),
            })
        }
        Err(e) => {
            // Slice-1 policy: every DX10 decode failure is reported with
            // its dxgiFormat value (loud, no guessing). Anything else
            // carries the decoder message.
            match dxgi {
                Some(fmt_id) => Err(ViewerError::UnsupportedFormat(fmt_id)),
                None => Err(ViewerError::Decode(e.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal synthetic DX10 header: magic + FourCC + dxgi value.
    /// Deliberately NOT a decodable image (no payload) — decode must
    /// fail loud via `UnsupportedFormat`.
    fn synthetic_dx10_header(dxgi: u32) -> Vec<u8> {
        let mut b = vec![0u8; DXGI_FORMAT_OFFSET + 4];
        b[0..4].copy_from_slice(DDS_MAGIC);
        b[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(FOURCC_DX10);
        b[DXGI_FORMAT_OFFSET..DXGI_FORMAT_OFFSET + 4].copy_from_slice(&dxgi.to_le_bytes());
        b
    }

    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("dds-viewer-test-{name}-{}", std::process::id()));
        std::fs::write(&p, bytes).expect("test temp file must be writable");
        p
    }

    #[test]
    fn valid_magic_is_accepted_not_rejected_as_not_dds() {
        let bytes = synthetic_dx10_header(98);
        assert!(has_dds_magic(&bytes));
        let path = write_temp("valid-magic", &bytes);
        let err = load_rgba8(&path).expect_err("header-only file cannot decode");
        assert!(
            !matches!(err, ViewerError::NotDds),
            "valid magic must not be NotDds, got: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bad_magic_gives_not_dds() {
        let mut bytes = synthetic_dx10_header(98);
        bytes[0..4].copy_from_slice(b"NOT!");
        assert!(!has_dds_magic(&bytes));
        let path = write_temp("bad-magic", &bytes);
        let err = load_rgba8(&path).expect_err("bad magic must fail");
        assert!(
            matches!(err, ViewerError::NotDds),
            "expected NotDds, got: {err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extracts_dx10_dxgi_format() {
        // 98 is the value observed on real TEX/*.DDS (see spec); here it
        // is just a synthetic u32 round-trip, no game bytes involved.
        assert_eq!(dxgi_format_of(&synthetic_dx10_header(98)), Some(98));
        assert_eq!(dxgi_format_of(&synthetic_dx10_header(0)), Some(0));
    }

    #[test]
    fn non_dx10_has_no_dxgi_format() {
        let mut bytes = synthetic_dx10_header(98);
        bytes[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(b"DXT1");
        assert_eq!(fourcc_of(&bytes), Some(*b"DXT1"));
        assert_eq!(dxgi_format_of(&bytes), None);
    }

    #[test]
    fn unsupported_format_error_names_value() {
        let bogus: u32 = 987_654;
        let path = write_temp("unsupported", &synthetic_dx10_header(bogus));
        let err = load_rgba8(&path).expect_err("bogus dxgi must fail");
        match err {
            ViewerError::UnsupportedFormat(v) => assert_eq!(v, bogus),
            other => panic!("expected UnsupportedFormat({bogus}), got: {other}"),
        }
        let msg = ViewerError::UnsupportedFormat(bogus).to_string();
        assert!(
            msg.contains(&bogus.to_string()),
            "error message must name the format value, got: {msg}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_gives_io_error() {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "dds-viewer-test-no-such-file-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        assert!(matches!(load_rgba8(&p), Err(ViewerError::Io(_))));
    }
}

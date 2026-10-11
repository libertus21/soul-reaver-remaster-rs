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

/// File offset of the surface height (`dwHeight`).
pub const HEIGHT_OFFSET: usize = 12;

/// File offset of the surface width (`dwWidth`).
pub const WIDTH_OFFSET: usize = 16;

/// File offset of the mipmap count (`dwMipMapCount`).
pub const MIPCOUNT_OFFSET: usize = 28;

/// File offset where BC7 base-mip blocks start
/// (4 magic + 124 header + 20 DX10 extension).
pub const BC7_DATA_OFFSET: usize = 148;

/// `DXGI_FORMAT_BC7_UNORM` — the only compressed format this viewer
/// uploads natively. Its sRGB sibling (99) stays loud on purpose.
pub const DXGI_BC7_UNORM: u32 = 98;

/// Bytes per BC7 block (4x4 texels).
pub const BC7_BLOCK_BYTES: usize = 16;

/// BC7 block edge length in texels.
pub const BC7_BLOCK_DIM: u32 = 4;

/// Decoded base mip level, always RGBA8.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

/// Raw BC7 base-mip blocks, uploaded to the GPU as-is (no CPU decode).
#[derive(Debug, Clone)]
pub struct CompressedBc7 {
    pub width: u32,
    pub height: u32,
    pub blocks: Vec<u8>,
}

/// Either texture flavor this viewer can show.
#[derive(Debug, Clone)]
pub enum LoadedTexture {
    Rgba8(DecodedImage),
    Bc7(CompressedBc7),
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
    /// `dxgiFormat` 98 file whose shape is invalid (zero dims, dims not
    /// multiples of the 4x4 BC7 block, or file shorter than header +
    /// base mip); carries a message naming the offending values.
    InvalidBc7(String),
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
            ViewerError::InvalidBc7(msg) => write!(f, "invalid BC7 texture: {msg}"),
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
    decode_rgba8(&bytes)
}

/// Shared RGBA8 decode used by [`load_rgba8`] and the non-BC7 arm of
/// [`load_texture`]. Behavior is slice-1 verbatim: every DX10 decode
/// failure reports its `dxgiFormat` value (loud, no guessing).
fn decode_rgba8(bytes: &[u8]) -> Result<DecodedImage, ViewerError> {
    let dxgi = dxgi_format_of(bytes);
    match image::load_from_memory(bytes) {
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

/// Read a little-endian u32 at a file offset (`None` when truncated).
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Slice the raw BC7 base-mip blocks out of `bytes` (no CPU decode).
/// The caller must have branched on `dxgiFormat == 98` already.
pub fn load_bc7_from_bytes(bytes: &[u8]) -> Result<CompressedBc7, ViewerError> {
    if !has_dds_magic(bytes) {
        return Err(ViewerError::NotDds);
    }
    let (Some(width), Some(height)) = (
        u32_at(bytes, WIDTH_OFFSET),
        u32_at(bytes, HEIGHT_OFFSET),
    ) else {
        return Err(ViewerError::InvalidBc7(format!(
            "truncated DDS header: file has {} bytes, need width/height fields",
            bytes.len()
        )));
    };
    if width == 0 || height == 0 {
        return Err(ViewerError::InvalidBc7(format!(
            "zero dimensions are not a texture: {width}x{height}"
        )));
    }
    if width % BC7_BLOCK_DIM != 0 || height % BC7_BLOCK_DIM != 0 {
        return Err(ViewerError::InvalidBc7(format!(
            "dimensions {width}x{height} are not multiples of the {0}x{0} BC7 block",
            BC7_BLOCK_DIM
        )));
    }
    let blocks = (width / BC7_BLOCK_DIM) as usize * (height / BC7_BLOCK_DIM) as usize;
    let expected = BC7_DATA_OFFSET + blocks * BC7_BLOCK_BYTES;
    if bytes.len() < expected {
        // Mip count is parsed for context only: the base mip is
        // self-sufficient, so lower mips are skipped by construction and
        // never gate the load (see design).
        let mips = u32_at(bytes, MIPCOUNT_OFFSET).unwrap_or(0);
        return Err(ViewerError::InvalidBc7(format!(
            "expected {expected} bytes for {width}x{height} base mip ({mips} mips claimed), file has {}",
            bytes.len()
        )));
    }
    Ok(CompressedBc7 {
        width,
        height,
        blocks: bytes[BC7_DATA_OFFSET..BC7_DATA_OFFSET + blocks * BC7_BLOCK_BYTES].to_vec(),
    })
}

/// Load `path` as BC7 blocks. Convenience wrapper over
/// [`load_bc7_from_bytes`] for callers that already know the format.
pub fn load_bc7(path: impl AsRef<Path>) -> Result<CompressedBc7, ViewerError> {
    let bytes = std::fs::read(path.as_ref()).map_err(ViewerError::Io)?;
    load_bc7_from_bytes(&bytes)
}

/// Load `path`, dispatching on the DX10 `dxgiFormat`: `98` takes the
/// native BC7 block path, everything else takes the legacy RGBA8
/// decode path (whose `UnsupportedFormat` loud-error behavior is
/// preserved verbatim for all other undecodable formats).
pub fn load_texture(path: impl AsRef<Path>) -> Result<LoadedTexture, ViewerError> {
    let bytes = std::fs::read(path.as_ref()).map_err(ViewerError::Io)?;
    if !has_dds_magic(&bytes) {
        return Err(ViewerError::NotDds);
    }
    match dxgi_format_of(&bytes) {
        Some(DXGI_BC7_UNORM) => load_bc7_from_bytes(&bytes).map(LoadedTexture::Bc7),
        _ => decode_rgba8(&bytes).map(LoadedTexture::Rgba8),
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

    /// Synthetic BC7 file: magic + DX10 FourCC + dxgi 98 + dims at the
    /// real header offsets + `payload_blocks` 16-byte blocks of filler.
    /// No game bytes — dims and filler are invented here.
    fn synthetic_bc7(width: u32, height: u32, payload_blocks: usize) -> Vec<u8> {
        let mut b = vec![0u8; BC7_DATA_OFFSET + payload_blocks * BC7_BLOCK_BYTES];
        b[0..4].copy_from_slice(DDS_MAGIC);
        b[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(FOURCC_DX10);
        b[DXGI_FORMAT_OFFSET..DXGI_FORMAT_OFFSET + 4]
            .copy_from_slice(&DXGI_BC7_UNORM.to_le_bytes());
        b[WIDTH_OFFSET..WIDTH_OFFSET + 4].copy_from_slice(&width.to_le_bytes());
        b[HEIGHT_OFFSET..HEIGHT_OFFSET + 4].copy_from_slice(&height.to_le_bytes());
        for (i, byte) in b[BC7_DATA_OFFSET..].iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        b
    }

    #[test]
    fn bc7_dispatch_slices_exact_base_mip() {
        // 8x8 needs (8/4)^2 = 4 blocks = 64 payload bytes.
        let bytes = synthetic_bc7(8, 8, 4);
        let path = write_temp("bc7-dispatch", &bytes);
        match load_texture(&path) {
            Ok(LoadedTexture::Bc7(c)) => {
                assert_eq!((c.width, c.height), (8, 8));
                assert_eq!(c.blocks.len(), 4 * BC7_BLOCK_BYTES);
                assert_eq!(c.blocks, bytes[BC7_DATA_OFFSET..]);
            }
            other => panic!("expected LoadedTexture::Bc7, got: {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bc7_path_wrapper_loads_blocks() {
        let bytes = synthetic_bc7(8, 8, 4);
        let path = write_temp("bc7-path", &bytes);
        let c = load_bc7(&path).expect("valid synthetic BC7 must load");
        assert_eq!((c.width, c.height), (8, 8));
        assert_eq!(c.blocks.len(), 4 * BC7_BLOCK_BYTES);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bc7_rejects_non_multiple_of_four_dims() {
        // 8x8 needs 4 blocks; 6x8 is not a multiple of the 4x4 block.
        let bytes = synthetic_bc7(6, 8, 4);
        let err = load_bc7_from_bytes(&bytes).expect_err("6-wide BC7 must fail");
        match err {
            ViewerError::InvalidBc7(msg) => {
                assert!(msg.contains("6x8"), "message must name dims, got: {msg}");
            }
            other => panic!("expected InvalidBc7, got: {other}"),
        }
    }

    #[test]
    fn bc7_rejects_zero_dims() {
        let bytes = synthetic_bc7(0, 8, 0);
        assert!(
            matches!(load_bc7_from_bytes(&bytes), Err(ViewerError::InvalidBc7(_))),
            "zero width must be InvalidBc7"
        );
    }

    #[test]
    fn bc7_rejects_truncated_payload_naming_sizes() {
        // 8x8 needs 64 payload bytes; give only 1 block (16 bytes).
        let bytes = synthetic_bc7(8, 8, 1);
        let err = load_bc7_from_bytes(&bytes).expect_err("truncated BC7 must fail");
        match err {
            ViewerError::InvalidBc7(msg) => {
                let expected = BC7_DATA_OFFSET + 4 * BC7_BLOCK_BYTES;
                assert!(
                    msg.contains(&expected.to_string()) && msg.contains(&bytes.len().to_string()),
                    "message must name expected vs found sizes, got: {msg}"
                );
            }
            other => panic!("expected InvalidBc7, got: {other}"),
        }
        // Same file through the dispatcher: still InvalidBc7, never image-decode.
        let path = write_temp("bc7-truncated", &bytes);
        assert!(
            matches!(load_texture(&path), Err(ViewerError::InvalidBc7(_))),
            "dispatcher must surface InvalidBc7 for truncated BC7"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn non_bc7_dxgi_values_stay_unsupported_format() {
        for dxgi in [99u32, 987_654] {
            let path = write_temp("still-unsupported", &synthetic_dx10_header(dxgi));
            match load_texture(&path).expect_err("undecodable dxgi must fail") {
                ViewerError::UnsupportedFormat(v) => assert_eq!(v, dxgi),
                other => panic!("expected UnsupportedFormat({dxgi}), got: {other}"),
            }
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn non_dx10_bytes_still_take_rgba8_path() {
        // DX10 FourCC replaced by DXT1 with a garbage payload: the
        // dispatcher must take the legacy decode path, i.e. a Decode
        // error — never UnsupportedFormat or InvalidBc7.
        let mut bytes = synthetic_dx10_header(98);
        bytes[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(b"DXT1");
        let path = write_temp("non-dx10-path", &bytes);
        match load_texture(&path).expect_err("garbage DXT1 must fail") {
            ViewerError::Decode(_) => {}
            other => panic!("expected Decode via the RGBA8 path, got: {other}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_bc7_message_renders() {
        let msg = ViewerError::InvalidBc7("expected 212 vs 100".to_string()).to_string();
        assert!(
            msg.contains("expected 212 vs 100"),
            "Display must carry the detail, got: {msg}"
        );
    }
}

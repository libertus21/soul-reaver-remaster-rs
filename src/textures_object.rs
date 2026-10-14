//! Per-material D/N/S texture loader for `.SRM` meshes (slice `mesh-textures`).
//!
//! One job: given a parsed [`crate::srm::Material`] and the game's `1\TEX\`
//! directory, open the three sibling DDS files (`<name>_D.DDS` diffuse,
//! `<name>_N.DDS` normal, `<name>_S.DDS` specular), validate their
//! headers, and decode the base mip to RGBA8.
//!
//! Format facts (probe 2026-10-12, byte-exact):
//! - D and S are BC7_UNORM (`DX10` fourCC, dxgiFormat 98).
//! - N is BC5_UNORM legacy (fourCC `ATI2`, NO DX10 extension).
//! All three use 512x512 with 6 mips; only the base mip is decoded.
//!
//! Failure policy: LOUD, never fall back. A missing file, wrong format,
//! or truncated payload is a distinct [`TextureError`] variant; the
//! caller exits nonzero with a clear stderr message.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::dds::DecodedImage;
use crate::srm::Material;

// --- DDS constants (independent of the ones in dds.rs to avoid
// coupling; these are the minimal subset this module needs) ---
const DDS_MAGIC: &[u8; 4] = b"DDS ";
const FOURCC_OFFSET: usize = 84;
const WIDTH_OFFSET: usize = 16;
const HEIGHT_OFFSET: usize = 12;
const DX10_FOURCC: &[u8; 4] = b"DX10";
const ATI2_FOURCC: &[u8; 4] = b"ATI2";
const DXGI_BC7_UNORM: u32 = 98;
const DX10_DATA_OFFSET: usize = 148;
const LEGACY_DATA_OFFSET: usize = 128;

#[derive(Debug, Clone)]
pub struct MaterialTextures {
    pub diffuse: DecodedImage,
    pub normal: DecodedImage,
    pub specular: DecodedImage,
}

#[derive(Debug)]
pub enum TextureError {
    Io(std::io::Error),
    Missing(PathBuf),
    WrongFormat { file: PathBuf, expected: String, found: String },
    Truncated { file: PathBuf, expected: u64, found: u64 },
    InvalidHeader { file: PathBuf, msg: String },
}

impl fmt::Display for TextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextureError::Io(e) => write!(f, "cannot read texture file: {e}"),
            TextureError::Missing(p) => write!(f, "missing texture file: {}", p.display()),
            TextureError::WrongFormat { file, expected, found } => write!(
                f,
                "wrong DDS format in '{}': expected {expected}, found {found}",
                file.display()
            ),
            TextureError::Truncated { file, expected, found } => write!(
                f,
                "truncated DDS '{}': expected {expected} bytes for base mip, file has {found}",
                file.display()
            ),
            TextureError::InvalidHeader { file, msg } => write!(
                f,
                "invalid DDS header in '{}': {msg}",
                file.display()
            ),
        }
    }
}

impl std::error::Error for TextureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TextureError::Io(e) => Some(e),
            _ => None,
        }
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn fourcc_at(bytes: &[u8]) -> Option<[u8; 4]> {
    bytes
        .get(FOURCC_OFFSET..FOURCC_OFFSET + 4)
        .map(|s| [s[0], s[1], s[2], s[3]])
}

/// BC block count for one 2D mip level of `w`x`h` (4x4 blocks, ceil).
fn block_count(w: u32, h: u32) -> u64 {
    let bw = ((w + 3) / 4) as u64;
    let bh = ((h + 3) / 4) as u64;
    bw * bh
}

/// Load and decode one DDS file with a KNOWN format (BC7 or BC5/ATI2).
/// Returns an RGBA8 base-mip image.
fn load_one(
    file: &Path,
    expected: DdsKind,
) -> Result<DecodedImage, TextureError> {
    let bytes = std::fs::read(file).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TextureError::Missing(file.to_path_buf())
        } else {
            TextureError::Io(e)
        }
    })?;
    if bytes.len() < 4 || &bytes[0..4] != DDS_MAGIC {
        return Err(TextureError::InvalidHeader {
            file: file.to_path_buf(),
            msg: "missing `DDS ` magic".to_string(),
        });
    }
    let fourcc = fourcc_at(&bytes).ok_or_else(|| TextureError::InvalidHeader {
        file: file.to_path_buf(),
        msg: format!("file too small to hold fourCC at offset {FOURCC_OFFSET}"),
    })?;
    let width = u32_at(&bytes, WIDTH_OFFSET).ok_or_else(|| TextureError::InvalidHeader {
        file: file.to_path_buf(),
        msg: "missing width field".to_string(),
    })?;
    let height = u32_at(&bytes, HEIGHT_OFFSET).ok_or_else(|| TextureError::InvalidHeader {
        file: file.to_path_buf(),
        msg: "missing height field".to_string(),
    })?;
    if width == 0 || height == 0 {
        return Err(TextureError::InvalidHeader {
            file: file.to_path_buf(),
            msg: format!("zero dimensions {width}x{height}"),
        });
    }

    let (data_offset, fmt_ok): (usize, bool) = match expected {
        DdsKind::Bc7 => {
            let fmt = u32_at(&bytes, 128).unwrap_or(0);
            (DX10_DATA_OFFSET, fourcc == *DX10_FOURCC && fmt == DXGI_BC7_UNORM)
        }
        DdsKind::Bc5Legacy => (LEGACY_DATA_OFFSET, fourcc == *ATI2_FOURCC),
    };
    if !fmt_ok {
        return Err(TextureError::WrongFormat {
            file: file.to_path_buf(),
            expected: match expected {
                DdsKind::Bc7 => format!("DX10/BC7_UNORM (dxgiFormat {DXGI_BC7_UNORM})"),
                DdsKind::Bc5Legacy => "legacy ATI2 (BC5_UNORM)".to_string(),
            },
            found: format!("fourCC {:?}", fourcc),
        });
    }

    let blocks = block_count(width, height);
    let expected_len = data_offset as u64 + blocks * 16;
    if (bytes.len() as u64) < expected_len {
        return Err(TextureError::Truncated {
            file: file.to_path_buf(),
            expected: expected_len,
            found: bytes.len() as u64,
        });
    }
    let blocks_bytes = &bytes[data_offset..data_offset + (blocks * 16) as usize];

    let w = width as usize;
    let h = height as usize;
    let mut rgba = vec![0u8; w * h * 4];
    let pitch = w * 4;
    let blocks_wide = ((width + 3) / 4) as usize;
    for (i, block) in blocks_bytes.chunks_exact(16).enumerate() {
        let bx = i % blocks_wide;
        let by = i / blocks_wide;
        let dst = by * 4 * pitch + bx * 4 * 4;
        match expected {
            DdsKind::Bc7 => bcdec_rs::bc7(block, &mut rgba[dst..], pitch),
            DdsKind::Bc5Legacy => bcdec_rs::bc5(block, &mut rgba[dst..], pitch, true),
        }
    }
    // BC5 writes only R, G, A (alpha as a third channel or zero); force
    // alpha to 255 so the normal map is fully opaque in the shader.
    if matches!(expected, DdsKind::Bc5Legacy) {
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
    }

    Ok(DecodedImage { width, height, rgba8: rgba })
}

#[derive(Copy, Clone)]
enum DdsKind {
    Bc7,
    Bc5Legacy,
}

fn sibling(tex_dir: &Path, name: &str, suffix: &str) -> PathBuf {
    tex_dir.join(format!("{name}_{suffix}.DDS"))
}

/// Load the three textures for one material. `Material.flags` bit
/// interpretation (per design.md):
/// bit 0 → D present, bit 1 → N present, bit 2 → S present.
/// A material that declares a bit for a map whose file is missing is a
/// LOUD error (never a silent skip).
pub fn load_material_textures(
    tex_dir: &Path,
    material: &Material,
) -> Result<MaterialTextures, TextureError> {
    let name = &material.name;
    if name.is_empty() {
        return Err(TextureError::InvalidHeader {
            file: tex_dir.to_path_buf(),
            msg: "material has an empty name (cannot resolve textures)".to_string(),
        });
    }
    let d = sibling(tex_dir, name, "D");
    let n = sibling(tex_dir, name, "N");
    let s = sibling(tex_dir, name, "S");

    if material.flags & 0b001 == 0 {
        return Err(TextureError::InvalidHeader {
            file: tex_dir.to_path_buf(),
            msg: format!("material '{name}' has no diffuse bit set in flags 0x{:02X}", material.flags),
        });
    }

    let diffuse = load_one(&d, DdsKind::Bc7)?;
    let normal = if material.flags & 0b010 != 0 {
        load_one(&n, DdsKind::Bc5Legacy)?
    } else {
        // Neutral flat normal (0.5, 0.5, 1.0) as an RGBA8 image.
        let mut rgba = vec![255u8; diffuse.rgba8.len()];
        for px in rgba.chunks_exact_mut(4) {
            px[0] = 128;
            px[1] = 128;
            px[2] = 255;
        }
        DecodedImage { width: diffuse.width, height: diffuse.height, rgba8: rgba }
    };
    let specular = if material.flags & 0b100 != 0 {
        load_one(&s, DdsKind::Bc7)?
    } else {
        // Neutral mid specular (0.5, 0.5, 0.5, 1.0).
        let mut rgba = vec![128u8; diffuse.rgba8.len()];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        DecodedImage { width: diffuse.width, height: diffuse.height, rgba8: rgba }
    };

    Ok(MaterialTextures { diffuse, normal, specular })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_dds_header_bc7(w: u32, h: u32, payload_bytes: usize) -> Vec<u8> {
        let mut b = vec![0u8; DX10_DATA_OFFSET + payload_bytes];
        b[0..4].copy_from_slice(DDS_MAGIC);
        b[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(DX10_FOURCC);
        b[WIDTH_OFFSET..WIDTH_OFFSET + 4].copy_from_slice(&w.to_le_bytes());
        b[HEIGHT_OFFSET..HEIGHT_OFFSET + 4].copy_from_slice(&h.to_le_bytes());
        b[128..132].copy_from_slice(&DXGI_BC7_UNORM.to_le_bytes());
        b
    }

    fn write_dds_header_ati2(w: u32, h: u32, payload_bytes: usize) -> Vec<u8> {
        let mut b = vec![0u8; LEGACY_DATA_OFFSET + payload_bytes];
        b[0..4].copy_from_slice(DDS_MAGIC);
        b[FOURCC_OFFSET..FOURCC_OFFSET + 4].copy_from_slice(ATI2_FOURCC);
        b[WIDTH_OFFSET..WIDTH_OFFSET + 4].copy_from_slice(&w.to_le_bytes());
        b[HEIGHT_OFFSET..HEIGHT_OFFSET + 4].copy_from_slice(&h.to_le_bytes());
        b
    }

    fn write_temp(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).expect("temp write");
        p
    }

    fn material(name: &str, flags: u8) -> Material {
        Material { name: name.to_string(), flags }
    }

    #[test]
    fn bc7_accepts_valid_header_and_rejects_short_payload() {
        let tmp = tempfile::tempdir().unwrap();
        // 4x4 = 1 block = 16 bytes payload.
        let p = write_temp(tmp.path(), "T_D.DDS", &write_dds_header_bc7(4, 4, 16));
        assert!(load_one(&p, DdsKind::Bc7).is_ok());
        let p2 = write_temp(tmp.path(), "T_SHORT.DDS", &write_dds_header_bc7(4, 4, 8));
        let e = load_one(&p2, DdsKind::Bc7).unwrap_err();
        assert!(matches!(e, TextureError::Truncated { .. }));
    }

    #[test]
    fn bc5_accepts_ati2_and_rejects_wrong_fourcc() {
        let tmp = tempfile::tempdir().unwrap();
        let p = write_temp(tmp.path(), "T_N.DDS", &write_dds_header_ati2(4, 4, 16));
        assert!(load_one(&p, DdsKind::Bc5Legacy).is_ok());
        // Wrong: BC7 file given to BC5 loader.
        let wrong = write_temp(tmp.path(), "T_WRONG.DDS", &write_dds_header_bc7(4, 4, 16));
        let e = load_one(&wrong, DdsKind::Bc5Legacy).unwrap_err();
        assert!(matches!(e, TextureError::WrongFormat { .. }));
    }

    #[test]
    fn missing_file_is_a_dedicated_error() {
        let tmp = tempfile::tempdir().unwrap();
        let e = load_one(&tmp.path().join("NOPE.DDS"), DdsKind::Bc7).unwrap_err();
        assert!(matches!(e, TextureError::Missing(_)));
        let msg = e.to_string();
        assert!(msg.contains("NOPE.DDS"));
    }

    #[test]
    fn full_material_load_uses_d_n_s_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        write_temp(tmp.path(), "MYMAT_D.DDS", &write_dds_header_bc7(4, 4, 16));
        write_temp(tmp.path(), "MYMAT_N.DDS", &write_dds_header_ati2(4, 4, 16));
        write_temp(tmp.path(), "MYMAT_S.DDS", &write_dds_header_bc7(4, 4, 16));
        let tex = load_material_textures(tmp.path(), &material("MYMAT", 0x07)).unwrap();
        assert_eq!(tex.diffuse.width, 4);
        assert_eq!(tex.normal.width, 4);
        assert_eq!(tex.specular.width, 4);
        // BC5 forces alpha to 255.
        for px in tex.normal.rgba8.chunks_exact(4) {
            assert_eq!(px[3], 255);
        }
    }

    #[test]
    fn flag_without_diffuse_bit_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let e = load_material_textures(tmp.path(), &material("X", 0x00)).unwrap_err();
        assert!(e.to_string().contains("no diffuse"));
    }

    #[test]
    fn empty_name_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let e = load_material_textures(tmp.path(), &material("", 0x07)).unwrap_err();
        assert!(e.to_string().contains("empty name"));
    }
}
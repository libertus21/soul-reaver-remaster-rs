//! Load one `.SRM` mesh together with its per-material D/N/S textures.
//!
//! Orchestration only: [`crate::srm::load`] parses the mesh, then
//! [`crate::textures_object::load_material_textures`] loads every
//! material's diffuse / normal / specular DDS. Both are already
//! byte-exact from prior slices. This module owns the "one mesh, ready
//! to render" contract used by `--mesh-textured` and `--mesh-all`.
//!
//! Failure policy: LOUD. Missing textures, wrong formats, and truncated
//! payloads each surface as distinct [`MeshEntryError`] variants; the
//! caller logs `warning:` and skips, or exits 1 when nothing loads.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::srm::{self, Material};
use crate::textures_object::{self, MaterialTextures};

/// A parsed mesh with every material's D/N/S textures already loaded.
#[derive(Debug)]
pub struct MeshEntry {
    pub srm_path: PathBuf,
    pub title: String,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[u8; 2]>,
    pub indices: Vec<u16>,
    pub material_indices: Vec<u8>,
    pub materials: Vec<Material>,
    pub textures: Vec<MaterialTextures>,
}

#[derive(Debug)]
pub enum MeshEntryError {
    Srm(srm::SrmError),
    Textures {
        material: String,
        error: textures_object::TextureError,
    },
    NoMaterials,
}

impl fmt::Display for MeshEntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshEntryError::Srm(e) => write!(f, "{e}"),
            MeshEntryError::Textures { material, error } => {
                write!(f, "material '{material}': {error}")
            }
            MeshEntryError::NoMaterials => {
                write!(f, "mesh has zero material records (nothing to texture)")
            }
        }
    }
}

impl std::error::Error for MeshEntryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MeshEntryError::Srm(e) => Some(e),
            MeshEntryError::Textures { error, .. } => Some(error),
            MeshEntryError::NoMaterials => None,
        }
    }
}

/// Load `srm_path` (a real `.SRM`) and every material's D/N/S textures
/// from `tex_dir`. All bytes stay in process memory.
pub fn load(srm_path: &Path, tex_dir: &Path) -> Result<MeshEntry, MeshEntryError> {
    let mesh = srm::load(srm_path).map_err(MeshEntryError::Srm)?;
    if mesh.materials.is_empty() {
        return Err(MeshEntryError::NoMaterials);
    }
    let mut textures = Vec::with_capacity(mesh.materials.len());
    for m in &mesh.materials {
        let t = textures_object::load_material_textures(tex_dir, m).map_err(|e| {
            MeshEntryError::Textures {
                material: m.name.clone(),
                error: e,
            }
        })?;
        textures.push(t);
    }
    let title = srm_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| srm_path.display().to_string());
    Ok(MeshEntry {
        srm_path: srm_path.to_path_buf(),
        title,
        positions: mesh.positions,
        uvs: mesh.uvs,
        indices: mesh.indices,
        material_indices: mesh.material_indices,
        materials: mesh.materials,
        textures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DDS_MAGIC: &[u8; 4] = b"DDS ";
    const DX10_FOURCC: &[u8; 4] = b"DX10";
    const ATI2_FOURCC: &[u8; 4] = b"ATI2";

    fn dds_bc7_4x4() -> Vec<u8> {
        let mut b = vec![0u8; 148 + 16];
        b[0..4].copy_from_slice(DDS_MAGIC);
        b[84..88].copy_from_slice(DX10_FOURCC);
        b[12..16].copy_from_slice(&4u32.to_le_bytes());
        b[16..20].copy_from_slice(&4u32.to_le_bytes());
        b[128..132].copy_from_slice(&98u32.to_le_bytes());
        b
    }

    fn dds_ati2_4x4() -> Vec<u8> {
        let mut b = vec![0u8; 128 + 16];
        b[0..4].copy_from_slice(DDS_MAGIC);
        b[84..88].copy_from_slice(ATI2_FOURCC);
        b[12..16].copy_from_slice(&4u32.to_le_bytes());
        b[16..20].copy_from_slice(&4u32.to_le_bytes());
        b
    }

    fn srm_with_one_material(name: &str, flags: u8) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"SRM\x01");
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        let mut rec = [0u8; 32];
        let nb = name.as_bytes();
        rec[..nb.len().min(11)].copy_from_slice(&nb[..nb.len().min(11)]);
        rec[31] = flags;
        b.extend_from_slice(&rec);
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend(std::iter::repeat_n(0u8, (32 * 48) as usize));
        b.extend(std::iter::repeat_n(0u8, 128));
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&3u32.to_le_bytes());
        for i in 0..3u8 {
            b.extend_from_slice(&(i as f32).to_le_bytes());
            b.extend_from_slice(&0.0f32.to_le_bytes());
            b.extend_from_slice(&0.0f32.to_le_bytes());
            b.extend(std::iter::repeat_n(0u8, 19 - 12));
            b.push(1);
            b.extend(std::iter::repeat_n(0u8, 23 - 20));
            b.push(i * 80);
            b.extend(std::iter::repeat_n(0u8, 27 - 24));
            b.push(i * 90);
            b.extend(std::iter::repeat_n(0u8, 32 - 28));
        }
        for f in [0u16, 1, 2] {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b
    }

    #[test]
    fn load_accepts_full_set() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("MAT_D.DDS"), dds_bc7_4x4()).unwrap();
        std::fs::write(tmp.path().join("MAT_N.DDS"), dds_ati2_4x4()).unwrap();
        std::fs::write(tmp.path().join("MAT_S.DDS"), dds_bc7_4x4()).unwrap();
        let srm = tmp.path().join("X.SRM");
        std::fs::write(&srm, srm_with_one_material("MAT", 0x07)).unwrap();
        let entry = load(&srm, tmp.path()).expect("full set must load");
        assert_eq!(entry.materials.len(), 1);
        assert_eq!(entry.textures.len(), 1);
        assert_eq!(entry.title, "X.SRM");
    }

    #[test]
    fn load_fails_loud_when_texture_missing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("MAT_D.DDS"), dds_bc7_4x4()).unwrap();
        let srm = tmp.path().join("X.SRM");
        std::fs::write(&srm, srm_with_one_material("MAT", 0x07)).unwrap();
        let e = load(&srm, tmp.path()).expect_err("missing N must fail");
        let msg = e.to_string();
        assert!(msg.contains("MAT"), "must name the material, got: {msg}");
        assert!(matches!(e, MeshEntryError::Textures { .. }));
    }

    #[test]
    fn load_rejects_zero_materials() {
        let tmp = tempfile::tempdir().unwrap();
        let mut b = Vec::new();
        b.extend_from_slice(b"SRM\x01");
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend(std::iter::repeat_n(0u8, (32 * 48) as usize));
        b.extend(std::iter::repeat_n(0u8, 128));
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&3u32.to_le_bytes());
        for _ in 0..3 {
            b.extend(std::iter::repeat_n(0u8, 32));
        }
        for f in [0u16, 1, 2] {
            b.extend_from_slice(&f.to_le_bytes());
        }
        let srm = tmp.path().join("X.SRM");
        std::fs::write(&srm, &b).unwrap();
        let e = load(&srm, tmp.path()).expect_err("zero materials must fail");
        assert!(matches!(e, MeshEntryError::NoMaterials));
        assert!(e.to_string().contains("zero material"));
    }
}
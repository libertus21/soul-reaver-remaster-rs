//! SR1-remaster `.SRM` mesh parser (positions + UVs + material indices +
//! material records + bone block + triangle indices).
//!
//! Layout facts (clean-room, shapes only; see
//! `docs/changes/models-3d/design.md`,
//! `docs/changes/mesh-textures/design.md`, and
//! `docs/notes/srm-bone-block.md`): magic `SRM\x01`, `u32` shader count
//! + 44-byte shader records, `u32` material count + 32-byte material
//! records (11-byte ASCII null-trimmed name + 20 zero bytes + 1 flag
//! byte), `u32` extra-bone count, `extra` 48-byte HD 3x4 affine
//! matrices (12 f32 LE), 32 joint records x 4 `vec3` f32 LE = 128 joint
//! slots, optional 4-byte `0x80000000` sentinel, 128 flag bytes, `u32`
//! vertex/face counts, `vertexCount` 32-byte vertices (positions are the
//! first `3 x f32` LE; material index at offset 19; bone indices at
//! 20..22; U at 23; weights at 24..26; V at 27), `faceCount` `u16` LE
//! indices (`faceCount / 3` triangles, winding carried as observed).
//! Tests use synthetic bytes only — never game files.

use std::fmt;
use std::path::Path;

pub const MAGIC: &[u8; 4] = b"SRM\x01";
pub const SHADER_RECORD: u64 = 44;
pub const MATERIAL_RECORD: u64 = 32;
pub const BONE_RECORD: u64 = 48;
pub const BASE_BONES: u64 = 32;
pub const JOINT_SLOTS: usize = 128;
pub const SENTINEL: u32 = 0x8000_0000;
pub const FLAG_BYTES: u64 = 128;
pub const VERTEX_STRIDE: u64 = 32;

/// Sanity ceiling for `extra` HD matrices (real maximum observed: 26).
pub const MAX_EXTRA_BONES: u32 = 4096;
/// Joint pool size: a bone index must stay below this.
pub const BONE_INDEX_LIMIT: u8 = 128;
/// Tolerance for the 3x3 unit-length / orthogonal basis check (the real
/// census stays below `2.2e-5`).
pub const AFFINE_EPS: f32 = 1e-3;

/// Per-vertex byte offsets inside the 32-byte stride (fixed by the
/// on-disk layout; documented in design.md, verified on real files).
pub const OFF_MATERIAL: usize = 19;
pub const OFF_BONE_INDICES: usize = 20;
pub const OFF_U: usize = 23;
pub const OFF_BONE_WEIGHTS: usize = 24;
pub const OFF_V: usize = 27;

pub const MAX_VERTICES: u32 = 1_000_000;
pub const MAX_FACE_INDICES: u32 = 3_000_000;

/// Material record: 11-byte ASCII name + flag byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    pub name: String,
    pub flags: u8,
}

/// Decoded bone block: `extra` HD matrices then 128 joint slots.
#[derive(Debug, Clone, PartialEq)]
pub struct BoneBlock {
    /// `extra` 3x4 affine records, row-major, semantics UNKNOWN; only
    /// `extra = 0` models are poseable in this slice.
    pub matrices: Vec<[f32; 12]>,
    /// Always [`JOINT_SLOTS`] positions in file order (four per record).
    pub joints: Vec<[f32; 3]>,
}

/// Positions + UVs + material indices + materials + bone block +
/// triangle indices.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[u8; 2]>,
    pub material_indices: Vec<u8>,
    pub materials: Vec<Material>,
    pub bones: BoneBlock,
    pub bone_indices: Vec<[u8; 3]>,
    pub bone_weights: Vec<[u8; 3]>,
    pub indices: Vec<u16>,
}

impl Mesh {
    /// Return the decoded bone block for posing, or a LOUD refusal when
    /// the model carries `extra > 0` HD matrices whose relationship to
    /// the 128 joints is UNKNOWN.
    pub fn poseable_bones(&self) -> Result<&BoneBlock, SrmError> {
        if !self.bones.matrices.is_empty() {
            return Err(SrmError::PoseUnsupported(format!(
                "this mesh has {} extra HD 3x4 matrix record(s); their relationship to the \
                 128 joints is UNKNOWN, so only extra=0 models are poseable",
                self.bones.matrices.len()
            )));
        }
        Ok(&self.bones)
    }
}

#[derive(Debug)]
pub enum SrmError {
    Io(std::io::Error),
    NotSrm(String),
    InvalidSrm(String),
    /// Parsed fine, but the bone class cannot be posed (`extra > 0`).
    PoseUnsupported(String),
}

impl fmt::Display for SrmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SrmError::Io(e) => write!(f, "cannot read SRM file: {e}"),
            SrmError::NotSrm(msg) => write!(f, "not an SRM mesh: {msg}"),
            SrmError::InvalidSrm(msg) => write!(f, "invalid SRM mesh: {msg}"),
            SrmError::PoseUnsupported(msg) => write!(f, "cannot pose SRM mesh: {msg}"),
        }
    }
}

impl std::error::Error for SrmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SrmError::Io(e) => Some(e),
            _ => None,
        }
    }
}

fn u32_at(bytes: &[u8], offset: u64) -> Option<u32> {
    let start = usize::try_from(offset).ok()?;
    bytes.get(start..start + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u32(bytes: &[u8], offset: &mut u64, file_len: u64, what: &str) -> Result<u32, SrmError> {
    let v = u32_at(bytes, *offset).ok_or_else(|| {
        SrmError::InvalidSrm(format!(
            "truncated {what} at offset {offset} (file has {file_len} bytes, need 4)"
        ))
    })?;
    *offset += 4;
    Ok(v)
}

fn skip(offset: &mut u64, file_len: u64, span: u64, what: &str) -> Result<(), SrmError> {
    if *offset > file_len || span > file_len - *offset {
        return Err(SrmError::InvalidSrm(format!(
            "truncated {what}: need {span} bytes at offset {offset}, file has {file_len}"
        )));
    }
    *offset += span;
    Ok(())
}

/// Read one f32 at a position already proven inside the file.
fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// Validate one HD 3x4 record: finite, rows unit length and pairwise
/// orthogonal. The last column is a translation and stays unchecked.
fn validate_affine(rec: &[f32; 12], index: usize) -> Result<(), SrmError> {
    let rows = [
        [rec[0], rec[1], rec[2]],
        [rec[4], rec[5], rec[6]],
        [rec[8], rec[9], rec[10]],
    ];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let unit = |r: [f32; 3]| (dot(r, r) - 1.0).abs() <= AFFINE_EPS;
    let orthogonal = |a: [f32; 3], b: [f32; 3]| dot(a, b).abs() <= AFFINE_EPS;
    let ok = unit(rows[0])
        && unit(rows[1])
        && unit(rows[2])
        && orthogonal(rows[0], rows[1])
        && orthogonal(rows[0], rows[2])
        && orthogonal(rows[1], rows[2]);
    if !ok {
        return Err(SrmError::InvalidSrm(format!(
            "extra bone matrix {index} is not a finite orthonormal 3x3 basis"
        )));
    }
    Ok(())
}

/// Trim the 11-byte name at first NUL; ASCII non-printable bytes become
/// `?` so the string never carries control characters into logs.
fn parse_material_name(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    raw[..end]
        .iter()
        .map(|&b| if (0x20..=0x7E).contains(&b) { b as char } else { '?' })
        .collect()
}

pub fn parse(bytes: &[u8]) -> Result<Mesh, SrmError> {
    let file_len = bytes.len() as u64;
    if file_len < 4 {
        return Err(SrmError::NotSrm(format!(
            "file has {file_len} bytes, fewer than the 4-byte magic"
        )));
    }
    if &bytes[0..4] != MAGIC {
        return Err(SrmError::NotSrm(format!(
            "bad magic {:02X?} (expected 53 52 4D 01 for `SRM\\x01`)",
            &bytes[0..4]
        )));
    }
    let mut offset: u64 = 4;

    let shaders = read_u32(bytes, &mut offset, file_len, "shader count")?;
    skip(&mut offset, file_len, u64::from(shaders) * SHADER_RECORD, "shader block")?;

    let material_count = read_u32(bytes, &mut offset, file_len, "material count")?;
    let materials_start = offset;
    skip(
        &mut offset,
        file_len,
        u64::from(material_count) * MATERIAL_RECORD,
        "material block",
    )?;
    let mut materials = Vec::with_capacity(material_count as usize);
    for i in 0..material_count as usize {
        let base = (materials_start as usize) + i * MATERIAL_RECORD as usize;
        let name = parse_material_name(&bytes[base..base + 11]);
        let flags = bytes[base + 31];
        materials.push(Material { name, flags });
    }

    let extra = read_u32(bytes, &mut offset, file_len, "bone count")?;
    if extra > MAX_EXTRA_BONES {
        return Err(SrmError::InvalidSrm(format!(
            "insane extra bone count {extra} (limit {MAX_EXTRA_BONES})"
        )));
    }
    let bone_start = offset as usize;
    skip(
        &mut offset,
        file_len,
        (BASE_BONES + u64::from(extra)) * BONE_RECORD,
        "bone block",
    )?;
    let mut matrices = Vec::with_capacity(extra as usize);
    for i in 0..extra as usize {
        let base = bone_start + i * BONE_RECORD as usize;
        let mut rec = [0.0f32; 12];
        for (slot, value) in rec.iter_mut().enumerate() {
            *value = f32_at(bytes, base + slot * 4);
        }
        validate_affine(&rec, i)?;
        matrices.push(rec);
    }
    let mut joints = Vec::with_capacity(JOINT_SLOTS);
    for record in 0..BASE_BONES as usize {
        let base = bone_start + (extra as usize + record) * BONE_RECORD as usize;
        for corner in 0..4 {
            let o = base + corner * 12;
            joints.push([f32_at(bytes, o), f32_at(bytes, o + 4), f32_at(bytes, o + 8)]);
        }
    }

    let word = u32_at(bytes, offset).ok_or_else(|| {
        SrmError::InvalidSrm(format!(
            "truncated sentinel peek at offset {offset} (file has {file_len} bytes, need 4)"
        ))
    })?;
    if word == SENTINEL {
        offset += 4;
    }
    skip(&mut offset, file_len, FLAG_BYTES, "flag bytes")?;

    let verts = read_u32(bytes, &mut offset, file_len, "vertex count")?;
    let faces = read_u32(bytes, &mut offset, file_len, "face count")?;
    if verts > MAX_VERTICES {
        return Err(SrmError::InvalidSrm(format!(
            "insane vertex count {verts} (limit {MAX_VERTICES})"
        )));
    }
    if faces > MAX_FACE_INDICES {
        return Err(SrmError::InvalidSrm(format!(
            "insane face count {faces} (limit {MAX_FACE_INDICES})"
        )));
    }
    if faces % 3 != 0 {
        return Err(SrmError::InvalidSrm(format!(
            "face count {faces} is not a multiple of 3 (one triangle is 3 indices)"
        )));
    }

    let vbase = offset as usize;
    skip(
        &mut offset,
        file_len,
        u64::from(verts) * VERTEX_STRIDE,
        "short vertex data",
    )?;
    let mut positions = Vec::with_capacity(verts as usize);
    let mut uvs = Vec::with_capacity(verts as usize);
    let mut material_indices = Vec::with_capacity(verts as usize);
    let mut bone_indices = Vec::with_capacity(verts as usize);
    let mut bone_weights = Vec::with_capacity(verts as usize);
    for i in 0..verts as usize {
        let o = vbase + i * VERTEX_STRIDE as usize;
        positions.push([
            f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()),
            f32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()),
            f32::from_le_bytes(bytes[o + 8..o + 12].try_into().unwrap()),
        ]);
        material_indices.push(bytes[o + OFF_MATERIAL]);
        uvs.push([bytes[o + OFF_U], bytes[o + OFF_V]]);
        let bi = [
            bytes[o + OFF_BONE_INDICES],
            bytes[o + OFF_BONE_INDICES + 1],
            bytes[o + OFF_BONE_INDICES + 2],
        ];
        let bw = [
            bytes[o + OFF_BONE_WEIGHTS],
            bytes[o + OFF_BONE_WEIGHTS + 1],
            bytes[o + OFF_BONE_WEIGHTS + 2],
        ];
        if bi.iter().any(|&b| b >= BONE_INDEX_LIMIT) {
            return Err(SrmError::InvalidSrm(format!(
                "vertex {i} has bone index {bi:?} outside the {JOINT_SLOTS}-slot pool"
            )));
        }
        let weight_sum: u32 = bw.iter().map(|&w| u32::from(w)).sum();
        if weight_sum != 255 {
            return Err(SrmError::InvalidSrm(format!(
                "vertex {i} bone weights {bw:?} sum to {weight_sum}, expected 255"
            )));
        }
        bone_indices.push(bi);
        bone_weights.push(bw);
    }

    let fbase = offset as usize;
    skip(&mut offset, file_len, u64::from(faces) * 2, "short face data")?;
    let mut indices = Vec::with_capacity(faces as usize);
    for i in 0..faces as usize {
        let o = fbase + i * 2;
        indices.push(u16::from_le_bytes([bytes[o], bytes[o + 1]]));
    }

    if offset != file_len {
        return Err(SrmError::InvalidSrm(format!(
            "trailing-shape mismatch: parse ended at offset {offset}, file has {file_len} bytes"
        )));
    }

    Ok(Mesh {
        positions,
        uvs,
        material_indices,
        materials,
        bones: BoneBlock { matrices, joints },
        bone_indices,
        bone_weights,
        indices,
    })
}

pub fn load(path: &Path) -> Result<Mesh, SrmError> {
    let bytes = std::fs::read(path).map_err(SrmError::Io)?;
    parse(&bytes)
}

/// Collect direct-child `*.SRM` files in `dir`, sorted by file name.
/// Case-insensitive extension match; subdirectories are skipped; an
/// empty result is a LOUD error naming the directory. Mirrors
/// [`crate::gallery::collect_dds_entries`] for the `.SRM` side.
pub fn collect_srm_entries(dir: &Path) -> Result<Vec<std::path::PathBuf>, SrmError> {
    let entries = std::fs::read_dir(dir).map_err(SrmError::Io)?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(SrmError::Io)?;
        if !entry.file_type().map_err(SrmError::Io)?.is_file() {
            continue;
        }
        let path = entry.path();
        let is_srm = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("srm"));
        if is_srm {
            out.push(path);
        }
    }
    out.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    if out.is_empty() {
        return Err(SrmError::NotSrm(format!(
            "no .SRM files in directory: {}",
            dir.display()
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(sentinel: bool, verts: &[[f32; 3]], faces: &[u16]) -> Vec<u8> {
        build_with_materials(sentinel, verts, faces, &[])
    }

    fn build_with_materials(
        sentinel: bool,
        verts: &[[f32; 3]],
        faces: &[u16],
        materials: &[(&str, u8)],
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(materials.len() as u32).to_le_bytes());
        for (name, flag) in materials {
            let mut rec = [0u8; MATERIAL_RECORD as usize];
            let nb = name.as_bytes();
            rec[..nb.len().min(11)].copy_from_slice(&nb[..nb.len().min(11)]);
            rec[31] = *flag;
            b.extend_from_slice(&rec);
        }
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend(std::iter::repeat_n(0u8, (BASE_BONES * BONE_RECORD) as usize));
        if sentinel {
            b.extend_from_slice(&SENTINEL.to_le_bytes());
        }
        b.extend(std::iter::repeat_n(0u8, FLAG_BYTES as usize));
        b.extend_from_slice(&(verts.len() as u32).to_le_bytes());
        b.extend_from_slice(&(faces.len() as u32).to_le_bytes());
        for (i, v) in verts.iter().enumerate() {
            b.extend_from_slice(&v[0].to_le_bytes());
            b.extend_from_slice(&v[1].to_le_bytes());
            b.extend_from_slice(&v[2].to_le_bytes());
            b.extend(std::iter::repeat_n(0u8, OFF_MATERIAL - 12));
            b.push(if materials.is_empty() { 0 } else { (i % materials.len() + 1) as u8 });
            b.extend(std::iter::repeat_n(0u8, OFF_U - OFF_MATERIAL - 1));
            b.push((i * 7) as u8);
            b.extend_from_slice(&[255, 0, 0]);
            b.push((i * 11) as u8);
            b.extend(std::iter::repeat_n(0u8, VERTEX_STRIDE as usize - OFF_V - 1));
        }
        for f in faces {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b
    }

    fn affine(tx: f32, ty: f32, tz: f32) -> [f32; 12] {
        [1.0, 0.0, 0.0, tx, 0.0, 1.0, 0.0, ty, 0.0, 0.0, 1.0, tz]
    }

    /// Three-vertex mesh with a caller-chosen matrix/joint pool.
    fn bone_file(
        extra: &[[f32; 12]],
        joints: &[[f32; 3]],
        verts: &[([f32; 3], [u8; 3], [u8; 3])],
        sentinel: bool,
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&(extra.len() as u32).to_le_bytes());
        for m in extra {
            for v in m {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        let mut pool = [[0.0f32; 3]; JOINT_SLOTS];
        pool[..joints.len()].copy_from_slice(joints);
        for j in pool.iter().flatten() {
            b.extend_from_slice(&j.to_le_bytes());
        }
        if sentinel {
            b.extend_from_slice(&SENTINEL.to_le_bytes());
        }
        b.extend(std::iter::repeat_n(0u8, FLAG_BYTES as usize));
        b.extend_from_slice(&(verts.len() as u32).to_le_bytes());
        b.extend_from_slice(&3u32.to_le_bytes());
        for (pos, idx, weights) in verts {
            for x in pos {
                b.extend_from_slice(&x.to_le_bytes());
            }
            b.extend(std::iter::repeat_n(0u8, OFF_MATERIAL - 12));
            b.push(0);
            b.extend_from_slice(idx);
            b.push(0);
            b.extend_from_slice(weights);
            b.push(0);
            b.extend(std::iter::repeat_n(0u8, VERTEX_STRIDE as usize - OFF_V - 1));
        }
        for f in [0u16, 1, 2] {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b
    }

    #[test]
    fn magic_accept_and_positions_indices_extracted() {
        let mesh = parse(&build(
            false,
            &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            &[0, 1, 2],
        ))
        .expect("synthetic mesh must parse");
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.positions[0], [1.0, 2.0, 3.0]);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn uv_and_material_index_extracted() {
        let mesh = parse(&build(
            false,
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[0, 1, 2],
        ))
        .expect("must parse");
        assert_eq!(mesh.uvs.len(), 3);
        assert_eq!(mesh.uvs[0], [0, 0]);
        assert_eq!(mesh.uvs[1], [7, 11]);
        assert_eq!(mesh.uvs[2], [14, 22]);
        assert_eq!(mesh.material_indices, vec![0, 0, 0]);
    }

    #[test]
    fn material_records_parsed() {
        let mesh = parse(&build_with_materials(
            false,
            &[[0.0; 3]; 3],
            &[0, 1, 2],
            &[("ALKLENS_1", 0x07), ("ALKLENS_1_2", 0x07), ("ALKLENS_1_3", 0x07)],
        ))
        .expect("must parse");
        assert_eq!(mesh.materials.len(), 3);
        assert_eq!(mesh.materials[0].name, "ALKLENS_1");
        assert_eq!(mesh.materials[0].flags, 0x07);
        assert_eq!(mesh.materials[2].name, "ALKLENS_1_3");
        assert_eq!(mesh.material_indices, vec![1, 2, 3]);
    }

    #[test]
    fn material_name_truncated_at_nul_and_non_ascii_replaced() {
        assert_eq!(parse_material_name(b"ALKLENS_1\0\0"), "ALKLENS_1");
        assert_eq!(parse_material_name(b"AB\x01CD\0"), "AB?CD");
    }

    #[test]
    fn both_sentinel_arms_parse_identically() {
        let v = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let a = parse(&build(false, &v, &[0, 1, 2])).expect("no-sentinel");
        let b = parse(&build(true, &v, &[0, 1, 2])).expect("sentinel");
        assert_eq!(a, b);
    }

    #[test]
    fn bone_block_decodes_128_ordered_joints() {
        let joints = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
        let verts = [([0.0; 3], [0, 0, 0], [255, 0, 0]); 3];
        let mesh = parse(&bone_file(&[], &joints, &verts, false)).expect("must parse");
        assert!(mesh.bones.matrices.is_empty());
        assert_eq!(mesh.bones.joints.len(), JOINT_SLOTS);
        assert_eq!(mesh.bones.joints[0], joints[0]);
        assert_eq!(mesh.bones.joints[2], joints[2]);
        assert_eq!(mesh.bones.joints[3], [0.0; 3]);
        assert_eq!(mesh.bones.joints[127], [0.0; 3]);
        assert!(mesh.poseable_bones().is_ok());
    }

    #[test]
    fn extra_matrices_decode_in_order_and_refuse_pose() {
        let matrices = [affine(10.0, 0.0, 0.0), affine(0.0, 20.0, 0.0)];
        let verts = [([0.0; 3], [0, 0, 0], [255, 0, 0]); 3];
        let mesh = parse(&bone_file(&matrices, &[[1.0, 1.0, 1.0]], &verts, false))
            .expect("extra > 0 must still decode");
        assert_eq!(mesh.bones.matrices, matrices);
        assert_eq!(mesh.bones.joints[0], [1.0, 1.0, 1.0]);
        let err = mesh.poseable_bones().expect_err("extra > 0 must refuse posing");
        assert!(matches!(err, SrmError::PoseUnsupported(_)));
        assert!(err.to_string().contains("UNKNOWN"));
    }

    #[test]
    fn non_orthonormal_extra_matrix_rejected() {
        let verts = [([0.0; 3], [0, 0, 0], [255, 0, 0]); 3];
        let mut bad = affine(0.0, 0.0, 0.0);
        bad[0] = 2.0;
        let err = parse(&bone_file(&[bad], &[], &verts, false)).unwrap_err();
        assert!(err.to_string().contains("orthonormal"), "got: {err}");
    }

    #[test]
    fn bone_fields_extracted_and_index_128_rejected() {
        let verts = [
            ([0.0, 0.0, 0.0], [3, 4, 5], [100, 155, 0]),
            ([1.0, 0.0, 0.0], [0, 0, 0], [255, 0, 0]),
            ([0.0, 1.0, 0.0], [1, 2, 3], [1, 2, 252]),
        ];
        let mesh = parse(&bone_file(&[], &[], &verts, true)).expect("must parse");
        assert_eq!(mesh.bone_indices[0], [3, 4, 5]);
        assert_eq!(mesh.bone_weights[0], [100, 155, 0]);
        assert_eq!(mesh.bone_weights[2], [1, 2, 252]);
        let bad = [
            ([0.0; 3], [128, 0, 0], [255, 0, 0]),
            ([0.0; 3], [0, 0, 0], [255, 0, 0]),
            ([0.0; 3], [0, 0, 0], [255, 0, 0]),
        ];
        let err = parse(&bone_file(&[], &[], &bad, false)).unwrap_err();
        assert!(err.to_string().contains("bone index"), "got: {err}");
    }

    #[test]
    fn weight_triple_not_255_rejected() {
        let bad = [
            ([0.0; 3], [0, 0, 0], [100, 100, 0]),
            ([0.0; 3], [0, 0, 0], [255, 0, 0]),
            ([0.0; 3], [0, 0, 0], [255, 0, 0]),
        ];
        let err = parse(&bone_file(&[], &[], &bad, false)).unwrap_err();
        assert!(err.to_string().contains("sum to 200"), "got: {err}");
    }

    #[test]
    fn extra_above_ceiling_rejected_before_allocation() {
        let verts = [([0.0; 3], [0, 0, 0], [255, 0, 0]); 3];
        let mut bytes = bone_file(&[], &[], &verts, false);
        bytes[12..16].copy_from_slice(&(MAX_EXTRA_BONES + 1).to_le_bytes());
        let err = parse(&bytes).unwrap_err();
        assert!(err.to_string().contains("extra bone count"), "got: {err}");
    }

    #[test]
    fn truncated_matrix_and_joint_blocks_are_loud() {
        let verts = [([0.0; 3], [0, 0, 0], [255, 0, 0]); 3];
        let full = bone_file(&[affine(0.0, 0.0, 0.0)], &[], &verts, false);
        for cut in [16 + 20, 16 + BONE_RECORD as usize + 10] {
            let err = parse(&full[..cut]).unwrap_err();
            assert!(matches!(err, SrmError::InvalidSrm(_)));
            assert!(err.to_string().contains("bone block"), "got: {err}");
        }
    }

    #[test]
    fn sentinel_arms_agree_with_bone_block() {
        let verts = [([0.0; 3], [0, 0, 0], [255, 0, 0]); 3];
        let joints = [[1.0, 2.0, 3.0]];
        let no = parse(&bone_file(&[], &joints, &verts, false)).expect("no sentinel");
        let yes = parse(&bone_file(&[], &joints, &verts, true)).expect("sentinel");
        assert_eq!(no, yes);
    }

    #[test]
    fn bad_magic_truncated_and_trailing_shapes_fail_loud() {
        let mut bad = build(false, &[[0.0; 3]; 3], &[0, 1, 2]);
        bad[0..4].copy_from_slice(b"NOPE");
        assert!(matches!(parse(&bad), Err(SrmError::NotSrm(_))));
        assert!(matches!(parse(b"SR"), Err(SrmError::NotSrm(_))));
        let mut cut = build(false, &[[0.0; 3]; 3], &[0, 1, 2]);
        cut.pop();
        assert!(matches!(parse(&cut), Err(SrmError::InvalidSrm(_))));
        let mut trail = build(false, &[[0.0; 3]; 3], &[0, 1, 2]);
        trail.push(0);
        let e = parse(&trail).expect_err("trailing byte");
        assert!(e.to_string().contains("trailing-shape"));
    }

    #[test]
    fn insane_counts_and_bad_face_multiple_rejected_without_panic() {
        let mut huge = build(false, &[[0.0; 3]; 3], &[0, 1, 2]);
        let vc_off = huge.len() - 3 * 32 - 6 - 8;
        huge[vc_off..vc_off + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&huge).unwrap_err().to_string().contains("insane vertex"));
        let mut odd = build(false, &[[0.0; 3]; 3], &[0, 1, 2]);
        odd[vc_off + 4..vc_off + 8].copy_from_slice(&4u32.to_le_bytes());
        assert!(parse(&odd).unwrap_err().to_string().contains("multiple of 3"));
        let mut past = build(false, &[[0.0; 3]; 3], &[0, 1, 2]);
        past[vc_off..vc_off + 4].copy_from_slice(&100_000u32.to_le_bytes());
        assert!(parse(&past).unwrap_err().to_string().contains("short vertex"));
    }

    #[test]
    fn load_missing_path_maps_to_io() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            load(&tmp.path().join("absent.srm")),
            Err(SrmError::Io(_))
        ));
    }

    #[test]
    fn collect_srm_entries_lists_only_srm_case_insensitive_and_sorted() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("b.SRM"), b"x").unwrap();
        std::fs::write(tmp.path().join("a.srm"), b"x").unwrap();
        std::fs::write(tmp.path().join("c.Srl"), b"x").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("sub.srm")).unwrap();
        let entries = collect_srm_entries(tmp.path()).expect("two .SRM must list");
        let names: Vec<String> = entries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.srm", "b.SRM"]);
    }

    #[test]
    fn collect_srm_entries_empty_dir_is_loud() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        assert!(matches!(
            collect_srm_entries(tmp.path()),
            Err(SrmError::NotSrm(_))
        ));
    }

    #[test]
    fn collect_srm_entries_missing_dir_is_io() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            collect_srm_entries(&tmp.path().join("no-such")),
            Err(SrmError::Io(_))
        ));
    }
}
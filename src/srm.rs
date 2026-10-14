//! SR1-remaster `.SRM` mesh parser (positions + triangle indices only).
//!
//! Layout facts (clean-room, shapes only; see `docs/changes/models-3d/design.md`
//! for the spec source + license note): magic `SRM\x01`, `u32` shader count +
//! 44-byte shader records, `u32` material count + 32-byte material records,
//! `u32` extra-bone count + `(32 + extra)` 48-byte bone records, optional
//! 4-byte `0x80000000` sentinel, 128 flag bytes, `u32` vertex/face counts,
//! `vertexCount` 32-byte vertices (positions are the first `3 x f32` LE),
//! `faceCount` `u16` LE indices (`faceCount / 3` triangles, winding carried
//! as observed). Everything but positions + indices is skipped as opaque
//! byte ranges. Tests use synthetic bytes only — never game files.

use std::fmt;
use std::path::Path;

/// Magic bytes opening every remaster `.SRM` file.
pub const MAGIC: &[u8; 4] = b"SRM\x01";
/// Opaque record sizes carried through uninterpreted.
pub const SHADER_RECORD: u64 = 44;
pub const MATERIAL_RECORD: u64 = 32;
pub const BONE_RECORD: u64 = 48;
/// Base bone rows present even when the extra count is zero.
pub const BASE_BONES: u64 = 32;
/// Sentinel value meaning "a 4-byte marker is present here".
pub const SENTINEL: u32 = 0x8000_0000;
/// Opaque bone-flag bytes after the optional sentinel.
pub const FLAG_BYTES: u64 = 128;
/// Per-vertex stride; positions are the first 12 bytes (`3 x f32` LE).
pub const VERTEX_STRIDE: u64 = 32;
/// Sanity caps (real meshes are far smaller; millions are never valid).
pub const MAX_VERTICES: u32 = 1_000_000;
pub const MAX_FACE_INDICES: u32 = 3_000_000;

/// Positions + triangle indices decoded from one `.SRM` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u16>,
}

/// Every way reading an `.SRM` mesh can fail. Each variant maps to a
/// clear stderr message + exit 1 in `main`; no window opens on error.
#[derive(Debug)]
pub enum SrmError {
    Io(std::io::Error),
    NotSrm(String),
    InvalidSrm(String),
}

impl fmt::Display for SrmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SrmError::Io(e) => write!(f, "cannot read SRM file: {e}"),
            SrmError::NotSrm(msg) => write!(f, "not an SRM mesh: {msg}"),
            SrmError::InvalidSrm(msg) => write!(f, "invalid SRM mesh: {msg}"),
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

/// Read one `u32` LE at `*offset`, advancing past it. Range-checked
/// BEFORE the read; `u64` arithmetic, overflow-safe.
fn read_u32(bytes: &[u8], offset: &mut u64, file_len: u64, what: &str) -> Result<u32, SrmError> {
    let v = u32_at(bytes, *offset).ok_or_else(|| {
        SrmError::InvalidSrm(format!(
            "truncated {what} at offset {offset} (file has {file_len} bytes, need 4)"
        ))
    })?;
    *offset += 4;
    Ok(v)
}

/// Skip `span` opaque bytes at `*offset`. Range-checked BEFORE the
/// advance; `u64` arithmetic, overflow-safe.
fn skip(offset: &mut u64, file_len: u64, span: u64, what: &str) -> Result<(), SrmError> {
    if *offset > file_len || span > file_len - *offset {
        return Err(SrmError::InvalidSrm(format!(
            "truncated {what}: need {span} bytes at offset {offset}, file has {file_len}"
        )));
    }
    *offset += span;
    Ok(())
}

/// Parse a whole `.SRM` file buffer into positions + indices. Every
/// header advance is range-checked BEFORE the read; the parse must land
/// exactly on EOF (any trailing-shape mismatch is a loud error).
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
    let materials = read_u32(bytes, &mut offset, file_len, "material count")?;
    skip(&mut offset, file_len, u64::from(materials) * MATERIAL_RECORD, "material block")?;
    let extra = read_u32(bytes, &mut offset, file_len, "bone count")?;
    skip(&mut offset, file_len, (BASE_BONES + u64::from(extra)) * BONE_RECORD, "bone block")?;
    // Optional sentinel: peek `0x80000000` -> skip 4, else rewind (stay).
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
    skip(&mut offset, file_len, u64::from(verts) * VERTEX_STRIDE, "short vertex data")?;
    let mut positions = Vec::with_capacity(verts as usize);
    // Range-checked above, so every stride read below is in-bounds.
    for i in 0..verts as usize {
        let o = vbase + i * VERTEX_STRIDE as usize;
        positions.push([
            f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()),
            f32::from_le_bytes(bytes[o + 4..o + 8].try_into().unwrap()),
            f32::from_le_bytes(bytes[o + 8..o + 12].try_into().unwrap()),
        ]);
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
    Ok(Mesh { positions, indices })
}

/// Read one `.SRM` file into memory and parse it. File bytes never
/// touch disk outside process memory.
pub fn load(path: &Path) -> Result<Mesh, SrmError> {
    let bytes = std::fs::read(path).map_err(SrmError::Io)?;
    parse(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(sentinel: bool, verts: &[[f32; 3]], faces: &[u16]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&0u32.to_le_bytes()); // 0 shaders
        b.extend_from_slice(&0u32.to_le_bytes()); // 0 materials
        b.extend_from_slice(&0u32.to_le_bytes()); // 0 extra bones
        b.extend(std::iter::repeat_n(0u8, (BASE_BONES * BONE_RECORD) as usize));
        if sentinel {
            b.extend_from_slice(&SENTINEL.to_le_bytes());
        }
        b.extend(std::iter::repeat_n(0u8, FLAG_BYTES as usize));
        b.extend_from_slice(&(verts.len() as u32).to_le_bytes());
        b.extend_from_slice(&(faces.len() as u32).to_le_bytes());
        for v in verts {
            b.extend_from_slice(&v[0].to_le_bytes());
            b.extend_from_slice(&v[1].to_le_bytes());
            b.extend_from_slice(&v[2].to_le_bytes());
            b.extend(std::iter::repeat_n(0u8, 20)); // rest of the 32-byte stride
        }
        for f in faces {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b
    }

    #[test]
    fn magic_accept_and_positions_indices_extracted() {
        let mesh = parse(&build(false, &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]], &[0, 1, 2]))
            .expect("synthetic mesh must parse");
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.positions[0], [1.0, 2.0, 3.0]);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn both_sentinel_arms_parse_identically() {
        let v = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let a = parse(&build(false, &v, &[0, 1, 2])).expect("no-sentinel must parse");
        let b = parse(&build(true, &v, &[0, 1, 2])).expect("sentinel must parse");
        assert_eq!(a, b);
    }

    #[test]
    fn bad_magic_truncated_and_trailing_shapes_fail_loud() {
        let mut bad = build(false, &[[0.0, 0.0, 0.0]; 3], &[0, 1, 2]);
        bad[0..4].copy_from_slice(b"NOPE");
        let e = parse(&bad).expect_err("bad magic must fail");
        assert!(matches!(e, SrmError::NotSrm(_)), "got {e:?}");
        assert!(e.to_string().contains("bad magic"), "got: {e}");
        assert!(matches!(parse(b"SR"), Err(SrmError::NotSrm(_))));
        let mut cut = build(false, &[[0.0, 0.0, 0.0]; 3], &[0, 1, 2]);
        cut.pop();
        let e = parse(&cut).expect_err("truncated tail must fail");
        assert!(matches!(e, SrmError::InvalidSrm(_)), "got {e:?}");
        assert!(e.to_string().contains("file has"), "got: {e}");
        let mut trail = build(false, &[[0.0, 0.0, 0.0]; 3], &[0, 1, 2]);
        trail.push(0);
        let e = parse(&trail).expect_err("trailing byte must fail");
        assert!(e.to_string().contains("trailing-shape"), "got: {e}");
    }

    #[test]
    fn insane_counts_and_bad_face_multiple_rejected_without_panic() {
        let mut huge = build(false, &[[0.0, 0.0, 0.0]; 3], &[0, 1, 2]);
        let vc_off = huge.len() - 3 * 32 - 6 - 8;
        huge[vc_off..vc_off + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let e = parse(&huge).expect_err("huge count must fail");
        assert!(e.to_string().contains("insane vertex"), "got: {e}");
        let mut odd = build(false, &[[0.0, 0.0, 0.0]; 3], &[0, 1, 2]);
        odd[vc_off + 4..vc_off + 8].copy_from_slice(&4u32.to_le_bytes());
        let e = parse(&odd).expect_err("face % 3 != 0 must fail");
        assert!(e.to_string().contains("multiple of 3"), "got: {e}");
        // Past-EOF range: claims far more vertices than bytes present.
        let mut past = build(false, &[[0.0, 0.0, 0.0]; 3], &[0, 1, 2]);
        past[vc_off..vc_off + 4].copy_from_slice(&100_000u32.to_le_bytes());
        let e = parse(&past).expect_err("past-EOF range must fail");
        assert!(e.to_string().contains("short vertex"), "got: {e}");
    }

    #[test]
    fn load_missing_path_maps_to_io() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let e = load(&tmp.path().join("absent.srm")).expect_err("missing file must fail");
        assert!(matches!(e, SrmError::Io(_)), "got {e:?}");
    }
}

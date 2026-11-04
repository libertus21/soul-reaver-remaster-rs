//! SR1 archived object-model containers: relocation directory, object header,
//! vertex/polygon arrays and the 16-byte material records they reference.
//! In-memory decode only: no rendering, bones, textures or world transform.

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::bigfile;
use crate::level;

pub const VERTEX_STRIDE: usize = 8;
pub const POLYGON_STRIDE: usize = 12;
pub const MATERIAL_STRIDE: usize = 16;
const MAX_RELOCATIONS: u32 = 1 << 20;
pub const USAGE: &str = "expected --object-info --bigfile <DAT> --entry <INDEX>";
/// `([x, y, z], normal id)` in model space; no fixed-point scaling.
pub type Vertex = ([i16; 3], u16);
/// `([v1, v2, v3], normal, flags, material offset or colour)`.
pub type Polygon = ([u16; 3], u8, u8, u32);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts { pub vertices: u32, pub polygons: u32, pub materials: u32 }

/// Validation numbers for the printed boundaries and index scan.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectReport {
    pub vertex_start: usize, pub vertex_count: u32, pub vertex_end: usize,
    pub polygon_start: usize, pub polygon_count: u32, pub polygon_end: usize,
    pub end_of_polygons: usize, pub out_of_range: u32,
}

/// One parsed model. `materials` holds the distinct 16-byte records that
/// textured polygons reference, in ascending offset order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ObjectModel {
    pub name: String, pub vertices: Vec<Vertex>, pub polygons: Vec<Polygon>,
    pub materials: Vec<[u8; MATERIAL_STRIDE]>, pub counts: Counts, pub report: ObjectReport,
}

/// Every way an object container can be refused. Each variant has its own text.
#[derive(Debug, PartialEq, Eq)]
pub enum ObjectError {
    Usage(&'static str), TruncatedDirectory, CountOutOfRange(u32), DirectoryPastEof,
    PaddingTruncated, HeaderTruncated, NotObjectContainer, TableOutOfRange, MultiPart,
    ModelHeaderOutOfRange, PolygonArrayOutOfRange,
    VertexBoundary { vertex_start: usize, vertex_count: u32, polygon_start: usize },
    PolygonBoundary { polygon_start: usize, polygon_count: u32, end_of_polygons: usize },
    MaterialOffsetInvalid { polygon: u32, offset: u32 },
    IndexOutOfRange { polygons: u32, vertex_count: u32 },
    ArchiveUnreadable(String), EntryIndexOutOfRange { index: usize, entries: usize },
    EntryOutsideFile(String),
}

impl fmt::Display for ObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(text) => f.write_str(text),
            Self::TruncatedDirectory => f.write_str("object container has no relocation count"),
            Self::CountOutOfRange(n) => write!(f, "relocation count {n} is outside the supported range"),
            Self::DirectoryPastEof => f.write_str("relocation directory runs past the entry"),
            Self::PaddingTruncated => f.write_str("relocation-directory padding is missing"),
            Self::HeaderTruncated => f.write_str("object header is truncated"),
            Self::NotObjectContainer => f.write_str("entry does not carry an object-container name"),
            Self::TableOutOfRange => f.write_str("object table lies outside the payload"),
            Self::MultiPart => f.write_str("UNKNOWN multi-part object: table[0] == 0"),
            Self::ModelHeaderOutOfRange => f.write_str("model header lies outside the payload"),
            Self::VertexBoundary { vertex_start, vertex_count, polygon_start } => write!(f, "vertex boundary violated: vertexStart {vertex_start} + vertexCount {vertex_count}*8 != polygonStart {polygon_start}"),
            Self::PolygonBoundary { polygon_start, polygon_count, end_of_polygons } => write!(f, "polygon boundary violated: polygonStart {polygon_start} + polygonCount {polygon_count}*12 != endOfPolygons {end_of_polygons}"),
            Self::PolygonArrayOutOfRange => f.write_str("polygon array runs outside the payload"),
            Self::MaterialOffsetInvalid { polygon, offset } => write!(f, "polygon {polygon} material offset {offset} is not a 16-byte record after the material start"),
            Self::IndexOutOfRange { polygons, vertex_count } => write!(f, "{polygons} polygons address vertices past the {vertex_count} loaded vertices"),
            Self::ArchiveUnreadable(detail) => write!(f, "cannot read the archive: {detail}"),
            Self::EntryIndexOutOfRange { index, entries } => write!(f, "entry index {index} is out of range (archive has {entries} entries)"),
            Self::EntryOutsideFile(detail) => write!(f, "entry slice lies outside the archive file: {detail}"),
        }
    }
}

impl ObjectError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) | Self::ArchiveUnreadable(_)
            | Self::EntryIndexOutOfRange { .. } | Self::EntryOutsideFile(_) => 1,
            _ => 2,
        }
    }
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at.checked_add(4)?).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    bytes.get(at..at.checked_add(2)?).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

/// First 12-byte object name, terminated by NUL or the `0xCD` filler.
fn object_name(header: &[u8]) -> Option<String> {
    let field = header.get(0x4C..0x58)?;
    let end = field.iter().position(|&byte| byte == 0 || byte == 0xCD)?;
    let name = std::str::from_utf8(&field[..end]).ok()?.trim_end_matches('_');
    let valid = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    valid.then(|| name.to_string())
}

/// Parse one single-model object container. `N` inflates the relocation
/// directory, not a record count; `level::data_base` is reused for the same
/// `4 * roundup512(N)` base as the level containers.
pub fn parse_object(bytes: &[u8]) -> Result<ObjectModel, ObjectError> {
    let records = read_u32(bytes, 0).ok_or(ObjectError::TruncatedDirectory)?;
    if !(1..=MAX_RELOCATIONS).contains(&records) {
        return Err(ObjectError::CountOutOfRange(records));
    }
    if 4 + 4 * records as usize > bytes.len() {
        return Err(ObjectError::DirectoryPastEof);
    }
    let base = level::data_base(records);
    if base > bytes.len() {
        return Err(ObjectError::PaddingTruncated);
    }
    let data = &bytes[base..];
    let header = data.get(..0x64).ok_or(ObjectError::HeaderTruncated)?;
    let name = object_name(header).ok_or(ObjectError::NotObjectContainer)?;
    let table_at = read_u32(header, 0x0C).ok_or(ObjectError::HeaderTruncated)? as usize;
    let table_len = usize::from(read_u16(header, 0x0A).ok_or(ObjectError::HeaderTruncated)?) + 1;
    let table_end = table_at.checked_add(4 * table_len).ok_or(ObjectError::TableOutOfRange)?;
    let table = data.get(table_at..table_end).ok_or(ObjectError::TableOutOfRange)?;
    let model_at = read_u32(table, 0).ok_or(ObjectError::TableOutOfRange)? as usize;
    if model_at == 0 {
        return Err(ObjectError::MultiPart);
    }
    let model_end = model_at.checked_add(0x34).ok_or(ObjectError::ModelHeaderOutOfRange)?;
    let model = data.get(model_at..model_end).ok_or(ObjectError::ModelHeaderOutOfRange)?;
    let vertex_count = read_u32(model, 0x00).unwrap_or(0);
    let vertex_start = read_u32(model, 0x04).unwrap_or(0) as usize;
    let end_of_polygons = read_u32(model, 0x0C).unwrap_or(0) as usize;
    let polygon_count = read_u32(model, 0x10).unwrap_or(0);
    let polygon_start = read_u32(model, 0x14).unwrap_or(0) as usize;
    let material_start = read_u32(model, 0x30).unwrap_or(0) as usize;
    let vertex_end = vertex_start + vertex_count as usize * VERTEX_STRIDE;
    if vertex_end != polygon_start {
        return Err(ObjectError::VertexBoundary { vertex_start, vertex_count, polygon_start });
    }
    let polygon_end = polygon_start + polygon_count as usize * POLYGON_STRIDE;
    if polygon_end != end_of_polygons {
        return Err(ObjectError::PolygonBoundary { polygon_start, polygon_count, end_of_polygons });
    }
    if polygon_end > data.len() {
        return Err(ObjectError::PolygonArrayOutOfRange);
    }
    let vertices = data[vertex_start..vertex_end].chunks_exact(VERTEX_STRIDE).map(|v| {
        let position = [i16::from_le_bytes([v[0], v[1]]), i16::from_le_bytes([v[2], v[3]]), i16::from_le_bytes([v[4], v[5]])];
        (position, u16::from_le_bytes([v[6], v[7]]))
    }).collect();
    let mut offsets = BTreeSet::new();
    let mut polygons = Vec::with_capacity(polygon_count as usize);
    let mut out_of_range = 0;
    for (index, p) in data[polygon_start..polygon_end].chunks_exact(POLYGON_STRIDE).enumerate() {
        let indices = [u16::from_le_bytes([p[0], p[1]]), u16::from_le_bytes([p[2], p[3]]), u16::from_le_bytes([p[4], p[5]])];
        let material = read_u32(p, 8).unwrap_or(0);
        if indices.iter().any(|&vertex| vertex as usize >= vertex_count as usize) {
            out_of_range += 1;
        }
        if p[7] & 0x02 != 0 {
            let offset = material as usize;
            let aligned = offset >= material_start && (offset - material_start) % MATERIAL_STRIDE == 0;
            if !aligned || offset + MATERIAL_STRIDE > data.len() {
                return Err(ObjectError::MaterialOffsetInvalid { polygon: index as u32, offset: material });
            }
            offsets.insert(material);
        }
        polygons.push((indices, p[6], p[7], material));
    }
    if out_of_range > 0 {
        return Err(ObjectError::IndexOutOfRange { polygons: out_of_range, vertex_count });
    }
    let materials = offsets.iter().map(|&offset| {
        let at = offset as usize;
        let mut record = [0u8; MATERIAL_STRIDE];
        record.copy_from_slice(&data[at..at + MATERIAL_STRIDE]);
        record
    }).collect();
    Ok(ObjectModel {
        name, vertices, polygons, materials,
        counts: Counts { vertices: vertex_count, polygons: polygon_count, materials: offsets.len() as u32 },
        report: ObjectReport {
            vertex_start, vertex_count, vertex_end, polygon_start, polygon_count,
            polygon_end, end_of_polygons, out_of_range,
        },
    })
}

/// Read and parse one archive entry. The bytes stay in memory; nothing is
/// extracted or written.
pub fn read_entry(path: &Path, index: usize) -> Result<ObjectModel, ObjectError> {
    let entries = bigfile::load_index(path)
        .map_err(|error| ObjectError::ArchiveUnreadable(error.to_string()))?;
    let entry = entries.get(index)
        .ok_or(ObjectError::EntryIndexOutOfRange { index, entries: entries.len() })?;
    let bytes = bigfile::read_entry(path, entry).map_err(|error| match error {
        error @ bigfile::BigfileError::BadEntry(_) => ObjectError::EntryOutsideFile(error.to_string()),
        error => ObjectError::ArchiveUnreadable(error.to_string()),
    })?;
    parse_object(&bytes)
}

/// Lines for `--object-info`: counts, both boundary identities and the index
/// scan. A violated boundary or a non-zero index count never reaches this call.
pub fn info_lines(model: &ObjectModel) -> Vec<String> {
    let r = &model.report;
    vec![
        format!("object {} vertices={} polygons={} materials={}", model.name, model.counts.vertices, model.counts.polygons, model.counts.materials),
        format!("  vertex boundary: vertexStart {} + vertexCount {}*8 = {} == polygonStart {} (satisfied)", r.vertex_start, r.vertex_count, r.vertex_end, r.polygon_start),
        format!("  polygon boundary: polygonStart {} + polygonCount {}*12 = {} == endOfPolygons {} (satisfied)", r.polygon_start, r.polygon_count, r.polygon_end, r.end_of_polygons),
        format!("  indices out_of_range={} (must be 0)", r.out_of_range),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic single-model container: 3 vertices, 2 polygons, 1 material.
    fn model() -> Vec<u8> {
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.resize(level::data_base(1), 0);
        let mut data = vec![0u8; 0x100];
        data[0x0C..0x10].copy_from_slice(&100u32.to_le_bytes());
        data[0x4C..0x54].copy_from_slice(b"raziel__");
        data[100..104].copy_from_slice(&120u32.to_le_bytes());
        data[120..124].copy_from_slice(&3u32.to_le_bytes());
        data[124..128].copy_from_slice(&176u32.to_le_bytes());
        data[132..136].copy_from_slice(&224u32.to_le_bytes());
        data[136..140].copy_from_slice(&2u32.to_le_bytes());
        data[140..144].copy_from_slice(&200u32.to_le_bytes());
        data[168..172].copy_from_slice(&228u32.to_le_bytes());
        for (i, vertex) in [[1i16, 2, 3], [4, 5, 6], [7, 8, 9]].iter().enumerate() {
            let at = 176 + i * 8;
            for (j, value) in vertex.iter().enumerate() {
                data[at + 2 * j..at + 2 * j + 2].copy_from_slice(&value.to_le_bytes());
            }
            data[at + 6..at + 8].copy_from_slice(&(7 + i as u16).to_le_bytes());
        }
        data[200..206].copy_from_slice(&[0, 0, 1, 0, 2, 0]);
        data[206] = 5;
        data[207] = 0x02;
        data[208..212].copy_from_slice(&228u32.to_le_bytes());
        data[212..218].copy_from_slice(&[0, 0, 2, 0, 1, 0]);
        data[220..224].copy_from_slice(&0x0011_2233u32.to_le_bytes());
        data[228..244].copy_from_slice(&[0xAB; 16]);
        bytes.extend_from_slice(&data);
        bytes
    }

    #[test]
    fn parses_counts_arrays_and_both_boundaries() {
        let parsed = parse_object(&model()).expect("synthetic object must parse");
        assert_eq!(parsed.name, "raziel");
        assert_eq!(parsed.counts, Counts { vertices: 3, polygons: 2, materials: 1 });
        assert_eq!(parsed.vertices[0], ([1, 2, 3], 7));
        assert_eq!(parsed.polygons[0], ([0, 1, 2], 5, 0x02, 228));
        assert_eq!(parsed.materials[0], [0xAB; 16]);
        assert_eq!((parsed.report.vertex_end, parsed.report.polygon_end), (200, 224));
        assert_eq!(parsed.report.out_of_range, 0);
    }

    #[test]
    fn refuses_multipart_and_non_object_headers() {
        let mut bytes = model();
        let table = level::data_base(1) + 100;
        bytes[table..table + 4].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(parse_object(&bytes), Err(ObjectError::MultiPart));
        let mut bytes = model();
        bytes[level::data_base(1) + 0x4C] = 0xFF;
        assert_eq!(parse_object(&bytes), Err(ObjectError::NotObjectContainer));
    }

    #[test]
    fn rejects_bad_indices_boundaries_and_material_offsets() {
        let polygon = level::data_base(1) + 200;
        let model_at = level::data_base(1) + 120;
        let mut bytes = model();
        bytes[polygon + 4..polygon + 6].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(
            parse_object(&bytes),
            Err(ObjectError::IndexOutOfRange { polygons: 1, vertex_count: 3 })
        ));
        let mut bytes = model();
        bytes[model_at + 0x0C..model_at + 0x10].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(parse_object(&bytes), Err(ObjectError::PolygonBoundary { .. })));
        let mut bytes = model();
        bytes[polygon + 8..polygon + 12].copy_from_slice(&229u32.to_le_bytes());
        assert!(matches!(parse_object(&bytes), Err(ObjectError::MaterialOffsetInvalid { .. })));
    }

    #[test]
    fn rejects_truncated_directories_and_headers() {
        let bytes = model();
        assert_eq!(parse_object(&[]), Err(ObjectError::TruncatedDirectory));
        assert_eq!(parse_object(&bytes[..4]), Err(ObjectError::DirectoryPastEof));
        assert_eq!(
            parse_object(&bytes[..level::data_base(1) - 1]),
            Err(ObjectError::PaddingTruncated)
        );
        assert_eq!(
            parse_object(&bytes[..level::data_base(1) + 0x20]),
            Err(ObjectError::HeaderTruncated)
        );
        assert_eq!(
            parse_object(&bytes[..level::data_base(1) + 100]),
            Err(ObjectError::TableOutOfRange)
        );
    }
}

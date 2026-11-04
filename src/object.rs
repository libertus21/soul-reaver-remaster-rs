//! SR1 archived object-model containers: relocation directory, object header,
//! bone table, vertex/polygon arrays and the 16-byte material records they
//! reference. Vertices are bone-local; [`rest_positions`] resolves them to the
//! rest pose. In-memory decode only: no rendering, textures or world transform.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::bigfile;
use crate::level;

pub const VERTEX_STRIDE: usize = 8;
pub const POLYGON_STRIDE: usize = 12;
pub const MATERIAL_STRIDE: usize = 16;
pub const BONE_STRIDE: usize = 24;
const MAX_RELOCATIONS: u32 = 1 << 20;
pub const USAGE: &str = "expected --object-info --bigfile <DAT> --entry <INDEX>";
/// `([x, y, z], normal id)`; the position is **bone-local**, not model-space.
pub type Vertex = ([i16; 3], u16);
/// `([v1, v2, v3], normal, flags, material offset or colour)`.
pub type Polygon = ([u16; 3], u8, u8, u32);

/// Low 11 bits of the texture word are the bank page; the upper five are
/// UNKNOWN flags and must never leak into the page index.
pub const TEXTURE_PAGE_MASK: u16 = 0x07FF;

/// Decoded PC `TextureFT3` face record. The attribute words and colour stay
/// raw and uninterpreted; only the UVs and the masked page are used.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TextureRef {
    pub uvs: [[u8; 2]; 3],
    pub texture_word: u16,
    pub page: u16,
    pub attributes_a: u16,
    pub attributes: u16,
    pub colour: u32,
}

impl TextureRef {
    fn from_record(record: &[u8; MATERIAL_STRIDE]) -> Self {
        let word = u16::from_le_bytes([record[2], record[3]]);
        Self {
            uvs: [[record[0], record[1]], [record[4], record[5]], [record[8], record[9]]],
            texture_word: word,
            page: word & TEXTURE_PAGE_MASK,
            attributes_a: u16::from_le_bytes([record[6], record[7]]),
            attributes: u16::from_le_bytes([record[10], record[11]]),
            colour: u32::from_le_bytes([record[12], record[13], record[14], record[15]]),
        }
    }
}

/// One 24-byte bone record. Vertex positions stay in this bone's local frame
/// until [`rest_positions`] adds the accumulated chain of `local` translations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bone {
    pub first_vertex: u16, pub last_vertex: u16,
    pub local: [i16; 3], pub parent: u16,
}

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
/// textured polygons reference, in ascending offset order. `bones` is the raw
/// bone table; `vertices` keep their on-disk bone-local coordinates.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ObjectModel {
    pub name: String, pub vertices: Vec<Vertex>, pub polygons: Vec<Polygon>,
    /// One entry per polygon: the decoded record for a textured face, else
    /// `None`. `materials` still holds the distinct raw records.
    pub textures: Vec<Option<TextureRef>>,
    pub materials: Vec<[u8; MATERIAL_STRIDE]>, pub bones: Vec<Bone>,
    pub counts: Counts, pub report: ObjectReport,
}

/// Every way an object container can be refused. Each variant has its own text.
#[derive(Debug, PartialEq, Eq)]
pub enum ObjectError {
    Usage(&'static str), TruncatedDirectory, CountOutOfRange(u32), DirectoryPastEof,
    PaddingTruncated, HeaderTruncated, NotObjectContainer, TableOutOfRange, MultiPart,
    ModelHeaderOutOfRange, PolygonArrayOutOfRange, BoneTableOutOfRange,
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
            Self::BoneTableOutOfRange => f.write_str("bone table runs outside the payload"),
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
    let bone_count = read_u32(model, 0x18).unwrap_or(0) as usize;
    let bone_start = read_u32(model, 0x1C).unwrap_or(0) as usize;
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
    let bone_end = bone_start.saturating_add(bone_count.saturating_mul(BONE_STRIDE));
    if bone_count > 0 && (bone_start == 0 || bone_end > data.len()) {
        return Err(ObjectError::BoneTableOutOfRange);
    }
    let vertices = data[vertex_start..vertex_end].chunks_exact(VERTEX_STRIDE).map(|v| {
        let position = [i16::from_le_bytes([v[0], v[1]]), i16::from_le_bytes([v[2], v[3]]), i16::from_le_bytes([v[4], v[5]])];
        (position, u16::from_le_bytes([v[6], v[7]]))
    }).collect();
    let bones = (0..bone_count).map(|index| {
        let record = &data[bone_start + index * BONE_STRIDE..bone_start + (index + 1) * BONE_STRIDE];
        Bone {
            first_vertex: u16::from_le_bytes([record[8], record[9]]),
            last_vertex: u16::from_le_bytes([record[10], record[11]]),
            local: [
                i16::from_le_bytes([record[12], record[13]]),
                i16::from_le_bytes([record[14], record[15]]),
                i16::from_le_bytes([record[16], record[17]]),
            ],
            parent: u16::from_le_bytes([record[18], record[19]]),
        }
    }).collect();
    let mut offsets = BTreeSet::new();
    let mut polygons = Vec::with_capacity(polygon_count as usize);
    let mut textures = Vec::with_capacity(polygon_count as usize);
    let mut out_of_range = 0;
    for (index, p) in data[polygon_start..polygon_end].chunks_exact(POLYGON_STRIDE).enumerate() {
        let indices = [u16::from_le_bytes([p[0], p[1]]), u16::from_le_bytes([p[2], p[3]]), u16::from_le_bytes([p[4], p[5]])];
        let material = read_u32(p, 8).unwrap_or(0);
        if indices.iter().any(|&vertex| vertex as usize >= vertex_count as usize) {
            out_of_range += 1;
        }
        let texture = if p[7] & 0x02 != 0 {
            let offset = material as usize;
            let aligned = offset >= material_start && (offset - material_start) % MATERIAL_STRIDE == 0;
            if !aligned || offset + MATERIAL_STRIDE > data.len() {
                return Err(ObjectError::MaterialOffsetInvalid { polygon: index as u32, offset: material });
            }
            offsets.insert(material);
            let mut record = [0u8; MATERIAL_STRIDE];
            record.copy_from_slice(&data[offset..offset + MATERIAL_STRIDE]);
            Some(TextureRef::from_record(&record))
        } else {
            None
        };
        textures.push(texture);
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
        name, vertices, polygons, textures, bones, materials,
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

/// Accumulated bone translation for each bone: its own `local` offset plus
/// every ancestor's, exactly once. A bone that lists itself as parent ends the
/// chain; `0xFFFF` marks an unparented (root) bone.
fn bone_world_offsets(bones: &[Bone]) -> Vec<[i32; 3]> {
    bones.iter().enumerate().map(|(index, _)| {
        let mut offset = [0i32; 3];
        let mut cursor = index;
        for _ in 0..bones.len() {
            let Some(bone) = bones.get(cursor) else { break };
            for axis in 0..3 {
                offset[axis] += i32::from(bone.local[axis]);
            }
            if bone.parent == u16::MAX || bone.parent as usize == cursor {
                break;
            }
            cursor = bone.parent as usize;
        }
        offset
    }).collect()
}

/// Rest-pose model-space positions: the on-disk values are bone-local, so each
/// vertex in a bone's `first_vertex..=last_vertex` range is translated by that
/// bone's accumulated translation. Skipping this leaves every body part at its
/// bone origin, which is the "triangle jumble" symptom.
pub fn rest_positions(model: &ObjectModel) -> Vec<[f32; 3]> {
    let mut positions: Vec<[i32; 3]> = model.vertices.iter().map(|(p, _)| p.map(i32::from)).collect();
    if positions.is_empty() {
        return Vec::new();
    }
    let offsets = bone_world_offsets(&model.bones);
    for (bone, offset) in model.bones.iter().zip(&offsets) {
        if bone.first_vertex == u16::MAX || bone.last_vertex == u16::MAX {
            continue;
        }
        let first = bone.first_vertex as usize;
        let last = (bone.last_vertex as usize).min(positions.len() - 1);
        if first > last {
            continue;
        }
        for position in &mut positions[first..=last] {
            for axis in 0..3 {
                position[axis] += offset[axis];
            }
        }
    }
    positions.into_iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect()
}

/// Rest-pose model-space triangle mesh: bone-local `i16` positions are
/// translated by the bone table and widened 1:1, and every polygon contributes
/// its three `u16` indices as one triangle. Double-sided faces stay as two
/// opposite-winding triangles; both are real draw calls.
pub fn triangle_mesh(model: &ObjectModel) -> (Vec<[f32; 3]>, Vec<u32>) {
    let positions = rest_positions(model);
    let indices = model
        .polygons
        .iter()
        .flat_map(|(corners, _, _, _)| corners.iter().map(|&corner| u32::from(corner)))
        .collect();
    (positions, indices)
}

/// Rest-pose triangle mesh with per-face UVs: every polygon contributes three
/// unique vertices so a face's three UVs never fight a shared corner. Material
/// 1 is the flat fallback (untextured face); used pages start at material 2 in
/// `pages` first-use order.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TexturedMesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[u8; 2]>,
    pub indices: Vec<u32>,
    pub material_indices: Vec<u8>,
    pub pages: Vec<u16>,
}

/// 1-based material index for `page`: flat is 1, then a stable page list.
pub(crate) fn page_material(pages: &mut Vec<u16>, page: u16) -> u8 {
    match pages.iter().position(|&known| known == page) {
        Some(index) => index as u8 + 2,
        None => {
            pages.push(page);
            pages.len() as u8 + 1
        }
    }
}

/// Per-face textured mesh in rest pose. Untextured faces stay as flat
/// material 1 so no geometry is dropped.
pub fn textured_mesh(model: &ObjectModel) -> TexturedMesh {
    let positions = rest_positions(model);
    let mut out = TexturedMesh::default();
    for ((corners, _, _, _), texture) in model.polygons.iter().zip(&model.textures) {
        let base = out.positions.len() as u32;
        let material = texture.map_or(1, |record| page_material(&mut out.pages, record.page));
        for (corner, &vertex) in corners.iter().enumerate() {
            out.positions.push(positions[vertex as usize]);
            out.uvs.push(texture.map_or([0, 0], |record| record.uvs[corner]));
            out.material_indices.push(material);
        }
        out.indices.extend([base, base + 1, base + 2]);
    }
    out
}

/// The index dword echoes the last four characters of an asset name,
/// uppercased; `None` for names shorter than four characters.
fn suffix_key(name: &str) -> Option<[u8; 4]> {
    let bytes = name.as_bytes();
    let mut key = [0u8; 4];
    key.copy_from_slice(bytes.get(bytes.len().checked_sub(4)?..)?);
    key.make_ascii_uppercase();
    Some(key)
}

/// Resolve placed-object names to archived models through the index suffix.
/// The index is loaded once; resolved meshes are cached per placed name.
pub struct ModelResolver {
    path: PathBuf,
    entries: Vec<bigfile::BigfileEntry>,
    by_suffix: HashMap<[u8; 4], Vec<usize>>,
    cache: HashMap<String, Option<(Vec<[f32; 3]>, Vec<u32>)>>,
    textured_cache: HashMap<String, Option<TexturedMesh>>,
}

impl ModelResolver {
    pub fn load(path: &Path) -> Result<Self, ObjectError> {
        let entries = bigfile::load_index(path)
            .map_err(|error| ObjectError::ArchiveUnreadable(error.to_string()))?;
        let mut by_suffix: HashMap<[u8; 4], Vec<usize>> = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            by_suffix.entry(entry.suffix).or_default().push(index);
        }
        Ok(Self { path: path.to_path_buf(), entries, by_suffix, cache: HashMap::new(), textured_cache: HashMap::new() })
    }

    /// Model-space mesh for `name`, or `None` when no container matches.
    pub fn mesh(&mut self, name: &str) -> Option<(Vec<[f32; 3]>, Vec<u32>)> {
        if !self.cache.contains_key(name) {
            let mesh = self.lookup(name).map(|model| triangle_mesh(&model));
            self.cache.insert(name.to_string(), mesh);
        }
        self.cache.get(name).cloned().flatten()
    }

    /// Per-face textured mesh for `name`, cached separately from the flat mesh.
    pub fn textured(&mut self, name: &str) -> Option<TexturedMesh> {
        if !self.textured_cache.contains_key(name) {
            let mesh = self.lookup(name).map(|model| textured_mesh(&model));
            self.textured_cache.insert(name.to_string(), mesh);
        }
        self.textured_cache.get(name).cloned().flatten()
    }

    fn lookup(&self, name: &str) -> Option<ObjectModel> {
        for &index in self.by_suffix.get(&suffix_key(name)?)? {
            let Some(entry) = self.entries.get(index) else { continue };
            let Ok(bytes) = bigfile::read_entry(&self.path, entry) else { continue };
            if let Ok(model) = parse_object(&bytes)
                && model.name.eq_ignore_ascii_case(name)
            {
                return Some(model);
            }
        }
        None
    }
}

/// Lines for `--object-info`: counts, both boundary identities and the index
/// scan. A violated boundary or a non-zero index count never reaches this call.
pub fn info_lines(model: &ObjectModel) -> Vec<String> {
    let r = &model.report;
    let ranged = model.bones.iter()
        .filter(|bone| bone.first_vertex != u16::MAX && bone.last_vertex != u16::MAX)
        .count();
    vec![
        format!("object {} vertices={} polygons={} materials={} bones={}", model.name, model.counts.vertices, model.counts.polygons, model.counts.materials, model.bones.len()),
        format!("  vertex boundary: vertexStart {} + vertexCount {}*8 = {} == polygonStart {} (satisfied)", r.vertex_start, r.vertex_count, r.vertex_end, r.polygon_start),
        format!("  polygon boundary: polygonStart {} + polygonCount {}*12 = {} == endOfPolygons {} (satisfied)", r.polygon_start, r.polygon_count, r.polygon_end, r.end_of_polygons),
        format!("  indices out_of_range={} (must be 0)", r.out_of_range),
        format!("  rest pose: {ranged}/{} bones displace their vertex ranges; vertices are bone-local until applied", model.bones.len()),
    ]
}

/// Synthetic single-model container shared with the CLI tests: 3 vertices,
/// 2 polygons, 1 material.
#[cfg(test)]
pub(crate) fn synthetic_model() -> Vec<u8> {
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

/// Synthetic two-bone container: a tetrahedron whose two upper vertices are
/// stored in a bone-local frame 10 units below their rest position, plus one
/// opposite-winding duplicate face (a double-sided quad-less triangle).
#[cfg(test)]
pub(crate) fn synthetic_bone_model() -> Vec<u8> {
    let mut bytes = 1u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&12u32.to_le_bytes());
    bytes.resize(level::data_base(1), 0);
    let mut data = vec![0u8; 0x200];
    data[0x0C..0x10].copy_from_slice(&100u32.to_le_bytes());
    data[0x4C..0x54].copy_from_slice(b"raziel__");
    data[100..104].copy_from_slice(&120u32.to_le_bytes());
    for (offset, value) in [(0x00, 4u32), (0x04, 220), (0x0C, 312), (0x10, 5), (0x14, 252), (0x18, 2), (0x1C, 172), (0x30, 312)] {
        data[120 + offset..124 + offset].copy_from_slice(&value.to_le_bytes());
    }
    data[180..182].copy_from_slice(&0u16.to_le_bytes());
    data[182..184].copy_from_slice(&1u16.to_le_bytes());
    data[190..192].copy_from_slice(&0u16.to_le_bytes());
    data[204..206].copy_from_slice(&2u16.to_le_bytes());
    data[206..208].copy_from_slice(&3u16.to_le_bytes());
    data[212..214].copy_from_slice(&10i16.to_le_bytes());
    data[214..216].copy_from_slice(&0u16.to_le_bytes());
    for (index, vertex) in [[0i16, 0, 0], [10, 0, 0], [0, 10, -10], [0, 0, 0]].iter().enumerate() {
        let at = 220 + index * 8;
        for (axis, value) in vertex.iter().enumerate() {
            data[at + 2 * axis..at + 2 * axis + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    for (index, corners) in [[0u16, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3], [0, 1, 2]].iter().enumerate() {
        let at = 252 + index * POLYGON_STRIDE;
        for (axis, value) in corners.iter().enumerate() {
            data[at + 2 * axis..at + 2 * axis + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&data);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Vec<u8> {
        synthetic_model()
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

    /// Minimal archive whose index entry carries `suffix` in its 4th dword.
    fn archive(suffix: [u8; 4], payload: Vec<u8>) -> Vec<u8> {
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&20u32.to_le_bytes());
        bytes.extend_from_slice(&suffix);
        bytes.extend_from_slice(&payload);
        bytes
    }

    #[test]
    fn triangle_mesh_widens_positions_and_keeps_each_first_triangle() {
        let parsed = parse_object(&model()).expect("synthetic object must parse");
        let (positions, indices) = triangle_mesh(&parsed);
        assert_eq!(positions, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 1]);
    }

    /// Unique vertices, unique faces and unique undirected edges after welding
    /// coincident positions and collapsing double-sided duplicates.
    fn welded_topology(positions: &[[f32; 3]], indices: &[u32]) -> (usize, usize, usize, usize) {
        let mut welded = Vec::with_capacity(positions.len());
        let mut seen: HashMap<[i32; 3], usize> = HashMap::new();
        for position in positions {
            let key = [position[0] as i32, position[1] as i32, position[2] as i32];
            let next = seen.len();
            welded.push(*seen.entry(key).or_insert(next));
        }
        let mut faces: BTreeSet<[usize; 3]> = BTreeSet::new();
        let mut edges: HashMap<(usize, usize), u32> = HashMap::new();
        let mut used: BTreeSet<usize> = BTreeSet::new();
        for triangle in indices.chunks_exact(3) {
            let mut corners = [
                welded[triangle[0] as usize],
                welded[triangle[1] as usize],
                welded[triangle[2] as usize],
            ];
            corners.sort_unstable();
            if corners[0] == corners[1] || corners[1] == corners[2] || !faces.insert(corners) {
                continue;
            }
            used.extend(corners);
            for (a, b) in [(corners[0], corners[1]), (corners[1], corners[2]), (corners[0], corners[2])] {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
        let nonmanifold = edges.values().filter(|&&count| count > 2).count();
        (used.len(), edges.len(), faces.len(), nonmanifold)
    }

    #[test]
    fn rest_pose_applies_bone_offsets_and_is_a_closed_euler_two_mesh() {
        let parsed = parse_object(&synthetic_bone_model()).expect("bone model must parse");
        assert_eq!(parsed.bones.len(), 2);
        assert_eq!(parsed.bones[1].local, [0, 0, 10]);
        let (positions, indices) = triangle_mesh(&parsed);
        assert_eq!(
            positions,
            vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 10.0]]
        );
        assert_eq!(indices, vec![0, 2, 1, 0, 1, 3, 1, 2, 3, 2, 0, 3, 0, 1, 2]);
        let (vertices, edges, faces, nonmanifold) = welded_topology(&positions, &indices);
        assert_eq!((vertices, edges, faces), (4, 6, 4), "double-sided duplicate counts once");
        assert_eq!(
            vertices as isize - edges as isize + faces as isize,
            2,
            "a closed rest-pose mesh has Euler characteristic 2"
        );
        assert_eq!(nonmanifold, 0);
    }

    #[test]
    fn rejects_a_bone_table_outside_the_payload() {
        let mut bytes = synthetic_bone_model();
        let model_at = level::data_base(1) + 120;
        bytes[model_at + 0x18..model_at + 0x1C].copy_from_slice(&20u32.to_le_bytes());
        assert_eq!(parse_object(&bytes), Err(ObjectError::BoneTableOutOfRange));
    }

    #[test]
    fn resolver_keys_on_the_uppercase_suffix_then_verifies_the_name() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, archive(*b"ZIEL", model())).expect("synthetic archive");
        let mut resolver = ModelResolver::load(&path).expect("index must load");
        assert_eq!(resolver.mesh("raziel").map(|(v, i)| (v.len(), i.len())), Some((3, 6)));
        assert_eq!(resolver.mesh("raziel").map(|(v, _)| v.len()), Some(3), "cached");
        assert!(resolver.mesh("campath").is_none(), "suffix LOCK does not match ZIEL");
        assert!(resolver.mesh("raz").is_none(), "names shorter than four characters do not map");
    }

    #[test]
    fn exposes_face_records_and_masks_the_page() {
        let mut bytes = model();
        let record = level::data_base(1) + 228;
        let raw = [40, 134, 0x04, 0x08, 22, 134, 0x08, 0x01, 41, 128, 0, 0, 0x80, 0x80, 0x80, 0x34];
        bytes[record..record + 16].copy_from_slice(&raw);
        let parsed = parse_object(&bytes).expect("synthetic object must parse");
        let texture = parsed.textures[0].expect("first face is textured");
        assert_eq!(texture.uvs, [[40, 134], [22, 134], [41, 128]]);
        assert_eq!(texture.texture_word, 0x0804, "the raw word keeps its flags");
        assert_eq!(texture.page, 4, "the page is the low 11 bits only");
        assert_eq!(
            (texture.attributes_a, texture.attributes, texture.colour),
            (0x0108, 0x0000, 0x3480_8080)
        );
        assert_eq!(parsed.textures[1], None, "the untextured face has no record");
        assert_eq!(&parsed.materials[0][0..4], &raw[0..4], "raw records stay exposed");
    }

    #[test]
    fn textured_mesh_expands_faces_and_keeps_flat_fallback() {
        let mut bytes = model();
        let record = level::data_base(1) + 228;
        bytes[record + 2..record + 4].copy_from_slice(&4u16.to_le_bytes());
        let parsed = parse_object(&bytes).expect("synthetic object must parse");
        let mesh = textured_mesh(&parsed);
        assert_eq!(mesh.pages, vec![4]);
        assert_eq!(mesh.positions.len(), 6);
        assert_eq!(mesh.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(mesh.material_indices, vec![2, 2, 2, 1, 1, 1]);
        assert_eq!(&mesh.uvs[..3], &[[0xAB, 0xAB]; 3]);
        assert_eq!(&mesh.uvs[3..], &[[0, 0]; 3], "the untextured face samples flat");
        assert_eq!(mesh.positions[3], [1.0, 2.0, 3.0]);
        assert_eq!(mesh.positions[5], [4.0, 5.0, 6.0]);
    }
}


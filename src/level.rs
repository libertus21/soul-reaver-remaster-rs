//! Untrusted SR1-remaster room containers: the `.PCM` header, name table, and
//! the room-geometry summary. Read-only: no placement, transform, or audio
//! decode.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::bigfile;

pub const MAX_RECORDS: u32 = 1 << 20;
pub const MAX_SEGMENTS: u16 = 1 << 10;
pub const MAX_NAME_LEN: usize = 20;

#[derive(Debug)]
pub struct LevelUnit {
    pub name: String,
    pub records: u32,
    pub names: Vec<String>,
    pub links: Vec<Link>,
    pub sidecar: Sidecar,
    /// One entry per offset-table record, in table order.
    pub block_sizes: Vec<u32>,
    /// Parallel to `block_sizes`: whether every byte of that block is zero.
    pub block_all_zero: Vec<bool>,
    /// Header sanity, relocation-directory integrity, and header offset slots.
    pub structure: LevelStructure,
    /// Records decoded from the header `0x34` link table, in table order.
    pub link_table: Vec<TableLink>,
}

/// Shape class of one block. Labels describe a size (and zero-ness) only;
/// no block carries a type tag, so no semantic name is claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockClass {
    Zero,
    Small4,
    Fixed40,
    Fixed48,
    Other(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub struct BlockCensus {
    pub total: u32,
    pub zero: u32,
    pub small4: u32,
    pub fixed40: u32,
    pub fixed48: u32,
    /// `(size, count)` for every other size, ascending by size.
    pub other: Vec<(u32, u32)>,
    /// `(ordinal, size)`; ties go to the lowest ordinal.
    pub largest: (u32, u32),
}

/// Header dword slots the builder walks as base-relative offsets. The note
/// enumerates these individually and leaves `0x1B..=0x32` unenumerated, so
/// that range is deliberately not claimed here.
pub const SCHEMA_OFFSET_SLOTS: [usize; 14] =
    [0x00, 0x01, 0x03, 0x0D, 0x15, 0x17, 0x23, 0x26, 0x34, 0x35, 0x37, 0x39, 0x3B, 0x3D];

/// 36-byte source records: count at header slot `0x3A`, offset at `0x3B`.
const TABLE36_STRIDE: usize = 0x24;
const SLOT_TABLE36_COUNT: usize = 0x3A;
const SLOT_TABLE36_OFFSET: usize = 0x3B;

/// Decoded link table: header slot `0x34` holds a data-base-relative offset and
/// slot `0x35` its end; records are 40 bytes,
/// `[5 x u32][12-byte name][2 x u32]`. The table ends at the first record whose
/// name is not `<target>,<digits>`.
const SLOT_LINK_TABLE: usize = 0x34;
const SLOT_LINK_END: usize = 0x35;
const LINK_RECORD_STRIDE: usize = 40;
const LINK_NAME_LEN: usize = 12;

/// Directory integrity: entry `i` is the position of a dword that holds a
/// non-zero data-base-relative offset.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RelocationReport {
    pub entries: u32,
    pub valid: u32,
    pub zero: u32,
    pub out_of_range: u32,
}

/// The 36-byte source table: count, stride, and one blob length or null per
/// record. Field meaning stays UNKNOWN; only the tagged-blob length applies.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Table36Report {
    pub count: u32,
    pub stride: u32,
    pub lengths: Vec<Option<u32>>,
}

/// Structural facts about one unit. `slots` holds every non-null enumerated
/// header offset as `(slot, data_base + value)`. No record type is claimed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LevelStructure {
    pub data_base: usize,
    pub data_len: usize,
    pub reloc: RelocationReport,
    pub slots: Vec<(usize, usize)>,
    pub table36: Table36Report,
}

/// Room geometry behind header slot `0x00`: counts, data-base-relative array
/// offsets, the `i16` position box, and face-index validation. The trailing
/// vertex triple is not read; no normal or UV is claimed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GeometryReport {
    pub vertices: u32,
    pub faces: u32,
    pub room_offset: u32,
    pub vertex_offset: u32,
    pub face_offset: u32,
    pub min: [i16; 3],
    pub max: [i16; 3],
    /// Face indices that do not address a vertex; zero on every valid unit.
    pub out_of_range: u32,
    pub max_index: Option<u16>,
}

/// CPU mesh for one unit: `i16` positions widened 1:1 to `f32` and the face
/// indices passed through unchanged. No normal, UV or transform is derived.
#[derive(Debug, Default, PartialEq)]
pub struct RoomMesh {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Per-vertex UVs and 1-based material indices, plus the page list for
    /// materials 2..; empty unless `--textures` was requested.
    pub uvs: Vec<[u8; 2]>,
    pub material_indices: Vec<u8>,
    pub pages: Vec<u16>,
    /// Placed-object render counts; zero unless `--objects` was requested.
    pub objects: ObjectStats,
    /// First object named `raziel`, widened to world `f32`; the fly camera's
    /// optional start point. `None` when the room places no such record.
    pub spawn: Option<[f32; 3]>,
    /// Faces that resolved to a 12-byte texture record, and the tail faces
    /// whose `+0x0A` offset was `0xFFFF` or not 12-aligned. Both are counted;
    /// unresolved faces draw flat, never dropped.
    pub textured_faces: u32,
    pub unresolved_faces: u32,
}

/// One placed object behind header slot `0x00`. `object_type` is the high 16
/// bits of the record's `+0x28` dword and `flags` is the raw `+0x2C` dword.
/// The record carries no rotation.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ObjectRecord {
    pub name: String,
    pub position: [i16; 3],
    pub object_type: u16,
    pub flags: u32,
}

/// Object array beside the room geometry: count, data-base-relative offset and
/// the decoded records. Positions are world space; nothing is transformed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ObjectReport {
    pub room_offset: u32,
    pub count: u32,
    pub array_offset: u32,
    pub objects: Vec<ObjectRecord>,
}

/// How `--objects` rendered the placed records: total, real meshes and the
/// cube fallbacks used for names with no archived model.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStats {
    pub total: u32,
    pub meshes: u32,
    pub markers: u32,
}

/// Resolves one placed-object name to a model-space triangle mesh.
pub type ObjectModelSource<'a> =
    &'a mut dyn FnMut(&str) -> Option<(Vec<[f32; 3]>, Vec<u32>)>;

/// Resolves one placed-object name to a per-face textured model-space mesh.
pub type TexturedObjectSource<'a> =
    &'a mut dyn FnMut(&str) -> Option<crate::object::TexturedMesh>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub index: u32,
}

/// One 40-byte link-table record. Every prelude value is reported as a raw
/// signed 32-bit integer. `target_id` repeats prelude field 4, the value that
/// stays stable per target across units; no other field is interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLink {
    pub target: String,
    pub index: u32,
    pub target_id: i32,
    pub prelude: [i32; 5],
}

#[derive(Debug, PartialEq, Eq)]
pub enum Sidecar {
    Absent,
    Present { segments: u16, rate_field: u32, pnf_entries: u32 },
}

#[derive(Debug, PartialEq, Eq)]
pub struct PmfHeader { pub id: u16, pub segments: u16, pub rate_field: u32 }

#[derive(Debug, PartialEq, Eq)]
pub struct PnfHeader { pub id: u16, pub entries: u32 }

#[derive(Debug, PartialEq, Eq)]
pub enum LevelError {
    Usage(&'static str),
    RefusedPath,
    Io,
    NoUnits,
    CountCeiling,
    TableNotAscending,
    TablePastEof,
    PaddingTruncated,
    RelocationOutOfRange,
    GeometryRecordOutOfRange,
    GeometryArrayOutOfRange,
    RoomTexturesMissing,
    RoomTextureArrayOutOfRange,
    ObjectRecordOutOfRange,
    ObjectArrayOutOfRange,
    RenderDirectory,
    RenderEmpty,
    RenderArrayOutOfRange,
    MissingUnitName,
    SidecarIncomplete,
    BadSidecarMagic,
    SidecarMismatch,
    ArchiveUnreadable { detail: String },
    EntryIndexOutOfRange { index: usize, entries: usize },
    EntryOutsideFile { detail: String },
    EntryNotPcm { detail: String },
    ArchiveWithoutEntry,
    EntryWithoutArchive,
    GroupEmptyPrefix,
    GroupNoMatch { prefix: String },
    GroupParseFailed { name: String, detail: String },
    GroupWithoutArchive,
    GroupWithEntry,
    NoArchivedLevels,
    ReachStartMissing { name: String },
}

impl LevelError {
    pub fn exit_code(&self) -> u8 {
        match self {
            LevelError::Usage(_) => 1,
            LevelError::RefusedPath => 3,
            LevelError::RenderDirectory | LevelError::RenderEmpty
            | LevelError::RenderArrayOutOfRange => 1,
            LevelError::ArchiveUnreadable { .. }
            | LevelError::EntryIndexOutOfRange { .. }
            | LevelError::EntryOutsideFile { .. }
            | LevelError::EntryNotPcm { .. }
            | LevelError::ArchiveWithoutEntry
            | LevelError::EntryWithoutArchive
            | LevelError::GroupEmptyPrefix
            | LevelError::GroupNoMatch { .. }
            | LevelError::GroupParseFailed { .. }
            | LevelError::GroupWithoutArchive
            | LevelError::GroupWithEntry
            | LevelError::NoArchivedLevels
            | LevelError::ReachStartMissing { .. } => 1,
            _ => 2,
        }
    }
}

impl fmt::Display for LevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            LevelError::Usage(message) => message,
            LevelError::RefusedPath => "not a .PCM file and not a directory",
            LevelError::Io => "cannot read the requested path",
            LevelError::NoUnits => "no .PCM units in the directory",
            LevelError::CountCeiling => "record count out of range",
            LevelError::TableNotAscending => "offset table must start 0, 4 and strictly ascend",
            LevelError::TablePastEof => "offset table or last record runs past the file",
            LevelError::PaddingTruncated => {
                "offset table padding is missing: file ends before the level-data base"
            }
            LevelError::RelocationOutOfRange => {
                "relocation directory entry points outside the level data"
            }
            LevelError::GeometryRecordOutOfRange => {
                "room geometry record points outside the level data"
            }
            LevelError::GeometryArrayOutOfRange => {
                "room geometry vertex or face array runs outside the level data"
            }
            LevelError::RoomTexturesMissing => {
                "unit has no room texture record array (room record +0x34/+0x38 are empty)"
            }
            LevelError::RoomTextureArrayOutOfRange => {
                "room texture record array runs outside the level data or is not 12-byte aligned"
            }
            LevelError::ObjectRecordOutOfRange => {
                "room object record points outside the level data"
            }
            LevelError::ObjectArrayOutOfRange => {
                "room object array runs outside the level data"
            }
            LevelError::RenderDirectory => "expected one .PCM file, not a directory",
            LevelError::RenderEmpty => "unit has no room geometry (0 vertices or 0 faces)",
            LevelError::RenderArrayOutOfRange => {
                "room geometry array or face index is out of range"
            }
            LevelError::MissingUnitName => "name table does not contain the unit's own name",
            LevelError::SidecarIncomplete => "PMF and PNF must be present together",
            LevelError::BadSidecarMagic => "sidecar magic or header length is invalid",
            LevelError::SidecarMismatch => "PMF/PNF id, count, or length disagreement",
            LevelError::ArchiveUnreadable { detail } => {
                return write!(f, "cannot read the archive: {detail}");
            }
            LevelError::EntryIndexOutOfRange { index, entries } => {
                return write!(
                    f,
                    "entry index {index} is out of range (archive has {entries} entries)"
                );
            }
            LevelError::EntryOutsideFile { detail } => {
                return write!(f, "entry slice lies outside the archive file: {detail}");
            }
            LevelError::EntryNotPcm { detail } => {
                return write!(f, "entry does not parse as a .PCM: {detail}");
            }
            LevelError::ArchiveWithoutEntry => "--bigfile requires --entry <index>",
            LevelError::EntryWithoutArchive => "--entry requires --bigfile <DAT>",
            LevelError::GroupEmptyPrefix => "group prefix is empty",
            LevelError::GroupNoMatch { prefix } => {
                return write!(f, "no archive entry matches group prefix '{prefix}'");
            }
            LevelError::GroupParseFailed { name, detail } => {
                return write!(f, "group entry '{name}' does not parse as a level: {detail}");
            }
            LevelError::GroupWithoutArchive => "--group requires --bigfile <DAT>",
            LevelError::GroupWithEntry => "--group cannot be combined with --entry",
            LevelError::NoArchivedLevels => "archive has no level units that parse as .PCM",
            LevelError::ReachStartMissing { name } => {
                return write!(f, "start unit '{name}' is not in the archive");
            }
        };
        f.write_str(message)
    }
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}
fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?))
}
fn read_i16(bytes: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

/// Header slot `0x00` points at the room/zone record: `+0x10`/`+0x1C` give a
/// 12-byte vertex array, `+0x14`/`+0x20` a 12-byte face array whose first
/// three `u16` are vertex indices.
const SLOT_ROOM_RECORD: usize = 0x00;
/// Header word pointing at the unit's own NUL-terminated name.
const SLOT_UNIT_NAME: usize = 0x26;
const ROOM_VERTEX_COUNT: usize = 0x10;
const ROOM_FACE_COUNT: usize = 0x14;
const ROOM_VERTEX_OFFSET: usize = 0x1C;
const ROOM_FACE_OFFSET: usize = 0x20;
/// Data-base-relative start/end of the per-room 12-byte texture record array.
const ROOM_TEXTURE_START: usize = 0x34;
const ROOM_TEXTURE_END: usize = 0x38;
const TEXTURE_STRIDE: usize = 12;
/// Face `+0x0A`: byte offset into the texture record array, or `0xFFFF`.
const FACE_TEXTURE_OFFSET: usize = 0x0A;
const VERTEX_STRIDE: usize = 12;
const FACE_STRIDE: usize = 12;

/// Object array in the same room record: count at `+0x08`, data-base-relative
/// array offset at `+0x0C`, records of `0x4C` bytes.
const OBJECT_COUNT: usize = 0x08;
const OBJECT_OFFSET: usize = 0x0C;
const OBJECT_STRIDE: usize = 0x4C;
const OBJECT_NAME_LEN: usize = 12;
const OBJECT_X: usize = 0x20;
const OBJECT_TYPE: usize = 0x28;
const OBJECT_FLAGS: usize = 0x2C;

/// Data-region range of `count` records at a data-base-relative `offset`.
/// A zero count is always valid and reads nothing; a non-zero array that would
/// leave the region yields `None` so the caller can fail loud, never clamp.
fn array_range(len: usize, offset: u32, count: u32, stride: usize) -> Option<(usize, usize)> {
    if count == 0 {
        return Some((offset as usize, offset as usize));
    }
    let start = offset as usize;
    let end = start.checked_add((count as usize).checked_mul(stride)?)?;
    (end <= len).then_some((start, end))
}

/// Read the room record behind header slot `0x00`. A zero slot means no
/// geometry; a record or non-empty array outside the data region is a distinct
/// error; face indices are validated but a bad one is reported, not fatal.
fn geometry_report(data: &[u8]) -> Result<GeometryReport, LevelError> {
    let room_offset = read_u32(data, 4 * SLOT_ROOM_RECORD)
        .ok_or(LevelError::GeometryRecordOutOfRange)?;
    let mut report = GeometryReport { room_offset, ..GeometryReport::default() };
    if room_offset == 0 {
        return Ok(report);
    }
    let record = room_offset as usize;
    if record.checked_add(ROOM_FACE_OFFSET + 4).is_none_or(|end| end > data.len()) {
        return Err(LevelError::GeometryRecordOutOfRange);
    }
    let vertices = read_u32(data, record + ROOM_VERTEX_COUNT).unwrap_or(0);
    let faces = read_u32(data, record + ROOM_FACE_COUNT).unwrap_or(0);
    let vertex_offset = read_u32(data, record + ROOM_VERTEX_OFFSET).unwrap_or(0);
    let face_offset = read_u32(data, record + ROOM_FACE_OFFSET).unwrap_or(0);
    let (vertex_start, vertex_end) = array_range(data.len(), vertex_offset, vertices, VERTEX_STRIDE)
        .ok_or(LevelError::GeometryArrayOutOfRange)?;
    let (face_start, face_end) =
        array_range(data.len(), face_offset, faces, FACE_STRIDE)
            .ok_or(LevelError::GeometryArrayOutOfRange)?;
    report.vertices = vertices;
    report.faces = faces;
    report.vertex_offset = vertex_offset;
    report.face_offset = face_offset;
    if vertices > 0 {
        let mut minimum = [i16::MAX; 3];
        let mut maximum = [i16::MIN; 3];
        for at in (vertex_start..vertex_end).step_by(VERTEX_STRIDE) {
            for axis in 0..3 {
                let value =
                    read_i16(data, at + 2 * axis).ok_or(LevelError::GeometryArrayOutOfRange)?;
                minimum[axis] = minimum[axis].min(value);
                maximum[axis] = maximum[axis].max(value);
            }
        }
        report.min = minimum;
        report.max = maximum;
    }
    if faces > 0 {
        let mut max_index = 0u16;
        for at in (face_start..face_end).step_by(FACE_STRIDE) {
            for slot in 0..3 {
                let index =
                    read_u16(data, at + 2 * slot).ok_or(LevelError::GeometryArrayOutOfRange)?;
                max_index = max_index.max(index);
                if u32::from(index) >= vertices {
                    report.out_of_range += 1;
                }
            }
        }
        report.max_index = Some(max_index);
    }
    Ok(report)
}

/// CPU mesh behind header slot `0x00`: the same record and bounds rule as
/// [`geometry_report`], with positions widened to `f32` and every face index
/// range-checked instead of counted.
fn room_mesh(data: &[u8]) -> Result<RoomMesh, LevelError> {
    let room_offset =
        read_u32(data, 4 * SLOT_ROOM_RECORD).ok_or(LevelError::GeometryRecordOutOfRange)?;
    if room_offset == 0 {
        return Ok(RoomMesh::default());
    }
    let record = room_offset as usize;
    if record.checked_add(ROOM_FACE_OFFSET + 4).is_none_or(|end| end > data.len()) {
        return Err(LevelError::GeometryRecordOutOfRange);
    }
    let field = |at| read_u32(data, record + at).unwrap_or(0);
    let vertices = field(ROOM_VERTEX_COUNT);
    let faces = field(ROOM_FACE_COUNT);
    let (vertex_start, vertex_end) =
        array_range(data.len(), field(ROOM_VERTEX_OFFSET), vertices, VERTEX_STRIDE)
            .ok_or(LevelError::GeometryArrayOutOfRange)?;
    let (face_start, face_end) =
        array_range(data.len(), field(ROOM_FACE_OFFSET), faces, FACE_STRIDE)
            .ok_or(LevelError::GeometryArrayOutOfRange)?;
    let mut mesh = RoomMesh::default();
    for at in (vertex_start..vertex_end).step_by(VERTEX_STRIDE) {
        let mut xyz = [0.0f32; 3];
        for (axis, value) in xyz.iter_mut().enumerate() {
            let raw = read_i16(data, at + 2 * axis).ok_or(LevelError::GeometryArrayOutOfRange)?;
            *value = f32::from(raw);
        }
        mesh.vertices.push(xyz);
    }
    for at in (face_start..face_end).step_by(FACE_STRIDE) {
        for slot in 0..3 {
            let index = read_u16(data, at + 2 * slot).ok_or(LevelError::GeometryArrayOutOfRange)?;
            if u32::from(index) >= vertices {
                return Err(LevelError::GeometryArrayOutOfRange);
            }
            mesh.indices.push(u32::from(index));
        }
    }
    Ok(mesh)
}

/// One decoded room face texture: the masked bank page and the three UV pairs
/// of the 12-byte `TextureFT3`-lite record. Attributes stay unread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceTexture {
    pub page: u16,
    pub uvs: [[u8; 2]; 3],
}

/// Per-face resolution for one room. `unresolved` counts `0xFFFF` and
/// non-12-aligned tails; those faces stay in the mesh with flat material 1.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RoomTextureFaces {
    pub faces: Vec<Option<FaceTexture>>,
    pub unresolved: u32,
}

/// Resolve every room face's `+0x0A` byte offset into the `+0x34` record array.
/// The offset is relative to that array, not the data base; `0xFFFF` and
/// non-12-aligned or out-of-span offsets are counted, never clamped.
fn room_texture_faces(data: &[u8]) -> Result<RoomTextureFaces, LevelError> {
    let room_offset =
        read_u32(data, 4 * SLOT_ROOM_RECORD).ok_or(LevelError::GeometryRecordOutOfRange)?;
    if room_offset == 0 {
        return Err(LevelError::RoomTexturesMissing);
    }
    let record = room_offset as usize;
    if record.checked_add(ROOM_TEXTURE_END + 4).is_none_or(|end| end > data.len()) {
        return Err(LevelError::GeometryRecordOutOfRange);
    }
    let start = read_u32(data, record + ROOM_TEXTURE_START).unwrap_or(0) as usize;
    let end = read_u32(data, record + ROOM_TEXTURE_END).unwrap_or(0) as usize;
    if start == 0 && end == 0 {
        return Err(LevelError::RoomTexturesMissing);
    }
    let span = end
        .checked_sub(start)
        .filter(|span| *span != 0 && *span % TEXTURE_STRIDE == 0 && end <= data.len())
        .ok_or(LevelError::RoomTextureArrayOutOfRange)?;
    let faces = read_u32(data, record + ROOM_FACE_COUNT).unwrap_or(0);
    let face_offset = read_u32(data, record + ROOM_FACE_OFFSET).unwrap_or(0);
    let (face_start, face_end) = array_range(data.len(), face_offset, faces, FACE_STRIDE)
        .ok_or(LevelError::GeometryArrayOutOfRange)?;
    let mut out =
        RoomTextureFaces { faces: Vec::with_capacity(faces as usize), ..RoomTextureFaces::default() };
    for at in (face_start..face_end).step_by(FACE_STRIDE) {
        let offset = read_u16(data, at + FACE_TEXTURE_OFFSET)
            .ok_or(LevelError::GeometryArrayOutOfRange)?;
        let resolved = (offset != u16::MAX)
            .then_some(offset as usize)
            .filter(|offset| offset % TEXTURE_STRIDE == 0 && offset + TEXTURE_STRIDE <= span)
            .map(|offset| start + offset);
        let Some(record_at) = resolved else {
            out.unresolved += 1;
            out.faces.push(None);
            continue;
        };
        let word = read_u16(data, record_at + 2).ok_or(LevelError::RoomTextureArrayOutOfRange)?;
        out.faces.push(Some(FaceTexture {
            page: word & crate::object::TEXTURE_PAGE_MASK,
            uvs: [
                [data[record_at], data[record_at + 1]],
                [data[record_at + 4], data[record_at + 5]],
                [data[record_at + 8], data[record_at + 9]],
            ],
        }));
    }
    Ok(out)
}

/// Per-face textured room mesh: every face contributes three unique vertices,
/// exactly as the object path does. Material 1 is flat; used pages start at 2.
fn textured_room_mesh(data: &[u8]) -> Result<RoomMesh, LevelError> {
    let flat = room_mesh(data)?;
    let textures = room_texture_faces(data)?;
    let mut mesh = RoomMesh::default();
    for (triangle, texture) in flat.indices.chunks_exact(3).zip(&textures.faces) {
        let base = mesh.vertices.len() as u32;
        let material = texture
            .map_or(1, |texture| crate::object::page_material(&mut mesh.pages, texture.page));
        for (corner, &index) in triangle.iter().enumerate() {
            mesh.vertices.push(flat.vertices[index as usize]);
            mesh.uvs.push(texture.map_or([0, 0], |texture| texture.uvs[corner]));
            mesh.material_indices.push(material);
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
    mesh.textured_faces = textures.faces.len() as u32 - textures.unresolved;
    mesh.unresolved_faces = textures.unresolved;
    Ok(mesh)
}

/// Read the object array behind the same room record as [`room_mesh`]. Each
/// record is a 12-byte NUL-terminated name, three `i16` world positions at
/// `+0x20`, a type in the high 16 bits of `+0x28`, and raw flags at `+0x2C`.
/// A record or array outside the data region is a distinct error.
fn object_report(data: &[u8]) -> Result<ObjectReport, LevelError> {
    let room_offset =
        read_u32(data, 4 * SLOT_ROOM_RECORD).ok_or(LevelError::ObjectRecordOutOfRange)?;
    let mut report = ObjectReport { room_offset, ..ObjectReport::default() };
    if room_offset == 0 {
        return Ok(report);
    }
    let record = room_offset as usize;
    if record.checked_add(OBJECT_OFFSET + 4).is_none_or(|end| end > data.len()) {
        return Err(LevelError::ObjectRecordOutOfRange);
    }
    let count = read_u32(data, record + OBJECT_COUNT).unwrap_or(0);
    let offset = read_u32(data, record + OBJECT_OFFSET).unwrap_or(0);
    let (start, end) = array_range(data.len(), offset, count, OBJECT_STRIDE)
        .ok_or(LevelError::ObjectArrayOutOfRange)?;
    report.count = count;
    report.array_offset = offset;
    for at in (start..end).step_by(OBJECT_STRIDE) {
        let raw = &data[at..at + OBJECT_NAME_LEN];
        let name = raw.split(|&byte| byte == 0).next().unwrap_or(&[]);
        let type_word = read_u32(data, at + OBJECT_TYPE).ok_or(LevelError::ObjectArrayOutOfRange)?;
        report.objects.push(ObjectRecord {
            name: String::from_utf8_lossy(name).into_owned(),
            position: [
                read_i16(data, at + OBJECT_X).ok_or(LevelError::ObjectArrayOutOfRange)?,
                read_i16(data, at + OBJECT_X + 2).ok_or(LevelError::ObjectArrayOutOfRange)?,
                read_i16(data, at + OBJECT_X + 4).ok_or(LevelError::ObjectArrayOutOfRange)?,
            ],
            object_type: (type_word >> 16) as u16,
            flags: read_u32(data, at + OBJECT_FLAGS).ok_or(LevelError::ObjectArrayOutOfRange)?,
        });
    }
    Ok(report)
}

/// First object named `raziel`, if the room places one: the fly camera's
/// optional start point. Names compare case-insensitively; the record carries
/// no facing, so the camera starts looking along -Z.
pub fn spawn_position(data: &[u8]) -> Option<[f32; 3]> {
    object_report(data).ok()?.objects.iter()
        .find(|object| object.name.eq_ignore_ascii_case("raziel"))
        .map(|object| object.position.map(f32::from))
}

/// Marker side as a fraction of the largest scene axis, and the smallest side
/// so a flat or zero-extent scene still shows something.
const MARKER_SCENE_FRACTION: f32 = 0.01;
const MARKER_MIN_SIDE: f32 = 2.0;

/// Largest axis span of `vertices`.
fn scene_extent(vertices: &[[f32; 3]]) -> f32 {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(vertex[axis]);
            maximum[axis] = maximum[axis].max(vertex[axis]);
        }
    }
    (0..3).map(|axis| maximum[axis] - minimum[axis]).fold(0.0, f32::max)
}

/// Append one small axis-aligned cube per object, merged into the same vertex
/// and index buffers. Every marker is identical: no per-type shape, colour or
/// orientation is claimed.
fn append_markers(mesh: &mut RoomMesh, objects: &[ObjectRecord]) -> Result<(), LevelError> {
    if objects.is_empty() {
        return Ok(());
    }
    let half =
        (scene_extent(&mesh.vertices) * MARKER_SCENE_FRACTION / 2.0).max(MARKER_MIN_SIDE / 2.0);
    const CORNERS: [[f32; 3]; 8] = [
        [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0],
    ];
    const FACES: [u32; 36] = [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    for object in objects {
        let base = mesh.vertices.len() as u32;
        for corner in CORNERS {
            mesh.vertices.push([
                f32::from(object.position[0]) + corner[0] * half,
                f32::from(object.position[1]) + corner[1] * half,
                f32::from(object.position[2]) + corner[2] * half,
            ]);
        }
        mesh.indices.extend(FACES.iter().map(|&index| base + index));
    }
    Ok(())
}

/// Append one real model per placed object and a cube for every name that does
/// not resolve. Model vertices are translated by the record's world position;
/// the record carries no rotation and no scale.
fn append_objects(
    mesh: &mut RoomMesh,
    objects: &[ObjectRecord],
    models: Option<ObjectModelSource<'_>>,
) -> Result<(), LevelError> {
    let Some(models) = models else {
        append_markers(mesh, objects)?;
        mesh.objects = ObjectStats { total: objects.len() as u32, markers: objects.len() as u32, ..ObjectStats::default() };
        return Ok(());
    };
    let mut markers = Vec::new();
    for object in objects {
        let Some((vertices, indices)) = models(&object.name) else {
            markers.push(object.clone());
            continue;
        };
        let base = mesh.vertices.len() as u32;
        let offset = object.position.map(f32::from);
        mesh.vertices.extend(vertices.iter().map(|v| {
            [v[0] + offset[0], v[1] + offset[1], v[2] + offset[2]]
        }));
        mesh.indices.extend(indices.iter().map(|&index| base + index));
        mesh.objects.meshes += 1;
    }
    append_markers(mesh, &markers)?;
    mesh.objects.total = objects.len() as u32;
    mesh.objects.markers = markers.len() as u32;
    Ok(())
}

/// [`append_markers`] plus flat-material UVs for the textured scene buffers.
fn append_markers_textured(
    mesh: &mut RoomMesh,
    objects: &[ObjectRecord],
) -> Result<(), LevelError> {
    let start = mesh.vertices.len();
    append_markers(mesh, objects)?;
    let added = mesh.vertices.len() - start;
    mesh.uvs.extend(std::iter::repeat([0, 0]).take(added));
    mesh.material_indices.extend(std::iter::repeat(1).take(added));
    Ok(())
}

/// Append one textured model per placed object and a flat cube for every
/// unresolved name. Object page ids are remapped onto the scene-wide material
/// table (1 = flat, 2.. = pages in first-use order).
fn append_objects_textured(
    mesh: &mut RoomMesh,
    objects: &[ObjectRecord],
    models: TexturedObjectSource<'_>,
) -> Result<(), LevelError> {
    let mut markers = Vec::new();
    for object in objects {
        let Some(model) = models(&object.name) else {
            markers.push(object.clone());
            continue;
        };
        let base = mesh.vertices.len() as u32;
        let offset = object.position.map(f32::from);
        for index in 0..model.positions.len() {
            let position = model.positions[index];
            mesh.vertices.push([
                position[0] + offset[0],
                position[1] + offset[1],
                position[2] + offset[2],
            ]);
            mesh.uvs.push(model.uvs[index]);
            let material = model.material_indices[index];
            mesh.material_indices.push(if material <= 1 {
                1
            } else {
                crate::object::page_material(
                    &mut mesh.pages,
                    model.pages[usize::from(material) - 2],
                )
            });
        }
        mesh.indices.extend(model.indices.iter().map(|&index| base + index));
        mesh.objects.meshes += 1;
    }
    append_markers_textured(mesh, &markers)?;
    mesh.objects.total = objects.len() as u32;
    mesh.objects.markers = markers.len() as u32;
    Ok(())
}

/// Append a textured room mesh onto the scene, remapping its local page
/// materials into the scene-wide table (flat 1, then pages in first-use order).
fn append_textured_mesh(merged: &mut RoomMesh, room: RoomMesh) {
    let RoomMesh {
        vertices,
        uvs,
        indices,
        material_indices,
        pages,
        textured_faces,
        unresolved_faces,
        ..
    } = room;
    let base = merged.vertices.len() as u32;
    for ((&uv, &material), position) in uvs.iter().zip(&material_indices).zip(vertices) {
        merged.vertices.push(position);
        merged.uvs.push(uv);
        merged.material_indices.push(if material <= 1 {
            1
        } else {
            crate::object::page_material(&mut merged.pages, pages[usize::from(material) - 2])
        });
    }
    merged.indices.extend(indices.iter().map(|&index| base + index));
    merged.textured_faces += textured_faces;
    merged.unresolved_faces += unresolved_faces;
}

/// Merge one or more units into a textured room scene. Rooms and markers keep
/// flat material 1 for unresolved faces; placed models keep their own UVs and
/// page materials when `models` is supplied.
fn textured_scene(
    rooms: Vec<(LevelUnit, Vec<u8>)>,
    prefix: Option<&str>,
    markers: bool,
    models: Option<TexturedObjectSource<'_>>,
) -> Result<(String, RoomMesh), LevelError> {
    let mut merged = RoomMesh::default();
    let mut placed = Vec::new();
    let mut title = None;
    for (unit, bytes) in rooms {
        let data = &bytes[unit.structure.data_base..];
        let mesh = textured_room_mesh(data).map_err(|error| match error {
            LevelError::GeometryRecordOutOfRange | LevelError::GeometryArrayOutOfRange => {
                LevelError::RenderArrayOutOfRange
            }
            other => other,
        })?;
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Err(LevelError::RenderEmpty);
        }
        append_textured_mesh(&mut merged, mesh);
        if merged.spawn.is_none() {
            merged.spawn = spawn_position(data);
        }
        if markers {
            placed.extend(object_report(data)?.objects);
        }
        title.get_or_insert(unit.name);
    }
    if markers {
        match models {
            Some(models) => append_objects_textured(&mut merged, &placed, models)?,
            None => append_markers_textured(&mut merged, &placed)?,
        }
    }
    let name = prefix.map_or_else(|| title.unwrap_or_default(), |p| format!("group {p}"));
    Ok((name, merged))
}

/// `--level-render <.PCM> --textures ...`: one loose unit, room textured.
pub fn read_room_mesh_textured(
    path: &Path,
    markers: bool,
) -> Result<(String, RoomMesh), LevelError> {
    if path.is_dir() {
        return Err(LevelError::RenderDirectory);
    }
    if !is_pcm(path) {
        return Err(LevelError::RefusedPath);
    }
    let stem =
        path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).ok_or(LevelError::Io)?;
    let bytes = std::fs::read(path).map_err(|_| LevelError::Io)?;
    let unit = parse_pcm(&bytes, &stem)?;
    let data = &bytes[unit.structure.data_base..];
    let mut mesh = textured_room_mesh(data)?;
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Err(LevelError::RenderEmpty);
    }
    mesh.spawn = spawn_position(data);
    if markers {
        append_markers_textured(&mut mesh, &object_report(data)?.objects)?;
    }
    Ok((unit.name, mesh))
}

/// `--level-render --bigfile ... --entry ... --textures ...`.
pub fn read_entry_room_mesh_textured(
    path: &Path,
    index: usize,
    markers: bool,
    models: Option<TexturedObjectSource<'_>>,
) -> Result<(String, RoomMesh), LevelError> {
    textured_scene(vec![entry_unit(path, index)?], None, markers, models)
}

/// `--level-render --bigfile ... --group ... --textures ...`.
pub fn read_group_room_mesh_textured(
    path: &Path,
    prefix: &str,
    markers: bool,
    models: Option<TexturedObjectSource<'_>>,
) -> Result<(String, RoomMesh), LevelError> {
    textured_scene(discover_group(path, prefix)?, Some(prefix), markers, models)
}

/// One-unit reader for `--level-render`: a directory is refused because this
/// slice renders exactly one `.PCM` file. An empty mesh is a distinct error so
/// the window never opens on nothing.
pub fn read_room_mesh(
    path: &Path,
    markers: bool,
) -> Result<(String, RoomMesh), LevelError> {
    if path.is_dir() {
        return Err(LevelError::RenderDirectory);
    }
    if !is_pcm(path) {
        return Err(LevelError::RefusedPath);
    }
    let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).ok_or(LevelError::Io)?;
    let bytes = std::fs::read(path).map_err(|_| LevelError::Io)?;
    let unit = parse_pcm(&bytes, &stem)?;
    room_mesh_of(unit, &bytes, markers, None)
}

/// Shared mesh step for a parsed unit: the same bounds rule as
/// [`read_room_mesh`], with an empty mesh refused so a window never opens on
/// nothing. `markers` appends the object cubes to the same buffers; `models`
/// replaces each cube with the real archived model when it resolves.
fn room_mesh_of(
    unit: LevelUnit,
    bytes: &[u8],
    markers: bool,
    models: Option<ObjectModelSource<'_>>,
) -> Result<(String, RoomMesh), LevelError> {
    let data = &bytes[unit.structure.data_base..];
    let mut mesh = room_mesh(data).map_err(|error| match error {
        LevelError::GeometryRecordOutOfRange | LevelError::GeometryArrayOutOfRange => {
            LevelError::RenderArrayOutOfRange
        }
        other => other,
    })?;
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Err(LevelError::RenderEmpty);
    }
    mesh.spawn = spawn_position(data);
    if markers {
        append_objects(&mut mesh, &object_report(data)?.objects, models)?;
    }
    Ok((unit.name, mesh))
}

/// Read one archive entry into memory and feed it to the same `.PCM` parser
/// as a loose file. No extraction, no temporary file.
fn entry_bytes(path: &Path, index: usize) -> Result<Vec<u8>, LevelError> {
    let entries = bigfile::load_index(path)
        .map_err(|error| LevelError::ArchiveUnreadable { detail: error.to_string() })?;
    let entry = entries
        .get(index)
        .ok_or(LevelError::EntryIndexOutOfRange { index, entries: entries.len() })?;
    bigfile::read_entry(path, entry).map_err(|error| match error {
        error @ bigfile::BigfileError::BadEntry(_) => {
            LevelError::EntryOutsideFile { detail: error.to_string() }
        }
        error => LevelError::ArchiveUnreadable { detail: error.to_string() },
    })
}

/// `--level-geometry --bigfile ... --entry ...`: one archive entry, in memory.
pub fn read_entry_geometry(
    path: &Path,
    index: usize,
) -> Result<Vec<(String, GeometryReport)>, LevelError> {
    let (unit, bytes) = entry_unit(path, index)?;
    let report = geometry_report(&bytes[unit.structure.data_base..])?;
    Ok(vec![(unit.name, report)])
}

/// `--level-render --bigfile ... --entry ...`: the same mesh path as a loose file.
pub fn read_entry_room_mesh(
    path: &Path,
    index: usize,
    markers: bool,
    models: Option<ObjectModelSource<'_>>,
) -> Result<(String, RoomMesh), LevelError> {
    let (unit, bytes) = entry_unit(path, index)?;
    room_mesh_of(unit, &bytes, markers, models)
}

/// `--level-objects <PATH>`: one loose `.PCM` unit's object array.
pub fn read_objects_path(path: &Path) -> Result<Vec<(String, ObjectReport)>, LevelError> {
    if path.is_dir() {
        return Err(LevelError::RenderDirectory);
    }
    if !is_pcm(path) {
        return Err(LevelError::RefusedPath);
    }
    let stem = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).ok_or(LevelError::Io)?;
    let bytes = std::fs::read(path).map_err(|_| LevelError::Io)?;
    let unit = parse_pcm(&bytes, &stem)?;
    let report = object_report(&bytes[unit.structure.data_base..])?;
    Ok(vec![(unit.name, report)])
}

/// `--level-objects --bigfile ... --entry ...`: one archive entry, in memory.
pub fn read_entry_objects(
    path: &Path,
    index: usize,
) -> Result<Vec<(String, ObjectReport)>, LevelError> {
    let (unit, bytes) = entry_unit(path, index)?;
    let report = object_report(&bytes[unit.structure.data_base..])?;
    Ok(vec![(unit.name, report)])
}

/// `--level-objects --bigfile ... --group ...`: one object report per matching
/// room, in numeric-suffix order.
pub fn read_group_objects(
    path: &Path,
    prefix: &str,
) -> Result<Vec<(String, ObjectReport)>, LevelError> {
    let mut rows = Vec::new();
    for (unit, bytes) in discover_group(path, prefix)? {
        let report = object_report(&bytes[unit.structure.data_base..]).map_err(|error| {
            LevelError::GroupParseFailed { name: unit.name.clone(), detail: error.to_string() }
        })?;
        rows.push((unit.name, report));
    }
    Ok(rows)
}

/// Parse one archive entry with the same in-memory reader as a loose `.PCM`.
fn entry_unit(path: &Path, index: usize) -> Result<(LevelUnit, Vec<u8>), LevelError> {
    let bytes = entry_bytes(path, index)?;
    let unit = parse_pcm_bytes(&bytes).map_err(|error| LevelError::EntryNotPcm {
        detail: error.to_string(),
    })?;
    Ok((unit, bytes))
}

/// True when `name` is exactly `prefix` followed by one or more ASCII digits.
fn group_member(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
}

/// Numeric suffix as the sort key, so `under10` follows `under9`.
fn group_rank(name: &str, prefix: &str) -> u64 {
    name.strip_prefix(prefix).and_then(|suffix| suffix.parse().ok()).unwrap_or(u64::MAX)
}

/// Scan every archive entry, keep those that parse as a `.PCM` whose own name
/// is `prefix` + digits, and return them in numeric-suffix order. Entries are
/// read into memory only; nothing is extracted.
fn discover_group(path: &Path, prefix: &str) -> Result<Vec<(LevelUnit, Vec<u8>)>, LevelError> {
    if prefix.is_empty() {
        return Err(LevelError::GroupEmptyPrefix);
    }
    let entries = bigfile::load_index(path)
        .map_err(|error| LevelError::ArchiveUnreadable { detail: error.to_string() })?;
    let mut rooms = Vec::new();
    for entry in &entries {
        let bytes = bigfile::read_entry(path, entry).map_err(|error| match error {
            error @ bigfile::BigfileError::BadEntry(_) => {
                LevelError::EntryOutsideFile { detail: error.to_string() }
            }
            error => LevelError::ArchiveUnreadable { detail: error.to_string() },
        })?;
        if let Ok(unit) = parse_pcm_bytes(&bytes)
            && group_member(&unit.name, prefix)
        {
            rooms.push((unit, bytes));
        }
    }
    if rooms.is_empty() {
        return Err(LevelError::GroupNoMatch { prefix: prefix.to_string() });
    }
    rooms.sort_by_key(|(unit, _)| group_rank(&unit.name, prefix));
    Ok(rooms)
}

/// One archived level unit and its outgoing link-target names.
#[derive(Debug, PartialEq, Eq)]
pub struct ArchivedUnit {
    pub name: String,
    pub links: Vec<String>,
}

/// Every level unit found in a BIGFILE, plus candidates that failed the parser.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ArchiveScan {
    pub units: Vec<ArchivedUnit>,
    pub unparseable: Vec<String>,
}

/// Cheap `.PCM` signature: sane count, offsets starting `0, 4`, strict ascent,
/// data base inside the bytes. The full parser still runs after.
fn pcm_candidate(bytes: &[u8]) -> bool {
    let Some(records) = read_u32(bytes, 0) else { return false };
    if !(2..=MAX_RECORDS).contains(&records) {
        return false;
    }
    let Some(table_end) = 4usize.checked_add(4 * records as usize) else { return false };
    let base = data_base(records);
    if table_end > bytes.len() || base > bytes.len() {
        return false;
    }
    let mut last = 0;
    for index in 0..records as usize {
        let Some(offset) = read_u32(bytes, 4 + 4 * index) else { return false };
        if (index < 2 && offset != 4 * index as u32) || (index > 0 && offset <= last) {
            return false;
        }
        last = offset;
    }
    base.checked_add(last as usize).is_some_and(|end| end <= bytes.len())
}

/// Keep the header name only when its last four characters match the BIGFILE
/// suffix; otherwise use a scanned name that matches, since the index's fourth
/// dword is the uppercase tail of the unit's own name.
fn resolve_unit_name(unit: &mut LevelUnit, suffix: &[u8; 4]) {
    let wanted = String::from_utf8_lossy(suffix).to_ascii_uppercase();
    let matches = |name: &str| {
        name.len() >= wanted.len()
            && name[name.len() - wanted.len()..].to_ascii_uppercase() == wanted
    };
    if matches(&unit.name) {
        return;
    }
    if let Some(name) = unit.names.iter().find(|name| matches(name)) {
        unit.name = name.clone();
    }
}

/// Scan every BIGFILE entry for `.PCM` units, one entry in memory at a time.
pub fn scan_archive_levels(path: &Path) -> Result<ArchiveScan, LevelError> {
    let entries = bigfile::load_index(path)
        .map_err(|error| LevelError::ArchiveUnreadable { detail: error.to_string() })?;
    let mut scan = ArchiveScan::default();
    for (index, entry) in entries.iter().enumerate() {
        let bytes = bigfile::read_entry(path, entry).map_err(|error| match error {
            error @ bigfile::BigfileError::BadEntry(_) => {
                LevelError::EntryOutsideFile { detail: error.to_string() }
            }
            error => LevelError::ArchiveUnreadable { detail: error.to_string() },
        })?;
        if !pcm_candidate(&bytes) {
            continue;
        }
        match parse_pcm_bytes(&bytes) {
            Ok(mut unit) => {
                resolve_unit_name(&mut unit, &entry.suffix);
                let links = unit.link_table.iter().map(|link| link.target.clone()).collect();
                scan.units.push(ArchivedUnit { name: unit.name, links });
            }
            Err(error) => scan.unparseable.push(format!(
                "entry {index} suffix {}: {error}",
                String::from_utf8_lossy(&entry.suffix)
            )),
        }
    }
    if scan.units.is_empty() && scan.unparseable.is_empty() {
        return Err(LevelError::NoArchivedLevels);
    }
    Ok(scan)
}

/// Reachability summary for one start unit.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReachReport {
    pub discovered: usize,
    pub reachable: Vec<String>,
    pub unreachable: Vec<String>,
    pub islands: Vec<String>,
    pub inbound_unreachable: Vec<String>,
    pub unparseable: Vec<String>,
}

impl ReachReport {
    /// Every discovered unit that is not reachable, including unparseable ones.
    pub fn unreachable_total(&self) -> usize {
        self.unreachable.len() + self.unparseable.len()
    }
}

/// Name prefix with digits stripped, e.g. `cathy62` -> `cathy`.
fn name_group(name: &str) -> String {
    name.chars().filter(|character| character.is_ascii_alphabetic()).collect()
}

fn group_counts(names: &[String]) -> Vec<(String, u32)> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for name in names {
        *counts.entry(name_group(name)).or_default() += 1;
    }
    counts.into_iter().collect()
}

/// Distinct source units whose link tables target each discovered unit. A
/// self-link does not count, and unresolved targets are ignored, so this is the
/// arrival-path count the reachability BFS cannot see.
fn inbound_degrees(known: &BTreeMap<&str, &ArchivedUnit>) -> BTreeMap<String, usize> {
    let mut degrees: BTreeMap<String, usize> =
        known.keys().map(|name| ((*name).to_string(), 0)).collect();
    for unit in known.values() {
        let targets: BTreeSet<&str> = unit
            .links
            .iter()
            .map(String::as_str)
            .filter(|target| *target != unit.name.as_str() && known.contains_key(target))
            .collect();
        for target in targets {
            *degrees.entry(target.to_string()).or_default() += 1;
        }
    }
    degrees
}

/// BFS from `start` over outgoing link targets that name a discovered unit.
pub fn read_reach(path: &Path, start: &str) -> Result<ReachReport, LevelError> {
    let scan = scan_archive_levels(path)?;
    let by_name: BTreeMap<&str, &ArchivedUnit> =
        scan.units.iter().map(|unit| (unit.name.as_str(), unit)).collect();
    if !by_name.contains_key(start) {
        return Err(LevelError::ReachStartMissing { name: start.to_string() });
    }
    let mut queue = VecDeque::from([start.to_string()]);
    let mut seen = BTreeSet::new();
    while let Some(name) = queue.pop_front() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(unit) = by_name.get(name.as_str()) {
            for target in &unit.links {
                if by_name.contains_key(target.as_str()) && !seen.contains(target) {
                    queue.push_back(target.clone());
                }
            }
        }
    }
    let reachable: Vec<String> = seen.into_iter().collect();
    let unreachable = scan
        .units
        .iter()
        .filter(|unit| !reachable.contains(&unit.name))
        .map(|unit| unit.name.clone())
        .collect();
    let degrees = inbound_degrees(&by_name);
    let reached: BTreeSet<&str> = reachable.iter().map(String::as_str).collect();
    let islands = degrees
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(name, _)| name.clone())
        .collect();
    let inbound_unreachable = degrees
        .iter()
        .filter(|(name, degree)| **degree > 0 && !reached.contains(name.as_str()))
        .map(|(name, _)| name.clone())
        .collect();
    Ok(ReachReport {
        discovered: scan.units.len() + scan.unparseable.len(),
        reachable,
        unreachable,
        islands,
        inbound_unreachable,
        unparseable: scan.unparseable,
    })
}

/// One `island-name <unit>` line per island, in sorted order.
pub fn island_name_lines(report: &ReachReport) -> Vec<String> {
    report.islands.iter().map(|name| format!("  island-name {name}")).collect()
}

/// `--level-reach` lines: totals, reachable/unreachable groups, in-degree
/// sections, then unparseable entries. `names` adds one `island-name` line per
/// island after the grouped island counts; the default shape is unchanged.
pub fn reach_lines(report: &ReachReport, start: &str, names: bool) -> Vec<String> {
    let reachable_groups = group_counts(&report.reachable);
    let unreachable_groups = group_counts(&report.unreachable);
    let island_groups = group_counts(&report.islands);
    let inbound_groups = group_counts(&report.inbound_unreachable);
    let mut lines = vec![format!(
        "reach start={start} discovered={} reachable={} unreachable={} unparseable={}",
        report.discovered,
        report.reachable.len(),
        report.unreachable_total(),
        report.unparseable.len()
    )];
    lines.push(format!("  reachable groups={}", reachable_groups.len()));
    lines.extend(
        reachable_groups.iter().map(|(group, count)| format!("  reachable {group} {count}")),
    );
    lines.push(format!("  unreachable groups={}", unreachable_groups.len()));
    lines.extend(
        unreachable_groups.iter().map(|(group, count)| format!("  unreachable {group} {count}")),
    );
    lines.push(format!("  islands total={}", report.islands.len()));
    lines.extend(island_groups.iter().map(|(group, count)| format!("  island {group} {count}")));
    if names {
        lines.extend(island_name_lines(report));
    }
    lines.push(format!("  inbound-unreachable total={}", report.inbound_unreachable.len()));
    lines.extend(
        inbound_groups
            .iter()
            .map(|(group, count)| format!("  inbound-unreachable {group} {count}")),
    );
    lines.extend(report.unparseable.iter().map(|label| format!("  unparseable {label}")));
    lines
}

/// `--level-links --bigfile ... --entry ...`: one archive entry's links.
pub fn read_entry_links(path: &Path, index: usize) -> Result<Vec<LevelUnit>, LevelError> {
    entry_unit(path, index).map(|(unit, _)| vec![unit])
}

/// `--level-links --bigfile ... --group ...`: one unit per matching room.
pub fn read_group_links(path: &Path, prefix: &str) -> Result<Vec<LevelUnit>, LevelError> {
    discover_group(path, prefix).map(|rooms| rooms.into_iter().map(|(unit, _)| unit).collect())
}

/// `--level-geometry --bigfile ... --group ...`: one geometry report per
/// matching room. A matched entry whose geometry fails to parse is loud.
pub fn read_group_geometry(
    path: &Path,
    prefix: &str,
) -> Result<Vec<(String, GeometryReport)>, LevelError> {
    let mut rows = Vec::new();
    for (unit, bytes) in discover_group(path, prefix)? {
        let report = geometry_report(&bytes[unit.structure.data_base..]).map_err(|error| {
            LevelError::GroupParseFailed {
                name: unit.name.clone(),
                detail: error.to_string(),
            }
        })?;
        rows.push((unit.name, report));
    }
    Ok(rows)
}

/// `--level-render --bigfile ... --group ...`: rooms are world space, so the
/// merge concatenates vertices and offsets each room's face indices by its
/// running vertex base. No transform is applied. `markers` appends one cube per
/// object of every room, after the whole scene is known.
pub fn read_group_room_mesh(
    path: &Path,
    prefix: &str,
    markers: bool,
    models: Option<ObjectModelSource<'_>>,
) -> Result<(String, RoomMesh), LevelError> {
    let mut merged = RoomMesh::default();
    let mut placed = Vec::new();
    for (unit, bytes) in discover_group(path, prefix)? {
        let data = &bytes[unit.structure.data_base..];
        let mesh = room_mesh(data).map_err(|error| match error {
            LevelError::GeometryRecordOutOfRange | LevelError::GeometryArrayOutOfRange => {
                LevelError::RenderArrayOutOfRange
            }
            other => other,
        })?;
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Err(LevelError::RenderEmpty);
        }
        let base = merged.vertices.len() as u32;
        merged.vertices.extend(mesh.vertices);
        merged.indices.extend(mesh.indices.iter().map(|&index| index + base));
        if merged.spawn.is_none() {
            merged.spawn = spawn_position(data);
        }
        if markers {
            placed.extend(object_report(data)?.objects);
        }
    }
    if markers {
        append_objects(&mut merged, &placed, models)?;
    }
    Ok((format!("group {prefix}"), merged))
}

/// Geometry rows for one file or room directory, sorted by unit name. This
/// keeps its own read pass so a malformed room record cannot break the other
/// level arms.
pub fn read_geometry_path(path: &Path) -> Result<Vec<(String, GeometryReport)>, LevelError> {
    let paths = if path.is_dir() {
        let bin = path.join("BIN");
        let directory = if bin.is_dir() { bin } else { path.to_path_buf() };
        let index = SidecarIndex::read(&directory)?;
        if index.pcm_paths.is_empty() {
            return Err(LevelError::NoUnits);
        }
        index.pcm_paths
    } else if is_pcm(path) {
        vec![path.to_path_buf()]
    } else {
        return Err(LevelError::RefusedPath);
    };
    let mut rows = Vec::with_capacity(paths.len());
    for pcm_path in paths {
        let stem = pcm_path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).ok_or(LevelError::Io)?;
        let bytes = std::fs::read(&pcm_path).map_err(|_| LevelError::Io)?;
        let unit = parse_pcm(&bytes, &stem)?;
        rows.push((unit.name, geometry_report(&bytes[unit.structure.data_base..])?));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(rows)
}

/// Word count the offset table is padded to: 512 words, i.e. a 2048-byte
/// level-data base.
pub(crate) const OFFSET_ALIGN: usize = 512;

/// Level-data base: the `4 + 4N` table padded up to a 2048-byte boundary,
/// i.e. `4 * roundup512(N)`. Equal to that form for every `N < 512`; the
/// count word is included so the base can never land inside the table when
/// `N` is an exact multiple of 512. Saturates instead of panicking.
pub(crate) fn data_base(records: u32) -> usize {
    let words = (records as usize).saturating_add(1);
    let rounded = words.saturating_add(OFFSET_ALIGN - 1) & !(OFFSET_ALIGN - 1);
    rounded.saturating_mul(4)
}

/// Classify every directory entry by the dword it points at, relative to the
/// data base: a non-zero offset inside the data region is valid.
fn relocation_report(data: &[u8], offsets: &[u32]) -> RelocationReport {
    let mut report = RelocationReport { entries: offsets.len() as u32, ..RelocationReport::default() };
    for &entry in offsets {
        match data.get(entry as usize..).and_then(|rest| read_u32(rest, 0)) {
            Some(0) => report.zero += 1,
            Some(value)
                if (value as usize).checked_add(4).is_some_and(|end| end <= data.len()) =>
            {
                report.valid += 1;
            }
            _ => report.out_of_range += 1,
        }
    }
    report
}

/// Read the enumerated offset slots: null when zero or past the header.
fn schema_slots(data: &[u8], data_base: usize) -> Vec<(usize, usize)> {
    SCHEMA_OFFSET_SLOTS
        .iter()
        .filter_map(|&slot| {
            let value = read_u32(data, 4 * slot)?;
            (value != 0).then_some((slot, data_base + value as usize))
        })
        .collect()
}

/// Length of the tagged blob at `at`, or `None` when the magic is absent, a
/// type byte is unknown, or the walk leaves the data region.
fn tagged_blob_len(data: &[u8], at: usize) -> Option<u32> {
    let blob = data.get(at..)?;
    if blob.first() != Some(&0xBE) || blob.get(1) != Some(&0xEF) {
        return None;
    }
    let count = *blob.get(2)?;
    let mut cursor = 4usize;
    for _ in 0..count {
        let kind = *blob.get(cursor)?;
        let length = usize::from(*blob.get(cursor.checked_add(1)?)?);
        let entry = match kind {
            0 => 0x12,
            1..=4 => 0x0E,
            _ => return None,
        };
        cursor = cursor.checked_add(entry + 2 * length)?;
    }
    u32::try_from(cursor).ok()
}

/// Walk the 36-byte table: one tagged-blob length or null per record. The
/// count is capped so an untrusted header cannot force unbounded work.
fn table36_report(data: &[u8]) -> Table36Report {
    let count = read_u32(data, 4 * SLOT_TABLE36_COUNT).unwrap_or(0).min(MAX_RECORDS);
    let offset = read_u32(data, 4 * SLOT_TABLE36_OFFSET).unwrap_or(0) as usize;
    let mut lengths = Vec::new();
    for index in 0..count as usize {
        let length = if offset == 0 {
            None
        } else {
            let at = offset + index * TABLE36_STRIDE;
            read_u32(data, at)
                .filter(|&relative| relative != 0)
                .and_then(|relative| tagged_blob_len(data, relative as usize))
        };
        lengths.push(length);
    }
    Table36Report { count, stride: TABLE36_STRIDE as u32, lengths }
}

/// Decode the 40-byte link record at `at`, or `None` when it is not link-shaped.
fn link_record_at(data: &[u8], at: usize) -> Option<TableLink> {
    let record = data.get(at..at.checked_add(LINK_RECORD_STRIDE)?)?;
    let (target, index) = decode_link_name(&record[20..20 + LINK_NAME_LEN])?;
    let mut prelude = [0i32; 5];
    for (slot, value) in prelude.iter_mut().enumerate() {
        *value = read_u32(record, 4 * slot)? as i32;
    }
    Some(TableLink { target, index, target_id: prelude[4], prelude })
}

/// Walk the link table from header slot `0x34`, bounded by slot `0x35`. A short
/// non-link prologue precedes the records in some archived units, so the first
/// record-shaped entry wins; an invalid name then ends the walk without error.
fn link_table(data: &[u8]) -> Vec<TableLink> {
    let Some(relative) = read_u32(data, 4 * SLOT_LINK_TABLE) else {
        return Vec::new();
    };
    if relative == 0 {
        return Vec::new();
    }
    let start = relative as usize;
    let end = read_u32(data, 4 * SLOT_LINK_END).map_or(data.len(), |value| value as usize);
    let limit = if end > start && end <= data.len() { end } else { data.len() };
    let mut at = start;
    while at + LINK_RECORD_STRIDE <= limit && link_record_at(data, at).is_none() {
        at += 4;
    }
    let mut links = Vec::new();
    while at + LINK_RECORD_STRIDE <= limit {
        let Some(link) = link_record_at(data, at) else {
            break;
        };
        links.push(link);
        at += LINK_RECORD_STRIDE;
    }
    links
}

/// `^[a-z][a-z0-9]*,[0-9]+$` inside one NUL-terminated 12-byte field.
fn decode_link_name(field: &[u8]) -> Option<(String, u32)> {
    let text = std::str::from_utf8(&field[..field.iter().position(|&byte| byte == 0)?]).ok()?;
    let (target, digits) = text.split_once(',')?;
    let (first, rest) = target.as_bytes().split_first()?;
    if !first.is_ascii_lowercase()
        || !rest.iter().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((target.to_string(), digits.parse().ok()?))
}

/// `u32 N` + `N` ascending relative offsets + zero padding up to
/// `4 * roundup512(N)`, which is the level-data base, then the level data and
/// the name table. Blocks are addressed relative to that padded base.
pub fn parse_pcm(bytes: &[u8], expected_name: &str) -> Result<LevelUnit, LevelError> {
    parse_pcm_inner(bytes, Some(expected_name))
}

/// In-memory `.PCM` reader for bytes with no loose-file name, such as an
/// archive entry. The unit name comes from header word `0x26`.
pub fn parse_pcm_bytes(bytes: &[u8]) -> Result<LevelUnit, LevelError> {
    parse_pcm_inner(bytes, None)
}

/// The level's own name: header word `0x26` is a data-base-relative offset to
/// a NUL-terminated lowercase name. Anything else is `None`, never a guess.
fn header_name(bytes: &[u8], data_base: usize) -> Option<String> {
    let offset = read_u32(bytes, data_base.checked_add(4 * SLOT_UNIT_NAME)?)? as usize;
    let tail = bytes.get(data_base.checked_add(offset)?..)?;
    let end = tail.iter().position(|&byte| byte == 0)?;
    accepted_name(tail.get(..end)?).map(str::to_string)
}

fn parse_pcm_inner(
    bytes: &[u8],
    expected_name: Option<&str>,
) -> Result<LevelUnit, LevelError> {
    let records = read_u32(bytes, 0).ok_or(LevelError::TablePastEof)?;
    if !(2..=MAX_RECORDS).contains(&records) {
        return Err(LevelError::CountCeiling);
    }
    let table_end = 4usize
        .checked_add(4 * records as usize)
        .ok_or(LevelError::TablePastEof)?;
    if table_end > bytes.len() {
        return Err(LevelError::TablePastEof);
    }
    let data_base = data_base(records);
    if data_base > bytes.len() {
        return Err(LevelError::PaddingTruncated);
    }
    let mut last = 0u32;
    let mut offsets = Vec::with_capacity(records as usize);
    for index in 0..records as usize {
        let offset = read_u32(bytes, 4 + 4 * index).ok_or(LevelError::TablePastEof)?;
        let bad_prefix = index < 2 && offset != 4 * index as u32;
        if bad_prefix || (index > 0 && offset <= last) {
            return Err(LevelError::TableNotAscending);
        }
        last = offset;
        offsets.push(offset);
    }
    if data_base.saturating_add(last as usize) > bytes.len() {
        return Err(LevelError::TablePastEof);
    }
    let mut block_sizes = Vec::with_capacity(records as usize);
    let mut block_all_zero = Vec::with_capacity(records as usize);
    for (index, offset) in offsets.iter().enumerate() {
        let start = data_base + *offset as usize;
        let end = match offsets.get(index + 1) {
            Some(next) => data_base + *next as usize,
            None => bytes.len(),
        };
        let block = &bytes[start..end];
        block_sizes.push((end - start) as u32);
        block_all_zero.push(block.iter().all(|&byte| byte == 0));
    }
    let data = &bytes[data_base..];
    let table = link_table(data);
    let structure = LevelStructure {
        data_base,
        data_len: data.len(),
        reloc: relocation_report(data, &offsets),
        slots: schema_slots(data, data_base),
        table36: table36_report(data),
    };
    let names = extract_names(bytes);
    let name = match expected_name {
        Some(expected) => names
            .iter()
            .find(|name| name.eq_ignore_ascii_case(expected))
            .ok_or(LevelError::MissingUnitName)?
            .clone(),
        None => header_name(bytes, data_base).ok_or(LevelError::MissingUnitName)?,
    };
    let links = table
        .iter()
        .map(|link| Link { target: link.target.clone(), index: link.index })
        .collect();
    Ok(LevelUnit {
        name,
        records,
        names,
        links,
        sidecar: Sidecar::Absent,
        block_sizes,
        block_all_zero,
        structure,
        link_table: table,
    })
}

/// Shape-only classifier: all-zero wins, then the three frequent sizes,
/// then the block's own size.
pub fn classify(size: u32, all_zero: bool) -> BlockClass {
    if all_zero {
        return BlockClass::Zero;
    }
    match size {
        4 => BlockClass::Small4,
        40 => BlockClass::Fixed40,
        48 => BlockClass::Fixed48,
        other => BlockClass::Other(other),
    }
}

/// Counts every block exactly once; the class counts sum to `records`.
pub fn census(unit: &LevelUnit) -> BlockCensus {
    debug_assert_eq!(unit.block_sizes.len(), unit.block_all_zero.len());
    let mut counts = BlockCensus {
        total: unit.records,
        zero: 0,
        small4: 0,
        fixed40: 0,
        fixed48: 0,
        other: Vec::new(),
        largest: (0, 0),
    };
    for (ordinal, &size) in unit.block_sizes.iter().enumerate() {
        match classify(size, unit.block_all_zero[ordinal]) {
            BlockClass::Zero => counts.zero += 1,
            BlockClass::Small4 => counts.small4 += 1,
            BlockClass::Fixed40 => counts.fixed40 += 1,
            BlockClass::Fixed48 => counts.fixed48 += 1,
            BlockClass::Other(size) => {
                match counts.other.iter_mut().find(|(known, _)| *known == size) {
                    Some((_, count)) => *count += 1,
                    None => counts.other.push((size, 1)),
                }
            }
        }
        if size > counts.largest.1 {
            counts.largest = (ordinal as u32, size);
        }
    }
    counts.other.sort_unstable_by_key(|(size, _)| *size);
    counts
}

/// One `blocks` header line, one line per present shape class in fixed
/// order, and one `largest` line. Counts and sizes only: no payload, file
/// offset, coordinate, or transform.
pub fn record_lines(unit: &LevelUnit) -> Vec<String> {
    let counts = census(unit);
    let mut lines = vec![format!("blocks {} total={}", unit.name, counts.total)];
    for (label, count) in [
        ("zero", counts.zero),
        ("small4", counts.small4),
        ("fixed40", counts.fixed40),
        ("fixed48", counts.fixed48),
    ] {
        if count > 0 {
            lines.push(format!("  shape {label} {count}"));
        }
    }
    lines.extend(
        counts
            .other
            .iter()
            .map(|(size, count)| format!("  shape other:{size} {count}")),
    );
    lines.push(format!("largest {} {}", counts.largest.0, counts.largest.1));
    lines
}

/// Structural report: header sanity, relocation integrity, header offset
/// slots. Fails loud when any directory entry points outside the level data.
pub fn structure_lines(unit: &LevelUnit) -> Result<Vec<String>, LevelError> {
    let structure = &unit.structure;
    if structure.reloc.out_of_range > 0 {
        return Err(LevelError::RelocationOutOfRange);
    }
    let reloc = &structure.reloc;
    let at: String = structure
        .slots
        .iter()
        .map(|(slot, position)| format!(" 0x{slot:02X}:{position}"))
        .collect();
    let list: Vec<String> = structure
        .table36
        .lengths
        .iter()
        .map(|length| length.map_or_else(|| "null".to_string(), |bytes| bytes.to_string()))
        .collect();
    let lengths = if list.is_empty() { "none".to_string() } else { list.join(",") };
    Ok(vec![
        format!(
            "level {} pcm_records={} data_base={} data_len={}",
            unit.name, unit.records, structure.data_base, structure.data_len
        ),
        format!(
            "  reloc entries={} valid={} zero={} out_of_range={}",
            reloc.entries, reloc.valid, reloc.zero, reloc.out_of_range
        ),
        format!(
            "  schema slots={} non_null={}{}",
            SCHEMA_OFFSET_SLOTS.len(),
            structure.slots.len(),
            at
        ),
        format!(
            "  table36 count={} stride={} lengths={lengths}",
            structure.table36.count, structure.table36.stride
        ),
    ])
}

/// Per-unit geometry summary: counts, offsets, the `i16` position box and its
/// centre, and face-index validation. No trailing vertex triple or face
/// attribute word is read.
pub fn geometry_lines(name: &str, g: &GeometryReport) -> Vec<String> {
    let box_line = if g.vertices > 0 {
        let [nx, ny, nz] = g.min;
        let [xx, xy, xz] = g.max;
        let center = format!(
            "{:.1},{:.1},{:.1}",
            (f64::from(nx) + f64::from(xx)) / 2.0,
            (f64::from(ny) + f64::from(xy)) / 2.0,
            (f64::from(nz) + f64::from(xz)) / 2.0
        );
        format!("  bbox min={nx},{ny},{nz} max={xx},{xy},{xz} center={center}")
    } else {
        "  bbox none".to_string()
    };
    let max_index =
        g.max_index.map_or_else(|| "none".to_string(), |index| index.to_string());
    vec![
        format!(
            "geometry {} vertices={} faces={} room={} vertex_at={} face_at={}",
            name, g.vertices, g.faces, g.room_offset, g.vertex_offset, g.face_offset
        ),
        box_line,
        format!("  indices out_of_range={} max_index={max_index}", g.out_of_range),
    ]
}

/// Per-unit object listing: a count line and one line per record. Positions are
/// world-space `i16`; type and flags print as raw record fields.
pub fn object_lines(name: &str, report: &ObjectReport) -> Vec<String> {
    let mut lines =
        vec![format!("objects {name} count={} array={}", report.count, report.array_offset)];
    lines.extend(report.objects.iter().map(|object| {
        format!(
            "  object {} pos={},{},{} type={} flags={}",
            object.name,
            object.position[0],
            object.position[1],
            object.position[2],
            object.object_type,
            object.flags
        )
    }));
    lines
}

/// `FMPa` magic, 16-bit id, segment count, and the rate field at `0x28`.
pub fn parse_pmf(bytes: &[u8]) -> Result<PmfHeader, LevelError> {
    if bytes.len() < 0x2C || bytes.get(0..4) != Some(b"FMPa".as_slice()) {
        return Err(LevelError::BadSidecarMagic);
    }
    let id = read_u16(bytes, 0x06).ok_or(LevelError::BadSidecarMagic)?;
    let segments = read_u16(bytes, 0x08).ok_or(LevelError::BadSidecarMagic)?;
    if segments == 0 || segments > MAX_SEGMENTS {
        return Err(LevelError::CountCeiling);
    }
    let rate_field = read_u32(bytes, 0x28).ok_or(LevelError::BadSidecarMagic)?;
    Ok(PmfHeader { id, segments, rate_field })
}

/// `FNPa` magic, 20-byte header + one `u16` per segment: `len == 20 + 2n`.
pub fn parse_pnf(bytes: &[u8]) -> Result<PnfHeader, LevelError> {
    if bytes.len() < 20 || bytes.get(0..4) != Some(b"FNPa".as_slice()) {
        return Err(LevelError::BadSidecarMagic);
    }
    let id = read_u16(bytes, 0x06).ok_or(LevelError::BadSidecarMagic)?;
    let entries = u32::from(read_u16(bytes, 0x0A).ok_or(LevelError::BadSidecarMagic)?);
    if bytes.len() != 20 + 2 * entries as usize {
        return Err(LevelError::SidecarMismatch);
    }
    Ok(PnfHeader { id, entries })
}

pub fn read_path(path: &Path) -> Result<Vec<LevelUnit>, LevelError> {
    if path.is_dir() {
        let bin = path.join("BIN");
        let directory = if bin.is_dir() { bin } else { path.to_path_buf() };
        let index = SidecarIndex::read(&directory)?;
        if index.pcm_paths.is_empty() {
            return Err(LevelError::NoUnits);
        }
        return load_units(&index.pcm_paths, &index);
    }
    if !is_pcm(path) {
        return Err(LevelError::RefusedPath);
    }
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let index = SidecarIndex::read(directory)?;
    load_units(&[path.to_path_buf()], &index)
}

pub fn report_lines(unit: &LevelUnit) -> Vec<String> {
    let (pmf, segments, rate, pnf) = match unit.sidecar {
        Sidecar::Absent => ("absent", 0, 0, 0),
        Sidecar::Present { segments, rate_field, pnf_entries } => {
            ("present", u32::from(segments), rate_field, pnf_entries)
        }
    };
    let mut lines = vec![format!(
        "level {} pcm_records={} names={} links={} pmf={pmf} segments={segments} rate_field={rate} pnf={pnf}",
        unit.name,
        unit.records,
        unit.names.len(),
        unit.links.len()
    )];
    lines.extend(unit.links.iter().map(|link| format!("  link {} {}", link.target, link.index)));
    lines
}

/// One `links` line per unit, then one line per decoded link-table record.
/// Prelude values are raw integers and are still printed in full; `target_id`
/// repeats field 4 so the stable target identity is visible without indexing.
pub fn link_lines(unit: &LevelUnit) -> Vec<String> {
    let mut lines = vec![format!("links {} count={}", unit.name, unit.link_table.len())];
    lines.extend(unit.link_table.iter().map(|link| {
        let [a, b, c, d, e] = link.prelude;
        format!(
            "  link {} {} target_id={} prelude={a},{b},{c},{d},{e}",
            link.target, link.index, link.target_id
        )
    }));
    lines
}

fn load_units(pcm_paths: &[PathBuf], index: &SidecarIndex) -> Result<Vec<LevelUnit>, LevelError> {
    let mut units = Vec::with_capacity(pcm_paths.len());
    for pcm_path in pcm_paths {
        units.push(load_unit(pcm_path, index)?);
    }
    units.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(units)
}

fn load_unit(pcm_path: &Path, index: &SidecarIndex) -> Result<LevelUnit, LevelError> {
    let stem = pcm_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .ok_or(LevelError::Io)?;
    let bytes = std::fs::read(pcm_path).map_err(|_| LevelError::Io)?;
    let mut unit = parse_pcm(&bytes, &stem)?;
    unit.sidecar = index.sidecar(&stem)?;
    Ok(unit)
}

fn is_pcm(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pcm"))
}

struct SidecarIndex {
    pcm_paths: Vec<PathBuf>,
    files: Vec<(String, PathBuf)>,
}

impl SidecarIndex {
    fn read(directory: &Path) -> Result<Self, LevelError> {
        let mut pcm_paths = Vec::new();
        let mut files = Vec::new();
        for entry in std::fs::read_dir(directory).map_err(|_| LevelError::Io)? {
            let path = entry.map_err(|_| LevelError::Io)?.path();
            if !path.is_file() {
                continue;
            }
            if is_pcm(&path) {
                pcm_paths.push(path.clone());
            }
            if let Some(name) = path.file_name() {
                files.push((name.to_string_lossy().to_ascii_lowercase(), path));
            }
        }
        pcm_paths.sort();
        Ok(Self { pcm_paths, files })
    }

    fn find(&self, stem: &str, extension: &str) -> Option<&Path> {
        let wanted = format!("{}.{extension}", stem.to_ascii_lowercase());
        self.files.iter().find(|(name, _)| *name == wanted).map(|(_, path)| path.as_path())
    }

    fn sidecar(&self, stem: &str) -> Result<Sidecar, LevelError> {
        let (pmf_path, pnf_path) = match (self.find(stem, "pmf"), self.find(stem, "pnf")) {
            (None, None) => return Ok(Sidecar::Absent),
            (Some(pmf_path), Some(pnf_path)) => (pmf_path, pnf_path),
            _ => return Err(LevelError::SidecarIncomplete),
        };
        let pmf = parse_pmf(&std::fs::read(pmf_path).map_err(|_| LevelError::Io)?)?;
        let pnf = parse_pnf(&std::fs::read(pnf_path).map_err(|_| LevelError::Io)?)?;
        if pmf.id != pnf.id || u32::from(pmf.segments) != pnf.entries {
            return Err(LevelError::SidecarMismatch);
        }
        Ok(Sidecar::Present {
            segments: pmf.segments,
            rate_field: pmf.rate_field,
            pnf_entries: pnf.entries,
        })
    }
}

fn extract_names(bytes: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        while index < bytes.len() && (0x20..=0x7E).contains(&bytes[index]) {
            index += 1;
        }
        if let Some(name) = accepted_name(&bytes[start..index]) {
            names.push(name.to_string());
        }
        index += 1;
    }
    names
}

/// `^[a-z][a-z0-9]*(,[0-9]{1,2})?$` within 3..=20 bytes.
fn accepted_name(run: &[u8]) -> Option<&str> {
    if !(3..=MAX_NAME_LEN).contains(&run.len()) {
        return None;
    }
    let text = std::str::from_utf8(run).ok()?;
    let (target, digits) = text.split_once(',').map_or((text, None), |(t, d)| (t, Some(d)));
    let (first, rest) = target.as_bytes().split_first()?;
    if !first.is_ascii_lowercase() || !rest.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return None;
    }
    if let Some(digits) = digits {
        if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm_bytes(offsets: &[u32], body: &[u8]) -> Vec<u8> {
        let mut bytes = (offsets.len() as u32).to_le_bytes().to_vec();
        for offset in offsets {
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.resize(data_base(offsets.len() as u32), 0);
        bytes.extend_from_slice(body);
        bytes
    }

    fn synthetic_pcm(name: &str, extra: &[u8]) -> Vec<u8> {
        let mut body = name.as_bytes().to_vec();
        body.push(0);
        body.extend_from_slice(extra);
        body.extend_from_slice(&[0u8; 9]);
        pcm_bytes(&[0, 4], &body)
    }

    fn pmf(id: u16, segments: u16, rate_field: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x2C];
        bytes[0..4].copy_from_slice(b"FMPa");
        bytes[0x06..0x08].copy_from_slice(&id.to_le_bytes());
        bytes[0x08..0x0A].copy_from_slice(&segments.to_le_bytes());
        bytes[0x28..0x2C].copy_from_slice(&rate_field.to_le_bytes());
        bytes
    }

    fn pnf(id: u16, entries: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 20 + 2 * usize::from(entries)];
        bytes[0..4].copy_from_slice(b"FNPa");
        bytes[0x06..0x08].copy_from_slice(&id.to_le_bytes());
        bytes[0x0A..0x0C].copy_from_slice(&entries.to_le_bytes());
        bytes
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).expect("synthetic file must be writable");
    }

    #[test]
    fn pcm_header_and_names_parse() {
        let unit = parse_pcm(&synthetic_pcm("lair5", b"lair6,8"), "LAIR5").expect("must parse");
        assert_eq!(unit.name, "lair5");
        assert_eq!(unit.records, 2);
        assert_eq!(unit.names, vec!["lair5", "lair6,8"]);
        assert_eq!(unit.links, Vec::new());
        assert_eq!(unit.sidecar, Sidecar::Absent);
    }

    #[test]
    fn a_unit_without_a_link_table_reports_no_links() {
        let unit = parse_pcm(&synthetic_pcm("lair5", b"lair6,8"), "lair5").expect("must parse");
        assert_eq!(unit.names.len(), 2, "the name scan still sees the link name");
        assert!(unit.link_table.is_empty());
        assert!(unit.links.is_empty(), "no table means no links");
        assert!(report_lines(&unit)[0].contains("links=0"));
    }

    #[test]
    fn pcm_rejects_count_outside_range() {
        let below = pcm_bytes(&[0], &[]);
        let above = (MAX_RECORDS + 1).to_le_bytes().to_vec();
        for bytes in [below, above] {
            assert_eq!(
                parse_pcm(&bytes, "lair5").err(),
                Some(LevelError::CountCeiling)
            );
        }
    }

    #[test]
    fn pcm_requires_first_offsets() {
        let body = [0u8; 24];
        assert_eq!(
            parse_pcm(&pcm_bytes(&[4, 8], &body), "lair5").err(),
            Some(LevelError::TableNotAscending)
        );
        assert_eq!(
            parse_pcm(&pcm_bytes(&[0, 8], &body), "lair5").err(),
            Some(LevelError::TableNotAscending)
        );
    }

    #[test]
    fn pcm_rejects_descending_offsets() {
        assert_eq!(
            parse_pcm(&pcm_bytes(&[0, 4, 4], &[0u8; 32]), "lair5").err(),
            Some(LevelError::TableNotAscending)
        );
    }

    #[test]
    fn pcm_rejects_tables_past_eof() {
        assert_eq!(
            parse_pcm(&2u32.to_le_bytes(), "lair5").err(),
            Some(LevelError::TablePastEof)
        );
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 3]);
        assert_eq!(
            parse_pcm(&bytes, "lair5").err(),
            Some(LevelError::PaddingTruncated)
        );
        assert_eq!(
            parse_pcm(&pcm_bytes(&[0, 4, 5000], &[0u8; 8]), "lair5").err(),
            Some(LevelError::TablePastEof)
        );
    }

    #[test]
    fn data_base_rounds_the_record_count_up_to_512() {
        for (records, expected) in [(2, 2048), (62, 2048), (74, 2048), (512, 4096), (513, 4096)] {
            assert_eq!(data_base(records), expected, "records={records}");
            assert_eq!(data_base(records) % 2048, 0);
        }
        assert!(data_base(MAX_RECORDS) > 4 + 4 * MAX_RECORDS as usize);
    }

    #[test]
    fn pcm_reads_blocks_from_the_aligned_data_base() {
        let blocks = vec![block_of(4, 0x01), block_of(40, 0x00), named_block(48, 0x03)];
        let (bytes, body_len) = blocks_pcm(&blocks);
        assert_eq!(data_base(3), 2048);
        assert_eq!(bytes.len(), 2048 + body_len);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(unit.block_sizes, vec![4, 40, 48]);
        assert_eq!(unit.block_all_zero, vec![false, true, false]);
    }

    #[test]
    fn pcm_rejects_a_file_truncated_inside_the_offset_padding() {
        let (bytes, _) = blocks_pcm(&[block_of(4, 0x01), block_of(40, 0x02)]);
        assert_eq!(data_base(2), 2048);
        assert_eq!(
            parse_pcm(&bytes[..data_base(2) - 1], "lair5").err(),
            Some(LevelError::PaddingTruncated)
        );
        assert_eq!(
            parse_pcm(&bytes[..11], "lair5").err(),
            Some(LevelError::TablePastEof)
        );
    }

    #[test]
    fn pcm_requires_own_name() {
        assert_eq!(
            parse_pcm(&synthetic_pcm("kain", b"lair6,8"), "lair5").err(),
            Some(LevelError::MissingUnitName)
        );
        assert!(parse_pcm(&synthetic_pcm("lair5", b""), "lair5").is_ok());
    }

    #[test]
    fn name_rule_bounds_and_noise() {
        let unit = parse_pcm(
            &synthetic_pcm("lair5", b"LAIR\0abcdefghijklmnopqrstu\0ab\0lair6,123"),
            "lair5",
        )
        .expect("must parse");
        assert_eq!(unit.names, vec!["lair5"]);
    }

    #[test]
    fn links_come_from_the_link_table_not_the_name_scan() {
        let mut bytes = link_pcm(
            &[
                ([1, 67, 18, 67, -4579], "lair29,66"),
                ([1, 52, 18, 52, -4605], "lair3,51"),
            ],
            2 * LINK_RECORD_STRIDE,
        );
        let base = data_base(2);
        bytes[base + 16..base + 24].copy_from_slice(b"lair9,9\0");
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert!(
            unit.names.iter().any(|name| name == "lair9,9"),
            "the scan sees a link-like name the table does not contain"
        );
        assert_eq!(unit.link_table.len(), 2);
        assert_eq!(
            unit.links,
            vec![
                Link { target: "lair29".into(), index: 66 },
                Link { target: "lair3".into(), index: 51 },
            ]
        );
        assert!(report_lines(&unit)[0].contains("links=2"));
    }

    #[test]
    fn pmf_header_parses_and_refuses() {
        assert_eq!(
            parse_pmf(&pmf(7, 3, 22_050)),
            Ok(PmfHeader {
                id: 7,
                segments: 3,
                rate_field: 22_050
            })
        );
        let mut bad_magic = pmf(7, 3, 22_050);
        bad_magic[0] = b'X';
        assert_eq!(parse_pmf(&bad_magic).err(), Some(LevelError::BadSidecarMagic));
        assert_eq!(parse_pmf(&vec![0u8; 0x2C]).err(), Some(LevelError::BadSidecarMagic));
        assert_eq!(parse_pmf(&pmf(7, 0, 22_050)).err(), Some(LevelError::CountCeiling));
        assert_eq!(
            parse_pmf(&pmf(7, MAX_SEGMENTS + 1, 22_050)).err(),
            Some(LevelError::CountCeiling)
        );
    }

    #[test]
    fn pnf_header_parses_and_refuses() {
        assert_eq!(parse_pnf(&pnf(7, 3)), Ok(PnfHeader { id: 7, entries: 3 }));
        let mut contradiction = pnf(7, 3);
        contradiction.push(0);
        assert_eq!(
            parse_pnf(&contradiction).err(),
            Some(LevelError::SidecarMismatch)
        );
        let mut bad_magic = pnf(7, 0);
        bad_magic[0] = b'X';
        assert_eq!(
            parse_pnf(&bad_magic).err(),
            Some(LevelError::BadSidecarMagic)
        );
        assert_eq!(parse_pnf(&[0u8; 19]).err(), Some(LevelError::BadSidecarMagic));
    }

    #[test]
    fn read_path_single_file_with_sidecar() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "lair5.PCM", &synthetic_pcm("lair5", b"lair6,8"));
        write(tmp.path(), "lair5.pmf", &pmf(7, 3, 22_050));
        write(tmp.path(), "lair5.pnf", &pnf(7, 3));
        let units = read_path(&tmp.path().join("lair5.PCM")).expect("must read");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].links.len(), 0);
        assert_eq!(
            units[0].sidecar,
            Sidecar::Present {
                segments: 3,
                rate_field: 22_050,
                pnf_entries: 3
            }
        );
    }

    #[test]
    fn read_path_directory_prefers_bin_and_sorts() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let bin = tmp.path().join("BIN");
        std::fs::create_dir(&bin).expect("BIN must be creatable");
        write(&bin, "ZETA.PCM", &synthetic_pcm("zeta", b""));
        write(&bin, "alpha.PCM", &synthetic_pcm("alpha", b""));
        write(&bin, "notes.txt", b"noise");
        let units = read_path(tmp.path()).expect("must read");
        let names: Vec<&str> = units.iter().map(|unit| unit.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn read_path_directory_without_bin() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "alpha.PCM", &synthetic_pcm("alpha", b""));
        let units = read_path(tmp.path()).expect("must read");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "alpha");
    }

    #[test]
    fn read_path_refusals() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "lair5.PMF", &pmf(7, 1, 22_050));
        let pmf_path = tmp.path().join("lair5.PMF");
        assert_eq!(read_path(&pmf_path).err(), Some(LevelError::RefusedPath));
        let empty = tmp.path().join("EMPTY");
        std::fs::create_dir(&empty).expect("EMPTY must be creatable");
        assert_eq!(read_path(&empty).err(), Some(LevelError::NoUnits));
        assert_eq!(
            read_path(&tmp.path().join("missing.PCM")).err(),
            Some(LevelError::Io)
        );
    }

    #[test]
    fn sidecar_pair_checks() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        write(tmp.path(), "lair5.PCM", &synthetic_pcm("lair5", b""));
        let pcm_path = tmp.path().join("lair5.PCM");
        write(tmp.path(), "lair5.PMF", &pmf(7, 3, 11_025));
        assert_eq!(
            read_path(&pcm_path).err(),
            Some(LevelError::SidecarIncomplete)
        );
        write(tmp.path(), "lair5.PNF", &pnf(8, 3));
        assert_eq!(
            read_path(&pcm_path).err(),
            Some(LevelError::SidecarMismatch)
        );
        write(tmp.path(), "lair5.PNF", &pnf(7, 2));
        assert_eq!(
            read_path(&pcm_path).err(),
            Some(LevelError::SidecarMismatch)
        );
        write(tmp.path(), "lair5.PNF", &pnf(7, 3));
        assert!(read_path(&pcm_path).is_ok());
    }

    #[test]
    fn report_lines_shape() {
        let bytes = link_pcm(&[([1, 2, 3, 4, 5], "lair6,8")], LINK_RECORD_STRIDE);
        let mut unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(unit.names, vec!["lair6,8", "lair5"]);
        assert_eq!(
            report_lines(&unit),
            vec![
                "level lair5 pcm_records=2 names=2 links=1 pmf=absent segments=0 rate_field=0 pnf=0",
                "  link lair6 8",
            ]
        );
        unit.sidecar = Sidecar::Present {
            segments: 3,
            rate_field: 22_050,
            pnf_entries: 3,
        };
        assert_eq!(
            report_lines(&unit)[0],
            "level lair5 pcm_records=2 names=2 links=1 pmf=present segments=3 rate_field=22050 pnf=3"
        );
    }

    /// Room record at 16; its fields and both arrays sit after it, and the
    /// unit name stays in the last six bytes behind header word `0x26`.
    fn geometry_pcm(vertices: &[[i16; 3]], faces: &[[u16; 3]]) -> Vec<u8> {
        let room = 16usize;
        let face_at = room + 0x3C;
        let vertex_at = face_at + faces.len() * FACE_STRIDE;
        let len = (vertex_at + vertices.len() * VERTEX_STRIDE + 6).max(0xA6);
        let name_at = len - 6;
        let mut bytes = structure_pcm(
            &[0, 4],
            &[(SLOT_ROOM_RECORD, room as u32), (SLOT_UNIT_NAME, name_at as u32)],
            len,
        );
        let base = data_base(2);
        let record = base + room;
        for (field, value) in [
            (ROOM_VERTEX_COUNT, vertices.len() as u32),
            (ROOM_FACE_COUNT, faces.len() as u32),
            (ROOM_VERTEX_OFFSET, vertex_at as u32),
            (ROOM_FACE_OFFSET, face_at as u32),
        ] {
            bytes[record + field..record + field + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (index, vertex) in vertices.iter().enumerate() {
            let at = base + vertex_at + index * VERTEX_STRIDE;
            for (axis, value) in vertex.iter().enumerate() {
                bytes[at + 2 * axis..at + 2 * axis + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        for (index, face) in faces.iter().enumerate() {
            let at = base + face_at + index * FACE_STRIDE;
            for (slot, value) in face.iter().enumerate() {
                bytes[at + 2 * slot..at + 2 * slot + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }

    /// `geometry_pcm` with the room's `+0x34` texture array appended. Each
    /// record carries `words[i] & 0x07FF` as its page and fixed UVs; face i
    /// gets `offsets[i]` at `+0x0A`.
    fn textured_pcm(faces: &[[u16; 3]], offsets: &[u16], words: &[u16]) -> Vec<u8> {
        let mut bytes = geometry_pcm(&[[0, 0, 0], [100, 0, 0], [0, 100, 0]], faces);
        let (base, room) = (data_base(2), 16usize);
        let array_start = (bytes.len() - base) as u32;
        for &word in words {
            let mut record = [0u8; TEXTURE_STRIDE];
            record[2..4].copy_from_slice(&word.to_le_bytes());
            record[0..2].copy_from_slice(&[1, 2]);
            record[4..6].copy_from_slice(&[3, 4]);
            record[8..10].copy_from_slice(&[5, 6]);
            bytes.extend_from_slice(&record);
        }
        let at = base + room + ROOM_TEXTURE_START;
        bytes[at..at + 4].copy_from_slice(&array_start.to_le_bytes());
        let end = array_start + (words.len() * TEXTURE_STRIDE) as u32;
        let at = base + room + ROOM_TEXTURE_END;
        bytes[at..at + 4].copy_from_slice(&end.to_le_bytes());
        for (index, &offset) in offsets.iter().enumerate() {
            let face = base + room + 0x3C + index * FACE_STRIDE + FACE_TEXTURE_OFFSET;
            bytes[face..face + 2].copy_from_slice(&offset.to_le_bytes());
        }
        bytes
    }

    /// `geometry_pcm` with an object array appended after the room geometry;
    /// the array offset is data-base-relative, as in the real record.
    fn object_pcm(objects: &[(&str, [i16; 3], u16)]) -> Vec<u8> {
        let mut bytes = textured_pcm(&[[0, 1, 2]], &[0], &[7]);
        let base = data_base(2);
        let room = 16usize;
        let array = (bytes.len() - base) as u32;
        let count_at = base + room + OBJECT_COUNT;
        let offset_at = base + room + OBJECT_OFFSET;
        bytes[count_at..count_at + 4].copy_from_slice(&(objects.len() as u32).to_le_bytes());
        bytes[offset_at..offset_at + 4].copy_from_slice(&array.to_le_bytes());
        for (name, position, kind) in objects {
            let mut record = [0u8; OBJECT_STRIDE];
            let len = name.len().min(OBJECT_NAME_LEN - 1);
            record[..len].copy_from_slice(&name.as_bytes()[..len]);
            for (axis, value) in position.iter().enumerate() {
                let at = OBJECT_X + 2 * axis;
                record[at..at + 2].copy_from_slice(&value.to_le_bytes());
            }
            record[OBJECT_TYPE..OBJECT_TYPE + 4]
                .copy_from_slice(&(u32::from(*kind) << 16).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        bytes
    }

    /// Minimal SR1PC archive: count, 16-byte entries, then raw payloads.
    fn archive(entries: &[Vec<u8>]) -> Vec<u8> {
        let mut dat = (entries.len() as u32).to_le_bytes().to_vec();
        let mut offset = 4 + 16 * entries.len();
        for (index, entry) in entries.iter().enumerate() {
            dat.extend_from_slice(&(index as u32).to_le_bytes());
            dat.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            dat.extend_from_slice(&(offset as u32).to_le_bytes());
            dat.extend_from_slice(&0u32.to_le_bytes());
            offset += entry.len();
        }
        for entry in entries {
            dat.extend_from_slice(entry);
        }
        dat
    }

    fn geometry_of(bytes: &[u8]) -> Result<GeometryReport, LevelError> {
        let unit = parse_pcm(bytes, "lair5")?;
        geometry_report(&bytes[unit.structure.data_base..])
    }

    /// `geometry_pcm` whose self-name is appended after the last block and
    /// pointed at by header word `0x26`.
    fn named_geometry_pcm(name: &str, vertices: &[[i16; 3]], faces: &[[u16; 3]]) -> Vec<u8> {
        let mut bytes = geometry_pcm(vertices, faces);
        let base = data_base(2);
        let relative = (bytes.len() - base) as u32;
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        let slot = base + 4 * SLOT_UNIT_NAME;
        bytes[slot..slot + 4].copy_from_slice(&relative.to_le_bytes());
        bytes
    }

    fn group_archive(dir: &Path) -> PathBuf {
        let one = named_geometry_pcm("under1", &[[0, 0, 0], [4, 0, 0], [0, 4, 0]], &[[0, 1, 2]]);
        let two = named_geometry_pcm("under2", &[[10, 0, 0], [14, 0, 0], [10, 4, 0]], &[[0, 1, 2]]);
        let noise = named_geometry_pcm("underx", &[[0, 0, 0]], &[[0, 0, 0]]);
        let path = dir.join("BIGFILE.DAT");
        std::fs::write(&path, archive(&[one, two, noise]))
            .expect("synthetic archive must be writable");
        path
    }

    #[test]
    fn group_discovers_prefix_digit_names_and_reports_totals() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let rows = read_group_geometry(&group_archive(tmp.path()), "under")
            .expect("group must discover two rooms");
        let names: Vec<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, vec!["under1", "under2"]);
        let vertices: u32 = rows.iter().map(|(_, g)| g.vertices).sum();
        let faces: u32 = rows.iter().map(|(_, g)| g.faces).sum();
        assert_eq!((vertices, faces), (6, 2));
    }

    #[test]
    fn group_mesh_offsets_each_room_by_the_running_vertex_base() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let (name, mesh) = read_group_room_mesh(&group_archive(tmp.path()), "under", false, None)
            .expect("group must merge");
        assert_eq!(name, "group under");
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.vertices[0], [0.0, 0.0, 0.0]);
        assert_eq!(mesh.vertices[3], [10.0, 0.0, 0.0], "world-space position is not transformed");
        assert_eq!(mesh.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn objects_use_models_when_they_resolve_and_cubes_otherwise() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let pcm = object_pcm(&[("raziel", [5, 0, 0], 1), ("campath", [0, 0, 0], 1)]);
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, archive(&[pcm])).expect("synthetic archive must be writable");
        let triangle = || (vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]], vec![0, 1, 2]);
        let mut source = |name: &str| (name == "raziel").then(triangle);
        let (_, mesh) = read_entry_room_mesh(&path, 0, true, Some(&mut source))
            .expect("entry 0 must merge");
        assert_eq!(mesh.objects, ObjectStats { total: 2, meshes: 1, markers: 1 });
        assert_eq!(mesh.vertices.len(), 3 + 3 + 8, "room, one model, one cube");
        assert_eq!(mesh.vertices[3], [5.0, 0.0, 0.0], "model follows the object position");
        assert_eq!(mesh.indices.len(), 3 + 3 + 36);
    }

    #[test]
    fn textured_entry_expands_room_faces_and_maps_object_pages() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let pcm = object_pcm(&[("raziel", [5, 0, 0], 1), ("campath", [0, 0, 0], 1)]);
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, archive(&[pcm])).expect("synthetic archive must be writable");
        let mut source = |name: &str| {
            (name == "raziel").then(|| crate::object::TexturedMesh {
                positions: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
                uvs: vec![[1, 2], [3, 4], [5, 6]],
                indices: vec![0, 1, 2],
                material_indices: vec![2, 2, 2],
                pages: vec![9],
            })
        };
        let (_, mesh) = read_entry_room_mesh_textured(&path, 0, true, Some(&mut source))
            .expect("entry 0 must merge");
        assert_eq!(mesh.objects, ObjectStats { total: 2, meshes: 1, markers: 1 });
        assert_eq!(mesh.vertices.len(), 3 + 3 + 8, "room, one model, one cube");
        assert_eq!(mesh.uvs.len(), mesh.vertices.len());
        assert_eq!(mesh.material_indices.len(), mesh.vertices.len());
        assert_eq!(&mesh.material_indices[..3], &[2, 2, 2], "room page 7 maps to 2");
        assert_eq!(&mesh.material_indices[3..6], &[3, 3, 3], "object page 9 maps to 3");
        assert_eq!(&mesh.uvs[..3], &[[1, 2], [3, 4], [5, 6]], "room record UVs");
        assert_eq!(mesh.pages, vec![7, 9]);
        assert_eq!((mesh.textured_faces, mesh.unresolved_faces), (1, 0));
        assert_eq!(mesh.indices.len(), 3 + 3 + 36);
    }

    #[test]
    fn room_texture_faces_resolve_offsets_mask_pages_and_count_the_tail() {
        let bytes = textured_pcm(
            &[[0, 1, 2], [0, 1, 2], [0, 1, 2]],
            &[0, 0xFFFF, 6],
            &[0x0181, 0x8000 | 0x0182],
        );
        let data = &bytes[data_base(2)..];
        let faces = room_texture_faces(data).expect("must resolve");
        assert_eq!(faces.faces.len(), 3);
        assert_eq!(
            faces.faces[0],
            Some(FaceTexture { page: 385, uvs: [[1, 2], [3, 4], [5, 6]] })
        );
        assert_eq!(faces.faces[1], None, "0xFFFF tail");
        assert_eq!(faces.faces[2], None, "6 is not 12-aligned");
        assert_eq!(faces.unresolved, 2);
        let mesh = textured_room_mesh(data).expect("must expand");
        assert_eq!((mesh.textured_faces, mesh.unresolved_faces), (1, 2));
        assert_eq!((mesh.vertices.len(), mesh.indices.len()), (9, 9));
        assert_eq!(mesh.pages, vec![385]);
        assert_eq!(&mesh.material_indices[..3], &[2, 2, 2]);
        assert_eq!(&mesh.material_indices[3..6], &[1, 1, 1], "tail stays flat");
        assert_eq!(mesh.uvs[0], [1, 2]);
        assert_eq!(mesh.uvs[3], [0, 0]);
    }

    #[test]
    fn textured_room_mesh_reaches_past_the_u16_vertex_range() {
        let faces = vec![[0u16, 1, 2]; 22_000];
        let offsets = vec![0u16; faces.len()];
        let bytes = textured_pcm(&faces, &offsets, &[7]);
        let mesh = textured_room_mesh(&bytes[data_base(2)..])
            .expect("per-face expansion above 65_535 vertices must build u32 indices");
        assert_eq!(mesh.vertices.len(), 66_000);
        assert_eq!(mesh.indices.len(), 66_000);
        assert_eq!(mesh.indices[65_536], 65_536, "index value exceeds u16::MAX");
        assert_eq!(mesh.indices.iter().copied().max(), Some(65_999));
    }

    #[test]
    fn room_texture_array_missing_or_out_of_range_fails_loud() {
        let plain = geometry_pcm(&[[0, 0, 0], [1, 0, 0], [0, 1, 0]], &[[0, 1, 2]]);
        assert_eq!(
            room_texture_faces(&plain[data_base(2)..]).err(),
            Some(LevelError::RoomTexturesMissing)
        );
        let mut bytes = textured_pcm(&[[0, 1, 2]], &[0], &[7]);
        let at = data_base(2) + 16 + ROOM_TEXTURE_START;
        bytes[at..at + 4].copy_from_slice(&2000u32.to_le_bytes());
        assert_eq!(
            room_texture_faces(&bytes[data_base(2)..]).err(),
            Some(LevelError::RoomTextureArrayOutOfRange)
        );
    }

    #[test]
    fn group_refuses_empty_prefix_no_match_and_bad_geometry() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let good = named_geometry_pcm("under1", &[[0, 0, 0]], &[[0, 0, 0]]);
        let mut bad = named_geometry_pcm("under3", &[[0, 0, 0]], &[[0, 0, 0]]);
        let slot = data_base(2) + 4 * SLOT_ROOM_RECORD;
        bad[slot..slot + 4].copy_from_slice(&9000u32.to_le_bytes());
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, archive(&[good, bad])).expect("synthetic archive must be writable");
        assert_eq!(
            read_group_geometry(&path, "").err(),
            Some(LevelError::GroupEmptyPrefix)
        );
        assert!(matches!(
            read_group_geometry(&path, "nosuch"),
            Err(LevelError::GroupNoMatch { .. })
        ));
        assert!(matches!(
            read_group_geometry(&path, "under"),
            Err(LevelError::GroupParseFailed { .. })
        ));
        assert_eq!(LevelError::GroupWithEntry.exit_code(), 1);
        assert_eq!(LevelError::GroupNoMatch { prefix: "x".into() }.exit_code(), 1);
    }

    #[test]
    fn archive_entry_feeds_the_same_parser_and_mesh_path() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let pcm = geometry_pcm(&[[0, 0, 0], [4, 0, 0], [0, 4, 0]], &[[0, 1, 2]]);
        let dat = archive(&[b"not a pcm".to_vec(), pcm]);
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, dat).expect("synthetic archive must be writable");

        let rows = read_entry_geometry(&path, 1).expect("entry 1 must read");
        assert_eq!(rows[0].0, "lair5");
        assert_eq!((rows[0].1.vertices, rows[0].1.faces), (3, 1));
        let (name, mesh) = read_entry_room_mesh(&path, 1, false, None).expect("entry 1 must mesh");
        assert_eq!((name.as_str(), mesh.vertices.len(), mesh.indices.len()), ("lair5", 3, 3));

        assert!(matches!(
            read_entry_geometry(&path, 0),
            Err(LevelError::EntryNotPcm { .. })
        ));
        assert!(matches!(
            read_entry_geometry(&path, 2),
            Err(LevelError::EntryIndexOutOfRange { index: 2, entries: 2 })
        ));
        assert!(
            LevelError::EntryOutsideFile { detail: "x".into() }
                .to_string()
                .contains("outside the archive")
        );
    }

    #[test]
    fn geometry_reports_counts_offsets_box_and_index_validation() {
        let vertices = [[-10, 4, 0], [30, -6, 12], [2, 8, -20]];
        let faces = [[0u16, 1, 2], [2, 2, 1]];
        let g = geometry_of(&geometry_pcm(&vertices, &faces)).expect("must read");
        assert_eq!((g.vertices, g.faces, g.room_offset), (3, 2, 16));
        assert_eq!((g.min, g.max), ([-10, -6, -20], [30, 8, 12]));
        assert_eq!((g.out_of_range, g.max_index), (0, Some(2)));
        assert!(geometry_lines("lair5", &g)[1].ends_with("center=10.0,1.0,-4.0"));
    }

    #[test]
    fn geometry_counts_out_of_range_indices_without_failing() {
        let g = geometry_of(&geometry_pcm(&[[0, 0, 0]], &[[0, 1, 9]])).expect("must read");
        assert_eq!((g.out_of_range, g.max_index), (2, Some(9)));
    }

    #[test]
    fn geometry_zero_counts_report_zero_and_no_box() {
        let g = geometry_of(&geometry_pcm(&[], &[])).expect("must read");
        assert_eq!((g.vertices, g.faces, g.max_index), (0, 0, None));
        assert_eq!(geometry_lines("lair5", &g)[1], "  bbox none");
    }

    #[test]
    fn geometry_record_or_array_out_of_range_fails_loud() {
        let mut record = geometry_pcm(&[], &[]);
        record[data_base(2)..data_base(2) + 4].copy_from_slice(&2000u32.to_le_bytes());
        assert_eq!(geometry_of(&record).err(), Some(LevelError::GeometryRecordOutOfRange));
        let mut array = geometry_pcm(&[[0, 0, 0]], &[]);
        let count = data_base(2) + 16 + ROOM_VERTEX_COUNT;
        array[count..count + 4].copy_from_slice(&20u32.to_le_bytes());
        assert_eq!(geometry_of(&array).err(), Some(LevelError::GeometryArrayOutOfRange));
    }

    #[test]
    fn room_mesh_widens_positions_and_passes_indices() {
        let bytes = geometry_pcm(&[[-3, 4, 5], [7, -8, 9], [0, 1, 2]], &[[0, 1, 2]]);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let mesh = room_mesh(&bytes[unit.structure.data_base..]).expect("must read");
        assert_eq!(mesh.vertices, vec![[-3.0, 4.0, 5.0], [7.0, -8.0, 9.0], [0.0, 1.0, 2.0]]);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn object_array_decodes_names_world_positions_types_and_flags() {
        let bytes = object_pcm(&[
            ("undhost", [-5914, 0, 0], 0),
            ("raziel", [0, 0, 0], 4),
            ("marker", [10, -20, 30], 2),
        ]);
        let report = object_report(&bytes[data_base(2)..]).expect("must decode");
        assert_eq!((report.count, report.objects.len()), (3, 3));
        assert_eq!(report.objects[0].name, "undhost");
        assert_eq!(report.objects[0].position, [-5914, 0, 0]);
        assert_eq!((report.objects[0].object_type, report.objects[0].flags), (0, 0));
        assert_eq!(report.objects[1].name, "raziel");
        assert_eq!(report.objects[1].object_type, 4);
        assert_eq!(
            (report.objects[2].name.as_str(), report.objects[2].position),
            ("marker", [10, -20, 30])
        );
    }

    #[test]
    fn spawn_position_finds_raziel_case_insensitively() {
        let bytes =
            object_pcm(&[("undhost", [-5, 0, 0], 0), ("Raziel", [1, 2, 3], 4)]);
        assert_eq!(spawn_position(&bytes[data_base(2)..]), Some([1.0, 2.0, 3.0]));
        let bytes = object_pcm(&[("marker", [0, 0, 0], 1)]);
        assert_eq!(spawn_position(&bytes[data_base(2)..]), None);
    }

    #[test]
    fn object_array_outside_the_data_region_fails_loud() {
        let mut bytes = object_pcm(&[("marker", [0, 0, 0], 1)]);
        let base = data_base(2);
        let at = base + 16 + OBJECT_OFFSET;
        bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            object_report(&bytes[base..]).err(),
            Some(LevelError::ObjectArrayOutOfRange)
        );
    }

    #[test]
    fn object_lines_report_count_and_each_record() {
        let bytes = object_pcm(&[("undhost", [-5914, 0, 0], 0), ("raziel", [0, 0, 0], 4)]);
        let report = object_report(&bytes[data_base(2)..]).expect("must decode");
        let lines = object_lines("under1", &report);
        assert_eq!(lines[0], format!("objects under1 count=2 array={}", report.array_offset));
        assert_eq!(lines[1], "  object undhost pos=-5914,0,0 type=0 flags=0");
        assert_eq!(lines[2], "  object raziel pos=0,0,0 type=4 flags=0");
    }

    #[test]
    fn markers_add_one_cube_per_object_into_the_same_buffers() {
        let bytes = object_pcm(&[("a", [0, 0, 0], 0), ("b", [100, 0, 0], 0)]);
        let data = &bytes[data_base(2)..];
        let mut mesh = room_mesh(data).expect("room must read");
        let report = object_report(data).expect("objects must read");
        let vertices = mesh.vertices.len();
        let indices = mesh.indices.len();
        append_markers(&mut mesh, &report.objects).expect("markers must append");
        assert_eq!(mesh.vertices.len(), vertices + 16);
        assert_eq!(mesh.indices.len(), indices + 72);
        assert_eq!(mesh.vertices[vertices + 8][0] - mesh.vertices[vertices][0], 100.0);
    }

    #[test]
    fn group_markers_are_optional_and_absent_without_objects() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let path = group_archive(tmp.path());
        let (_, plain) = read_group_room_mesh(&path, "under", false, None).expect("plain merge");
        let (_, marked) = read_group_room_mesh(&path, "under", true, None).expect("marked merge");
        assert_eq!(plain, marked, "zero-count rooms must add no marker");
    }

    #[test]
    fn read_room_mesh_refuses_a_directory_and_codeless_units() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        assert_eq!(read_room_mesh(tmp.path(), false).err(), Some(LevelError::RenderDirectory));
        let pcm = tmp.path().join("lair5.PCM");
        write(tmp.path(), "lair5.PCM", &geometry_pcm(&[], &[]));
        assert_eq!(read_room_mesh(&pcm, false).err(), Some(LevelError::RenderEmpty));
        assert_eq!(read_room_mesh(&tmp.path().join("missing.PCM"), false).err(), Some(LevelError::Io));
        write(tmp.path(), "lair5.PCM", &geometry_pcm(&[[0, 0, 0]], &[[0, 1, 9]]));
        assert_eq!(read_room_mesh(&pcm, false).err(), Some(LevelError::RenderArrayOutOfRange));
    }

    fn block_of(size: usize, byte: u8) -> Vec<u8> {
        vec![byte; size]
    }

    fn named_block(size: usize, byte: u8) -> Vec<u8> {
        let mut block = vec![byte; size];
        block[..5].copy_from_slice(b"lair5");
        block
    }

    /// `N` blocks whose ascending table starts at 0, 4, so block 0 must be
    /// 4 bytes. Returns the file bytes and the data-region length.
    fn blocks_pcm(blocks: &[Vec<u8>]) -> (Vec<u8>, usize) {
        let mut offsets = Vec::with_capacity(blocks.len());
        let mut body = Vec::new();
        for block in blocks {
            offsets.push(body.len() as u32);
            body.extend_from_slice(block);
        }
        let bytes = pcm_bytes(&offsets, &body);
        let data_len = body.len();
        (bytes, data_len)
    }

    /// Body with chosen header dwords and the unit name in its last 6 bytes.
    fn structure_pcm(offsets: &[u32], slots: &[(usize, u32)], len: usize) -> Vec<u8> {
        let mut body = vec![0u8; len];
        for &(slot, value) in slots {
            body[4 * slot..4 * slot + 4].copy_from_slice(&value.to_le_bytes());
        }
        body[len - 6..].copy_from_slice(b"lair5\0");
        pcm_bytes(offsets, &body)
    }

    #[test]
    fn structure_reports_relocation_and_schema_slots() {
        let bytes = structure_pcm(&[0, 4, 12], &[(0x00, 16), (0x01, 0), (0x03, 20)], 40);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!((unit.structure.data_base, unit.structure.data_len), (2048, 40));
        let reloc = &unit.structure.reloc;
        assert_eq!(
            (reloc.entries, reloc.valid, reloc.zero, reloc.out_of_range),
            (3, 2, 1, 0)
        );
        assert_eq!(unit.structure.slots, vec![(0x00, 2064), (0x03, 2068)]);
        assert_eq!(
            structure_lines(&unit).expect("in range"),
            vec![
                "level lair5 pcm_records=3 data_base=2048 data_len=40",
                "  reloc entries=3 valid=2 zero=1 out_of_range=0",
                "  schema slots=14 non_null=2 0x00:2064 0x03:2068",
                "  table36 count=0 stride=36 lengths=none",
            ]
        );
    }

    #[test]
    fn table36_reports_blob_lengths_and_nulls() {
        let mut bytes = structure_pcm(
            &[0, 4],
            &[(0x00, 16), (0x01, 20), (0x3A, 6), (0x3B, 256)],
            800,
        );
        let base = data_base(2);
        for (index, first) in [500u32, 0, 600, 640, 700, 660].iter().enumerate() {
            let at = base + 256 + index * TABLE36_STRIDE;
            bytes[at..at + 4].copy_from_slice(&first.to_le_bytes());
        }
        let mut typed = vec![0u8; 20];
        typed[..6].copy_from_slice(&[0xBE, 0xEF, 0x02, 0x00, 0x01, 0x00]);
        for (at, blob) in [
            (500, &b"\xBE\xEF\x01\x00\x01\x03"[..]),
            (640, &b"\xBE\xEF\x00"[..]),
            (660, &typed[..]),
        ] {
            bytes[base + at..base + at + blob.len()].copy_from_slice(blob);
        }
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let table = &unit.structure.table36;
        assert_eq!((table.count, table.stride), (6, 36));
        assert_eq!(
            table.lengths,
            vec![Some(24), None, None, Some(4), None, Some(36)]
        );
        assert!(structure_lines(&unit).expect("in range")[3].ends_with(
            "table36 count=6 stride=36 lengths=24,null,null,4,null,36"
        ));
    }

    #[test]
    fn structure_fails_loud_on_an_out_of_range_directory_entry() {
        let bytes = structure_pcm(&[0, 4], &[(0x01, 90)], 40);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!((unit.structure.reloc.zero, unit.structure.reloc.out_of_range), (1, 1));
        assert_eq!(
            structure_lines(&unit).err(),
            Some(LevelError::RelocationOutOfRange)
        );
        let past = structure_pcm(&[0, 4, 40], &[], 40);
        let unit = parse_pcm(&past, "lair5").expect("must parse");
        assert_eq!(unit.structure.reloc.out_of_range, 1);
        assert!(structure_lines(&unit).is_err());
    }

    /// Header slot `0x34` points 256 bytes into the data region; each record
    /// fills its 12-byte name field and leaves its tail as written.
    fn link_pcm(records: &[([i32; 5], &str)], tail: usize) -> Vec<u8> {
        let offset = 256usize;
        let mut bytes = structure_pcm(&[0, 4], &[(SLOT_LINK_TABLE, offset as u32)], offset + tail);
        let base = data_base(2);
        for (index, (prelude, name)) in records.iter().enumerate() {
            let at = base + offset + index * LINK_RECORD_STRIDE;
            for (slot, value) in prelude.iter().enumerate() {
                bytes[at + 4 * slot..at + 4 * slot + 4].copy_from_slice(&value.to_le_bytes());
            }
            bytes[at + 20..at + 20 + name.len()].copy_from_slice(name.as_bytes());
        }
        bytes
    }

    fn links_of(bytes: &[u8]) -> Vec<TableLink> {
        parse_pcm(bytes, "lair5").expect("must parse").link_table
    }

    #[test]
    fn link_table_decodes_records_and_stops_at_the_terminator() {
        let bytes = link_pcm(
            &[
                ([1, 67, 18, 67, -4579], "lair29,66"),
                ([1, 52, 18, 52, -4605], "lair3,51"),
            ],
            2 * LINK_RECORD_STRIDE,
        );
        let links = links_of(&bytes);
        assert_eq!(links.len(), 2);
        assert_eq!(
            links[0],
            TableLink {
                target: "lair29".into(),
                index: 66,
                target_id: -4579,
                prelude: [1, 67, 18, 67, -4579]
            }
        );
        assert_eq!((links[1].target.as_str(), links[1].index), ("lair3", 51));
        assert_eq!(links[1].target_id, -4605);
    }

    #[test]
    fn target_id_exposes_the_fourth_prelude_field() {
        let bytes = link_pcm(
            &[
                ([1, 67, 18, 67, -4579], "lair29,66"),
                ([1, 52, 18, 52, -4605], "lair3,51"),
            ],
            2 * LINK_RECORD_STRIDE,
        );
        let links = links_of(&bytes);
        for link in &links {
            assert_eq!(link.target_id, link.prelude[4]);
        }
        assert_ne!(links[0].target_id, links[1].target_id);
    }

    #[test]
    fn a_zero_slot_or_a_non_link_first_record_means_no_table() {
        let no_slot = structure_pcm(&[0, 4], &[], 300);
        assert!(links_of(&no_slot).is_empty());
        let non_link = link_pcm(&[([9, 8, 7, 6, 5], "junk")], LINK_RECORD_STRIDE);
        assert!(links_of(&non_link).is_empty());
    }

    #[test]
    fn a_record_that_leaves_the_data_region_ends_the_walk() {
        let bytes = link_pcm(&[([1, 2, 3, 4, 5], "lair7,3")], LINK_RECORD_STRIDE + 20);
        let links = links_of(&bytes);
        assert_eq!(links.len(), 1);
        assert_eq!((links[0].target.as_str(), links[0].index), ("lair7", 3));
    }

    #[test]
    fn link_lines_report_the_count_targets_target_ids_and_raw_preludes() {
        let bytes = link_pcm(&[([1, 67, 18, 67, -4579], "lair29,66")], LINK_RECORD_STRIDE);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(
            link_lines(&unit),
            vec![
                "links lair5 count=1",
                "  link lair29 66 target_id=-4579 prelude=1,67,18,67,-4579",
            ]
        );
    }

    #[test]
    fn classify_is_zero_first_then_size() {
        for size in [0, 4, 40, 48, 72] {
            assert_eq!(classify(size, true), BlockClass::Zero);
        }
        assert_eq!(classify(4, false), BlockClass::Small4);
        assert_eq!(classify(40, false), BlockClass::Fixed40);
        assert_eq!(classify(48, false), BlockClass::Fixed48);
        assert_eq!(classify(72, false), BlockClass::Other(72));
    }

    #[test]
    fn census_counts_shape_classes_and_the_largest_block() {
        let blocks = vec![
            block_of(4, 0),
            block_of(4, 0x01),
            block_of(40, 0),
            block_of(40, 0x02),
            block_of(48, 0x03),
            named_block(72, 0x04),
        ];
        let (bytes, data_len) = blocks_pcm(&blocks);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let counts = census(&unit);
        assert_eq!(counts.total, 6);
        assert_eq!(
            (counts.zero, counts.small4, counts.fixed40, counts.fixed48),
            (2, 1, 1, 1)
        );
        assert_eq!(counts.other, vec![(72, 1)]);
        assert_eq!(counts.largest, (5, 72));
        let classified: u32 = counts.zero
            + counts.small4
            + counts.fixed40
            + counts.fixed48
            + counts.other.iter().map(|(_, count)| count).sum::<u32>();
        assert_eq!(classified, unit.records);
        assert_eq!(unit.block_sizes.iter().sum::<u32>() as usize, data_len);
    }

    #[test]
    fn largest_tie_keeps_the_lowest_ordinal_and_its_class() {
        let blocks = vec![block_of(4, 0), named_block(40, 0x02), block_of(40, 0x02)];
        let (bytes, _) = blocks_pcm(&blocks);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let counts = census(&unit);
        assert_eq!(counts.fixed40, 2, "the largest keeps its shape class");
        assert_eq!(counts.largest, (1, 40), "ties go to the lowest ordinal");
    }

    #[test]
    fn other_rows_ascend_by_size_and_lines_keep_fixed_order() {
        let blocks = vec![block_of(4, 0), block_of(72, 0x02), named_block(28, 0x04)];
        let (bytes, _) = blocks_pcm(&blocks);
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        let counts = census(&unit);
        assert_eq!(counts.other, vec![(28, 1), (72, 1)]);
        assert_eq!(counts.largest, (1, 72));
        assert_eq!(
            record_lines(&unit),
            vec![
                "blocks lair5 total=3",
                "  shape zero 1",
                "  shape other:28 1",
                "  shape other:72 1",
                "largest 1 72",
            ]
        );
    }

    /// Minimal unit with a header name, a link table, and the given targets.
    fn reach_pcm(name: &str, links: &[&str]) -> Vec<u8> {
        let offset = 256usize;
        let tail = links.len() * LINK_RECORD_STRIDE;
        let mut bytes = structure_pcm(&[0, 4], &[(SLOT_LINK_TABLE, offset as u32)], offset + tail);
        let base = data_base(2);
        let name_at = (bytes.len() - base) as u32;
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        let slot = base + 4 * SLOT_UNIT_NAME;
        bytes[slot..slot + 4].copy_from_slice(&name_at.to_le_bytes());
        for (index, target) in links.iter().enumerate() {
            let at = base + offset + index * LINK_RECORD_STRIDE;
            bytes[at + 20..at + 20 + target.len()].copy_from_slice(target.as_bytes());
        }
        bytes
    }

    fn archive_with_suffixes(units: &[(&[u8], [u8; 4])]) -> Vec<u8> {
        let mut dat = (units.len() as u32).to_le_bytes().to_vec();
        let mut offset = 4 + 16 * units.len();
        for (index, (entry, suffix)) in units.iter().enumerate() {
            dat.extend_from_slice(&(index as u32).to_le_bytes());
            dat.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            dat.extend_from_slice(&(offset as u32).to_le_bytes());
            dat.extend_from_slice(suffix);
            offset += entry.len();
        }
        for (entry, _) in units {
            dat.extend_from_slice(entry);
        }
        dat
    }

    #[test]
    fn suffix_resolves_the_unit_name_when_the_header_tail_disagrees() {
        let bytes = reach_pcm("under1", &[]);
        let mut unit = parse_pcm_bytes(&bytes).expect("must parse");
        unit.name = "wrongname".into();
        resolve_unit_name(&mut unit, b"DER1");
        assert_eq!(unit.name, "under1", "the scanned suffix match wins");
    }

    #[test]
    fn reach_bfs_reports_groups_orphans_and_unparseable_units() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let under1 = reach_pcm("under1", &["under2,1", "under3,1"]);
        let under2 = reach_pcm("under2", &["under1,1"]);
        let under3 = reach_pcm("under3", &[]);
        let city = reach_pcm("city1", &["lair9,1"]);
        let lair = reach_pcm("lair9", &[]);
        let broken = structure_pcm(&[0, 4], &[], 40);
        let dat = archive_with_suffixes(&[
            (&under1, *b"DER1"),
            (&under2, *b"DER2"),
            (&under3, *b"DER3"),
            (&city, *b"ITY1"),
            (&lair, *b"AIR9"),
            (&broken, *b"ROKN"),
        ]);
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, dat).expect("synthetic archive must be writable");
        let report = read_reach(&path, "under1").expect("reach must scan");
        assert_eq!(report.discovered, 6);
        assert_eq!(report.reachable, vec!["under1", "under2", "under3"]);
        assert_eq!(report.unreachable, vec!["city1", "lair9"]);
        assert_eq!(report.islands, vec!["city1"]);
        assert_eq!(report.inbound_unreachable, vec!["lair9"]);
        assert_eq!(report.unparseable.len(), 1);
        assert_eq!(report.unreachable_total(), 3);
        let lines = reach_lines(&report, "under1", false);
        assert_eq!(
            lines[0],
            "reach start=under1 discovered=6 reachable=3 unreachable=3 unparseable=1"
        );
        assert!(lines.contains(&"  reachable under 3".to_string()));
        assert!(lines.contains(&"  unreachable city 1".to_string()));
        assert!(lines.contains(&"  unreachable lair 1".to_string()));
        assert!(lines.contains(&"  islands total=1".to_string()));
        assert!(lines.contains(&"  island city 1".to_string()));
        assert!(lines.contains(&"  inbound-unreachable total=1".to_string()));
        assert!(lines.contains(&"  inbound-unreachable lair 1".to_string()));
        assert!(!lines.iter().any(|line| line.contains("island-name")));
        let named = reach_lines(&report, "under1", true);
        assert!(named.contains(&"  island-name city1".to_string()));
        assert_eq!(named.len(), lines.len() + 1, "names only adds lines");
        assert_eq!(
            read_reach(&path, "city1").expect("reach must scan").islands,
            report.islands,
            "islands are start-independent"
        );
        assert!(matches!(
            read_reach(&path, "nope"),
            Err(LevelError::ReachStartMissing { .. })
        ));
    }

    #[test]
    fn link_table_skips_a_short_prologue_and_stops_at_the_end_slot() {
        let mut bytes = link_pcm(&[([1, 2, 18, 2, -9], "under2,2")], LINK_RECORD_STRIDE + 24);
        let base = data_base(2);
        let record = bytes[base + 256..base + 296].to_vec();
        bytes[base + 256..base + 280].fill(0);
        bytes[base + 256..base + 262].copy_from_slice(b"lair5\0");
        bytes[base + 280..base + 320].copy_from_slice(&record);
        let slot = base + 4 * SLOT_LINK_END;
        bytes[slot..slot + 4].copy_from_slice(&320u32.to_le_bytes());
        let unit = parse_pcm(&bytes, "lair5").expect("must parse");
        assert_eq!(unit.link_table.len(), 1);
        assert_eq!(unit.link_table[0].target, "under2");
    }

    #[test]
    fn links_arms_accept_an_archive_entry_and_a_group() {
        let tmp = tempfile::tempdir().expect("tempdir must succeed");
        let one = reach_pcm("under1", &["under2,1"]);
        let two = reach_pcm("under2", &[]);
        let noise = reach_pcm("city1", &[]);
        let dat = archive_with_suffixes(&[
            (&one, *b"DER1"),
            (&two, *b"DER2"),
            (&noise, *b"ITY1"),
        ]);
        let path = tmp.path().join("BIGFILE.DAT");
        std::fs::write(&path, dat).expect("synthetic archive must be writable");
        let entry = read_entry_links(&path, 0).expect("entry links must read");
        assert_eq!(link_lines(&entry[0])[0], "links under1 count=1");
        let group = read_group_links(&path, "under").expect("group links must read");
        let names: Vec<&str> = group.iter().map(|unit| unit.name.as_str()).collect();
        assert_eq!(names, vec!["under1", "under2"]);
    }

    #[test]
    fn exit_codes_follow_spec() {
        assert_eq!(LevelError::Usage("usage").exit_code(), 1);
        assert_eq!(LevelError::RefusedPath.exit_code(), 3);
        for error in [
            LevelError::Io,
            LevelError::NoUnits,
            LevelError::CountCeiling,
            LevelError::TableNotAscending,
            LevelError::TablePastEof,
            LevelError::PaddingTruncated,
            LevelError::RelocationOutOfRange,
            LevelError::GeometryRecordOutOfRange,
            LevelError::GeometryArrayOutOfRange,
            LevelError::MissingUnitName,
            LevelError::SidecarIncomplete,
            LevelError::BadSidecarMagic,
            LevelError::SidecarMismatch,
        ] {
            assert_eq!(error.exit_code(), 2);
        }
    }
}

//! World and level data, constructed once from a loaded unit.
//!
//! Construction reuses the existing level parser: [`level::resolve_unit_entry`]
//! finds the archive entry by the unit's self-declared name, then
//! [`level::read_entry_objects`] reads the object array. No parsing is
//! duplicated here. The placed database is built once; the tick loop (T4+)
//! queries it and never iterates it.

use std::fmt;
use std::path::Path;

use crate::engine::collision::CollisionMesh;
use crate::engine::placed::{PlacedDb, PlacedObject};
use crate::level::{self, GroupWorldRoom, LevelError};

/// Loaded unit identity. `rooms` is 1 for a single unit and the number of
/// family members for a `--group` load.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LevelData {
    pub name: String,
    pub entry: usize,
    pub rooms: usize,
}

/// The authored world: level identity, the placed-object database and the
/// horizontal collision mesh built from the same loaded room triangles.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct World {
    pub level: LevelData,
    pub placed: PlacedDb,
    pub collision: CollisionMesh,
}

/// Why a world could not be built.
#[derive(Debug)]
pub enum WorldError {
    UnitNotFound { name: String },
    EmptyReport,
    Level(LevelError),
}

impl From<LevelError> for WorldError {
    fn from(error: LevelError) -> Self {
        Self::Level(error)
    }
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnitNotFound { name } => {
                write!(f, "unit '{name}' was not found in the archive")
            }
            Self::EmptyReport => write!(f, "archive entry produced no object report"),
            Self::Level(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for WorldError {}

impl World {
    /// Resolve `name` in the archive and build the world from that entry.
    ///
    /// The entry is read once: `read_entry_world` returns both the placed-object
    /// report and the integer room triangles, so the collision mesh is built
    /// from the same parse that feeds placement.
    pub fn load(dat: &Path, name: &str) -> Result<Self, WorldError> {
        let entry = level::resolve_unit_entry(dat, name)
            .ok_or_else(|| WorldError::UnitNotFound { name: name.to_string() })?;
        let (unit_name, report, triangles) = level::read_entry_world(dat, entry)?;
        let mut world = Self::from_report(unit_name, entry, &report);
        world.collision =
            CollisionMesh::from_room_triangles(&triangles.vertices, &triangles.faces);
        Ok(world)
    }

    /// Build the world from an already parsed object report.
    ///
    /// The collision mesh stays empty; callers that need geometry use
    /// [`World::load`] or assign [`World::collision`] explicitly.
    pub fn from_report(name: String, entry: usize, report: &level::ObjectReport) -> Self {
        let placed = PlacedDb::from_records(
            report.objects.iter().map(PlacedObject::from_record).collect(),
        );
        Self {
            level: LevelData { name, entry, rooms: 1 },
            placed,
            collision: CollisionMesh::default(),
        }
    }

    /// Load a whole archive level family: every unit named `prefix` + digits.
    ///
    /// The family selection and numeric-suffix order are [`level::read_group_world`]'s,
    /// the same discovery the `--level-* --group` modes use. Every room's placed
    /// records enter one [`PlacedDb`], and every room's triangles are merged into
    /// one collision mesh with face indices rebased by the running vertex base.
    /// `level.entry` is the first member's archive entry, the room that also
    /// binds the player (the first `raziel` placed record in family order).
    pub fn load_group(dat: &Path, prefix: &str) -> Result<Self, WorldError> {
        let rooms = level::read_group_world(dat, prefix)?;
        let first = rooms.first().map(|room| room.name.clone());
        let entry = first
            .and_then(|name| level::resolve_unit_entry(dat, &name))
            .unwrap_or(0);
        Ok(Self::from_group_reports(
            format!("group-{prefix}"),
            entry,
            rooms,
        ))
    }

    /// Merge already parsed family rooms into one world.
    fn from_group_reports(
        name: String,
        entry: usize,
        rooms: Vec<GroupWorldRoom>,
    ) -> Self {
        let room_count = rooms.len();
        let mut records = Vec::new();
        let mut vertices: Vec<[i32; 3]> = Vec::new();
        let mut faces: Vec<[u32; 3]> = Vec::new();
        for room in rooms {
            records.extend(
                room.objects
                    .objects
                    .iter()
                    .map(PlacedObject::from_record),
            );
            let base = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
            vertices.extend(room.triangles.vertices);
            faces.extend(room.triangles.faces.iter().map(|face| {
                [
                    face[0].saturating_add(base),
                    face[1].saturating_add(base),
                    face[2].saturating_add(base),
                ]
            }));
        }
        Self {
            level: LevelData {
                name,
                entry,
                rooms: room_count,
            },
            placed: PlacedDb::from_records(records),
            collision: CollisionMesh::from_room_triangles(&vertices, &faces),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LevelData, World};
    use crate::engine::frame::FrameState;
    use crate::level::{GroupWorldRoom, ObjectRecord, ObjectReport, RoomTriangles};

    fn record(
        name: &str,
        position: [i16; 3],
        object_type: u16,
        archetype: u16,
        instance_id: u16,
    ) -> ObjectRecord {
        ObjectRecord {
            name: name.to_string(),
            position,
            object_type,
            archetype,
            instance_id,
            flags: 0,
        }
    }

    #[test]
    fn world_builds_the_placed_db_and_keeps_duplicate_names() {
        let report = ObjectReport {
            room_offset: 16,
            count: 3,
            array_offset: 100,
            objects: vec![
                record("marker", [0, 0, 0], 4, 7, 1),
                record("marker", [10, 0, 0], 4, 7, 2),
                record("raziel", [1, 2, 3], 4, 8, 3),
            ],
        };
        let world = World::from_report("under1".to_string(), 115, &report);
        assert_eq!(
            world.level,
            LevelData { name: "under1".to_string(), entry: 115, rooms: 1 }
        );
        assert_eq!(world.placed.len(), 3);

        let markers = world.placed.by_name("marker");
        assert_eq!(markers.len(), 2);
        assert_ne!(markers[0], markers[1]);
        assert_eq!(
            world.placed.record(markers[1]).expect("second marker").position,
            [10, 0, 0]
        );
        assert_eq!(world.placed.by_id(2), &[1]);

        let raziel = world.placed.record(2).expect("raziel");
        assert_eq!((raziel.archetype, raziel.instance_id), (8, 3));
    }

    fn room(name: &str, objects: Vec<ObjectRecord>, triangles: RoomTriangles) -> GroupWorldRoom {
        GroupWorldRoom {
            name: name.to_string(),
            objects: ObjectReport {
                room_offset: 0,
                count: objects.len() as u32,
                array_offset: 0,
                objects,
            },
            triangles,
        }
    }

    #[test]
    fn group_world_merges_rooms_rebases_faces_and_binds_the_first_player() {
        let first = room(
            "under1",
            vec![record("raziel", [1, 0, 0], 4, 7, 1)],
            RoomTriangles {
                vertices: vec![[0, 0, 0], [4, 0, 0], [0, 4, 0]],
                faces: vec![[0, 1, 2]],
            },
        );
        let second = room(
            "under2",
            vec![record("capsule0", [50, 0, 0], 4, 7, 2)],
            RoomTriangles {
                vertices: vec![[10, 0, 0], [14, 0, 0], [10, 4, 0]],
                faces: vec![[0, 1, 2]],
            },
        );

        let world = World::from_group_reports("group-under".to_string(), 0, vec![first, second]);
        assert_eq!(
            world.level,
            LevelData {
                name: "group-under".to_string(),
                entry: 0,
                rooms: 2
            }
        );
        assert_eq!(world.placed.len(), 2, "both rooms contribute records");
        assert_eq!(world.placed.by_name("capsule0").len(), 1);
        assert_eq!(
            world.collision.surface_count(),
            2,
            "both rooms contribute a floor"
        );
        assert_eq!(
            world.collision.ground_height(12, 1, 100),
            Some(0),
            "the second room's face indices must be rebased, or this point has no floor"
        );

        let frame = FrameState::new(world);
        assert_eq!(
            frame.player_position(),
            Some([1, 0, 0]),
            "the player binds the first raziel record in family order"
        );
    }
}

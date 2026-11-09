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
use crate::level::{self, LevelError};

/// Loaded unit identity. `rooms` is 1 for the single-unit milestone.
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
}

#[cfg(test)]
mod tests {
    use super::{LevelData, World};
    use crate::level::{ObjectRecord, ObjectReport};

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
}

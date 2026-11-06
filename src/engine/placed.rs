//! Authored placed-object database.
//!
//! The database is **queried, never iterated per frame**. `records` is private
//! and there is no `iter()`: tick code must ask [`PlacedDb::by_name`] or
//! [`PlacedDb::by_id`] for the slots it needs and resolve them with
//! [`PlacedDb::record`]. Construction happens once when a unit loads; the tick
//! loop never walks the database.
//!
//! Duplicate names are normal (markers, repeated props). Every record stays
//! separate; the name index maps a name to every record slot that carries it,
//! so a duplicate is never silently collapsed.

use std::collections::BTreeMap;

use crate::level::ObjectRecord;

/// One placement read from a level unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedObject {
    pub name: String,
    pub position: [i16; 3],
    pub object_type: u16,
    pub archetype: u16,
    pub flags: u32,
    pub instance_id: u32,
}

impl PlacedObject {
    /// Widen one parser record; `instance_id` becomes a `u32` so later slices
    /// can address placements without depending on the record width.
    pub fn from_record(record: &ObjectRecord) -> Self {
        Self {
            name: record.name.clone(),
            position: record.position,
            object_type: record.object_type,
            archetype: record.archetype,
            flags: record.flags,
            instance_id: u32::from(record.instance_id),
        }
    }
}

/// Placed records plus a name index and an id index.
///
/// Both indexes store record slots, so a lookup is a query and never an
/// iteration over the whole database. `BTreeMap` keeps any debug listing
/// deterministic; tick paths do not iterate either map.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlacedDb {
    records: Vec<PlacedObject>,
    by_name: BTreeMap<String, Vec<usize>>,
    by_id: BTreeMap<u32, Vec<usize>>,
}

impl PlacedDb {
    /// Build the indexes once from the parser output.
    pub fn from_records(records: Vec<PlacedObject>) -> Self {
        let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut by_id: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (slot, record) in records.iter().enumerate() {
            by_name.entry(record.name.clone()).or_default().push(slot);
            by_id.entry(record.instance_id).or_default().push(slot);
        }
        Self { records, by_name, by_id }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Resolve one slot returned by [`by_name`](Self::by_name) or
    /// [`by_id`](Self::by_id).
    pub fn record(&self, slot: usize) -> Option<&PlacedObject> {
        self.records.get(slot)
    }

    /// Record slots carrying `name`, in placement order. Empty when absent.
    pub fn by_name(&self, name: &str) -> &[usize] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    /// Record slots carrying `instance_id`, in placement order. Empty when
    /// absent. Duplicate ids are preserved, not collapsed.
    pub fn by_id(&self, instance_id: u32) -> &[usize] {
        self.by_id.get(&instance_id).map_or(&[], Vec::as_slice)
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::{PlacedDb, PlacedObject};

    fn object(name: &str, instance_id: u32, position: [i16; 3]) -> PlacedObject {
        PlacedObject {
            name: name.to_string(),
            position,
            object_type: 4,
            archetype: 7,
            flags: 0,
            instance_id,
        }
    }

    #[test]
    fn duplicate_names_keep_separate_records() {
        let db = PlacedDb::from_records(vec![
            object("marker", 1, [0, 0, 0]),
            object("marker", 2, [10, 0, 0]),
            object("raziel", 3, [0, 0, 0]),
        ]);
        let slots = db.by_name("marker");
        assert_eq!(slots, &[0, 1]);
        assert_eq!(db.record(slots[0]).expect("slot 0").position, [0, 0, 0]);
        assert_eq!(db.record(slots[1]).expect("slot 1").position, [10, 0, 0]);
        assert_ne!(slots[0], slots[1], "duplicate names must not collapse");
        assert_eq!(db.by_name("raziel"), &[2]);
        assert_eq!(db.len(), 3);
    }

    #[test]
    fn id_index_keeps_duplicate_instance_ids() {
        let db = PlacedDb::from_records(vec![
            object("a", 7, [0, 0, 0]),
            object("b", 7, [1, 0, 0]),
        ]);
        assert_eq!(db.by_id(7), &[0, 1]);
        assert!(db.by_id(9).is_empty());
    }

    #[test]
    fn missing_name_is_an_empty_slice() {
        let db = PlacedDb::from_records(vec![object("a", 1, [0, 0, 0])]);
        assert!(db.by_name("b").is_empty());
        assert!(!db.contains_name("b"));
        assert!(db.contains_name("a"));
    }
}

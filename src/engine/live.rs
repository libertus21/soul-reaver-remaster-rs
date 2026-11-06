//! Live-object arena: ordered, generational ids and an explicit live order.
//!
//! The original keeps spawned objects in a global live list and advances them in
//! list order. Removals never reorder the survivors, and a reused slot must not
//! resurrect an old id. [`LiveArena`] reproduces that shape with three parts:
//!
//! - `slots` holds the optional object plus a per-slot generation;
//! - `free` is the free list used by [`LiveArena::spawn`];
//! - `order` is the explicit live order (spawn order).
//!
//! Iteration follows `order`, never the slot array, so tick order is spawn order
//! and a free-list reuse appends the new object at the end of the live list. The
//! slot index of a reused object can therefore sit in the middle of the array
//! while its live position stays last.

use std::fmt;

use crate::engine::class::RenderClass;

/// Stable identity of a live object.
///
/// `index` addresses the slot; `generation` distinguishes objects that reused
/// the same slot. A [`LiveId`] from before a reuse carries the older generation
/// and is rejected instead of silently addressing the new occupant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiveId {
    pub index: u32,
    pub generation: u32,
}

/// One live object: the authored identity plus the runtime state this milestone
/// names.
///
/// `slot` and `anim_index` are placeholders for the tracker slot and the tracker
/// animation index (`+0x1A`) that later slices fill; they stay numeric and carry
/// no invented behaviour. Actor state and the class hooks belong to later
/// slices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveObject {
    pub name: String,
    pub class: RenderClass,
    pub position: [i32; 3],
    pub visible: bool,
    pub slot: Option<usize>,
    pub anim_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    generation: u32,
    object: Option<LiveObject>,
}

/// Why an id could not be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveArenaError {
    /// The index is past the last slot ever allocated.
    OutOfRange { index: u32 },
    /// The slot was reused: the id's generation is older than the occupant's.
    Stale { id: LiveId },
    /// The slot still carries this generation but the object was removed.
    Dead { id: LiveId },
}

impl fmt::Display for LiveArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { index } => write!(f, "live slot {index} is out of range"),
            Self::Stale { id } => write!(
                f,
                "live id slot {} generation {} is stale",
                id.index, id.generation
            ),
            Self::Dead { id } => write!(
                f,
                "live id slot {} generation {} was removed",
                id.index, id.generation
            ),
        }
    }
}

impl std::error::Error for LiveArenaError {}

/// Ordered live list with generational slot reuse.
///
/// The container is deliberately not a map: the live order and the generation
/// check are observable behaviour, exactly like the original live list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveArena {
    slots: Vec<Slot>,
    free: Vec<usize>,
    order: Vec<LiveId>,
}

impl LiveArena {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `object` at the end of the live order and return its id.
    ///
    /// A reused slot bumps its generation, so every older [`LiveId`] for that
    /// slot becomes [`LiveArenaError::Stale`]. The new object always lands at
    /// the end of `order`, even when its slot index is in the middle of the
    /// array.
    pub fn spawn(&mut self, object: LiveObject) -> LiveId {
        let (index, generation) = match self.free.pop() {
            Some(index) => {
                let slot = &mut self.slots[index];
                slot.generation = slot.generation.wrapping_add(1);
                (index, slot.generation)
            }
            None => {
                let index = self.slots.len();
                self.slots.push(Slot {
                    generation: 0,
                    object: None,
                });
                (index, 0)
            }
        };
        self.slots[index].object = Some(object);
        let id = LiveId {
            index: index as u32,
            generation,
        };
        self.order.push(id);
        id
    }

    /// Remove `id`, preserving the relative order of every survivor.
    pub fn despawn(&mut self, id: LiveId) -> Result<LiveObject, LiveArenaError> {
        let index = self.resolve_index(id)?;
        let object = self.slots[index]
            .object
            .take()
            .expect("a resolved slot is occupied");
        self.order.retain(|live| *live != id);
        self.free.push(index);
        Ok(object)
    }

    /// Borrow the live object behind `id`; stale and dead ids are typed errors.
    pub fn get(&self, id: LiveId) -> Result<&LiveObject, LiveArenaError> {
        let index = self.resolve_index(id)?;
        Ok(self.slots[index]
            .object
            .as_ref()
            .expect("a resolved slot is occupied"))
    }

    /// Mutably borrow the live object behind `id`.
    pub fn get_mut(&mut self, id: LiveId) -> Result<&mut LiveObject, LiveArenaError> {
        let index = self.resolve_index(id)?;
        Ok(self.slots[index]
            .object
            .as_mut()
            .expect("a resolved slot is occupied"))
    }

    /// Live ids in spawn order. Survivors keep their relative order.
    pub fn order(&self) -> &[LiveId] {
        &self.order
    }

    /// Live objects in spawn order, never in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (LiveId, &LiveObject)> + '_ {
        self.order.iter().filter_map(move |id| {
            self.slots
                .get(id.index as usize)
                .and_then(|slot| slot.object.as_ref())
                .map(|object| (*id, object))
        })
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    fn resolve_index(&self, id: LiveId) -> Result<usize, LiveArenaError> {
        let slot = self
            .slots
            .get(id.index as usize)
            .ok_or(LiveArenaError::OutOfRange { index: id.index })?;
        if slot.generation != id.generation {
            return Err(LiveArenaError::Stale { id });
        }
        if slot.object.is_none() {
            return Err(LiveArenaError::Dead { id });
        }
        Ok(id.index as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::{LiveArena, LiveArenaError, LiveId, LiveObject};
    use crate::engine::class::RenderClass;

    fn object(name: &str) -> LiveObject {
        LiveObject {
            name: name.to_string(),
            class: RenderClass::Unclassified,
            position: [0, 0, 0],
            visible: true,
            slot: None,
            anim_index: 0,
        }
    }

    fn names(arena: &LiveArena) -> Vec<&str> {
        arena
            .iter()
            .map(|(_, object)| object.name.as_str())
            .collect()
    }

    fn ids(arena: &LiveArena) -> Vec<(u32, u32)> {
        arena
            .order()
            .iter()
            .map(|id| (id.index, id.generation))
            .collect()
    }

    #[test]
    fn live_order_is_spawn_order_after_removals() {
        let mut arena = LiveArena::new();
        let a = arena.spawn(object("raziel__"));
        let b = arena.spawn(object("monster_wallcr"));
        let c = arena.spawn(object("particle"));
        assert_eq!(names(&arena), vec!["raziel__", "monster_wallcr", "particle"]);

        arena.despawn(b).expect("remove the middle object");
        assert_eq!(names(&arena), vec!["raziel__", "particle"]);
        assert_eq!(arena.order(), &[a, c]);
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn free_list_reuse_does_not_disturb_live_order() {
        let mut arena = LiveArena::new();
        let a = arena.spawn(object("a"));
        let b = arena.spawn(object("b"));
        let c = arena.spawn(object("c"));

        arena.despawn(b).expect("free the middle slot");
        let d = arena.spawn(object("d"));
        assert_eq!(d.index, b.index, "the freed slot index is reused");
        assert_eq!(d.generation, b.generation + 1, "reuse bumps generation");

        assert_eq!(
            names(&arena),
            vec!["a", "c", "d"],
            "the reused object appends to the live order"
        );
        assert_eq!(arena.order(), &[a, c, d]);
        assert_eq!(ids(&arena), vec![(0, 0), (2, 0), (1, 1)]);
    }

    #[test]
    fn stale_id_is_rejected_after_slot_reuse() {
        let mut arena = LiveArena::new();
        let first = arena.spawn(object("first"));
        arena.despawn(first).expect("remove the first object");
        let second = arena.spawn(object("second"));

        assert_eq!(second.index, first.index, "the slot is reused");
        assert_ne!(second.generation, first.generation);
        assert_eq!(
            arena.get(first),
            Err(LiveArenaError::Stale { id: first }),
            "the old generation must not resolve to the new occupant"
        );
        assert_eq!(arena.despawn(first), Err(LiveArenaError::Stale { id: first }));
        assert_eq!(
            arena.get(second).expect("the new id resolves").name,
            "second"
        );
    }

    #[test]
    fn removed_id_before_reuse_is_dead_not_out_of_range() {
        let mut arena = LiveArena::new();
        let id = arena.spawn(object("soul"));
        arena.despawn(id).expect("remove");
        assert_eq!(arena.get(id), Err(LiveArenaError::Dead { id }));
    }

    #[test]
    fn unknown_index_is_out_of_range() {
        let mut arena = LiveArena::new();
        let id = LiveId {
            index: 7,
            generation: 0,
        };
        assert_eq!(
            arena.get(id),
            Err(LiveArenaError::OutOfRange { index: 7 })
        );
        assert_eq!(
            arena.despawn(id),
            Err(LiveArenaError::OutOfRange { index: 7 })
        );
    }

    #[test]
    fn get_mut_updates_the_object_in_place() {
        let mut arena = LiveArena::new();
        let id = arena.spawn(object("raziel__"));
        let entry = arena.get_mut(id).expect("live id");
        entry.position = [3, 4, 5];
        entry.visible = false;
        let entry = arena.get(id).expect("live id");
        assert_eq!(entry.position, [3, 4, 5]);
        assert!(!entry.visible);
        assert_eq!(ids(&arena), vec![(0, 0)]);
    }

    #[test]
    fn live_object_keeps_its_render_class() {
        let mut arena = LiveArena::new();
        let id = arena.spawn(LiveObject {
            class: RenderClass::Monster,
            ..object("monster_wallcr")
        });
        assert_eq!(
            arena.get(id).expect("live id").class,
            RenderClass::Monster
        );
    }

    #[test]
    fn identical_spawn_remove_sequences_replay_identically() {
        fn run() -> LiveArena {
            let mut arena = LiveArena::new();
            let a = arena.spawn(object("a"));
            let b = arena.spawn(object("b"));
            let c = arena.spawn(object("c"));
            arena.despawn(b).expect("free middle");
            arena.spawn(object("d"));
            arena.despawn(a).expect("free first");
            let _ = c;
            arena
        }

        let first = run();
        let second = run();
        assert_eq!(first, second, "the arena must replay byte for byte");
        assert_eq!(ids(&first), ids(&second));
        assert_eq!(ids(&first), vec![(2, 0), (1, 1)]);
        assert_eq!(names(&first), vec!["c", "d"]);
    }
}

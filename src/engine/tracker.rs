//! Fixed 48-slot, name-keyed object tracker.
//!
//! The original keeps per-object runtime state outside the placed record, in a
//! fixed 48-slot table keyed by the object name. Spawning scans for a live slot
//! with that name; when absent it takes the first free slot. Duplicate names
//! share one slot, and the lowest free index always wins.
//!
//! The cap is a hard invariant: the 49th distinct name is a typed
//! [`TrackerError::Full`], never a panic (exit 4 in the CLI contract).

use std::fmt;

/// Hard cap on tracked objects. The original prints and fails on overflow.
pub const MAX_OBJECTS: usize = 48;

/// Slot lifecycle. The numeric codes match the original `+0x18` field
/// (`0` empty, `1` loading, `2` ready) and are the canonical fold values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum SlotState {
    #[default]
    Empty = 0,
    Loading = 1,
    Ready = 2,
}

impl SlotState {
    /// Stable code used by the determinism fold. Never reorder the variants.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// One tracker slot. A name exists only while the slot is not empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrackerSlot {
    name: Option<String>,
    state: SlotState,
}

impl TrackerSlot {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn state(&self) -> SlotState {
        self.state
    }
}

/// Why an acquisition or lifecycle transition failed. Data, not a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerError {
    Full,
    InvalidSlot { slot: usize },
    EmptySlot { slot: usize },
}

impl fmt::Display for TrackerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "The Object tracker is full MAX_OBJECTS=48."),
            Self::InvalidSlot { slot } => {
                write!(f, "tracker slot {slot} is out of range (0..{MAX_OBJECTS})")
            }
            Self::EmptySlot { slot } => write!(f, "tracker slot {slot} is empty"),
        }
    }
}

impl std::error::Error for TrackerError {}

/// Fixed-capacity name-keyed tracker.
///
/// The array is deliberately not a map: the cap and the first-free allocation
/// order are observable behaviour, exactly like the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectTracker {
    slots: [TrackerSlot; MAX_OBJECTS],
}

impl ObjectTracker {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| TrackerSlot::default()),
        }
    }

    /// Acquire `name`.
    ///
    /// An existing name shares its slot and is returned unchanged. Otherwise
    /// the lowest empty slot becomes `Loading` and its index is returned. When
    /// all 48 slots are occupied, returns [`TrackerError::Full`] and mutates
    /// nothing.
    pub fn acquire(&mut self, name: &str) -> Result<usize, TrackerError> {
        if let Some(index) = self.find(name) {
            return Ok(index);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == SlotState::Empty)
            .ok_or(TrackerError::Full)?;
        self.slots[index] = TrackerSlot {
            name: Some(name.to_string()),
            state: SlotState::Loading,
        };
        Ok(index)
    }

    /// Mark a non-empty slot `Ready` (its asset load finished).
    pub fn complete(&mut self, slot: usize) -> Result<(), TrackerError> {
        let entry = self.slot_mut(slot)?;
        if entry.state == SlotState::Empty {
            return Err(TrackerError::EmptySlot { slot });
        }
        entry.state = SlotState::Ready;
        Ok(())
    }

    /// Free a non-empty slot. The next acquisition reuses the lowest free index.
    pub fn release(&mut self, slot: usize) -> Result<(), TrackerError> {
        let entry = self.slot_mut(slot)?;
        if entry.state == SlotState::Empty {
            return Err(TrackerError::EmptySlot { slot });
        }
        *entry = TrackerSlot::default();
        Ok(())
    }

    /// Slot index holding `name`, whether it is loading or ready.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.name.as_deref() == Some(name))
    }

    /// Ready slot indices in ascending (slot) order.
    pub fn ready_slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.state == SlotState::Ready)
            .map(|(index, _)| index)
    }

    /// The whole table in slot order. Read-only: mutation goes through the
    /// lifecycle methods so the name/state invariant cannot be broken.
    pub fn slots(&self) -> &[TrackerSlot; MAX_OBJECTS] {
        &self.slots
    }

    fn slot_mut(&mut self, index: usize) -> Result<&mut TrackerSlot, TrackerError> {
        self.slots
            .get_mut(index)
            .ok_or(TrackerError::InvalidSlot { slot: index })
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_OBJECTS, ObjectTracker, SlotState, TrackerError};

    #[test]
    fn the_49th_distinct_name_is_a_typed_full_error() {
        let mut tracker = ObjectTracker::new();
        for index in 0..MAX_OBJECTS {
            tracker
                .acquire(&format!("object{index}"))
                .expect("first 48 names fit");
        }

        let error = tracker.acquire("overflow").expect_err("the 49th must fail");
        assert_eq!(error, TrackerError::Full);
        assert_eq!(
            error.to_string(),
            "The Object tracker is full MAX_OBJECTS=48.",
            "the documented message is part of the contract"
        );
        assert!(
            tracker.find("overflow").is_none(),
            "a failed acquire mutates nothing"
        );
    }

    #[test]
    fn duplicate_names_share_exactly_one_slot() {
        let mut tracker = ObjectTracker::new();
        assert_eq!(tracker.acquire("raziel").expect("first"), 0);
        assert_eq!(tracker.acquire("raziel").expect("duplicate"), 0);
        assert_eq!(tracker.acquire("raziel").expect("duplicate again"), 0);
        assert_eq!(tracker.find("raziel"), Some(0));
        assert_eq!(tracker.slots()[0].name(), Some("raziel"));
        assert_eq!(tracker.slots()[0].state(), SlotState::Loading);
    }

    #[test]
    fn release_then_acquire_reuses_the_lowest_free_slot() {
        let mut tracker = ObjectTracker::new();
        tracker.acquire("a").expect("slot 0");
        tracker.acquire("b").expect("slot 1");
        tracker.acquire("c").expect("slot 2");

        tracker.release(1).expect("free middle");
        assert_eq!(tracker.acquire("d").expect("reuse middle"), 1);

        tracker.release(2).expect("free last");
        tracker.release(0).expect("free first");
        assert_eq!(
            tracker.acquire("e").expect("lowest free wins"),
            0,
            "slot 0 is lower than the free slot 2"
        );
        assert_eq!(tracker.acquire("f").expect("next free"), 2);
    }

    #[test]
    fn lifecycle_transitions_are_typed_and_ordered() {
        let mut tracker = ObjectTracker::new();
        assert_eq!(
            tracker.complete(MAX_OBJECTS),
            Err(TrackerError::InvalidSlot { slot: MAX_OBJECTS })
        );
        assert_eq!(
            tracker.release(MAX_OBJECTS),
            Err(TrackerError::InvalidSlot { slot: MAX_OBJECTS })
        );
        assert_eq!(
            tracker.complete(0),
            Err(TrackerError::EmptySlot { slot: 0 })
        );
        assert_eq!(tracker.release(0), Err(TrackerError::EmptySlot { slot: 0 }));

        let slot = tracker.acquire("soul").expect("acquire");
        assert_eq!(tracker.slots()[slot].state(), SlotState::Loading);
        assert!(tracker.ready_slots().next().is_none());
        tracker.complete(slot).expect("complete");
        assert_eq!(tracker.slots()[slot].state(), SlotState::Ready);
        assert_eq!(tracker.ready_slots().collect::<Vec<_>>(), vec![slot]);
        tracker.release(slot).expect("release");
        assert_eq!(tracker.slots()[slot].state(), SlotState::Empty);
        assert_eq!(tracker.slots()[slot].name(), None);
    }

    #[test]
    fn identical_call_sequences_produce_identical_slot_tables() {
        fn run() -> ObjectTracker {
            let mut tracker = ObjectTracker::new();
            tracker.acquire("raziel").expect("slot 0");
            tracker.complete(0).expect("ready");
            tracker.acquire("soul").expect("slot 1");
            tracker.release(0).expect("free 0");
            tracker.acquire("paths").expect("reuse 0");
            tracker
                .acquire("raziel")
                .expect("slot 2, the released name is gone");
            tracker
        }

        let first = run();
        let second = run();
        assert_eq!(first, second, "slot tables must match across replays");
        assert_eq!(first.find("paths"), Some(0));
        assert_eq!(first.find("soul"), Some(1));
        assert_eq!(first.find("raziel"), Some(2));
    }
}

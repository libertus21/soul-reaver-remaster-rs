//! Event/message carrier for the actor layer.
//!
//! This module is **plumbing, never semantics**. The evidence pins the carrier
//! shapes, the producing/consuming frame stages and one deterministic ordering;
//! it does not pin what any event does to an actor. Nothing here applies damage,
//! health, knockback, a state transition or a relation change. The transition
//! entry point [`crate::engine::actor::ActorRuntime::advance`] receives these
//! events and still selects nothing.
//!
//! Evidence base:
//!
//! - The event/message entry is hook slot 3 of the six-entry class table; it
//!   receives the world context and hit payloads (`docs/notes/actor-ai.md`
//!   section 2).
//! - The actor base update assembles a hit record (target, bone, radius mask)
//!   and queues it for the hit-record pass (`docs/notes/actor-ai.md` section 3).
//! - Stage 13 is a spatial query from a cylinder around each object
//!   (`docs/notes/actor-ai.md` section 1), which is the only attested
//!   contact/trigger source.
//! - The event/message dispatcher writes the state byte on two specific message
//!   ids (`docs/notes/actor-state-writer.md` section 3). The id space and the
//!   payload layout are UNKNOWN, so the code stays a raw integer here.

use crate::engine::live::LiveId;

/// One hit record, the payload an attacker assembles and the hit-record pass
/// resolves.
///
/// The four fields are exactly the ones the evidence names: the attacker that
/// assembled the record, the target, the bone and the radius mask. The integer
/// widths are this engine's representation; the notes do not pin a width, and
/// no width implies a meaning. This type carries data only: it decides no
/// damage and no state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HitRecord {
    /// The actor that assembled the record.
    pub attacker: LiveId,
    /// The actor the record is resolved against.
    pub target: LiveId,
    /// Bone id, as carried by the record. No bone table is pinned.
    pub bone: u16,
    /// Radius mask, as carried by the record. No mask bit is interpreted.
    pub radius_mask: u16,
}

/// One contact/trigger result.
///
/// Stage 13 queries a cylinder around each object. The cylinder dimensions and
/// the trigger record source are UNKNOWN (`docs/notes/actor-ai.md` section 1,
/// section 10), so the carrier keeps only the two live ids and invents no
/// extent, layer or filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContactRecord {
    /// The live object the query ran for.
    pub source: LiveId,
    /// The live object the query reported.
    pub target: LiveId,
}

/// One event/message addressed to a live object.
///
/// The evidence records an event/message dispatcher that writes the state byte
/// on two specific message ids (`docs/notes/actor-state-writer.md` section 3).
/// The id space, the payload and the meaning of any id are UNKNOWN, so `code`
/// stays a raw integer and this type gives it no interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Message {
    /// The live object the message is delivered to.
    pub target: LiveId,
    /// Raw message code. No code is named or interpreted here.
    pub code: u16,
}

/// The event categories the evidence supports.
///
/// The list is closed to the three attested families: a hit record, a
/// contact/trigger result and an event/message. No gameplay event (damage,
/// death, pickup, script command) is invented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineEvent {
    /// An attacker-assembled hit record.
    Hit(HitRecord),
    /// A contact/trigger query result.
    Contact(ContactRecord),
    /// An event/message addressed to an object.
    Message(Message),
}

impl EngineEvent {
    /// The live object the event is addressed to.
    ///
    /// Every category has exactly one delivery target: the hit victim, the
    /// queried object or the message recipient. The attacker/source is not the
    /// target of delivery.
    pub const fn target(self) -> LiveId {
        match self {
            Self::Hit(record) => record.target,
            Self::Contact(record) => record.target,
            Self::Message(message) => message.target,
        }
    }
}

/// Deterministic FIFO event queue.
///
/// One queue, one order: push order. There is no producer priority, no sort and
/// no hash iteration. Two producers that push in the same interleaving drain the
/// same sequence, so ordering is independent of *who* pushed and depends only on
/// *when* a push happened relative to the others.
///
/// The queue is drained once per tick, at the documented delivery stage, and the
/// drain clears it. An event therefore belongs to exactly one tick and can never
/// be delivered by the next one. The queue is transient: it is deliberately not
/// part of the canonical determinism fold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventQueue {
    pending: Vec<EngineEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one event at the tail. Push order is the only ordering rule.
    pub fn push(&mut self, event: EngineEvent) {
        self.pending.push(event);
    }

    /// The events that have not been drained, in FIFO order.
    pub fn pending(&self) -> &[EngineEvent] {
        &self.pending
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Remove and return every pending event in FIFO order.
    ///
    /// The queue is empty afterwards. A second drain on the same queue returns
    /// nothing, which is what makes cross-tick leakage impossible.
    pub fn drain(&mut self) -> Vec<EngineEvent> {
        std::mem::take(&mut self.pending)
    }

    /// Drop every pending event without delivering it.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{ContactRecord, EngineEvent, EventQueue, HitRecord, Message};
    use crate::engine::live::LiveId;

    fn id(index: u32) -> LiveId {
        LiveId {
            index,
            generation: 0,
        }
    }

    fn hit(attacker: u32, target: u32, bone: u16) -> EngineEvent {
        EngineEvent::Hit(HitRecord {
            attacker: id(attacker),
            target: id(target),
            bone,
            radius_mask: 0x0001,
        })
    }

    fn message(target: u32, code: u16) -> EngineEvent {
        EngineEvent::Message(Message {
            target: id(target),
            code,
        })
    }

    fn contact(source: u32, target: u32) -> EngineEvent {
        EngineEvent::Contact(ContactRecord {
            source: id(source),
            target: id(target),
        })
    }

    #[test]
    fn fifo_order_is_push_order() {
        let mut queue = EventQueue::new();
        queue.push(hit(1, 2, 3));
        queue.push(message(2, 7));
        queue.push(contact(4, 2));

        assert_eq!(
            queue.drain(),
            vec![hit(1, 2, 3), message(2, 7), contact(4, 2)]
        );
    }

    #[test]
    fn replaying_the_same_pushes_drains_the_same_sequence() {
        fn run() -> Vec<EngineEvent> {
            let mut queue = EventQueue::new();
            queue.push(hit(1, 2, 3));
            queue.push(hit(1, 2, 4));
            queue.push(message(2, 9));
            queue.drain()
        }

        let first = run();
        let second = run();
        assert_eq!(first, second, "the queue must replay byte for byte");
        assert_eq!(first.len(), 3);
    }

    #[test]
    fn a_second_drain_is_empty_and_cannot_leak() {
        let mut queue = EventQueue::new();
        queue.push(hit(1, 2, 3));
        assert_eq!(queue.drain().len(), 1);
        assert_eq!(queue.drain(), Vec::new(), "the drained tick owns its event");
        assert!(queue.is_empty());
    }

    #[test]
    fn ordering_is_independent_of_the_producer() {
        // Two producers push into one queue. The same interleaving must drain
        // identically; the queue never sorts by producer, class or target.
        fn interleaved(split: bool) -> Vec<EngineEvent> {
            let mut queue = EventQueue::new();
            let first = [hit(10, 20, 1), hit(10, 20, 2), hit(10, 20, 3)];
            let second = [message(30, 1), message(30, 2), message(30, 3)];
            if split {
                for (a, b) in first.iter().zip(second.iter()) {
                    queue.push(*a);
                    queue.push(*b);
                }
            } else {
                for event in first {
                    queue.push(event);
                }
                for event in second {
                    queue.push(event);
                }
            }
            queue.drain()
        }

        let one_producer_at_a_time = interleaved(false);
        assert_eq!(
            one_producer_at_a_time,
            vec![
                hit(10, 20, 1),
                hit(10, 20, 2),
                hit(10, 20, 3),
                message(30, 1),
                message(30, 2),
                message(30, 3),
            ]
        );

        let interleaved_push = interleaved(true);
        assert_eq!(
            interleaved_push,
            vec![
                hit(10, 20, 1),
                message(30, 1),
                hit(10, 20, 2),
                message(30, 2),
                hit(10, 20, 3),
                message(30, 3),
            ],
            "push order is the whole contract; source never reorders"
        );
    }

    #[test]
    fn every_category_reports_its_delivery_target() {
        assert_eq!(hit(1, 2, 0).target(), id(2));
        assert_eq!(contact(3, 4).target(), id(4));
        assert_eq!(message(5, 6).target(), id(5));
    }
}

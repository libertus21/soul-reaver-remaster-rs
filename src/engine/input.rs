//! Tick-boundary input sampling and edge detection.
//!
//! The platform pushes [`InputEvent`]s between ticks; the simulation samples
//! the queue **exactly once per tick boundary**. Sampling drains the queue in
//! arrival order and keeps a held-button level plus the pressed/released edges
//! observed at that boundary.
//!
//! This module never touches a clock. Input is data the clock consumes; it must
//! never drive simulation stepping, so a burst of 1 or 100 events inside the
//! same wall-time window produces the same number of ticks. The frame pipeline
//! (`engine/frame.rs`) samples at the start of every fixed tick.

/// Digital controls this milestone reads.
///
/// The discriminants are stable bit positions and must never be renumbered:
/// they are the bits of [`InputFrame::held`], `pressed` and `released`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InputButton {
    Forward = 0,
    Back = 1,
    Left = 2,
    Right = 3,
    Action = 4,
    Jump = 5,
}

impl InputButton {
    /// Number of buttons in [`InputButton::ALL`].
    pub const COUNT: usize = 6;

    /// Every button in stable code order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Forward,
        Self::Back,
        Self::Left,
        Self::Right,
        Self::Action,
        Self::Jump,
    ];

    /// Stable bit for this button.
    pub const fn bit(self) -> u16 {
        1 << (self as u8)
    }

    /// Stable ordinal used by tests and diagnostics. Never renumber.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// One requested button transition.
///
/// The queue does not coalesce at push time: duplicate presses are legal input
/// and are resolved at the tick boundary, where a press on an already-held
/// button produces no second edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub button: InputButton,
    pub pressed: bool,
}

impl InputEvent {
    pub const fn press(button: InputButton) -> Self {
        Self {
            button,
            pressed: true,
        }
    }

    pub const fn release(button: InputButton) -> Self {
        Self {
            button,
            pressed: false,
        }
    }
}

/// One tick's sampled input: held level plus the edges observed at that
/// boundary.
///
/// `held`, `pressed` and `released` are bit sets over [`InputButton::bit`].
/// A button may appear in both edge sets in the same sample when a press and a
/// release were queued inside one tick; the held level then reflects the final
/// event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputFrame {
    held: u16,
    pressed: u16,
    released: u16,
}

impl InputFrame {
    /// No buttons held and no edges.
    pub const NONE: Self = Self {
        held: 0,
        pressed: 0,
        released: 0,
    };

    pub const fn held(self) -> u16 {
        self.held
    }

    pub const fn pressed(self) -> u16 {
        self.pressed
    }

    pub const fn released(self) -> u16 {
        self.released
    }

    pub fn is_held(self, button: InputButton) -> bool {
        self.held & button.bit() != 0
    }

    pub fn is_pressed(self, button: InputButton) -> bool {
        self.pressed & button.bit() != 0
    }

    pub fn is_released(self, button: InputButton) -> bool {
        self.released & button.bit() != 0
    }

    /// Direct translation axes for this tick: `x` is right minus left and `y` is
    /// forward minus back, each in `-1..=1`.
    ///
    /// This is a synthetic, camera-free convention for the milestone: in the
    /// Z-up world (see [`crate::engine::axis`]) forward is `+Y` and right is
    /// `+X`, so stage 9 moves the player on the horizontal `X`/`Y` plane. The
    /// original applies a camera-relative delta in stage 9; camera orientation
    /// arrives with the renderer slice.
    pub fn move_axis(self) -> [i32; 2] {
        let x = i32::from(self.is_held(InputButton::Right))
            - i32::from(self.is_held(InputButton::Left));
        let y = i32::from(self.is_held(InputButton::Forward))
            - i32::from(self.is_held(InputButton::Back));
        [x, y]
    }
}

/// Edge queue between ticks.
///
/// Events accumulate until [`InputQueue::sample`] drains them at a tick
/// boundary. The queue owns the held level, so sampling is the only place that
/// decides whether an event is an edge. It has no clock and no way to ask for a
/// tick: the caller decides when to sample.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputQueue {
    pending: Vec<InputEvent>,
    held: u16,
}

impl InputQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue one transition. Duplicates are allowed and resolved on sample.
    pub fn push(&mut self, event: InputEvent) {
        self.pending.push(event);
    }

    pub fn press(&mut self, button: InputButton) {
        self.push(InputEvent::press(button));
    }

    pub fn release(&mut self, button: InputButton) {
        self.push(InputEvent::release(button));
    }

    /// Events waiting for the next tick boundary.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Buttons currently held according to the last sample plus events applied
    /// by later samples. Before the first sample this is the empty set.
    pub fn held_bits(&self) -> u16 {
        self.held
    }

    /// Drain the queue in arrival order and produce the frame for this tick.
    ///
    /// A press on an already-held button is not an edge; a release on a button
    /// that is not held is not an edge either. The held level always reflects
    /// the last event for that button. This makes a burst of duplicate events
    /// indistinguishable from a single event.
    pub fn sample(&mut self) -> InputFrame {
        let mut pressed = 0u16;
        let mut released = 0u16;
        for event in self.pending.drain(..) {
            let bit = event.button.bit();
            if event.pressed {
                if self.held & bit == 0 {
                    pressed |= bit;
                    self.held |= bit;
                }
            } else if self.held & bit != 0 {
                released |= bit;
                self.held &= !bit;
            }
        }
        InputFrame {
            held: self.held,
            pressed,
            released,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InputButton, InputEvent, InputFrame, InputQueue};

    #[test]
    fn button_codes_and_bits_are_stable() {
        for (code, button) in InputButton::ALL.iter().enumerate() {
            assert_eq!(button.code() as usize, code);
            assert_eq!(button.bit(), 1 << code);
        }
        assert_eq!(InputButton::COUNT, 6);
    }

    #[test]
    fn empty_queue_samples_to_none_and_keeps_held_level() {
        let mut queue = InputQueue::new();
        assert_eq!(queue.sample(), InputFrame::NONE);

        queue.press(InputButton::Forward);
        let first = queue.sample();
        assert!(first.is_held(InputButton::Forward));
        assert!(first.is_pressed(InputButton::Forward));

        let second = queue.sample();
        assert!(second.is_held(InputButton::Forward));
        assert!(!second.is_pressed(InputButton::Forward));
        assert_eq!(second.pressed(), 0);
        assert_eq!(second.released(), 0);
    }

    #[test]
    fn duplicate_presses_collapse_to_one_edge() {
        let mut once = InputQueue::new();
        once.press(InputButton::Action);
        let single = once.sample();

        let mut burst = InputQueue::new();
        for _ in 0..100 {
            burst.press(InputButton::Action);
        }
        let many = burst.sample();

        assert_eq!(single, many, "a duplicate burst is one tick-boundary edge");
        assert!(many.is_pressed(InputButton::Action));
        assert_eq!(many.pressed().count_ones(), 1);
    }

    #[test]
    fn duplicate_releases_collapse_to_one_edge() {
        let mut queue = InputQueue::new();
        queue.press(InputButton::Jump);
        let _ = queue.sample();

        for _ in 0..100 {
            queue.release(InputButton::Jump);
        }
        let frame = queue.sample();
        assert!(frame.is_released(InputButton::Jump));
        assert!(!frame.is_held(InputButton::Jump));
        assert_eq!(frame.released().count_ones(), 1);
    }

    #[test]
    fn a_release_without_a_press_is_not_an_edge() {
        let mut queue = InputQueue::new();
        queue.release(InputButton::Forward);
        let frame = queue.sample();
        assert!(!frame.is_released(InputButton::Forward));
        assert_eq!(frame, InputFrame::NONE);
    }

    #[test]
    fn press_and_release_inside_one_tick_show_both_edges() {
        let mut queue = InputQueue::new();
        queue.push(InputEvent::press(InputButton::Right));
        queue.push(InputEvent::release(InputButton::Right));
        let frame = queue.sample();
        assert!(frame.is_pressed(InputButton::Right));
        assert!(frame.is_released(InputButton::Right));
        assert!(!frame.is_held(InputButton::Right));
    }

    #[test]
    fn sampling_drains_the_queue() {
        let mut queue = InputQueue::new();
        queue.press(InputButton::Forward);
        queue.press(InputButton::Left);
        assert_eq!(queue.pending_len(), 2);
        let _ = queue.sample();
        assert!(queue.is_empty());
        assert_eq!(queue.pending_len(), 0);
        assert_eq!(queue.held_bits(), InputButton::Forward.bit() | InputButton::Left.bit());
    }

    #[test]
    fn move_axis_maps_held_buttons_to_direct_translation() {
        let mut queue = InputQueue::new();
        queue.press(InputButton::Forward);
        let frame = queue.sample();
        assert_eq!(frame.move_axis(), [0, 1]);

        queue.release(InputButton::Forward);
        queue.press(InputButton::Back);
        queue.press(InputButton::Right);
        let frame = queue.sample();
        assert_eq!(frame.move_axis(), [1, -1]);

        queue.release(InputButton::Back);
        queue.release(InputButton::Right);
        queue.press(InputButton::Left);
        let frame = queue.sample();
        assert_eq!(frame.move_axis(), [-1, 0]);
    }

    #[test]
    fn opposite_axes_cancel() {
        let mut queue = InputQueue::new();
        queue.press(InputButton::Forward);
        queue.press(InputButton::Back);
        queue.press(InputButton::Left);
        queue.press(InputButton::Right);
        let frame = queue.sample();
        assert_eq!(frame.move_axis(), [0, 0]);
    }

    #[test]
    fn sampled_frames_are_deterministic_across_replays() {
        fn replay() -> Vec<InputFrame> {
            let mut queue = InputQueue::new();
            let mut frames = Vec::new();
            for tick in 0..8 {
                match tick {
                    0 => queue.press(InputButton::Forward),
                    3 => queue.release(InputButton::Forward),
                    4 => {
                        queue.press(InputButton::Right);
                        queue.press(InputButton::Right);
                    }
                    6 => queue.release(InputButton::Right),
                    _ => {}
                }
                frames.push(queue.sample());
            }
            frames
        }

        assert_eq!(replay(), replay());
    }

    #[test]
    fn one_and_one_hundred_events_sample_identically() {
        fn burst(count: usize) -> InputFrame {
            let mut queue = InputQueue::new();
            for _ in 0..count {
                queue.press(InputButton::Forward);
            }
            queue.sample()
        }

        assert_eq!(burst(1), burst(100));
    }
}

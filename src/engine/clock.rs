//! Fixed 60 Hz simulation clock with a rational per-object step.
//!
//! The original advances on a fixed rational tick (`num/den`, never `f32`) so
//! the same inputs always produce the same state. [`SimClock`] turns elapsed
//! wall time into integer-indexed [`ObjectTick`]s, runs at most
//! [`MAX_CATCH_UP`] steps per frame and keeps the unconsumed remainder, so no
//! time is silently dropped and no drift accumulates.

use std::time::Duration;

/// Fixed simulation rate, in ticks per second.
pub const TICK_HZ: u32 = 60;

/// Maximum simulation steps executed for one [`SimClock::advance`] call.
///
/// A large stall (debugger, disk, window drag) must not turn into an unbounded
/// catch-up loop. The rest of the elapsed time stays in the accumulator and is
/// paid on later frames.
pub const MAX_CATCH_UP: u32 = 5;

/// One fixed simulation step, expressed as an exact rational `num/den` seconds.
///
/// `index` is a monotonic, integer tick counter: two runs that execute the same
/// number of ticks produce the same indices. The step is `1/60 s`, never a
/// rounded `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectTick {
    pub index: u64,
    pub num: u32,
    pub den: u32,
}

impl ObjectTick {
    /// Build the fixed `1/60 s` step at `index`.
    pub const fn step(index: u64) -> Self {
        Self {
            index,
            num: 1,
            den: TICK_HZ,
        }
    }
}

/// The ticks produced by one [`SimClock::advance`] call, in index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickBatch {
    ticks: Vec<ObjectTick>,
}

impl TickBatch {
    pub fn ticks(&self) -> &[ObjectTick] {
        &self.ticks
    }

    pub fn len(&self) -> usize {
        self.ticks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ticks.is_empty()
    }
}

/// Accumulated time is held in thirds of a nanosecond so the `1/60 s` step is
/// an exact integer (`50_000_000` units) and no floating point ever touches
/// time. `1 ns = 3` units; `1 tick = 1_000_000_000 * 3 / 60` units.
const SUB_UNITS_PER_NANOSECOND: u128 = 3;
const SUB_UNITS_PER_TICK: u128 = 1_000_000_000 * SUB_UNITS_PER_NANOSECOND / TICK_HZ as u128;

/// Fixed-step accumulator. The caller feeds wall time; the clock never reads a
/// system clock itself.
#[derive(Debug, Clone, Default)]
pub struct SimClock {
    accumulated: u128,
    next_index: u64,
}

impl SimClock {
    pub const fn new() -> Self {
        Self {
            accumulated: 0,
            next_index: 0,
        }
    }

    /// Convert `delta` into at most [`MAX_CATCH_UP`] ticks and keep the rest.
    ///
    /// All elapsed time is accounted for: the returned batch consumes whole
    /// steps, and the unconsumed remainder stays in the accumulator for the next
    /// call. A capped advance therefore delays ticks but never loses them.
    pub fn advance(&mut self, delta: Duration) -> TickBatch {
        self.accumulated += delta.as_nanos() * SUB_UNITS_PER_NANOSECOND;
        let due = self.accumulated / SUB_UNITS_PER_TICK;
        let steps = due.min(u128::from(MAX_CATCH_UP)) as u32;
        self.accumulated -= SUB_UNITS_PER_TICK * u128::from(steps);

        let mut ticks = Vec::with_capacity(steps as usize);
        for _ in 0..steps {
            ticks.push(ObjectTick::step(self.next_index));
            self.next_index += 1;
        }
        TickBatch { ticks }
    }

    /// Time accumulated but not yet consumed, floored to whole nanoseconds.
    pub fn remainder(&self) -> Duration {
        Duration::from_nanos((self.accumulated / SUB_UNITS_PER_NANOSECOND) as u64)
    }

    /// Index the next emitted tick will carry.
    pub fn next_index(&self) -> u64 {
        self.next_index
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjectTick, SimClock, TickBatch, MAX_CATCH_UP, TICK_HZ};
    use std::time::Duration;

    #[test]
    fn step_is_the_rational_one_sixtieth() {
        let tick = ObjectTick::step(7);
        assert_eq!(tick.index, 7);
        assert_eq!((tick.num, tick.den), (1, TICK_HZ));
        assert_eq!(TICK_HZ, 60);
        assert_eq!(MAX_CATCH_UP, 5);
    }

    #[test]
    fn ten_second_advance_is_capped_and_keeps_the_remainder() {
        let mut clock = SimClock::new();
        let batch: TickBatch = clock.advance(Duration::from_secs(10));
        assert_eq!(batch.len(), MAX_CATCH_UP as usize);
        let indices: Vec<u64> = batch.ticks().iter().map(|tick| tick.index).collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
        // 10 s - 5 * (1/60 s) = 9_916_666_666.6 ns, floored.
        assert_eq!(clock.remainder(), Duration::from_nanos(9_916_666_666));
    }

    #[test]
    fn capped_remainder_is_paid_back_without_loss() {
        let mut clock = SimClock::new();
        let mut total = clock.advance(Duration::from_secs(10)).len();
        loop {
            let batch = clock.advance(Duration::ZERO);
            if batch.is_empty() {
                break;
            }
            total += batch.len();
        }
        // 10 s is exactly 600 ticks; the cap delays them, it never drops them.
        assert_eq!(total, 600);
        assert_eq!(clock.remainder(), Duration::ZERO);
        assert_eq!(clock.next_index(), 600);
    }

    #[test]
    fn indices_are_integer_and_monotonic() {
        let mut clock = SimClock::new();
        let mut seen = Vec::new();
        for _ in 0..10 {
            for tick in clock.advance(Duration::from_millis(16)).ticks() {
                seen.push(tick.index);
            }
        }
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(seen.first().copied(), Some(0));
    }

    #[test]
    fn no_tick_before_a_full_step() {
        let mut clock = SimClock::new();
        // 1/60 s = 16_666_666.6 ns; one nanosecond less is not a whole step.
        assert!(clock.advance(Duration::from_nanos(16_666_666)).is_empty());
        let batch = clock.advance(Duration::from_nanos(1));
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.ticks()[0].index, 0);
    }
}

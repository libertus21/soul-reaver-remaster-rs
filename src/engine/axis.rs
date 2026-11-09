//! World axis convention for the shipped room and object data.
//!
//! Phase-1 evidence (`docs/notes/up-axis.md`) establishes that the world is
//! **Z-up**: the vertical component is index 2 with **+Z up**, and the
//! horizontal plane is world X (index 0) / world Y (index 1). The engine before
//! this module treated index 1 (Y) as up, which inverted gravity and tipped the
//! player camera 90 degrees.
//!
//! Every vertical decision - gravity, ground and ceiling height, floor/wall/
//! ceiling classification, the player camera and the movement axis - must read
//! these constants instead of hard-coding an index. That is the single source
//! of truth that keeps the convention from drifting again.
//!
//! Positions are integer world units stored in the authored order `[x, y, z]`.
//! Nothing here rotates or rewrites the level data; it only names which stored
//! component is vertical.

/// Index of the vertical (up) component in a world position `[x, y, z]`.
pub const UP: usize = 2;

/// Index of the first horizontal component (world X).
pub const HORIZONTAL_A: usize = 0;

/// Index of the second horizontal component (world Y).
pub const HORIZONTAL_B: usize = 1;

/// Sign of "up" along [`UP`]. The evidence fixes `+Z` as up.
pub const UP_SIGN: i32 = 1;

/// Index of the up component in an **object-model local** position.
///
/// Phase-1 corpus evidence (`docs/notes/object-up-axis.md`) establishes that the
/// archived object containers and the loose `.SRM` meshes store their vertical
/// on index 2, with `+Z` up: a standing humanoid is tallest along index 2 and
/// its bone chain extends along index 2. This matches the world vertical, so a
/// placed model needs **no rotation** to stand upright; placement stays a pure
/// translation. Object viewers derive their camera basis from this constant.
pub const OBJECT_UP: usize = 2;

/// Sign of object-local up along [`OBJECT_UP`]. The evidence fixes `+Z` as up.
pub const OBJECT_UP_SIGN: i32 = 1;

/// World up (vertical) component of a position.
pub const fn up_of(position: [i32; 3]) -> i32 {
    position[UP]
}

/// The two horizontal components of a world position, in `[A, B]` order.
pub const fn horizontal_of(position: [i32; 3]) -> [i32; 2] {
    [position[HORIZONTAL_A], position[HORIZONTAL_B]]
}

/// Compose a world position from a horizontal point and an up value.
pub const fn from_horizontal(horizontal: [i32; 2], up: i32) -> [i32; 3] {
    let mut position = [0; 3];
    position[HORIZONTAL_A] = horizontal[0];
    position[HORIZONTAL_B] = horizontal[1];
    position[UP] = up;
    position
}

/// Replace the up component of `position`, keeping the horizontal point.
pub const fn with_up(position: [i32; 3], up: i32) -> [i32; 3] {
    let mut next = position;
    next[UP] = up;
    next
}

/// The three world axes, named by the role they play in this engine.
///
/// This enum exists only for documentation and diagnostics; the compile-time
/// constants above are what the hot paths read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldAxis {
    /// World X, a horizontal axis.
    X,
    /// World Y, a horizontal axis.
    Y,
    /// World Z, the vertical axis. Up is `+Z`.
    Z,
}

impl WorldAxis {
    /// Index of this axis inside a `[i32; 3]` position.
    pub const fn index(self) -> usize {
        match self {
            Self::X => HORIZONTAL_A,
            Self::Y => HORIZONTAL_B,
            Self::Z => UP,
        }
    }

    /// True for the vertical axis.
    pub const fn is_up(self) -> bool {
        matches!(self, Self::Z)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HORIZONTAL_A, HORIZONTAL_B, OBJECT_UP, OBJECT_UP_SIGN, UP, UP_SIGN, WorldAxis,
        from_horizontal, horizontal_of, up_of, with_up,
    };

    #[test]
    fn the_world_vertical_axis_is_z_up() {
        assert_eq!(UP, 2, "phase-1 evidence: the vertical component is index 2");
        assert_eq!(HORIZONTAL_A, 0);
        assert_eq!(HORIZONTAL_B, 1);
        assert_eq!(UP_SIGN, 1, "+Z is up");
        assert!(WorldAxis::Z.is_up());
        assert!(!WorldAxis::X.is_up());
        assert!(!WorldAxis::Y.is_up());
        assert_eq!(WorldAxis::Z.index(), UP);
    }

    #[test]
    fn the_object_local_up_axis_matches_the_world_up_axis() {
        // Both checks are expressed against the shared constants so a future
        // axis change updates one place, never a scattered literal.
        assert_eq!(
            OBJECT_UP, UP,
            "object-local up is the same index as world up: placement needs no rotation"
        );
        assert_eq!(
            OBJECT_UP_SIGN, UP_SIGN,
            "object-local up points the same way as world up"
        );
    }

    #[test]
    fn up_and_horizontal_accessors_round_trip() {
        let position = [11, 22, 33];
        assert_eq!(up_of(position), 33);
        assert_eq!(horizontal_of(position), [11, 22]);
        assert_eq!(from_horizontal([11, 22], 33), position);
        assert_eq!(with_up(position, 44), [11, 22, 44]);
    }
}

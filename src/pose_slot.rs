//! Pure procedural-slot rotation composition (no I/O, no engine wiring).
//!
//! The animation trace (`docs/notes/animation-loader-trace.md`) says a slot's
//! runtime orientation is produced from descriptor mode bits, a per-slot
//! authored fixed-point aim vector, the object transform and a world target.
//! This module ships only the part the corpus and trace actually support:
//!
//! - **CONFIRMED:** a [`OrientationMode::Static`] slot, or a procedural slot
//!   with an all-zero aim triplet, has no authored aim to rotate, so its local
//!   term is the identity.
//! - **INFERRED:** a procedural slot is expressed in the object frame, so the
//!   object transform is the rotation base.
//! - **UNKNOWN:** the local aim-to-rotation term and the world-target term. Their
//!   exact formulas are not established (see
//!   `docs/notes/frame-mode-composition.md`), so the current implementation
//!   omits the local term rather than inventing one.
//!
//! The mode-bit source is also UNKNOWN: the file-form words `+4`, `+8`, `+12`
//! and `+16` do not partition slots into static versus procedural, so callers
//! must supply [`OrientationMode`] from authority outside the descriptor parser.

use crate::slot_descriptor::FixedPoint;

/// Row-major 3x3 rotation, matching `crate::pose::Mat3`.
pub type Rotation = [[f32; 3]; 3];

/// Which orientation path a slot follows.
///
/// The enum names the two outcomes the trace describes without claiming a
/// descriptor bit source; the file corpus does not resolve one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationMode {
    /// No procedural aim/face orientation; the local slot term is identity.
    Static,
    /// A procedural aim/face orientation exists; its exact algebra is UNKNOWN.
    Procedural,
}

/// The 3x3 identity rotation.
pub fn identity_rotation() -> Rotation {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

/// Compose a slot's rotation from its aim triplet, orientation mode and object
/// transform.
///
/// # Evidence (do not overclaim)
///
/// - **CONFIRMED:** [`OrientationMode::Static`], or a procedural slot with an
///   all-zero aim triplet, yields the identity. A slot with no aim data has no
///   authored aim vector, so there is nothing local to rotate.
/// - **INFERRED:** a procedural slot with a non-zero aim triplet returns the
///   object transform, used as the object-frame rotation base. The trace names
///   the object transform as a contributor.
/// - **UNKNOWN:** the local aim-to-rotation term and the world-target term. The
///   local term is omitted (treated as identity) instead of being fabricated;
///   the returned matrix is therefore an `INFERRED` lower bound, not the
///   runtime pose.
///
/// `aim` is the candidate Q12 triplet (for example from
/// `frame::leading_triplet`); only its zero-ness is interpreted here.
pub fn slot_rotation(
    aim: [FixedPoint; 3],
    mode: OrientationMode,
    object_transform: Rotation,
) -> Rotation {
    match mode {
        OrientationMode::Static => identity_rotation(),
        OrientationMode::Procedural => {
            if aim.iter().all(|component| component.raw() == 0) {
                identity_rotation()
            } else {
                object_transform
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUARTER_TURN: Rotation = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];

    fn aim() -> [FixedPoint; 3] {
        [
            FixedPoint::from_raw(10),
            FixedPoint::ZERO,
            FixedPoint::from_raw(-7),
        ]
    }

    #[test]
    fn static_rotation_is_the_identity() {
        assert_eq!(
            slot_rotation(aim(), OrientationMode::Static, QUARTER_TURN),
            identity_rotation()
        );
    }

    #[test]
    fn procedural_with_zero_aim_is_the_identity() {
        assert_eq!(
            slot_rotation([FixedPoint::ZERO; 3], OrientationMode::Procedural, QUARTER_TURN),
            identity_rotation()
        );
    }

    #[test]
    fn procedural_nonzero_aim_returns_the_object_transform_as_the_inferred_base() {
        assert_eq!(
            slot_rotation(aim(), OrientationMode::Procedural, QUARTER_TURN),
            QUARTER_TURN
        );
    }
}

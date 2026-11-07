//! Engine core: a headless simulation layer built on the read-only asset
//! layer. This slice wires the `--play` option facade, the fixed clock and the
//! game-state machine; the world model, tracker, hooks and renderer arrive in
//! later slices.

use std::fmt;

use crate::engine::state::{GameState, StateEvent};

#[allow(dead_code)]
pub mod actor;
#[allow(dead_code)]
pub mod assets;
#[allow(dead_code)]
pub mod backend_wgpu;
#[allow(dead_code)]
pub mod class;
#[allow(dead_code)]
pub mod clock;
#[allow(dead_code)]
pub mod determinism;
#[allow(dead_code)]
pub mod events;
#[allow(dead_code)]
pub mod frame;
#[allow(dead_code)]
pub mod input;
#[allow(dead_code)]
pub mod live;
#[allow(dead_code)]
pub mod placed;
pub mod play;
#[allow(dead_code)]
pub mod render_backend;
#[allow(dead_code)]
pub mod scene;
#[allow(dead_code)]
pub mod spawn;
#[allow(dead_code)]
pub mod state;
#[allow(dead_code)]
pub mod tracker;
#[allow(dead_code)]
pub mod world;

/// Typed engine failures.
///
/// `UnsupportedState` is the state machine's only failure: an illegal or
/// not-yet-implemented transition is data, not a panic.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    UnsupportedState { from: GameState, event: StateEvent },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedState { from, event } => {
                write!(f, "unsupported state transition from {from:?} on {event:?}")
            }
        }
    }
}

impl std::error::Error for EngineError {}

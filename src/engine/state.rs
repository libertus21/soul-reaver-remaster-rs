//! Game-state machine.
//!
//! One explicit transition function owns every legal move. Unsupported moves
//! return [`EngineError::UnsupportedState`], never a panic: the milestone path
//! is `Boot -> Loading -> InGame -> Shutdown`, and the intro/menu states are
//! real variants that deliberately report "not supported yet".

use crate::engine::EngineError;

/// High-level engine state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameState {
    Boot,
    Intro,
    KainIntro,
    Menu,
    Logos,
    Loading { unit: String },
    InGame,
    Shutdown,
}

/// An intent that asks the machine to move.
///
/// `LoadsComplete` is the caller's assertion that the asset load finished; the
/// machine cannot leave `Loading` without it. `BeginIntro`, `BeginKainIntro`,
/// `BeginMenu` and `BeginLogos` exist so the variants are addressable, but they
/// are unsupported in this milestone and return a typed error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateEvent {
    BeginLoading { unit: String },
    LoadsComplete,
    BeginIntro,
    BeginKainIntro,
    BeginMenu,
    BeginLogos,
    Shutdown,
}

/// The single transition function.
///
/// `Shutdown` is accepted from any state so the process can always exit
/// cleanly. Every other move is either listed here or rejected with
/// [`EngineError::UnsupportedState`].
pub fn transition(current: GameState, event: StateEvent) -> Result<GameState, EngineError> {
    use GameState::{Boot, InGame, Loading, Shutdown};
    match (current, event) {
        (Boot, StateEvent::BeginLoading { unit }) => Ok(Loading { unit }),
        (Loading { .. }, StateEvent::LoadsComplete) => Ok(InGame),
        (_, StateEvent::Shutdown) => Ok(Shutdown),
        (from, event) => Err(EngineError::UnsupportedState { from, event }),
    }
}

#[cfg(test)]
mod tests {
    use super::{transition, GameState, StateEvent};
    use crate::engine::EngineError;

    fn loading(unit: &str) -> GameState {
        GameState::Loading {
            unit: unit.to_string(),
        }
    }

    #[test]
    fn milestone_path_reaches_in_game_only_after_loads() {
        let state = transition(
            GameState::Boot,
            StateEvent::BeginLoading {
                unit: "under1".to_string(),
            },
        )
        .expect("Boot -> Loading is legal");
        assert_eq!(state, loading("under1"));

        let state = transition(state, StateEvent::LoadsComplete).expect("loads done -> InGame");
        assert_eq!(state, GameState::InGame);

        let state = transition(state, StateEvent::Shutdown).expect("InGame -> Shutdown");
        assert_eq!(state, GameState::Shutdown);
    }

    #[test]
    fn illegal_jumps_are_typed_errors() {
        let jump = transition(GameState::Boot, StateEvent::LoadsComplete);
        assert_eq!(
            jump,
            Err(EngineError::UnsupportedState {
                from: GameState::Boot,
                event: StateEvent::LoadsComplete,
            })
        );

        let backward = transition(
            GameState::InGame,
            StateEvent::BeginLoading {
                unit: "under2".to_string(),
            },
        );
        assert!(matches!(
            backward,
            Err(EngineError::UnsupportedState {
                from: GameState::InGame,
                ..
            })
        ));
    }

    #[test]
    fn intro_and_menu_states_are_unsupported_not_panics() {
        for event in [
            StateEvent::BeginIntro,
            StateEvent::BeginKainIntro,
            StateEvent::BeginMenu,
            StateEvent::BeginLogos,
        ] {
            assert!(
                matches!(
                    transition(GameState::Boot, event.clone()),
                    Err(EngineError::UnsupportedState { .. })
                ),
                "event {event:?} must be a typed error"
            );
        }

        // The states exist but have no legal outgoing edge yet.
        for state in [
            GameState::Intro,
            GameState::KainIntro,
            GameState::Menu,
            GameState::Logos,
        ] {
            assert!(matches!(
                transition(state, StateEvent::LoadsComplete),
                Err(EngineError::UnsupportedState { .. })
            ));
        }
    }

    #[test]
    fn shutdown_is_accepted_from_any_state() {
        for state in [
            GameState::Boot,
            loading("under1"),
            GameState::InGame,
            GameState::Menu,
        ] {
            assert_eq!(
                transition(state, StateEvent::Shutdown),
                Ok(GameState::Shutdown)
            );
        }
    }
}

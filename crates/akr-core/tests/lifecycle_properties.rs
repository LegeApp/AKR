//! Exit criterion 3, second half: lifecycle reachability.
//!
//! Every terminal state is reachable from every initial state, and no state is
//! unreachable. A lifecycle with an unreachable state is a modelling error that would
//! otherwise only surface when somebody tried to author it.

use akr_core::model::{Class, State};
use std::collections::BTreeSet;

fn reachable_from(class: Class, start: State) -> BTreeSet<State> {
    let mut seen: BTreeSet<State> = [start].into();
    let mut frontier = vec![start];
    while let Some(current) = frontier.pop() {
        for transition in class.transitions().iter().filter(|t| t.from == current) {
            if seen.insert(transition.to) {
                frontier.push(transition.to);
            }
        }
    }
    seen
}

#[test]
fn every_terminal_state_is_reachable_from_some_initial_state() {
    for class in Class::ALL {
        let mut reachable: BTreeSet<State> = BTreeSet::new();
        for initial in class.initial() {
            reachable.extend(reachable_from(*class, *initial));
        }
        for terminal in class.terminal() {
            assert!(
                reachable.contains(terminal),
                "{class}: {terminal} is unreachable"
            );
        }
    }
}

/// The stronger property — every terminal reachable from *every* initial — is false, and
/// correctly so. The normative class has two initial states, and `rejected` is reachable
/// only from `proposed`: a record already in force is withdrawn or superseded, never
/// "considered and declined". This test pins that asymmetry so it cannot be lost by
/// accident.
#[test]
fn rejected_is_reachable_only_from_proposed() {
    let from_proposed = reachable_from(Class::Normative, State::Proposed);
    let from_active = reachable_from(Class::Normative, State::Active);
    assert!(from_proposed.contains(&State::Rejected));
    assert!(!from_active.contains(&State::Rejected));
    // Every initial state must still be able to reach *some* terminal state: a live
    // state you can never leave would be a modelling error.
    for class in Class::ALL {
        for initial in class.initial() {
            let reachable = reachable_from(*class, *initial);
            assert!(
                class.terminal().iter().any(|t| reachable.contains(t)),
                "{class}: {initial} can never terminate"
            );
        }
    }
}

#[test]
fn no_state_is_unreachable() {
    for class in Class::ALL {
        let mut reachable: BTreeSet<State> = BTreeSet::new();
        for initial in class.initial() {
            reachable.extend(reachable_from(*class, *initial));
        }
        for state in class.states() {
            assert!(
                reachable.contains(state),
                "{class}: {state} is unreachable from any initial"
            );
        }
    }
}

#[test]
fn live_and_terminal_partition_the_state_set() {
    for class in Class::ALL {
        let states: BTreeSet<State> = class.states().iter().copied().collect();
        let live: BTreeSet<State> = class.live().iter().copied().collect();
        let terminal: BTreeSet<State> = class.terminal().iter().copied().collect();

        assert!(
            live.is_disjoint(&terminal),
            "{class}: a state is both live and terminal"
        );
        assert_eq!(
            live.union(&terminal).copied().collect::<BTreeSet<_>>(),
            states,
            "{class}: live and terminal must partition the states"
        );
    }
}

#[test]
fn initial_states_are_live_and_transitions_stay_in_the_class() {
    for class in Class::ALL {
        let states: BTreeSet<State> = class.states().iter().copied().collect();
        for initial in class.initial() {
            assert!(
                initial.is_live_in(*class),
                "{class}: initial state {initial} is not live"
            );
        }
        for transition in class.transitions() {
            assert!(
                states.contains(&transition.from),
                "{class}: unknown from-state"
            );
            assert!(states.contains(&transition.to), "{class}: unknown to-state");
            assert!(
                transition.from.is_live_in(*class),
                "{class}: {} leaves a terminal state",
                transition.trigger
            );
        }
    }
}

#[test]
fn no_transition_leads_out_of_a_terminal_state() {
    for class in Class::ALL {
        for terminal in class.terminal() {
            assert!(
                !class.transitions().iter().any(|t| t.from == *terminal),
                "{class}: {terminal} is not terminal after all"
            );
        }
    }
}

//! Lifecycle states and the four class state machines (D-002, D-003).

use super::kind::Class;
use std::fmt;

/// Every lifecycle state across the four classes.
///
/// `superseded` and `withdrawn` are shared between classes; which states are legal for a
/// record is decided by its kind's class ([`Class::states`]).
///
/// `needs-review` is deliberately absent: staleness is derived at build time, never
/// authored (D-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    /// Normative or planning: written, not yet binding.
    Proposed,
    /// Normative: in force.
    Active,
    /// Normative: considered and declined.
    Rejected,
    /// Any class: replaced by a later revision.
    Superseded,
    /// Normative or empirical: retracted, with no replacement.
    Withdrawn,
    /// Empirical: somebody looked.
    Verified,
    /// Empirical: somebody checked, and it is not true.
    Disproven,
    /// Planning: scheduled, not started. (Planning also uses `Active`.)
    Ready,
    /// Planning: stalled behind a blocker.
    Blocked,
    /// Planning: every acceptance check satisfied.
    Completed,
    /// Planning: not happening.
    Abandoned,
    /// Inquiry: unanswered and current.
    Open,
    /// Inquiry: unanswered, and not now.
    Deferred,
    /// Inquiry: answered, with the answer recorded.
    Resolved,
    /// Inquiry: it stopped mattering.
    ClosedWithoutResolution,
}

impl State {
    /// Every state.
    pub const ALL: &'static [State] = &[
        State::Proposed,
        State::Active,
        State::Rejected,
        State::Superseded,
        State::Withdrawn,
        State::Verified,
        State::Disproven,
        State::Ready,
        State::Blocked,
        State::Completed,
        State::Abandoned,
        State::Open,
        State::Deferred,
        State::Resolved,
        State::ClosedWithoutResolution,
    ];

    /// The state name as written in source.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Active => "active",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
            Self::Withdrawn => "withdrawn",
            Self::Verified => "verified",
            Self::Disproven => "disproven",
            Self::Ready => "ready",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
            Self::Open => "open",
            Self::Deferred => "deferred",
            Self::Resolved => "resolved",
            Self::ClosedWithoutResolution => "closed-without-resolution",
        }
    }

    /// Looks up a state by name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|s| s.name() == name)
    }

    /// Whether the state is live for the given class.
    #[must_use]
    pub fn is_live_in(self, class: Class) -> bool {
        class.live().contains(&self)
    }

    /// Whether the state is terminal for the given class.
    #[must_use]
    pub fn is_terminal_in(self, class: Class) -> bool {
        class.terminal().contains(&self)
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One edge of a class lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    /// The state moved from.
    pub from: State,
    /// The state moved to.
    pub to: State,
    /// The operation that causes it, as named in the vocabulary.
    pub trigger: &'static str,
}

const fn t(from: State, to: State, trigger: &'static str) -> Transition {
    Transition { from, to, trigger }
}

use State as S;

const NORMATIVE_TRANSITIONS: &[Transition] = &[
    t(S::Proposed, S::Active, "accept"),
    t(S::Proposed, S::Rejected, "reject"),
    t(S::Proposed, S::Withdrawn, "withdraw"),
    t(S::Proposed, S::Superseded, "supersede"),
    t(S::Active, S::Superseded, "supersede"),
    t(S::Active, S::Withdrawn, "withdraw"),
];

const EMPIRICAL_TRANSITIONS: &[Transition] = &[
    t(S::Verified, S::Disproven, "disprove"),
    t(S::Verified, S::Superseded, "supersede"),
    t(S::Verified, S::Withdrawn, "withdraw"),
];

const PLANNING_TRANSITIONS: &[Transition] = &[
    t(S::Proposed, S::Ready, "ready"),
    t(S::Proposed, S::Abandoned, "abandon"),
    t(S::Proposed, S::Superseded, "supersede"),
    t(S::Ready, S::Active, "start"),
    t(S::Ready, S::Blocked, "block"),
    t(S::Ready, S::Abandoned, "abandon"),
    t(S::Ready, S::Superseded, "supersede"),
    t(S::Active, S::Blocked, "block"),
    t(S::Active, S::Completed, "complete"),
    t(S::Active, S::Abandoned, "abandon"),
    t(S::Active, S::Superseded, "supersede"),
    t(S::Blocked, S::Active, "unblock"),
    t(S::Blocked, S::Abandoned, "abandon"),
    t(S::Blocked, S::Superseded, "supersede"),
];

const INQUIRY_TRANSITIONS: &[Transition] = &[
    t(S::Open, S::Deferred, "defer"),
    t(S::Open, S::Resolved, "resolve"),
    t(S::Open, S::ClosedWithoutResolution, "close"),
    t(S::Open, S::Superseded, "supersede"),
    t(S::Deferred, S::Open, "reopen"),
    t(S::Deferred, S::Resolved, "resolve"),
    t(S::Deferred, S::ClosedWithoutResolution, "close"),
    t(S::Deferred, S::Superseded, "supersede"),
];

impl Class {
    /// Every state legal for this class.
    #[must_use]
    pub const fn states(self) -> &'static [State] {
        use State as S;
        match self {
            Self::Normative => &[
                S::Proposed,
                S::Active,
                S::Rejected,
                S::Superseded,
                S::Withdrawn,
            ],
            Self::Empirical => &[S::Verified, S::Disproven, S::Superseded, S::Withdrawn],
            Self::Planning => &[
                S::Proposed,
                S::Ready,
                S::Active,
                S::Blocked,
                S::Completed,
                S::Abandoned,
                S::Superseded,
            ],
            Self::Inquiry => &[
                S::Open,
                S::Deferred,
                S::Resolved,
                S::ClosedWithoutResolution,
                S::Superseded,
            ],
        }
    }

    /// The states a record of this class may be authored in.
    #[must_use]
    pub const fn initial(self) -> &'static [State] {
        use State as S;
        match self {
            Self::Normative => &[S::Proposed, S::Active],
            Self::Empirical => &[S::Verified],
            Self::Planning => &[S::Proposed],
            Self::Inquiry => &[S::Open],
        }
    }

    /// The live states: the record still speaks for itself.
    #[must_use]
    pub const fn live(self) -> &'static [State] {
        use State as S;
        match self {
            Self::Normative => &[S::Proposed, S::Active],
            Self::Empirical => &[S::Verified],
            Self::Planning => &[S::Proposed, S::Ready, S::Active, S::Blocked],
            Self::Inquiry => &[S::Open, S::Deferred],
        }
    }

    /// The terminal states: the record no longer speaks for itself.
    #[must_use]
    pub const fn terminal(self) -> &'static [State] {
        use State as S;
        match self {
            Self::Normative => &[S::Rejected, S::Superseded, S::Withdrawn],
            Self::Empirical => &[S::Disproven, S::Superseded, S::Withdrawn],
            Self::Planning => &[S::Completed, S::Abandoned, S::Superseded],
            Self::Inquiry => &[S::Resolved, S::ClosedWithoutResolution, S::Superseded],
        }
    }

    /// The lifecycle transitions for this class.
    #[must_use]
    pub const fn transitions(self) -> &'static [Transition] {
        match self {
            Self::Normative => NORMATIVE_TRANSITIONS,
            Self::Empirical => EMPIRICAL_TRANSITIONS,
            Self::Planning => PLANNING_TRANSITIONS,
            Self::Inquiry => INQUIRY_TRANSITIONS,
        }
    }
}

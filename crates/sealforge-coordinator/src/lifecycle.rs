//! Auction lifecycle state machine (docs/05-BACKEND-SCHEMA.md §5.2).
//!
//! `draft → dkg → ready → open → closed → published → refunding → settled`,
//! plus `cancelled` from `draft`, `dkg`, `ready`, or `open` with zero bids.

use sealforge_api::types::AuctionStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Organiser,
    /// The coordinator itself (DKG completion, close height, refunds).
    System,
}

/// Whether `actor` may move an auction from `from` to `to`. Preconditions that
/// need data (e.g. zero bids before cancelling an open auction) are checked by
/// the caller.
pub fn can_transition(from: AuctionStatus, to: AuctionStatus, actor: Actor) -> bool {
    use AuctionStatus::*;
    matches!(
        (from, to, actor),
        (Draft, Dkg, Actor::System)
            | (Dkg, Ready, Actor::System)
            | (Ready, Open, Actor::Organiser)
            | (Open, Closed, Actor::System)
            | (Closed, Published, _)
            | (Published, Refunding, Actor::System)
            | (Refunding, Settled, Actor::System)
            | (Draft | Dkg | Ready | Open, Cancelled, Actor::Organiser)
    )
}

/// Auction details can change only before the auction opens.
pub fn is_editable(status: AuctionStatus) -> bool {
    matches!(
        status,
        AuctionStatus::Draft | AuctionStatus::Dkg | AuctionStatus::Ready
    )
}

/// Statuses visible to the public.
pub fn is_public(status: AuctionStatus) -> bool {
    use AuctionStatus::*;
    matches!(status, Open | Closed | Published | Refunding | Settled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use AuctionStatus::*;

    const ALL: [AuctionStatus; 9] = [
        Draft, Dkg, Ready, Open, Closed, Published, Refunding, Settled, Cancelled,
    ];

    #[test]
    fn happy_path_is_allowed() {
        let path = [
            (Draft, Dkg, Actor::System),
            (Dkg, Ready, Actor::System),
            (Ready, Open, Actor::Organiser),
            (Open, Closed, Actor::System),
            (Closed, Published, Actor::Organiser),
            (Published, Refunding, Actor::System),
            (Refunding, Settled, Actor::System),
        ];
        for (from, to, actor) in path {
            assert!(can_transition(from, to, actor), "{from:?} -> {to:?}");
        }
    }

    #[test]
    fn organisers_cannot_skip_steps_or_reopen() {
        assert!(!can_transition(Draft, Open, Actor::Organiser));
        assert!(
            !can_transition(Dkg, Ready, Actor::Organiser),
            "only DKG completion makes an auction ready"
        );
        assert!(
            !can_transition(Open, Closed, Actor::Organiser),
            "close happens at close height"
        );
        assert!(!can_transition(Closed, Open, Actor::Organiser));
        assert!(!can_transition(Published, Cancelled, Actor::Organiser));
    }

    #[test]
    fn terminal_states_have_no_exits() {
        for to in ALL {
            for actor in [Actor::Organiser, Actor::System] {
                assert!(!can_transition(Settled, to, actor));
                assert!(!can_transition(Cancelled, to, actor));
            }
        }
    }

    #[test]
    fn editability_and_visibility() {
        assert!(is_editable(Draft) && is_editable(Ready));
        assert!(!is_editable(Open));
        assert!(!is_public(Draft) && !is_public(Cancelled));
        assert!(is_public(Open) && is_public(Settled));
    }
}

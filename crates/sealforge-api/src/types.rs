//! Request and response bodies for the coordinator's `/v1` API.
//! Amounts are integer zatoshi. Heights are block heights.

use serde::{Deserialize, Serialize};

macro_rules! db_enum {
    ($(#[$meta:meta])* $name:ident, $pg:literal { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(type_name = $pg, rename_all = "snake_case"))]
        pub enum $name { $($variant),+ }
    };
}

db_enum!(Network, "network_kind" { Regtest, Testnet, Mainnet });
db_enum!(AuctionMode, "auction_mode" { UniformPrice, FirstPrice });
db_enum!(BidMode, "bid_mode" { Deposit, CommitReveal });
db_enum!(
    /// Auction lifecycle (docs/05-BACKEND-SCHEMA.md §5.2).
    AuctionStatus, "auction_status" {
        Draft, Dkg, Ready, Open, Closed, Published, Refunding, Settled, Cancelled
    }
);
db_enum!(DkgStatus, "dkg_status" { Invited, Round1, Round2, Complete });

/// `POST /v1/organisers`, signed with the new organiser key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterOrganiser {
    pub display_name: String,
    pub invite_code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrganiserView {
    pub id: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrusteeSlotInput {
    pub slot: u16,
    pub label: String,
}

/// `POST /v1/auctions` (organiser).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAuction {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub lot_image_url: Option<String>,
    pub network: Network,
    pub mode: AuctionMode,
    #[serde(default = "default_bid_mode")]
    pub bid_mode: BidMode,
    pub quantity: u32,
    pub reserve_zat: u64,
    pub close_height: u32,
    pub payout_ua: String,
    pub trustees: Vec<TrusteeSlotInput>,
}

fn default_bid_mode() -> BidMode {
    BidMode::Deposit
}

/// `PATCH /v1/auctions/{code}` (organiser, before the auction opens).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpdateAuction {
    pub title: Option<String>,
    pub description: Option<String>,
    pub lot_image_url: Option<String>,
    pub quantity: Option<u32>,
    pub reserve_zat: Option<u64>,
    pub close_height: Option<u32>,
    pub payout_ua: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrusteeView {
    pub slot: u16,
    pub label: String,
    pub dkg_status: DkgStatus,
}

/// Fields only the owning organiser sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerFields {
    pub payout_ua: String,
    pub fee_buffer_zat: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionView {
    pub code: String,
    pub title: String,
    pub description: String,
    pub lot_image_url: Option<String>,
    pub network: Network,
    pub mode: AuctionMode,
    pub bid_mode: BidMode,
    pub quantity: u32,
    pub reserve_zat: u64,
    pub open_height: Option<u32>,
    pub close_height: u32,
    pub status: AuctionStatus,
    pub escrow_ua: Option<String>,
    pub trustees: Vec<TrusteeView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<OwnerFields>,
}

/// One-time trustee invite, returned only in the create response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrusteeInvite {
    pub slot: u16,
    pub label: String,
    pub invite_code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatedAuction {
    pub auction: AuctionView,
    pub trustee_invites: Vec<TrusteeInvite>,
}

/// Error body for every non-2xx response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Stable machine-readable code, e.g. `invalid_transition`.
    pub code: String,
    pub error: String,
}

// ---------------------------------------------------------------------------
// Trustees and DKG (task 2.4). Binary payloads are hex.

/// `POST /v1/trustees/join`, signed with the trustee's new Ed25519 key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JoinAsTrustee {
    pub invite_code: String,
    /// X25519 public key that round 2 packages are sealed to.
    pub enc_pubkey: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrusteeJoined {
    pub auction_code: String,
    pub slot: u16,
    pub label: String,
    pub min_signers: u16,
    pub max_signers: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DkgTrustee {
    pub slot: u16,
    pub label: String,
    /// Ed25519 key that signs this trustee's packages (hex). `None` until joined.
    pub pubkey: Option<String>,
    pub enc_pubkey: Option<String>,
    pub dkg_status: DkgStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DkgPackage {
    pub from_slot: u16,
    /// `None` for broadcast (round 1) packages.
    pub to_slot: Option<u16>,
    /// Round 1: serialized package. Round 2: sealed box to the recipient.
    pub payload: String,
    pub signature: String,
}

/// `GET /v1/auctions/{code}/dkg`: everything the calling trustee needs next.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DkgState {
    pub auction_code: String,
    pub auction_status: AuctionStatus,
    pub my_slot: u16,
    pub min_signers: u16,
    pub max_signers: u16,
    pub trustees: Vec<DkgTrustee>,
    pub round1: Vec<DkgPackage>,
    /// Round 2 packages addressed to the caller.
    pub round2_for_me: Vec<DkgPackage>,
    pub escrow_ua: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostRound1 {
    pub payload: String,
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Round2Out {
    pub to_slot: u16,
    pub payload: String,
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostRound2 {
    pub packages: Vec<Round2Out>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompleteDkg {
    /// Serialized FROST public key package (hex).
    pub public_key_package: String,
}

// ---------------------------------------------------------------------------
// Results (task 3.3). Public once an auction is published.

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultRules {
    pub mode: AuctionMode,
    pub quantity: u32,
    pub reserve_zat: u64,
}

/// One bid in the published result. Everything here is already visible to
/// anyone holding the published viewing key; refund addresses are not included.
/// With txid, index, amount and commitment, a bidder can recompute their own
/// leaf and Merkle proof from a receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultBid {
    pub txid: String,
    pub output_index: u32,
    pub block_height: u32,
    pub amount_zat: u64,
    pub commitment: String,
    pub leaf_hash: String,
    pub is_winner: bool,
    pub price_paid_zat: u64,
    pub refund_due_zat: u64,
}

/// `GET /v1/auctions/{code}/result`: everything an independent auditor needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedResult {
    pub auction_code: String,
    pub network: Network,
    pub rules: ResultRules,
    /// Scan exactly this range with the UIVK to recompute the result.
    pub scan_from_height: u32,
    pub scan_to_height: u32,
    /// Unified incoming viewing key of the escrow.
    pub uivk: String,
    pub algorithm_version: String,
    pub clearing_price_zat: Option<u64>,
    pub total_volume_zat: u64,
    pub valid_bid_count: u32,
    pub winner_count: u32,
    pub merkle_root: String,
    pub bids: Vec<ResultBid>,
}

// ---------------------------------------------------------------------------
// Batch signing (task 3.9). Binary values are hex.

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpendAuthView {
    pub action_index: u32,
    /// Spend authorization randomizer the signature must use.
    pub alpha: String,
}

/// An escrow transaction awaiting 2-of-3 trustee signatures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchView {
    pub id: String,
    pub auction_code: String,
    /// `refund` or `payout`
    pub kind: String,
    /// `awaiting_signatures`, `signed`, `broadcast`, `confirmed`
    pub status: String,
    pub sighash: String,
    pub spends: Vec<SpendAuthView>,
    /// Shown to trustees before they sign.
    pub total_out_zat: u64,
    pub output_count: u32,
    /// The two trustees whose commitments were accepted (empty until two commit).
    pub signer_slots: Vec<u16>,
    /// Round 1 commitments of the chosen signers, per slot, one per spend.
    pub signer_commitments: std::collections::BTreeMap<u16, Vec<String>>,
    pub i_committed: bool,
    pub i_shared: bool,
}

/// Round 1: one FROST signing commitment per spend, in `spends` order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostCommitments {
    pub commitments: Vec<String>,
}

/// Round 2: one FROST signature share per spend, in `spends` order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostShares {
    pub shares: Vec<String>,
}

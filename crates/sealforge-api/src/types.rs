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
    /// Commit-reveal only: last block for reveals (at least 144 after the close).
    #[serde(default)]
    pub reveal_close_height: Option<u32>,
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
    /// The trustee's Ed25519 key for this auction (hex). `None` until joined.
    #[serde(default)]
    pub pubkey: Option<String>,
    /// The trustee's signature over [`crate::auth::escrow_attestation_message`]
    /// for `escrow_ua` (hex). Anyone can check it with `pubkey`; the auditor
    /// does (`sealforge_auditor::check_escrow`).
    #[serde(default)]
    pub escrow_attestation: Option<String>,
    /// The trustee's long-term key vouching for `pubkey` in this auction.
    /// `None` if the trustee hasn't certified (or the coordinator withholds it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<TrusteeIdentityCert>,
}

/// A trustee's long-term Ed25519 key vouching for its key in one auction
/// ([`crate::auth::trustee_cert_message`]). The trustee publishes `pubkey` at
/// `published_at` (a URL or handle it controls); bidders compare the two.
/// Also the body of `POST /v1/auctions/{code}/trustee-identity`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrusteeIdentityCert {
    /// Long-term public key (hex).
    pub pubkey: String,
    pub published_at: String,
    /// Signature over the certificate message (hex).
    pub signature: String,
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
    #[serde(default)]
    pub reveal_close_height: Option<u32>,
    pub status: AuctionStatus,
    pub escrow_ua: Option<String>,
    pub trustees: Vec<TrusteeView>,
    /// Payments to the escrow so far. Hidden below [`MIN_PUBLIC_BID_COUNT`] so
    /// early activity doesn't leak; the owner always sees it.
    #[serde(default)]
    pub bid_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<OwnerFields>,
    /// sealForge's fee on this auction's proceeds, in basis points, fixed when
    /// the auction was created. 0 (omitted) means no fee.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub fee_bps: u16,
    /// Where the fee is paid, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_ua: Option<String>,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

/// The public bid count appears only from this many bids (docs/03-APP-FLOW.md §3.7).
pub const MIN_PUBLIC_BID_COUNT: u32 = 5;

/// Every auction's escrow is 2-of-3: exactly these trustee slots, any two of
/// which can sign. The coordinator refuses anything else at creation, and the
/// auditor refuses to call an escrow verified unless exactly these slots, each
/// with its own key, signed it.
pub const TRUSTEE_SLOTS: [u16; 3] = [1, 2, 3];

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
// Trustees and DKG. Binary payloads are hex.

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
    /// The trustee rebuilt the escrow address from its own group key and
    /// signed an attestation over it.
    #[serde(default)]
    pub escrow_verified: bool,
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
    /// `nk ‖ rivk` of the escrow FVK, sealed to the caller's X25519 key (hex).
    /// With its own group key the trustee rebuilds the escrow address and
    /// checks it against `escrow_ua` before attesting.
    #[serde(default)]
    pub escrow_view_secret_for_me: Option<String>,
    /// Chain height when the escrow was created: where a wallet restored from
    /// a trustee backup starts scanning.
    #[serde(default)]
    pub escrow_birthday_height: Option<u32>,
}

/// A trustee's attestation that the escrow address derives from the FROST
/// group key it holds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifyEscrow {
    pub escrow_ua: String,
    /// Ed25519 signature over [`crate::auth::escrow_attestation_message`] (hex).
    pub signature: String,
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
// Results. Public once an auction is published.

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultRules {
    pub mode: AuctionMode,
    pub quantity: u32,
    pub reserve_zat: u64,
    #[serde(default = "default_bid_mode")]
    pub bid_mode: BidMode,
    /// Commit-reveal only: deposits count up to this height, and reveals from
    /// the next block up to `scan_to_height` (the reveal close). In deposit
    /// mode the bidding close is `scan_to_height`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_height: Option<u32>,
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
    /// The bid: the payment amount, or in commit-reveal mode the revealed price.
    pub amount_zat: u64,
    /// Commit-reveal only: the deposit (budget) that carried the bid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposit_zat: Option<u64>,
    /// Commit-reveal only: the amount of the reveal payment that counted. It is
    /// refunded with the deposit, so refund = deposit + reveal - price paid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reveal_zat: Option<u64>,
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
    /// The escrow address bidders paid. The auditor checks that `uivk` views
    /// exactly this address and that every trustee attested to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escrow_ua: Option<String>,
    /// Each trustee's attestation over `escrow_ua`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trustees: Vec<TrusteeAttestation>,
    /// sealForge's fee rate for this auction, in basis points (omitted: none).
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub fee_bps: u16,
    /// Where the fee is paid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_ua: Option<String>,
    /// The fee owed: `sealforge_core::fee::protocol_fee_zat` of the sum of
    /// every bid's `price_paid_zat` at `fee_bps`. Paid as a second output of
    /// the payout; the organiser gets the rest.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub fee_zat: u64,
}

/// A trustee's signed statement that `escrow_ua` derives from the FROST group
/// key it holds. Signature over [`crate::auth::escrow_attestation_message`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrusteeAttestation {
    pub slot: u16,
    pub label: String,
    /// Ed25519 public key (hex), `None` if the trustee never joined.
    pub pubkey: Option<String>,
    /// Ed25519 signature (hex), `None` if the trustee never attested.
    pub signature: Option<String>,
    /// The trustee's long-term key vouching for `pubkey`, if it certified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<TrusteeIdentityCert>,
}

// ---------------------------------------------------------------------------
// Batch signing. Binary values are hex.

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
    /// `refund`, `payout`, or `consolidate` (merges escrow notes into one note
    /// at the escrow's internal address so a refund fits in one transaction)
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
    /// The transaction itself (PCZT bytes, hex). Trustees decode it, check it
    /// and compute the sighash themselves before signing (security review D1).
    #[serde(default)]
    pub pczt: Option<String>,
    /// Payout batches: the organiser's payout address, which the one payment
    /// must go to.
    #[serde(default)]
    pub payout_ua: Option<String>,
    /// Refund batches: which bid each refund note pays back. Trustees read
    /// each bid from the chain and check the note goes to its refund address.
    #[serde(default)]
    pub refunds: Vec<RefundNoteView>,
    /// Each chosen signer's signed statement of the round 2 inputs it will sign
    /// over. A signer releases its shares only once its co-signer's statement
    /// matches its own (a mismatch means the coordinator equivocated).
    #[serde(default)]
    pub signer_inputs: std::collections::BTreeMap<u16, SignedInputs>,
}

/// A chosen signer's statement of its round 2 inputs: the
/// [`crate::auth::round2_inputs_digest`] and the signer's Ed25519 signature
/// over [`crate::auth::round2_inputs_message`], both hex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedInputs {
    pub digest: String,
    pub signature: String,
}

/// One refund note in a batch, and the bid (payment to the escrow) it refunds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundNoteView {
    /// The bid's transaction id (hex, as in the published result).
    pub bid_txid: String,
    pub bid_output_index: u32,
    pub value_zat: u64,
}

/// `GET /v1/console/auctions/{code}/batches`: the organiser's view of escrow
/// transactions. No signing material.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsoleBatch {
    pub id: String,
    /// `refund` or `payout`
    pub kind: String,
    /// `building`, `awaiting_signatures`, `signed`, `broadcast`, `confirmed`, `failed`,
    /// `expired` (not mined by its expiry height; its notes moved to a new batch)
    pub status: String,
    pub output_count: Option<u32>,
    pub total_out_zat: Option<u64>,
    /// Trustees who have committed (round 1) and signed (round 2).
    pub committed: u16,
    pub signed: u16,
    /// Transaction id in the usual display order, once broadcast.
    pub txid: Option<String>,
    pub confirmed_height: Option<u32>,
}

/// `GET /v1/chain`: the coordinator's view of the chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainView {
    /// `None` until the scanner has reached the lightwallet server.
    pub tip: Option<u32>,
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

/// Between rounds: a chosen signer's signed statement of its round 2 inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostInputs {
    pub digest: String,
    pub signature: String,
}

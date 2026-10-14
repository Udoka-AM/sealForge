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

//! Chain access for sealForge (task 2.5).
//!
//! Each auction escrow gets its own `zcash_client_sqlite` wallet, imported from
//! the escrow UFVK as a view-only account and synced from a lightwalletd-compatible
//! server (Zaino). The same wallet later provides note witnesses and PCZTs for
//! refunds (task 3.9).

pub mod cache;
#[cfg(feature = "dev-wallet")]
pub mod dev;
pub mod params;
pub mod spend;
pub mod wallet;

pub use params::ChainParams;
pub use wallet::{EscrowWallet, ReceivedPayment};

use zcash_client_backend::proto::service::compact_tx_streamer_client::CompactTxStreamerClient;

pub type Client = CompactTxStreamerClient<tonic::transport::Channel>;

/// Connect to a lightwalletd-compatible gRPC server, e.g. `http://127.0.0.1:8137`.
pub async fn connect(url: &str) -> anyhow::Result<Client> {
    let channel = tonic::transport::Channel::from_shared(url.to_owned())?
        .connect()
        .await?;
    Ok(CompactTxStreamerClient::new(channel))
}

/// Height of the server's chain tip.
pub async fn chain_tip(client: &mut Client) -> anyhow::Result<u32> {
    let tip = client
        .get_latest_block(zcash_client_backend::proto::service::ChainSpec::default())
        .await?
        .into_inner();
    Ok(u32::try_from(tip.height)?)
}

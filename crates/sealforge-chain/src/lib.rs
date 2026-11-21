//! Chain access for sealForge (task 2.5).
//!
//! Each auction escrow gets its own `zcash_client_sqlite` wallet, imported from
//! the escrow UFVK as a view-only account and synced from a lightwalletd-compatible
//! server (Zaino). The same wallet later provides note witnesses and PCZTs for
//! refunds (task 3.9).

pub mod audit;
pub mod cache;
#[cfg(feature = "dev-wallet")]
pub mod dev;
pub mod spend;
pub mod wallet;

pub use sealforge_scan::{ChainParams, ReceivedPayment};
pub use wallet::EscrowWallet;

use zcash_client_backend::proto::service::compact_tx_streamer_client::CompactTxStreamerClient;

pub type Client = CompactTxStreamerClient<tonic::transport::Channel>;

/// Connect to a lightwalletd-compatible gRPC server, e.g. `http://127.0.0.1:8137`.
/// `https://` endpoints (public lightwallet servers) use TLS with the webpki roots.
pub async fn connect(url: &str) -> anyhow::Result<Client> {
    let mut endpoint = tonic::transport::Channel::from_shared(url.to_owned())?;
    if url.starts_with("https://") {
        // Several TLS backends are compiled in across the workspace; pick one.
        let _ = rustls::crypto::ring::default_provider().install_default();
        endpoint =
            endpoint.tls_config(tonic::transport::ClientTlsConfig::new().with_webpki_roots())?;
    }
    let channel = endpoint.connect().await?;
    Ok(CompactTxStreamerClient::new(channel))
}

/// A transaction as the server has it: its raw bytes, and the height it was
/// mined at (`None` while it is only in the mempool). `Ok(None)` if the server
/// doesn't know it.
pub async fn fetch_transaction(
    client: &mut Client,
    txid: [u8; 32],
) -> anyhow::Result<Option<(Vec<u8>, Option<u32>)>> {
    match client
        .get_transaction(zcash_client_backend::proto::service::TxFilter {
            block: None,
            index: 0,
            hash: txid.to_vec(),
        })
        .await
    {
        Ok(raw) => {
            let raw = raw.into_inner();
            let height = u32::try_from(raw.height)
                .ok()
                .filter(|h| *h > 0 && *h != u32::MAX);
            Ok(Some((raw.data, height)))
        }
        Err(status) if status.code() == tonic::Code::NotFound => Ok(None),
        // Servers word a missing transaction differently; treat their
        // "not found" errors as such, and anything else as a failure.
        Err(status) if status.message().to_lowercase().contains("not found") => Ok(None),
        Err(status) => Err(status.into()),
    }
}

/// Height of the server's chain tip.
pub async fn chain_tip(client: &mut Client) -> anyhow::Result<u32> {
    let tip = client
        .get_latest_block(zcash_client_backend::proto::service::ChainSpec::default())
        .await?
        .into_inner();
    Ok(u32::try_from(tip.height)?)
}

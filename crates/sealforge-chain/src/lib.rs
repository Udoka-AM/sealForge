//! Chain access for sealForge.
//!
//! Each auction escrow gets its own `zcash_client_sqlite` wallet, imported from
//! the escrow UFVK as a view-only account and synced from a lightwalletd-compatible
//! server (Zaino). The same wallet later provides note witnesses and PCZTs for
//! refunds.

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
        Err(status) if is_missing_transaction(status.code(), status.message()) => Ok(None),
        Err(status) => Err(status.into()),
    }
}

/// Whether a `GetTransaction` error means the transaction isn't known, as
/// opposed to the server failing. Servers word this differently: a gRPC
/// `NotFound`, a "not found" message, or the node's RPC error -5 ("No such
/// mempool or main chain transaction", e.g. Zaino over a regtest node for a
/// transaction that expired unmined). Anything else is a failure.
fn is_missing_transaction(code: tonic::Code, message: &str) -> bool {
    let message = message.to_lowercase();
    code == tonic::Code::NotFound
        || message.contains("not found")
        || message.contains("no such mempool or main chain transaction")
}

/// Height of the server's chain tip.
pub async fn chain_tip(client: &mut Client) -> anyhow::Result<u32> {
    let tip = client
        .get_latest_block(zcash_client_backend::proto::service::ChainSpec::default())
        .await?
        .into_inner();
    Ok(u32::try_from(tip.height)?)
}

#[cfg(test)]
mod tests {
    use super::is_missing_transaction;
    use tonic::Code;

    #[test]
    fn missing_transaction_errors_are_recognised() {
        assert!(is_missing_transaction(Code::NotFound, ""));
        assert!(is_missing_transaction(
            Code::Internal,
            "transaction not found"
        ));
        // Seen from Zaino on regtest for a batch that expired before broadcast.
        assert!(is_missing_transaction(
            Code::Internal,
            "RPC error: LegacyRpcError { code: -5, message: \"No such mempool or main chain transaction\" }"
        ));
        assert!(!is_missing_transaction(
            Code::Unavailable,
            "connection refused"
        ));
        assert!(!is_missing_transaction(
            Code::Internal,
            "error receiving data from backing node"
        ));
    }
}

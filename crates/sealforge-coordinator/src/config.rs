use clap::Parser;

/// Coordinator configuration, read from flags or environment (see .env.example).
#[derive(Debug, Parser)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "SF_LISTEN_ADDR", default_value = "0.0.0.0:8080")]
    pub listen_addr: String,

    /// regtest | testnet | mainnet
    #[arg(long, env = "SF_NETWORK", default_value = "testnet")]
    pub network: String,

    /// Zaino / lightwalletd gRPC endpoint used by the scanner and broadcaster.
    #[arg(long, env = "SF_ZAINO_URL", default_value = "http://127.0.0.1:8137")]
    pub zaino_url: String,

    /// Key used by pgcrypto for encrypted columns.
    #[arg(long, env = "SF_DATA_KEY", hide_env_values = true)]
    pub data_key: Option<String>,

    /// Pause the refund scheduler (rollback lever).
    #[arg(long, env = "SF_REFUNDS_PAUSED", default_value_t = false)]
    pub refunds_paused: bool,

    /// Comma-separated organiser invite codes.
    #[arg(
        long,
        env = "SF_INVITE_CODES",
        default_value = "",
        hide_env_values = true
    )]
    pub invite_codes: String,

    /// Directory for escrow wallet databases (one SQLite file per auction).
    #[arg(long, env = "SF_WALLET_DIR", default_value = "data/wallets")]
    pub wallet_dir: std::path::PathBuf,

    /// Regtest only: refund delays as "min,max" seconds instead of 10–240 minutes,
    /// so a local end-to-end run doesn't wait hours. Ignored on other networks.
    #[arg(long, env = "SF_REFUND_DELAY_SECS")]
    pub refund_delay_secs: Option<String>,

    /// Development only: pretend the chain tip is at this height (when no node is available).
    #[arg(long, env = "SF_DEV_CHAIN_HEIGHT")]
    pub dev_chain_height: Option<u32>,
}

impl Config {
    /// The refund policy: the PRD default, or the regtest override.
    pub fn refund_policy(&self) -> anyhow::Result<sealforge_core::refunds::RefundPolicy> {
        let mut policy = sealforge_core::refunds::RefundPolicy::default();
        let Some(spec) = &self.refund_delay_secs else {
            return Ok(policy);
        };
        if self.network != "regtest" {
            anyhow::bail!("SF_REFUND_DELAY_SECS is only allowed with SF_NETWORK=regtest");
        }
        let (min, max) = spec
            .split_once(',')
            .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
            .filter(|(a, b): &(u64, u64)| a <= b)
            .ok_or_else(|| anyhow::anyhow!("SF_REFUND_DELAY_SECS must be \"min,max\" seconds"))?;
        policy.min_delay_secs = min;
        policy.max_delay_secs = max;
        policy.max_hold_secs = max;
        Ok(policy)
    }

    pub fn invite_codes(&self) -> Vec<String> {
        self.invite_codes
            .split(',')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

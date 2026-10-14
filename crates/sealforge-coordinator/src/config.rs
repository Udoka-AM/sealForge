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

    /// Development only: pretend the chain tip is at this height until the scanner exists.
    #[arg(long, env = "SF_DEV_CHAIN_HEIGHT")]
    pub dev_chain_height: Option<u32>,
}

impl Config {
    pub fn invite_codes(&self) -> Vec<String> {
        self.invite_codes
            .split(',')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

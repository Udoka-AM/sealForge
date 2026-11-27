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

    /// Test networks only: refund delays as "min,max" seconds instead of 10–240
    /// minutes, so a demo auction fits in an hour. Refused on mainnet.
    #[arg(long, env = "SF_REFUND_DELAY_SECS")]
    pub refund_delay_secs: Option<String>,

    /// Test networks only: the shortest bidding and reveal window in blocks
    /// (default 144). Refused on mainnet.
    #[arg(long, env = "SF_MIN_WINDOW_BLOCKS")]
    pub min_window_blocks: Option<u32>,

    /// Test networks only: lower the most escrow notes one transaction may spend
    /// (default and maximum: `sealforge_chain::spend::MAX_NOTES_PER_TX`), to
    /// exercise consolidation and split payouts with a few bids. Refused on mainnet.
    #[arg(long, env = "SF_MAX_NOTES_PER_TX")]
    pub max_notes_per_tx: Option<usize>,

    /// sealForge's default fee on new auctions' proceeds, in basis points
    /// (100 = 1%): 0, the default, for no fee, or 500 to 1500 (5% to 15%).
    /// Recorded on each auction when it is created; the admin can set an
    /// auction's own rate before it opens. Changing this doesn't affect
    /// existing auctions.
    #[arg(long, env = "SF_FEE_BPS", default_value_t = 0)]
    pub fee_bps: u16,

    /// Where the fee is paid: a unified address with an Orchard receiver on
    /// this network. Required when SF_FEE_BPS is above 0.
    #[arg(long, env = "SF_FEE_UA")]
    pub fee_ua: Option<String>,

    /// Bearer token for the operator's admin endpoints (setting an auction's
    /// fee rate). Unset: the admin endpoints are off.
    #[arg(long, env = "SF_ADMIN_TOKEN", hide_env_values = true)]
    pub admin_token: Option<String>,

    /// Sustained API reads (GET) allowed per client per minute; 0 turns read
    /// limits off. A quarter of it may arrive at once.
    #[arg(long, env = "SF_RATE_LIMIT_READS_PER_MIN", default_value_t = 600)]
    pub rate_limit_reads_per_min: u32,

    /// Sustained API writes allowed per client per minute; 0 turns write
    /// limits off.
    #[arg(long, env = "SF_RATE_LIMIT_WRITES_PER_MIN", default_value_t = 120)]
    pub rate_limit_writes_per_min: u32,

    /// Header in which a trusted proxy passes the client's address (e.g.
    /// `x-forwarded-for` behind the web app). Unset: the TCP peer address.
    #[arg(long, env = "SF_CLIENT_IP_HEADER")]
    pub client_ip_header: Option<String>,

    /// Most API requests handled at once; more wait their turn. Kept below the
    /// database pool size so the scanner, closer and signing always have
    /// connections.
    #[arg(long, env = "SF_HTTP_MAX_IN_FLIGHT", default_value_t = 12)]
    pub http_max_in_flight: usize,

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
        if self.network == "mainnet" {
            anyhow::bail!("SF_REFUND_DELAY_SECS is not allowed on mainnet");
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

    pub fn rate_limits(&self) -> anyhow::Result<crate::ratelimit::RateLimits> {
        let client_header = self
            .client_ip_header
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(|h| {
                axum::http::HeaderName::from_bytes(h.to_ascii_lowercase().as_bytes())
                    .map_err(|_| anyhow::anyhow!("SF_CLIENT_IP_HEADER isn't a valid header name"))
            })
            .transpose()?;
        Ok(crate::ratelimit::RateLimits {
            reads_per_minute: self.rate_limit_reads_per_min,
            writes_per_minute: self.rate_limit_writes_per_min,
            client_header,
        })
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

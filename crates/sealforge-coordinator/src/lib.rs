//! sealForge coordinator: REST API, escrow scanner, refund scheduler.
//! See docs/02-TRD.md §2.3 and §2.6.

pub mod auth;
pub mod codes;
pub mod config;
pub mod db;
pub mod error;
pub mod lifecycle;
pub mod routes;
pub mod validate;

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use sqlx::PgPool;

/// Shared state for every request.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub settings: Arc<Settings>,
    /// Latest known chain height, updated by the scanner (task 2.5). 0 = unknown.
    pub chain_tip: Arc<AtomicU32>,
}

#[derive(Debug, Default)]
pub struct Settings {
    /// Invite codes accepted by `POST /v1/organisers` (the hackathon instance is invite-only).
    pub invite_codes: Vec<String>,
    /// Key for pgcrypto-encrypted columns (escrow UFVK). DKG can't complete without it.
    pub data_key: Option<String>,
}

impl AppState {
    pub fn new(db: PgPool, settings: Settings) -> Self {
        Self {
            db,
            settings: Arc::new(settings),
            chain_tip: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn chain_tip(&self) -> Option<u32> {
        match self.chain_tip.load(Ordering::Relaxed) {
            0 => None,
            h => Some(h),
        }
    }

    pub fn set_chain_tip(&self, height: u32) {
        self.chain_tip.store(height, Ordering::Relaxed);
    }
}

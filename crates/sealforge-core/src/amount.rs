//! Integer zatoshi amounts. 1 ZEC = 100,000,000 zat.

use serde::{Deserialize, Serialize};

/// Zatoshi per ZEC.
pub const COIN: u64 = 100_000_000;

/// Maximum ZEC supply in zatoshi.
pub const MAX_MONEY: u64 = 21_000_000 * COIN;

/// An amount in zatoshi.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Zatoshi(pub u64);

impl Zatoshi {
    pub const ZERO: Zatoshi = Zatoshi(0);

    pub fn checked_add(self, other: Zatoshi) -> Option<Zatoshi> {
        self.0
            .checked_add(other.0)
            .filter(|v| *v <= MAX_MONEY)
            .map(Zatoshi)
    }

    pub fn checked_sub(self, other: Zatoshi) -> Option<Zatoshi> {
        self.0.checked_sub(other.0).map(Zatoshi)
    }
}

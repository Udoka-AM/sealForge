//! Keys from a BIP-39 recovery phrase, derived with librustzcash (ZIP 32).

use bip0039::{English, Mnemonic};
use orchard::keys::{FullViewingKey, Scope, SpendAuthorizingKey};
use sealforge_scan::ChainParams;
use zcash_address::unified::{self, Encoding};
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_protocol::consensus::Parameters;

/// A fresh 24-word phrase from 32 bytes of caller-supplied entropy (the
/// browser's `crypto.getRandomValues`, or the OS generator natively).
pub fn phrase_from_entropy(entropy: [u8; 32]) -> Result<String, String> {
    Mnemonic::<English>::from_entropy(entropy.to_vec())
        .map(|m| m.phrase().to_owned())
        .map_err(|e| format!("bad entropy: {e}"))
}

/// Whether a phrase is a valid English BIP-39 phrase (words and checksum).
pub fn phrase_is_valid(phrase: &str) -> bool {
    Mnemonic::<English>::from_phrase(normalize(phrase)).is_ok()
}

fn normalize(phrase: &str) -> String {
    phrase
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Account 0's Orchard keys. The same keys spend Orchard and Ironwood notes.
pub struct Keys {
    params: ChainParams,
    pub(crate) fvk: FullViewingKey,
    /// Present only while the wallet is unlocked for spending.
    pub(crate) ask: Option<SpendAuthorizingKey>,
}

impl Keys {
    pub fn from_phrase(params: ChainParams, phrase: &str) -> Result<Self, String> {
        let mnemonic = Mnemonic::<English>::from_phrase(normalize(phrase))
            .map_err(|_| "That isn't a valid 24-word recovery phrase.".to_owned())?;
        let seed = mnemonic.to_seed("");
        let usk = UnifiedSpendingKey::from_seed(&params, &seed, zip32::AccountId::ZERO)
            .map_err(|e| format!("deriving keys: {e:?}"))?;
        let sk = usk.orchard();
        Ok(Self {
            params,
            fvk: FullViewingKey::from(sk),
            ask: Some(SpendAuthorizingKey::from(sk)),
        })
    }

    /// Watch-only keys from a serialized full viewing key (96 bytes, hex).
    pub fn from_fvk_hex(params: ChainParams, fvk: &str) -> Result<Self, String> {
        let bytes: [u8; 96] = hex::decode(fvk)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or("bad viewing key")?;
        let fvk = FullViewingKey::from_bytes(&bytes).ok_or("bad viewing key")?;
        Ok(Self {
            params,
            fvk,
            ask: None,
        })
    }

    pub fn fvk_hex(&self) -> String {
        hex::encode(self.fvk.to_bytes())
    }

    pub fn params(&self) -> ChainParams {
        self.params
    }

    /// The Unified Address (Orchard receiver only) at diversifier index `index`.
    /// Every index gives an address that outsiders can't link to the others.
    pub fn address(&self, index: u32) -> String {
        let receiver = self
            .fvk
            .address_at(index, Scope::External)
            .to_raw_address_bytes();
        unified::Address::try_from_items(
            unified::Revision::R0,
            vec![unified::Uitem::Data(unified::Receiver::Orchard(receiver))],
        )
        .expect("one Orchard receiver is a valid Unified Address")
        .encode(&self.params.network_type())
    }

    pub(crate) fn change_address(&self) -> orchard::Address {
        self.fvk.address_at(0u32, Scope::Internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_round_trip_and_addresses() {
        let phrase = phrase_from_entropy([7; 32]).unwrap();
        assert_eq!(phrase.split(' ').count(), 24);
        assert!(phrase_is_valid(&phrase));
        assert!(phrase_is_valid(&format!("  {}  ", phrase.to_uppercase())));
        assert!(!phrase_is_valid("abandon abandon abandon"));

        let keys = Keys::from_phrase(ChainParams::Testnet, &phrase).unwrap();
        let (a0, a1) = (keys.address(0), keys.address(1));
        assert!(a0.starts_with("utest1"), "{a0}");
        assert_ne!(a0, a1, "each index is a different address");
        // Same phrase, same addresses.
        assert_eq!(
            Keys::from_phrase(ChainParams::Testnet, &phrase)
                .unwrap()
                .address(1),
            a1
        );
        // Watch-only keys see the same addresses but can't spend.
        let watch = Keys::from_fvk_hex(ChainParams::Testnet, &keys.fvk_hex()).unwrap();
        assert_eq!(watch.address(0), a0);
        assert!(watch.ask.is_none());
    }
}

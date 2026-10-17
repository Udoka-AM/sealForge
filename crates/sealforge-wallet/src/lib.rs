//! The bidder's in-browser wallet (task 4.7, PRD F8 / App Flow S12).
//!
//! Everything here is librustzcash: BIP-39 and ZIP 32 keys (`zcash_keys`),
//! trial decryption (`orchard`, `zcash_note_encryption`), note commitment
//! witnesses (`incrementalmerkletree`) and transactions built, proved, signed
//! and extracted with `zcash_primitives` and the `pczt` roles, the same path
//! the escrow uses (`sealforge-chain::spend`). There is no I/O: the browser
//! fetches compact blocks and tree states over gRPC-web and broadcasts.
//!
//! Sync keeps one incremental witness per received note, starting from the
//! commitment tree at the wallet's birthday, and checks its tree size against
//! each block's chain metadata so a missed action is caught, never spent on.
//!
//! The recovery phrase and spending key live only in the browser. The page
//! stores this state encrypted under the user's passphrase (WebCrypto).

pub mod keys;
mod pay;
#[cfg(feature = "wasm")]
mod wasm;

use incrementalmerkletree::{frontier::CommitmentTree, witness::IncrementalWitness};
use orchard::{
    keys::{PreparedIncomingViewingKey, Scope},
    note::{ExtractedNoteCommitment, RandomSeed, Rho},
    note_encryption::{
        CompactAction, DomainVersion, IronwoodVersion, NoteEncryptionDomain, OrchardVersion,
    },
    tree::MerkleHashOrchard,
    value::NoteValue,
    Note,
};
use sealforge_scan::ChainParams;
use serde::{Deserialize, Serialize};
use zcash_client_backend::proto::{compact_formats::CompactBlock, service::TreeState};
use zcash_note_encryption::try_compact_note_decryption;
use zcash_primitives::merkle_tree::{
    read_commitment_tree, read_incremental_witness, write_commitment_tree,
    write_incremental_witness,
};

pub use keys::{phrase_from_entropy, phrase_is_valid, Keys};
pub use pay::{prepare_prover, Built, PaymentRequest};

const DEPTH: u8 = 32;
type Tree = CommitmentTree<MerkleHashOrchard, DEPTH>;
type Witness = IncrementalWitness<MerkleHashOrchard, DEPTH>;

/// Blocks a transaction can wait for before it expires (zcash_primitives' default).
pub const EXPIRY_DELTA: u32 = 40;

pub fn chain_params(network: &str) -> Result<ChainParams, String> {
    match network {
        "mainnet" => Ok(ChainParams::Mainnet),
        "testnet" => Ok(ChainParams::Testnet),
        "regtest" => Ok(ChainParams::regtest()),
        other => Err(format!("unknown network {other}")),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pool {
    Orchard,
    Ironwood,
}

/// A note the wallet received, with what's needed to spend it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteRecord {
    pub pool: Pool,
    /// Transaction id, display order (as block explorers show it).
    pub txid: String,
    pub height: u32,
    pub action_index: u32,
    pub value_zat: u64,
    /// The external address index it was sent to; `None` for change.
    pub address_index: Option<u32>,
    pub change: bool,
    recipient: String,
    rho: String,
    rseed: String,
    note_version: u8,
    pub nullifier: String,
    witness: String,
    pub spent_txid: Option<String>,
    pub spent_height: Option<u32>,
}

/// A transaction this wallet built and broadcast.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SentTx {
    pub txid: String,
    pub to: String,
    pub amount_zat: u64,
    pub fee_zat: u64,
    pub memo: Option<String>,
    /// Caller's label, e.g. "bid:<auction code>".
    pub label: Option<String>,
    pub built_at_height: u32,
    pub expiry_height: u32,
    /// Nullifiers of the notes it spends (hex).
    pub spends: Vec<String>,
    /// Set when its nullifiers appear in a block: the on-chain confirmation.
    pub mined_height: Option<u32>,
    pub expired: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PoolState {
    tree: String,
    notes: Vec<NoteRecord>,
}

/// Everything the page persists (encrypted) between visits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletState {
    pub version: u8,
    pub network: String,
    /// First block scanned.
    pub birthday: u32,
    /// Commitment trees just before the birthday, for a rescan after a reorg.
    birthday_orchard_tree: String,
    birthday_ironwood_tree: String,
    pub scanned_height: u32,
    /// Hash of the last scanned block (internal byte order, hex).
    pub scanned_hash: String,
    pub next_address_index: u32,
    orchard: PoolState,
    ironwood: PoolState,
    pub sent: Vec<SentTx>,
}

/// Runtime form of one pool: the tree and live witnesses.
struct PoolRuntime {
    tree: Tree,
    notes: Vec<(NoteRecord, Witness)>,
}

pub struct Wallet {
    keys: Keys,
    network: String,
    birthday: u32,
    birthday_trees: (String, String),
    scanned_height: u32,
    scanned_hash: Vec<u8>,
    next_address_index: u32,
    orchard: PoolRuntime,
    ironwood: PoolRuntime,
    sent: Vec<SentTx>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScanError {
    /// The block doesn't follow the last one scanned: rescan with [`Wallet::reset`].
    Reorg,
    OutOfOrder {
        expected: u32,
        got: u32,
    },
    /// Our tree disagrees with the block's metadata. Rescan rather than spend on bad witnesses.
    TreeMismatch(Pool),
    Bad(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::Reorg => write!(f, "reorg: the chain changed under the wallet"),
            ScanError::OutOfOrder { expected, got } => {
                write!(f, "expected block {expected}, got {got}")
            }
            ScanError::TreeMismatch(p) => write!(f, "{p:?} commitment tree mismatch"),
            ScanError::Bad(e) => f.write_str(e),
        }
    }
}

/// What a block changed, for the page's activity feed.
#[derive(Debug, Default, Serialize)]
pub struct ScanOutcome {
    pub height: u32,
    pub received: Vec<NoteRecord>,
    pub spent: Vec<String>,
    pub confirmed: Vec<String>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Balance {
    /// Spendable now: confirmed Ironwood notes not locked by a pending payment.
    pub spendable_zat: u64,
    /// Locked by a payment that hasn't confirmed yet (released if it expires).
    pub pending_spend_zat: u64,
    /// Orchard-pool notes. They count, but this wallet spends only Ironwood notes.
    pub orchard_zat: u64,
    pub total_zat: u64,
}

fn tree_hex(tree: &Tree) -> String {
    let mut out = Vec::new();
    write_commitment_tree(tree, &mut out).expect("writing to a Vec");
    hex::encode(out)
}

fn tree_from_hex(s: &str) -> Result<Tree, String> {
    if s.is_empty() {
        return Ok(Tree::empty());
    }
    let bytes = hex::decode(s).map_err(|e| e.to_string())?;
    read_commitment_tree::<MerkleHashOrchard, _, DEPTH>(&bytes[..]).map_err(|e| e.to_string())
}

fn witness_hex(w: &Witness) -> String {
    let mut out = Vec::new();
    write_incremental_witness(w, &mut out).expect("writing to a Vec");
    hex::encode(out)
}

fn witness_from_hex(s: &str) -> Result<Witness, String> {
    let bytes = hex::decode(s).map_err(|e| e.to_string())?;
    read_incremental_witness::<MerkleHashOrchard, _, DEPTH>(&bytes[..]).map_err(|e| e.to_string())
}

fn display_txid(internal: &[u8]) -> String {
    let mut b = internal.to_vec();
    b.reverse();
    hex::encode(b)
}

impl NoteRecord {
    pub(crate) fn note(&self) -> Result<Note, String> {
        let recipient: [u8; 43] = hex::decode(&self.recipient)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or("bad note recipient")?;
        let recipient =
            Option::from(orchard::Address::from_raw_address_bytes(&recipient)).ok_or("bad note")?;
        let rho = hex32(&self.rho)?;
        let rho: Rho = Option::from(Rho::from_bytes(&rho)).ok_or("bad note rho")?;
        let rseed: RandomSeed =
            Option::from(RandomSeed::from_bytes(hex32(&self.rseed)?, &rho)).ok_or("bad rseed")?;
        let version = match self.note_version {
            2 => orchard::note::NoteVersion::V2,
            3 => orchard::note::NoteVersion::V3,
            v => return Err(format!("unknown note version {v}")),
        };
        Option::from(Note::from_parts(
            recipient,
            NoteValue::from_raw(self.value_zat),
            rho,
            rseed,
            version,
        ))
        .ok_or_else(|| "bad note".to_owned())
    }
}

fn hex32(s: &str) -> Result<[u8; 32], String> {
    hex::decode(s)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| "expected 32 bytes of hex".to_owned())
}

impl PoolRuntime {
    fn from_state(state: &PoolState) -> Result<Self, String> {
        Ok(Self {
            tree: tree_from_hex(&state.tree)?,
            notes: state
                .notes
                .iter()
                .map(|n| Ok((n.clone(), witness_from_hex(&n.witness)?)))
                .collect::<Result<_, String>>()?,
        })
    }

    fn to_state(&self) -> PoolState {
        PoolState {
            tree: tree_hex(&self.tree),
            notes: self
                .notes
                .iter()
                .map(|(n, w)| NoteRecord {
                    witness: witness_hex(w),
                    ..n.clone()
                })
                .collect(),
        }
    }
}

impl Wallet {
    /// A new wallet born at `birthday`. `tree_state` is the lightwalletd
    /// `TreeState` for block `birthday - 1`.
    pub fn new(
        keys: Keys,
        network: &str,
        birthday: u32,
        tree_state: &TreeState,
    ) -> Result<Self, String> {
        if tree_state.height + 1 != birthday as u64 {
            return Err(format!(
                "tree state is for block {}, need {}",
                tree_state.height,
                birthday.saturating_sub(1)
            ));
        }
        let orchard = tree_state.orchard_tree().map_err(|e| e.to_string())?;
        let ironwood = tree_state.ironwood_tree().map_err(|e| e.to_string())?;
        let hash = hex::decode(&tree_state.hash).map_err(|e| e.to_string())?;
        let mut scanned_hash = hash;
        scanned_hash.reverse(); // TreeState carries the display-order hash.
        Ok(Self {
            keys,
            network: network.to_owned(),
            birthday,
            birthday_trees: (tree_hex(&orchard), tree_hex(&ironwood)),
            scanned_height: birthday - 1,
            scanned_hash,
            next_address_index: 0,
            orchard: PoolRuntime {
                tree: orchard,
                notes: vec![],
            },
            ironwood: PoolRuntime {
                tree: ironwood,
                notes: vec![],
            },
            sent: vec![],
        })
    }

    pub fn from_state(keys: Keys, state: &WalletState) -> Result<Self, String> {
        if state.version != 1 {
            return Err(format!(
                "unsupported wallet state version {}",
                state.version
            ));
        }
        Ok(Self {
            keys,
            network: state.network.clone(),
            birthday: state.birthday,
            birthday_trees: (
                state.birthday_orchard_tree.clone(),
                state.birthday_ironwood_tree.clone(),
            ),
            scanned_height: state.scanned_height,
            scanned_hash: hex::decode(&state.scanned_hash).map_err(|e| e.to_string())?,
            next_address_index: state.next_address_index,
            orchard: PoolRuntime::from_state(&state.orchard)?,
            ironwood: PoolRuntime::from_state(&state.ironwood)?,
            sent: state.sent.clone(),
        })
    }

    pub fn to_state(&self) -> WalletState {
        WalletState {
            version: 1,
            network: self.network.clone(),
            birthday: self.birthday,
            birthday_orchard_tree: self.birthday_trees.0.clone(),
            birthday_ironwood_tree: self.birthday_trees.1.clone(),
            scanned_height: self.scanned_height,
            scanned_hash: hex::encode(&self.scanned_hash),
            next_address_index: self.next_address_index,
            orchard: self.orchard.to_state(),
            ironwood: self.ironwood.to_state(),
            sent: self.sent.clone(),
        }
    }

    pub fn keys(&self) -> &Keys {
        &self.keys
    }

    /// Drop the spending key: the wallet keeps syncing but can't pay.
    pub fn lock(&mut self) {
        self.keys.ask = None;
    }

    pub fn can_spend(&self) -> bool {
        self.keys.ask.is_some()
    }

    pub fn birthday(&self) -> u32 {
        self.birthday
    }

    pub fn scanned_height(&self) -> u32 {
        self.scanned_height
    }

    /// The wallet's main receiving address (index 0), for funding.
    pub fn funding_address(&self) -> String {
        self.keys.address(0)
    }

    /// A never-before-used address: one per bid, so refunds can't be linked
    /// to each other or to the funding address.
    pub fn fresh_address(&mut self) -> (u32, String) {
        self.next_address_index = self.next_address_index.max(1);
        let index = self.next_address_index;
        self.next_address_index += 1;
        (index, self.keys.address(index))
    }

    pub fn address(&self, index: u32) -> String {
        self.keys.address(index)
    }

    /// Forget everything scanned and start again from the birthday (after a reorg).
    /// Payments we sent stay listed, and are re-confirmed as their spends are seen again.
    pub fn reset(&mut self) -> Result<(), String> {
        self.orchard = PoolRuntime {
            tree: tree_from_hex(&self.birthday_trees.0)?,
            notes: vec![],
        };
        self.ironwood = PoolRuntime {
            tree: tree_from_hex(&self.birthday_trees.1)?,
            notes: vec![],
        };
        self.scanned_height = self.birthday - 1;
        self.scanned_hash.clear();
        for tx in &mut self.sent {
            tx.mined_height = None;
        }
        Ok(())
    }

    pub fn notes(&self) -> impl Iterator<Item = &NoteRecord> {
        self.orchard
            .notes
            .iter()
            .chain(self.ironwood.notes.iter())
            .map(|(n, _)| n)
    }

    pub fn sent(&self) -> &[SentTx] {
        &self.sent
    }

    fn pending_nullifiers(&self) -> Vec<&str> {
        self.sent
            .iter()
            .filter(|t| t.mined_height.is_none() && !t.expired)
            .flat_map(|t| t.spends.iter().map(String::as_str))
            .collect()
    }

    pub fn balance(&self) -> Balance {
        let pending = self.pending_nullifiers();
        let mut b = Balance::default();
        for n in self.notes().filter(|n| n.spent_txid.is_none()) {
            b.total_zat += n.value_zat;
            if n.pool == Pool::Orchard {
                b.orchard_zat += n.value_zat;
            } else if pending.contains(&n.nullifier.as_str()) {
                b.pending_spend_zat += n.value_zat;
            } else {
                b.spendable_zat += n.value_zat;
            }
        }
        b
    }

    /// Process the next compact block. Blocks must arrive in order.
    pub fn scan_block(&mut self, block: &CompactBlock) -> Result<ScanOutcome, ScanError> {
        let height = block.height as u32;
        if height != self.scanned_height + 1 {
            return Err(ScanError::OutOfOrder {
                expected: self.scanned_height + 1,
                got: height,
            });
        }
        if !self.scanned_hash.is_empty() && block.prev_hash != self.scanned_hash {
            return Err(ScanError::Reorg);
        }
        let fvk = &self.keys.fvk;
        let ivks = [
            (
                Scope::External,
                PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::External)),
            ),
            (
                Scope::Internal,
                PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::Internal)),
            ),
        ];
        let external_ivk = fvk.to_ivk(Scope::External);
        let mut outcome = ScanOutcome {
            height,
            ..Default::default()
        };

        for tx in &block.vtx {
            let txid = display_txid(&tx.txid);
            // Spends of our notes, from either pool's bundle.
            for action in tx.actions.iter().chain(tx.ironwood_actions.iter()) {
                let nf = hex::encode(&action.nullifier);
                for (note, _) in self
                    .orchard
                    .notes
                    .iter_mut()
                    .chain(self.ironwood.notes.iter_mut())
                {
                    if note.nullifier == nf && note.spent_txid.is_none() {
                        note.spent_txid = Some(txid.clone());
                        note.spent_height = Some(height);
                        outcome.spent.push(nf.clone());
                    }
                }
            }
            for (pool, actions) in [
                (Pool::Orchard, &tx.actions),
                (Pool::Ironwood, &tx.ironwood_actions),
            ] {
                let runtime = match pool {
                    Pool::Orchard => &mut self.orchard,
                    Pool::Ironwood => &mut self.ironwood,
                };
                for (index, action) in actions.iter().enumerate() {
                    // Every commitment goes into the tree and every live witness.
                    let cmx: [u8; 32] = action
                        .cmx
                        .as_slice()
                        .try_into()
                        .map_err(|_| ScanError::Bad("bad cmx".into()))?;
                    let cmx = Option::from(ExtractedNoteCommitment::from_bytes(&cmx))
                        .ok_or_else(|| ScanError::Bad("bad cmx".into()))?;
                    let node = MerkleHashOrchard::from_cmx(&cmx);
                    for (_, w) in runtime.notes.iter_mut() {
                        w.append(node)
                            .map_err(|_| ScanError::Bad("tree full".into()))?;
                    }
                    runtime
                        .tree
                        .append(node)
                        .map_err(|_| ScanError::Bad("tree full".into()))?;

                    // Ours?
                    let decrypted = match pool {
                        Pool::Orchard => try_decrypt::<OrchardVersion>(action, &ivks),
                        Pool::Ironwood => try_decrypt::<IronwoodVersion>(action, &ivks),
                    };
                    if let Some((note, addr, scope)) = decrypted {
                        let witness = Witness::from_tree(runtime.tree.clone())
                            .ok_or_else(|| ScanError::Bad("empty tree".into()))?;
                        let address_index = (scope == Scope::External)
                            .then(|| external_ivk.diversifier_index(&addr))
                            .flatten()
                            .and_then(|d| u32::try_from(d).ok());
                        let record = NoteRecord {
                            pool,
                            txid: txid.clone(),
                            height,
                            action_index: index as u32,
                            value_zat: note.value().inner(),
                            address_index,
                            change: scope == Scope::Internal,
                            recipient: hex::encode(note.recipient().to_raw_address_bytes()),
                            rho: hex::encode(note.rho().to_bytes()),
                            rseed: hex::encode(note.rseed().as_bytes()),
                            note_version: match note.version() {
                                orchard::note::NoteVersion::V2 => 2,
                                orchard::note::NoteVersion::V3 => 3,
                            },
                            nullifier: hex::encode(note.nullifier(fvk).to_bytes()),
                            witness: String::new(),
                            spent_txid: None,
                            spent_height: None,
                        };
                        if let Some(index) = address_index {
                            // An address handed out elsewhere (another device) moves the counter on.
                            self.next_address_index = self.next_address_index.max(index + 1);
                        }
                        outcome.received.push(record.clone());
                        runtime.notes.push((record, witness));
                    }
                }
            }
        }

        // Our own payments confirm when their spends appear.
        for tx in &mut self.sent {
            if tx.mined_height.is_none() && tx.spends.iter().any(|nf| outcome.spent.contains(nf)) {
                tx.mined_height = Some(height);
                tx.expired = false;
                outcome.confirmed.push(tx.txid.clone());
            }
        }
        if let Some(meta) = &block.chain_metadata {
            if meta.orchard_commitment_tree_size != 0
                && self.orchard.tree.size() as u32 != meta.orchard_commitment_tree_size
            {
                return Err(ScanError::TreeMismatch(Pool::Orchard));
            }
            if meta.ironwood_commitment_tree_size != 0
                && self.ironwood.tree.size() as u32 != meta.ironwood_commitment_tree_size
            {
                return Err(ScanError::TreeMismatch(Pool::Ironwood));
            }
        }
        // Payments that can no longer be mined release their notes.
        for tx in &mut self.sent {
            if tx.mined_height.is_none() && height > tx.expiry_height {
                tx.expired = true;
            }
        }
        self.scanned_height = height;
        self.scanned_hash = block.hash.clone();
        Ok(outcome)
    }
}

fn try_decrypt<V: DomainVersion>(
    action: &zcash_client_backend::proto::compact_formats::CompactOrchardAction,
    ivks: &[(Scope, PreparedIncomingViewingKey); 2],
) -> Option<(Note, orchard::Address, Scope)> {
    let action = CompactAction::try_from(action).ok()?;
    let domain = NoteEncryptionDomain::<V>::for_compact_action(&action);
    ivks.iter().find_map(|(scope, ivk)| {
        try_compact_note_decryption(&domain, ivk, &action).map(|(n, a)| (n, a, *scope))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zcash_client_backend::proto::compact_formats::ChainMetadata;

    fn wallet() -> Wallet {
        let phrase = phrase_from_entropy([3; 32]).unwrap();
        let keys = Keys::from_phrase(ChainParams::Testnet, &phrase).unwrap();
        // An empty commitment tree just before block 100 (no notes yet).
        let tree = TreeState {
            network: "test".into(),
            height: 99,
            hash: hex::encode([9u8; 32]),
            ..Default::default()
        };
        Wallet::new(keys, "testnet", 100, &tree).unwrap()
    }

    fn block(height: u64, prev: Vec<u8>, hash: u8) -> CompactBlock {
        CompactBlock {
            height,
            hash: vec![hash; 32],
            prev_hash: prev,
            ..Default::default()
        }
    }

    #[test]
    fn scanning_follows_the_chain_and_refuses_gaps_and_reorgs() {
        let mut w = wallet();
        // The tree state's hash is display order; the block's prev_hash is internal order.
        let mut prev = [9u8; 32].to_vec();
        prev.reverse();
        w.scan_block(&block(100, prev, 1)).unwrap();
        assert_eq!(w.scanned_height(), 100);
        assert_eq!(
            w.scan_block(&block(102, vec![1; 32], 2)).unwrap_err(),
            ScanError::OutOfOrder {
                expected: 101,
                got: 102
            }
        );
        assert_eq!(
            w.scan_block(&block(101, vec![7; 32], 2)).unwrap_err(),
            ScanError::Reorg
        );
        // A block claiming a bigger Ironwood tree than we built: never spend on that.
        let mut b = block(101, vec![1; 32], 2);
        b.chain_metadata = Some(ChainMetadata {
            ironwood_commitment_tree_size: 5,
            ..Default::default()
        });
        assert_eq!(
            w.scan_block(&b).unwrap_err(),
            ScanError::TreeMismatch(Pool::Ironwood)
        );
        // After a reset it starts again from the birthday.
        w.reset().unwrap();
        assert_eq!(w.scanned_height(), 99);
    }

    #[test]
    fn payments_are_refused_without_funds_keys_or_a_valid_recipient() {
        let mut w = wallet();
        let to = w.address(5);
        assert!(w
            .quote(10_000)
            .unwrap_err()
            .starts_with("Not enough spendable funds"));
        let bad = w.pay(&PaymentRequest {
            to: "u1notanaddress".into(),
            amount_zat: 1,
            memo: None,
            label: None,
        });
        assert!(bad.unwrap_err().contains("isn't a testnet Unified Address"));
        let zero = w.pay(&PaymentRequest {
            to: to.clone(),
            amount_zat: 0,
            memo: None,
            label: None,
        });
        assert_eq!(zero.unwrap_err(), "Enter an amount above zero.");
        let broke = w.pay(&PaymentRequest {
            to: to.clone(),
            amount_zat: 1,
            memo: None,
            label: None,
        });
        assert!(broke.unwrap_err().starts_with("Not enough spendable funds"));
        w.lock();
        let locked = w.pay(&PaymentRequest {
            to,
            amount_zat: 1,
            memo: None,
            label: None,
        });
        assert_eq!(locked.unwrap_err(), "Unlock the wallet to pay.");
        assert!(w.sent().is_empty(), "nothing is recorded as sent");
    }

    #[test]
    fn fresh_addresses_never_repeat_and_state_round_trips() {
        let mut w = wallet();
        let (i1, a1) = w.fresh_address();
        let (i2, a2) = w.fresh_address();
        assert!(i1 >= 1 && i2 == i1 + 1, "index 0 is the funding address");
        assert_ne!(a1, a2);
        assert_ne!(a1, w.funding_address());
        let state = w.to_state();
        let json = serde_json::to_string(&state).unwrap();
        let back: WalletState = serde_json::from_str(&json).unwrap();
        let phrase = phrase_from_entropy([3; 32]).unwrap();
        let keys = Keys::from_phrase(ChainParams::Testnet, &phrase).unwrap();
        let mut w2 = Wallet::from_state(keys, &back).unwrap();
        assert_eq!(
            w2.fresh_address().0,
            i2 + 1,
            "the counter survives a reload"
        );
    }
}

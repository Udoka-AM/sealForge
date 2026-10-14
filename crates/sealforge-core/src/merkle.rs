//! Bid Merkle tree and inclusion proofs (Milestone 3, task 3.2).
//!
//! - Leaf = BLAKE2b-256(personal = "sealforge_leaf1", txid ‖ output_index_le32 ‖ amount_le64 ‖ commitment).
//! - Leaves are sorted ascending and must be unique, so the root does not
//!   depend on scan order.
//! - The tree is padded to a power of two with all-zero leaves.
//!   Node = BLAKE2b-256(personal = "sealforge_node1", left ‖ right).
//! - Published root = BLAKE2b-256(personal = "sealforge_root1", leaf_count_le64 ‖ tree_root).
//!   Binding the leaf count means padding can never be passed off as a bid.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// BLAKE2b personalisation for leaves.
pub const LEAF_PERSONAL: &[u8; 16] = b"sealforge_leaf1\0";
/// BLAKE2b personalisation for internal nodes.
pub const NODE_PERSONAL: &[u8; 16] = b"sealforge_node1\0";
/// BLAKE2b personalisation for the published root.
pub const ROOT_PERSONAL: &[u8; 16] = b"sealforge_root1\0";

/// Padding leaf.
const EMPTY: [u8; 32] = [0; 32];

pub type Hash = [u8; 32];

fn blake2b(personal: &[u8; 16], parts: &[&[u8]]) -> Hash {
    let mut state = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(personal)
        .to_state();
    for part in parts {
        state.update(part);
    }
    state
        .finalize()
        .as_bytes()
        .try_into()
        .expect("32-byte hash")
}

/// Hash a bid into a Merkle leaf.
pub fn leaf_hash(
    txid: &[u8; 32],
    output_index: u32,
    amount_zat: u64,
    commitment: &[u8; 32],
) -> Hash {
    blake2b(
        LEAF_PERSONAL,
        &[
            txid,
            &output_index.to_le_bytes(),
            &amount_zat.to_le_bytes(),
            commitment,
        ],
    )
}

fn node_hash(left: &Hash, right: &Hash) -> Hash {
    blake2b(NODE_PERSONAL, &[left, right])
}

fn published_root(leaf_count: u64, tree_root: &Hash) -> Hash {
    blake2b(ROOT_PERSONAL, &[&leaf_count.to_le_bytes(), tree_root])
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MerkleError {
    #[error("duplicate leaf {0}")]
    DuplicateLeaf(String),
    #[error("leaf is not in the tree")]
    UnknownLeaf,
}

/// Inclusion proof for one bid, as stored in a receipt check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InclusionProof {
    pub leaf_count: u64,
    /// Position of the leaf in the sorted leaf list.
    pub index: u64,
    /// Sibling hashes from the leaf level up to (excluding) the tree root.
    #[serde(with = "hex_hashes")]
    pub siblings: Vec<Hash>,
}

#[derive(Clone, Debug)]
pub struct MerkleTree {
    leaf_count: u64,
    /// `levels[0]` holds the padded leaves, the last level holds the tree root.
    levels: Vec<Vec<Hash>>,
}

impl MerkleTree {
    /// Build a tree from leaf hashes in any order.
    pub fn from_leaves(mut leaves: Vec<Hash>) -> Result<Self, MerkleError> {
        leaves.sort_unstable();
        if let Some(pair) = leaves.windows(2).find(|w| w[0] == w[1]) {
            return Err(MerkleError::DuplicateLeaf(hex::encode(pair[0])));
        }
        let leaf_count = leaves.len() as u64;
        let width = leaves.len().max(1).next_power_of_two();
        leaves.resize(width, EMPTY);

        let mut levels = vec![leaves];
        while levels.last().expect("non-empty").len() > 1 {
            let next = levels
                .last()
                .expect("non-empty")
                .chunks(2)
                .map(|pair| node_hash(&pair[0], &pair[1]))
                .collect();
            levels.push(next);
        }
        Ok(Self { leaf_count, levels })
    }

    pub fn leaf_count(&self) -> u64 {
        self.leaf_count
    }

    /// The root that gets published and recomputed by the auditor.
    pub fn root(&self) -> Hash {
        let tree_root = self.levels.last().expect("non-empty")[0];
        published_root(self.leaf_count, &tree_root)
    }

    /// Inclusion proof for a leaf.
    pub fn proof(&self, leaf: &Hash) -> Result<InclusionProof, MerkleError> {
        let leaves = &self.levels[0][..self.leaf_count as usize];
        let mut index = leaves
            .binary_search(leaf)
            .map_err(|_| MerkleError::UnknownLeaf)?;
        let leaf_index = index as u64;
        let mut siblings = Vec::with_capacity(self.levels.len() - 1);
        for level in &self.levels[..self.levels.len() - 1] {
            siblings.push(level[index ^ 1]);
            index /= 2;
        }
        Ok(InclusionProof {
            leaf_count: self.leaf_count,
            index: leaf_index,
            siblings,
        })
    }
}

/// Check that `leaf` is included under the published `root`.
pub fn verify_proof(root: &Hash, leaf: &Hash, proof: &InclusionProof) -> bool {
    if proof.index >= proof.leaf_count {
        return false;
    }
    let width = proof.leaf_count.max(1).next_power_of_two();
    if proof.siblings.len() != width.trailing_zeros() as usize {
        return false;
    }
    let mut index = proof.index;
    let mut acc = *leaf;
    for sibling in &proof.siblings {
        acc = if index % 2 == 0 {
            node_hash(&acc, sibling)
        } else {
            node_hash(sibling, &acc)
        };
        index /= 2;
    }
    published_root(proof.leaf_count, &acc) == *root
}

mod hex_hashes {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::Hash;

    pub fn serialize<S: Serializer>(v: &[Hash], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(hex::encode))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Hash>, D::Error> {
        Vec::<String>::deserialize(d)?
            .iter()
            .map(|s| {
                hex::decode(s)
                    .ok()
                    .and_then(|b| b.try_into().ok())
                    .ok_or_else(|| serde::de::Error::custom("expected 32-byte hex"))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(n: u32) -> Vec<Hash> {
        (0..n)
            .map(|i| leaf_hash(&[i as u8; 32], i, u64::from(i) * 1_000, &[0xcc; 32]))
            .collect()
    }

    #[test]
    fn every_leaf_has_a_valid_proof() {
        for n in [1, 2, 3, 5, 8, 13, 64, 100] {
            let ls = leaves(n);
            let tree = MerkleTree::from_leaves(ls.clone()).unwrap();
            let root = tree.root();
            for leaf in &ls {
                let proof = tree.proof(leaf).unwrap();
                assert!(verify_proof(&root, leaf, &proof), "n={n}");
            }
        }
    }

    #[test]
    fn root_ignores_input_order() {
        let mut ls = leaves(37);
        let a = MerkleTree::from_leaves(ls.clone()).unwrap().root();
        ls.reverse();
        assert_eq!(a, MerkleTree::from_leaves(ls).unwrap().root());
    }

    #[test]
    fn tampering_fails() {
        let ls = leaves(9);
        let tree = MerkleTree::from_leaves(ls.clone()).unwrap();
        let root = tree.root();
        let mut proof = tree.proof(&ls[3]).unwrap();

        assert!(!verify_proof(&root, &[0xab; 32], &proof), "foreign leaf");

        proof.siblings[0][0] ^= 1;
        assert!(!verify_proof(&root, &ls[3], &proof), "tampered sibling");
        proof.siblings[0][0] ^= 1;

        proof.leaf_count += 1;
        assert!(!verify_proof(&root, &ls[3], &proof), "wrong leaf count");
        proof.leaf_count -= 1;

        proof.index = proof.leaf_count;
        assert!(!verify_proof(&root, &ls[3], &proof), "index out of range");
    }

    #[test]
    fn padding_cannot_be_proven_as_a_bid() {
        let tree = MerkleTree::from_leaves(leaves(3)).unwrap();
        // Width 4: position 3 is padding. A forged proof for it must fail.
        let real = tree.proof(&tree.levels[0][2]).unwrap();
        let forged = InclusionProof {
            leaf_count: 3,
            index: 3,
            siblings: vec![tree.levels[0][2], real.siblings[1]],
        };
        assert!(!verify_proof(&tree.root(), &EMPTY, &forged));
    }

    #[test]
    fn empty_tree_and_errors() {
        let empty = MerkleTree::from_leaves(vec![]).unwrap();
        assert_eq!(empty.leaf_count(), 0);
        assert_eq!(empty.proof(&[1; 32]), Err(MerkleError::UnknownLeaf));
        assert!(matches!(
            MerkleTree::from_leaves(vec![[1; 32], [1; 32]]),
            Err(MerkleError::DuplicateLeaf(_))
        ));
    }

    #[test]
    fn proof_serializes_as_hex() {
        let ls = leaves(4);
        let tree = MerkleTree::from_leaves(ls.clone()).unwrap();
        let proof = tree.proof(&ls[0]).unwrap();
        let json = serde_json::to_string(&proof).unwrap();
        assert_eq!(
            serde_json::from_str::<InclusionProof>(&json).unwrap(),
            proof
        );
    }
}

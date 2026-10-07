//! Bid Merkle tree and inclusion proofs (Milestone 3, task 3.2).
//!
//! Leaf = BLAKE2b-256(personal = "sealforge_leaf1", txid ‖ output_index_le32 ‖ amount_le64 ‖ commitment).
//! Leaves are sorted ascending before the tree is built, so the root does not
//! depend on scan order.

/// BLAKE2b personalisation for leaves (16 bytes max).
pub const LEAF_PERSONAL: &[u8; 16] = b"sealforge_leaf1\0";

/// BLAKE2b personalisation for internal nodes.
pub const NODE_PERSONAL: &[u8; 16] = b"sealforge_node1\0";

/// Hash a bid into a Merkle leaf.
pub fn leaf_hash(
    txid: &[u8; 32],
    output_index: u32,
    amount_zat: u64,
    commitment: &[u8; 32],
) -> [u8; 32] {
    let hash = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(LEAF_PERSONAL)
        .to_state()
        .update(txid)
        .update(&output_index.to_le_bytes())
        .update(&amount_zat.to_le_bytes())
        .update(commitment)
        .finalize();
    hash.as_bytes().try_into().expect("32-byte hash")
}

// TODO(3.2): `MerkleTree::from_leaves`, `root()`, `proof(index)`, `verify_proof(...)`.

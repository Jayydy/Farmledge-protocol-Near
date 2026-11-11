use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};

use crate::errors::ContractError;
use crate::types::{LeafInput, WeightSource};

pub const LEAF_DOMAIN: [u8; 20] = *b"FARMLEDGE_LEAF_V3\0\0\0";
pub const ZERO_HEAD: [u8; 32] = [0u8; 32];

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Eq, PartialEq)]
pub struct LeafHashPreimage {
    pub domain: [u8; 20],
    pub version: u32,
    pub kind: u8,
    pub warehouse_id: String,
    pub day: u32,
    pub index: u32,
    pub deposit_no: u64,
    pub deposit_id: String,
    pub receipt_id: u64,
    pub receipt_version: u32,
    pub owner: String,
    pub commodity: String,
    pub grade: String,
    pub kg: i128,
    pub weight_source: u8, // 0 = BagEstimate, 1 = Scale
    pub bags: u32,
    pub moisture_bps: u32,
    pub custodian: String,
    pub verifier: String,
    pub deposited_at: u64,
    pub at: u64,
    pub salt: [u8; 32],
}

impl LeafHashPreimage {
    pub fn from_input(leaf: &LeafInput) -> Result<Self, ContractError> {
        let salt_bytes = hex::decode(&leaf.salt).map_err(|_| ContractError::InvalidProof)?;
        if salt_bytes.len() != 32 {
            return Err(ContractError::InvalidProof);
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&salt_bytes);

        let ws_discriminant = match leaf.weight_source {
            WeightSource::BagEstimate => 0,
            WeightSource::Scale => 1,
        };

        Ok(Self {
            domain: LEAF_DOMAIN,
            version: leaf.version,
            kind: leaf.kind,
            warehouse_id: leaf.warehouse_id.clone(),
            day: leaf.day,
            index: leaf.index,
            deposit_no: leaf.deposit_no,
            deposit_id: leaf.deposit_id.clone(),
            receipt_id: leaf.receipt_id,
            receipt_version: leaf.receipt_version,
            owner: leaf.owner.clone(),
            commodity: leaf.commodity.clone(),
            grade: leaf.grade.clone(),
            kg: leaf.kg,
            weight_source: ws_discriminant,
            bags: leaf.bags,
            moisture_bps: leaf.moisture_bps,
            custodian: leaf.custodian.clone(),
            verifier: leaf.verifier.clone(),
            deposited_at: leaf.deposited_at,
            at: leaf.at,
            salt,
        })
    }
}

/// Computes Borsh bytes of `LeafHashPreimage`.
pub fn leaf_borsh_bytes(leaf: &LeafInput) -> Result<Vec<u8>, ContractError> {
    let preimage = LeafHashPreimage::from_input(leaf)?;
    borsh::to_vec(&preimage).map_err(|_| ContractError::CorruptRecord)
}

/// Computes `sha256(0x00 || borsh(LeafHashPreimage))`
pub fn compute_leaf_hash(leaf: &LeafInput) -> Result<[u8; 32], ContractError> {
    let bytes = leaf_borsh_bytes(leaf)?;
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(&bytes);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    Ok(out)
}

/// Computes intermediate node hash: `sha256(0x01 || left || right)`
pub fn hash_nodes(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Calculates required Merkle tree depth for `leaves` count.
pub fn depth_for(leaves: u32) -> usize {
    if leaves <= 1 {
        0
    } else {
        (32 - (leaves - 1).leading_zeros()) as usize
    }
}

/// Folds a Merkle proof against a leaf hash and leaf index.
pub fn fold_proof(
    leaf_hash: &[u8; 32],
    index: u32,
    proof: &[[u8; 32]],
) -> [u8; 32] {
    let mut current = *leaf_hash;
    for (i, sibling) in proof.iter().enumerate() {
        let bit = (index >> i) & 1;
        if bit == 0 {
            current = hash_nodes(&current, sibling);
        } else {
            current = hash_nodes(sibling, &current);
        }
    }
    current
}

/// Computes next head in anchor hash chain:
/// `sha256(prev_head || u32_be(day) || u32_be(leaves) || root)`
pub fn compute_head(
    prev_head: &[u8; 32],
    day: u32,
    leaves: u32,
    root: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(prev_head);
    hasher.update(day.to_be_bytes());
    hasher.update(leaves.to_be_bytes());
    hasher.update(root);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

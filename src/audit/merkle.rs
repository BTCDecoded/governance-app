//! Merkle Tree for Audit Logs
//!
//! Implements Merkle tree construction and verification for audit log entries
//! to enable efficient anchoring of large audit logs to Bitcoin.

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use tracing::{debug, info};

use crate::audit::entry::AuditLogEntry;

/// Merkle tree node
#[derive(Debug, Clone)]
pub struct MerkleNode {
    pub hash: String,
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
}

impl MerkleNode {
    /// Create leaf node from audit entry
    pub fn leaf(entry: &AuditLogEntry) -> Self {
        Self {
            hash: entry.this_log_hash.clone(),
            left: None,
            right: None,
        }
    }

    /// Create internal node from two child nodes
    pub fn internal(left: MerkleNode, right: MerkleNode) -> Self {
        let combined = format!("{}{}", left.hash, right.hash);
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let hash = format!("sha256:{}", hex::encode(hasher.finalize()));

        Self {
            hash,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }

    /// Create single child node (for odd number of entries)
    pub fn single_child(child: MerkleNode) -> Self {
        // For odd number of entries, duplicate the last entry
        let combined = format!("{}{}", child.hash, child.hash);
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let hash = format!("sha256:{}", hex::encode(hasher.finalize()));

        Self {
            hash,
            left: Some(Box::new(child.clone())),
            right: Some(Box::new(child)),
        }
    }
}

/// Build Merkle tree from audit log entries
pub fn build_merkle_tree(entries: &[AuditLogEntry]) -> Result<MerkleNode> {
    if entries.is_empty() {
        return Err(anyhow!("Cannot build Merkle tree from empty entries"));
    }

    info!("Building Merkle tree from {} entries", entries.len());

    // Create leaf nodes
    let mut nodes: VecDeque<MerkleNode> = entries.iter().map(MerkleNode::leaf).collect();

    // Build tree bottom-up
    while nodes.len() > 1 {
        let mut next_level = VecDeque::new();

        while !nodes.is_empty() {
            let left = nodes.pop_front().unwrap();

            if let Some(right) = nodes.pop_front() {
                // Two nodes - create internal node
                next_level.push_back(MerkleNode::internal(left, right));
            } else {
                // One node left - create single child node
                next_level.push_back(MerkleNode::single_child(left));
            }
        }

        nodes = next_level;
    }

    let root = nodes.pop_front().unwrap();
    debug!("Merkle tree root: {}", root.hash);
    Ok(root)
}

/// Get Merkle root hash
pub fn get_merkle_root(entries: &[AuditLogEntry]) -> Result<String> {
    let tree = build_merkle_tree(entries)?;
    Ok(tree.hash)
}

/// Verify Merkle root against entries
pub fn verify_merkle_root(entries: &[AuditLogEntry], claimed_root: &str) -> Result<bool> {
    let actual_root = get_merkle_root(entries)?;
    Ok(actual_root == claimed_root)
}

/// Generate Merkle proof for a specific entry
pub fn generate_merkle_proof(entries: &[AuditLogEntry], entry_index: usize) -> Result<MerkleProof> {
    if entry_index >= entries.len() {
        return Err(anyhow!("Entry index out of range"));
    }

    // Build tree and collect proof hashes by traversing from leaf to root
    let tree = build_merkle_tree(entries)?;
    let mut proof_hashes = Vec::new();

    // Rebuild tree structure to find path
    let current_index = entry_index;
    let current_level = entries.len();

    // Build levels bottom-up to track path
    let mut levels: Vec<Vec<String>> =
        vec![entries.iter().map(|e| e.this_log_hash.clone()).collect()];
    let mut current_entries = entries.len();

    while current_entries > 1 {
        let mut next_level = Vec::new();
        let mut i = 0;
        while i < current_entries {
            if i + 1 < current_entries {
                // Two entries - combine them
                let combined = format!(
                    "{}{}",
                    levels.last().unwrap()[i],
                    levels.last().unwrap()[i + 1]
                );
                let mut hasher = Sha256::new();
                hasher.update(combined.as_bytes());
                next_level.push(format!("sha256:{}", hex::encode(hasher.finalize())));
                i += 2;
            } else {
                // One entry left - duplicate it
                let combined =
                    format!("{}{}", levels.last().unwrap()[i], levels.last().unwrap()[i]);
                let mut hasher = Sha256::new();
                hasher.update(combined.as_bytes());
                next_level.push(format!("sha256:{}", hex::encode(hasher.finalize())));
                i += 1;
            }
        }
        levels.push(next_level);
        current_entries = levels.last().unwrap().len();
    }

    // Now traverse from leaf to root to collect proof hashes with order info
    let mut idx = entry_index;
    let mut proof_order = Vec::new();
    for level in 0..levels.len() - 1 {
        let level_size = levels[level].len();
        let is_left = idx.is_multiple_of(2);

        if is_left && idx + 1 < level_size {
            // We're on the left, add right sibling (current_hash + proof_hash)
            proof_hashes.push(levels[level][idx + 1].clone());
            proof_order.push(true); // current is on left
        } else if !is_left {
            // We're on the right, add left sibling (proof_hash + current_hash)
            proof_hashes.push(levels[level][idx - 1].clone());
            proof_order.push(false); // current is on right
        } else if is_left && idx + 1 >= level_size {
            // Odd number, duplicate last entry (current_hash + current_hash)
            proof_hashes.push(levels[level][idx].clone());
            proof_order.push(true); // current is on left
        }

        idx /= 2;
    }

    Ok(MerkleProof {
        leaf_hash: entries[entry_index].this_log_hash.clone(),
        proof_hashes,
        root_hash: tree.hash,
        proof_order,
    })
}

/// Verify Merkle proof
pub fn verify_merkle_proof(proof: &MerkleProof, leaf_hash: &str, root_hash: &str) -> bool {
    let mut current_hash = leaf_hash.to_string();

    for (i, proof_hash) in proof.proof_hashes.iter().enumerate() {
        // Use the order information if available
        let is_left = proof.proof_order.get(i).copied().unwrap_or(true);

        let combined = if is_left {
            // Current hash is on left: (current_hash, proof_hash)
            format!("{current_hash}{proof_hash}")
        } else {
            // Current hash is on right: (proof_hash, current_hash)
            format!("{proof_hash}{current_hash}")
        };

        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        current_hash = format!("sha256:{}", hex::encode(hasher.finalize()));
    }

    current_hash == root_hash
}

/// Merkle proof structure
#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub leaf_hash: String,
    pub proof_hashes: Vec<String>,
    pub root_hash: String,
    /// Track whether each proof hash is on the left (true) or right (false) side
    /// When true: combine as (current_hash, proof_hash)
    /// When false: combine as (proof_hash, current_hash)
    pub proof_order: Vec<bool>,
}

impl MerkleProof {
    /// Verify this proof
    pub fn verify(&self) -> bool {
        verify_merkle_proof(self, &self.leaf_hash, &self.root_hash)
    }

    /// Get proof size
    pub fn size(&self) -> usize {
        self.proof_hashes.len()
    }

    /// Get human-readable representation
    pub fn summary(&self) -> String {
        format!(
            "Merkle proof: {} hashes, root: {}",
            self.proof_hashes.len(),
            self.root_hash
        )
    }
}

/// Calculate Merkle root for a month's audit logs
pub fn calculate_monthly_merkle_root(
    entries: &[AuditLogEntry],
    month: &str,
) -> Result<MonthlyMerkleRoot> {
    let root = get_merkle_root(entries)?;
    let entry_count = entries.len();

    let first_entry = entries
        .first()
        .ok_or_else(|| anyhow!("No entries for month {}", month))?;
    let last_entry = entries
        .last()
        .ok_or_else(|| anyhow!("No entries for month {}", month))?;

    Ok(MonthlyMerkleRoot {
        month: month.to_string(),
        entry_count,
        first_entry_hash: first_entry.this_log_hash.clone(),
        last_entry_hash: last_entry.this_log_hash.clone(),
        merkle_root: root,
    })
}

/// Monthly Merkle root information
#[derive(Debug, Clone)]
pub struct MonthlyMerkleRoot {
    pub month: String,
    pub entry_count: usize,
    pub first_entry_hash: String,
    pub last_entry_hash: String,
    pub merkle_root: String,
}

impl MonthlyMerkleRoot {
    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Month {}: {} entries, root: {}",
            self.month, self.entry_count, self.merkle_root
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_entries(count: usize) -> Vec<AuditLogEntry> {
        let mut entries = Vec::new();

        // Create genesis entry
        let genesis = crate::audit::entry::create_genesis_entry("test".to_string());
        entries.push(genesis);

        // Create test entries
        for i in 1..count {
            let mut metadata = HashMap::new();
            metadata.insert("index".to_string(), i.to_string());

            let entry = AuditLogEntry::new(
                format!("job-{}", i),
                "test_type".to_string(),
                "test".to_string(),
                format!("sha256:input{}", i),
                format!("sha256:output{}", i),
                entries[i - 1].this_log_hash.clone(),
                metadata,
            );
            entries.push(entry);
        }

        entries
    }

    #[test]
    fn test_merkle_tree_construction() {
        let entries = create_test_entries(4);
        let tree = build_merkle_tree(&entries).unwrap();

        assert!(!tree.hash.is_empty());
        assert!(tree.hash.starts_with("sha256:"));
    }

    #[test]
    fn test_merkle_root_verification() {
        let entries = create_test_entries(8);
        let root = get_merkle_root(&entries).unwrap();

        assert!(verify_merkle_root(&entries, &root).unwrap());
        assert!(!verify_merkle_root(&entries, "sha256:invalid").unwrap());
    }

    #[test]
    fn test_merkle_proof_generation() {
        let entries = create_test_entries(8);
        let proof = generate_merkle_proof(&entries, 0).unwrap();

        assert_eq!(proof.leaf_hash, entries[0].this_log_hash);
        assert!(!proof.proof_hashes.is_empty());
        assert!(proof.verify());
    }

    #[test]
    fn test_merkle_proof_verification() {
        let entries = create_test_entries(4);
        let proof = generate_merkle_proof(&entries, 1).unwrap();

        assert!(verify_merkle_proof(
            &proof,
            &entries[1].this_log_hash,
            &proof.root_hash
        ));
    }

    #[test]
    fn test_monthly_merkle_root() {
        let entries = create_test_entries(10);
        let monthly_root = calculate_monthly_merkle_root(&entries, "2025-01").unwrap();

        assert_eq!(monthly_root.month, "2025-01");
        assert_eq!(monthly_root.entry_count, 10);
        assert!(!monthly_root.merkle_root.is_empty());
    }
}

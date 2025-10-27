//! Test binary for content hash verification system
//! This tests Track 1 of the cryptographic layer synchronization implementation

use governance_app::validation::content_hash::{ContentHashValidator, FileCorrespondence, CorrespondenceType};
use std::collections::HashMap;

fn main() {
    println!("Testing Content Hash Verification System (Track 1)");
    
    // Create a content hash validator
    let mut validator = ContentHashValidator::new();
    
    // Load correspondence mappings
    let mappings = ContentHashValidator::generate_correspondence_map();
    validator.load_correspondence_mappings(mappings);
    
    println!("Loaded {} correspondence mappings", validator.correspondence_mappings.len());
    
    // Test file hash computation
    let test_content = b"test consensus rule content";
    let hash = validator.compute_file_hash(test_content);
    println!("File hash: {}", hash);
    
    // Test directory hash computation
    let test_files = vec![
        ("file1.txt".to_string(), b"content1".to_vec()),
        ("file2.txt".to_string(), b"content2".to_vec()),
    ];
    let dir_result = validator.compute_directory_hash(&test_files);
    println!("Directory hash: {}", dir_result.merkle_root);
    println!("File count: {}", dir_result.file_count);
    println!("Total size: {}", dir_result.total_size);
    
    // Test correspondence verification
    let mut orange_files = HashMap::new();
    orange_files.insert("consensus-rules/block-validation.md".to_string(), b"block validation rules".to_vec());
    
    let mut consensus_files = HashMap::new();
    consensus_files.insert("proofs/block-validation.rs".to_string(), b"proof implementation".to_vec());
    
    let result = validator.verify_correspondence(
        "consensus-rules/block-validation.md",
        &b"block validation rules"[..],
        &consensus_files,
    );
    
    match result {
        Ok(verification) => {
            println!("Correspondence verification: {}", verification.is_valid);
            println!("Source hash: {}", verification.computed_hash);
            if let Some(expected) = verification.expected_hash {
                println!("Target hash: {}", expected);
            }
        }
        Err(e) => {
            println!("Correspondence verification failed: {}", e);
        }
    }
    
    // Test bidirectional sync
    let changed_files = vec!["consensus-rules/block-validation.md".to_string()];
    let sync_result = validator.check_bidirectional_sync(&orange_files, &consensus_files, &changed_files);
    
    match sync_result {
        Ok(sync_report) => {
            println!("Sync status: {:?}", sync_report.sync_status);
            println!("Changed files: {:?}", sync_report.changed_files);
            println!("Missing files: {:?}", sync_report.missing_files);
            println!("Outdated files: {:?}", sync_report.outdated_files);
        }
        Err(e) => {
            println!("Bidirectional sync failed: {}", e);
        }
    }
    
    println!("Content Hash Verification System test completed!");
}



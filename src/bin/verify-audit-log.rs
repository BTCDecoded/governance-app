use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::path::Path;
use tracing::{info, error};

use governance_app::audit::{AuditLogger, verify_audit_log, build_merkle_tree, verify_merkle_root};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let matches = Command::new("verify-audit-log")
        .version("1.0.0")
        .about("Verify BTCDecoded governance audit log integrity")
        .arg(
            Arg::new("log-path")
                .short('l')
                .long("log-path")
                .value_name("PATH")
                .help("Path to audit log file")
                .required(true)
        )
        .arg(
            Arg::new("merkle-root")
                .short('m')
                .long("merkle-root")
                .value_name("HASH")
                .help("Expected Merkle root hash")
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress output except errors")
        )
        .get_matches();

    let log_path = matches.get_one::<String>("log-path").unwrap();
    let expected_merkle_root = matches.get_one::<String>("merkle-root");
    let verbose = matches.get_flag("verbose");
    let quiet = matches.get_flag("quiet");

    // Set log level based on flags
    if quiet {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::ERROR)
            .init();
    } else if verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    // Verify audit log
    if let Err(e) = verify_audit_log_file(log_path, expected_merkle_root, verbose).await {
        error!("Audit log verification failed: {}", e);
        std::process::exit(1);
    }

    if !quiet {
        println!("✓ Audit log verification completed successfully");
    }

    Ok(())
}

async fn verify_audit_log_file(
    log_path: &str,
    expected_merkle_root: Option<&String>,
    verbose: bool,
) -> Result<()> {
    info!("Verifying audit log: {}", log_path);

    // Check if file exists
    if !Path::new(log_path).exists() {
        return Err(anyhow!("Audit log file not found: {}", log_path));
    }

    // Load audit logger
    let logger = AuditLogger::new(log_path.to_string())?;
    
    // Load all entries
    let entries = logger.load_all_entries().await?;
    
    if entries.is_empty() {
        return Err(anyhow!("Audit log is empty"));
    }

    if verbose {
        println!("Loaded {} audit log entries", entries.len());
    }

    // Verify hash chain
    info!("Verifying hash chain integrity");
    verify_audit_log(&entries)?;
    
    if verbose {
        println!("✓ Hash chain integrity verified");
    }

    // Calculate Merkle root
    info!("Calculating Merkle root");
    let merkle_tree = build_merkle_tree(&entries)?;
    let merkle_root = merkle_tree.hash.clone();
    
    if verbose {
        println!("Merkle root: {}", merkle_root);
    }

    // Verify Merkle root if provided
    if let Some(expected) = expected_merkle_root {
        info!("Verifying Merkle root against expected value");
        if !verify_merkle_root(&entries, expected)? {
            return Err(anyhow!("Merkle root mismatch. Expected: {}, Got: {}", expected, merkle_root));
        }
        
        if verbose {
            println!("✓ Merkle root matches expected value");
        }
    }

    // Display summary
    if verbose {
        println!("\nAudit Log Summary:");
        println!("  Total entries: {}", entries.len());
        println!("  First entry: {}", entries[0].timestamp);
        println!("  Last entry: {}", entries[entries.len() - 1].timestamp);
        println!("  Merkle root: {}", merkle_root);
        println!("  Head hash: {}", entries[entries.len() - 1].this_log_hash);
    }

    // Check for common issues
    check_audit_log_health(&entries, verbose)?;

    Ok(())
}

fn check_audit_log_health(entries: &[governance_app::audit::entry::AuditLogEntry], verbose: bool) -> Result<()> {
    info!("Checking audit log health");

    // Check for duplicate job IDs
    let mut job_ids = std::collections::HashSet::new();
    for entry in entries {
        if !job_ids.insert(&entry.job_id) {
            return Err(anyhow!("Duplicate job ID found: {}", entry.job_id));
        }
    }

    if verbose {
        println!("✓ No duplicate job IDs found");
    }

    // Check timestamp ordering
    for i in 1..entries.len() {
        if entries[i].timestamp < entries[i-1].timestamp {
            return Err(anyhow!("Timestamp ordering violation at entry {}", entries[i].job_id));
        }
    }

    if verbose {
        println!("✓ Timestamps are properly ordered");
    }

    // Check for gaps in timestamps (more than 1 hour apart)
    let mut gaps = Vec::new();
    for i in 1..entries.len() {
        let gap = entries[i].timestamp - entries[i-1].timestamp;
        if gap.num_hours() > 1 {
            gaps.push((i, gap));
        }
    }

    if !gaps.is_empty() && verbose {
        println!("⚠ Found {} timestamp gaps:", gaps.len());
        for (index, gap) in gaps {
            println!("  Entry {}: {} hours gap", index, gap.num_hours());
        }
    }

    // Check for common job types
    let mut job_types = std::collections::HashMap::new();
    for entry in entries {
        *job_types.entry(&entry.job_type).or_insert(0) += 1;
    }

    if verbose {
        println!("\nJob type distribution:");
        for (job_type, count) in job_types {
            println!("  {}: {}", job_type, count);
        }
    }

    Ok(())
}

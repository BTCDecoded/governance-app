//! Economic Node Verification CLI Tool
//! 
//! This tool allows anyone to verify economic node registrations and veto signals.

use std::env;
use std::fs;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "economic-node-verify")]
#[command(about = "Verify economic node registrations and veto signals")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify a node registration
    Registration {
        /// Node name
        #[arg(short, long)]
        name: String,
        
        /// Registration file path
        #[arg(short, long)]
        file: Option<String>,
    },
    /// Verify a veto signal
    Veto {
        /// Veto file path
        #[arg(short, long)]
        file: String,
    },
    /// Verify all registrations
    AllRegistrations {
        /// Registration directory
        #[arg(short, long, default_value = "economic-registrations")]
        dir: String,
    },
    /// Verify all veto signals
    AllVetoes {
        /// Veto directory
        #[arg(short, long, default_value = "veto-signals")]
        dir: String,
    },
    /// Check governance system status
    Status {
        /// Repository name
        #[arg(short, long)]
        repo: String,
        
        /// Pull request number
        #[arg(short, long)]
        pr: u64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Registration { name, file } => {
            verify_registration(&name, file.as_deref())?;
        }
        Commands::Veto { file } => {
            verify_veto(&file)?;
        }
        Commands::AllRegistrations { dir } => {
            verify_all_registrations(&dir)?;
        }
        Commands::AllVetoes { dir } => {
            verify_all_vetoes(&dir)?;
        }
        Commands::Status { repo, pr } => {
            check_governance_status(&repo, pr)?;
        }
    }
    
    Ok(())
}

fn verify_registration(
    name: &str,
    file: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Verifying registration for node: {}", name);
    
    let file_path = file.unwrap_or(&format!("economic-registrations/{}.json", name));
    
    if !fs::metadata(file_path).is_ok() {
        println!("❌ Registration file not found: {}", file_path);
        return Ok(());
    }
    
    let content = fs::read_to_string(file_path)?;
    let registration: serde_json::Value = serde_json::from_str(&content)?;
    
    println!("📋 Registration details:");
    println!("  Name: {}", registration["name"]);
    println!("  Type: {}", registration["node_type"]);
    println!("  Active: {}", registration["active"]);
    println!("  Registered: {}", registration["registration_timestamp"]);
    
    // Validate required fields
    let mut valid = true;
    
    if registration["name"].is_null() {
        println!("❌ Missing node name");
        valid = false;
    }
    
    if registration["node_type"].is_null() {
        println!("❌ Missing node type");
        valid = false;
    }
    
    if registration["public_key"].is_null() {
        println!("❌ Missing public key");
        valid = false;
    }
    
    // Validate node type specific fields
    match registration["node_type"].as_str() {
        Some("mining_pool") => {
            if registration["hash_rate_percent"].is_null() {
                println!("❌ Mining pools must specify hash_rate_percent");
                valid = false;
            } else if let Some(percent) = registration["hash_rate_percent"].as_f64() {
                if percent <= 0.0 || percent > 100.0 {
                    println!("❌ Invalid hash rate percentage: {}", percent);
                    valid = false;
                }
            }
        }
        Some("exchange") | Some("custodian") => {
            if registration["economic_activity_percent"].is_null() {
                println!("❌ Exchanges and custodians must specify economic_activity_percent");
                valid = false;
            } else if let Some(percent) = registration["economic_activity_percent"].as_f64() {
                if percent <= 0.0 || percent > 100.0 {
                    println!("❌ Invalid economic activity percentage: {}", percent);
                    valid = false;
                }
            }
        }
        _ => {
            println!("❌ Invalid node type: {}", registration["node_type"]);
            valid = false;
        }
    }
    
    // Validate timestamp
    if let Some(timestamp) = registration["registration_timestamp"].as_str() {
        if chrono::DateTime::parse_from_rfc3339(timestamp).is_err() {
            println!("❌ Invalid timestamp format");
            valid = false;
        }
    } else {
        println!("❌ Missing registration timestamp");
        valid = false;
    }
    
    if valid {
        println!("✅ Registration is valid!");
    } else {
        println!("❌ Registration has validation errors");
    }
    
    Ok(())
}

fn verify_veto(
    file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Verifying veto signal: {}", file);
    
    if !fs::metadata(file).is_ok() {
        println!("❌ Veto file not found: {}", file);
        return Ok(());
    }
    
    let content = fs::read_to_string(file)?;
    let veto: serde_json::Value = serde_json::from_str(&content)?;
    
    println!("📋 Veto details:");
    println!("  Node: {}", veto["node_name"]);
    println!("  Repository: {}", veto["repository"]);
    println!("  PR: #{}", veto["pr_number"]);
    println!("  Reason: {}", veto["reason"]);
    println!("  Strength: {}%", veto["strength"]);
    println!("  Active: {}", veto["active"]);
    println!("  Timestamp: {}", veto["timestamp"]);
    
    // Validate required fields
    let mut valid = true;
    
    if veto["node_name"].is_null() {
        println!("❌ Missing node name");
        valid = false;
    }
    
    if veto["repository"].is_null() {
        println!("❌ Missing repository");
        valid = false;
    }
    
    if veto["pr_number"].is_null() {
        println!("❌ Missing PR number");
        valid = false;
    }
    
    if veto["reason"].is_null() {
        println!("❌ Missing veto reason");
        valid = false;
    }
    
    if let Some(strength) = veto["strength"].as_u64() {
        if strength == 0 || strength > 100 {
            println!("❌ Invalid veto strength: {}", strength);
            valid = false;
        }
    } else {
        println!("❌ Missing or invalid veto strength");
        valid = false;
    }
    
    // Validate timestamp
    if let Some(timestamp) = veto["timestamp"].as_str() {
        if chrono::DateTime::parse_from_rfc3339(timestamp).is_err() {
            println!("❌ Invalid timestamp format");
            valid = false;
        }
    } else {
        println!("❌ Missing timestamp");
        valid = false;
    }
    
    if valid {
        println!("✅ Veto signal is valid!");
    } else {
        println!("❌ Veto signal has validation errors");
    }
    
    Ok(())
}

fn verify_all_registrations(
    dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Verifying all registrations in: {}", dir);
    
    if !fs::metadata(dir).is_ok() {
        println!("❌ Directory not found: {}", dir);
        return Ok(());
    }
    
    let mut total = 0;
    let mut valid = 0;
    
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            total += 1;
            
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(registration) = serde_json::from_str::<serde_json::Value>(&content) {
                    if registration["name"].is_string() && 
                       registration["node_type"].is_string() && 
                       registration["public_key"].is_string() {
                        valid += 1;
                        println!("✅ {}", registration["name"].as_str().unwrap_or("unknown"));
                    } else {
                        println!("❌ {} (invalid format)", path.display());
                    }
                } else {
                    println!("❌ {} (invalid JSON)", path.display());
                }
            } else {
                println!("❌ {} (read error)", path.display());
            }
        }
    }
    
    println!("");
    println!("📊 Verification summary:");
    println!("  Total files: {}", total);
    println!("  Valid: {}", valid);
    println!("  Invalid: {}", total - valid);
    
    Ok(())
}

fn verify_all_vetoes(
    dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Verifying all veto signals in: {}", dir);
    
    if !fs::metadata(dir).is_ok() {
        println!("❌ Directory not found: {}", dir);
        return Ok(());
    }
    
    let mut total = 0;
    let mut valid = 0;
    let mut active = 0;
    
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            total += 1;
            
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(veto) = serde_json::from_str::<serde_json::Value>(&content) {
                    if veto["node_name"].is_string() && 
                       veto["repository"].is_string() && 
                       veto["pr_number"].is_number() {
                        valid += 1;
                        
                        if veto["active"] == true {
                            active += 1;
                            println!("✅ {} (active)", veto["node_name"].as_str().unwrap_or("unknown"));
                        } else {
                            println!("✅ {} (withdrawn)", veto["node_name"].as_str().unwrap_or("unknown"));
                        }
                    } else {
                        println!("❌ {} (invalid format)", path.display());
                    }
                } else {
                    println!("❌ {} (invalid JSON)", path.display());
                }
            } else {
                println!("❌ {} (read error)", path.display());
            }
        }
    }
    
    println!("");
    println!("📊 Verification summary:");
    println!("  Total files: {}", total);
    println!("  Valid: {}", valid);
    println!("  Active: {}", active);
    println!("  Withdrawn: {}", valid - active);
    println!("  Invalid: {}", total - valid);
    
    Ok(())
}

fn check_governance_status(
    repo: &str,
    pr: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Checking governance status for {}/{}#{}", repo, pr);
    
    // Check for veto signals
    let veto_dir = "veto-signals";
    let mut veto_count = 0;
    let mut total_strength = 0;
    
    if fs::metadata(veto_dir).is_ok() {
        for entry in fs::read_dir(veto_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(veto) = serde_json::from_str::<serde_json::Value>(&content) {
                        if veto["repository"] == repo && veto["pr_number"] == pr && veto["active"] == true {
                            veto_count += 1;
                            total_strength += veto["strength"].as_u64().unwrap_or(0);
                        }
                    }
                }
            }
        }
    }
    
    println!("📊 Governance status:");
    println!("  Repository: {}", repo);
    println!("  PR: #{}", pr);
    println!("  Active vetoes: {}", veto_count);
    println!("  Total veto strength: {}%", total_strength);
    
    if total_strength >= 30 {
        println!("🚫 VETO THRESHOLD MET - PR is blocked");
    } else if veto_count > 0 {
        println!("⚠️  Veto signals present but threshold not met");
    } else {
        println!("✅ No veto signals - PR can proceed");
    }
    
    Ok(())
}

//! Economic Node Veto CLI Tool
//! 
//! This tool allows economic nodes to submit veto signals for Tier 3+ governance changes.

use std::env;
use std::fs;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "economic-node-veto")]
#[command(about = "Submit veto signals for governance changes")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Submit a veto signal
    Veto {
        /// Node name
        #[arg(short, long)]
        node: String,
        
        /// Private key file path
        #[arg(short, long)]
        key: String,
        
        /// Repository name
        #[arg(short, long)]
        repo: String,
        
        /// Pull request number
        #[arg(short, long)]
        pr: u64,
        
        /// Veto reason
        #[arg(short, long)]
        reason: String,
        
        /// Veto strength (1-100)
        #[arg(short, long, default_value = "100")]
        strength: u8,
    },
    /// Check veto status for a PR
    Status {
        /// Repository name
        #[arg(short, long)]
        repo: String,
        
        /// Pull request number
        #[arg(short, long)]
        pr: u64,
    },
    /// List active vetoes
    List {
        /// Repository name (optional)
        #[arg(short, long)]
        repo: Option<String>,
    },
    /// Withdraw a veto
    Withdraw {
        /// Node name
        #[arg(short, long)]
        node: String,
        
        /// Private key file path
        #[arg(short, long)]
        key: String,
        
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
        Commands::Veto { node, key, repo, pr, reason, strength } => {
            submit_veto(&node, &key, &repo, pr, &reason, strength)?;
        }
        Commands::Status { repo, pr } => {
            check_veto_status(&repo, pr)?;
        }
        Commands::List { repo } => {
            list_vetoes(repo.as_deref())?;
        }
        Commands::Withdraw { node, key, repo, pr } => {
            withdraw_veto(&node, &key, &repo, pr)?;
        }
    }
    
    Ok(())
}

fn submit_veto(
    node: &str,
    key: &str,
    repo: &str,
    pr: u64,
    reason: &str,
    strength: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚫 Submitting veto signal for {}/{}#{}", repo, pr);
    
    // Validate strength
    if strength == 0 || strength > 100 {
        return Err("Veto strength must be between 1 and 100".into());
    }
    
    // Load private key
    let private_key = fs::read_to_string(key)?;
    
    // Create veto message
    let message = format!("veto:{}:{}:{}:{}", node, repo, pr, reason);
    
    // Sign the veto message (simplified - in real implementation, use proper crypto)
    let signature = format!("signature_for_{}", message.replace(":", "_"));
    
    // Create veto signal
    let veto_signal = json!({
        "node_name": node,
        "repository": repo,
        "pr_number": pr,
        "reason": reason,
        "strength": strength,
        "message": message,
        "signature": signature,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "active": true
    });
    
    // Save veto signal
    let veto_file = format!("veto-signals/{}_{}_{}.json", repo.replace("/", "_"), pr, node);
    fs::create_dir_all("veto-signals")?;
    fs::write(&veto_file, serde_json::to_string_pretty(&veto_signal)?)?;
    
    println!("✅ Veto signal submitted successfully!");
    println!("📁 Veto saved to: {}", veto_file);
    println!("");
    println!("📋 Veto details:");
    println!("  Node: {}", node);
    println!("  Repository: {}", repo);
    println!("  PR: #{}", pr);
    println!("  Reason: {}", reason);
    println!("  Strength: {}%", strength);
    println!("  Timestamp: {}", veto_signal["timestamp"]);
    
    println!("");
    println!("📤 To submit to governance system:");
    println!("  curl -X POST http://governance-app:8080/api/veto \\");
    println!("    -H 'Content-Type: application/json' \\");
    println!("    -d @{}", veto_file);
    
    Ok(())
}

fn check_veto_status(
    repo: &str,
    pr: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Checking veto status for {}/{}#{}", repo, pr);
    
    // Look for veto signals for this PR
    let veto_dir = "veto-signals";
    if !fs::metadata(veto_dir).is_ok() {
        println!("❌ No veto signals found");
        return Ok(());
    }
    
    let mut veto_count = 0;
    let mut total_strength = 0;
    let mut active_vetoes = Vec::new();
    
    for entry in fs::read_dir(veto_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(veto) = serde_json::from_str::<serde_json::Value>(&content) {
                    if veto["repository"] == repo && veto["pr_number"] == pr && veto["active"] == true {
                        veto_count += 1;
                        total_strength += veto["strength"].as_u64().unwrap_or(0);
                        active_vetoes.push(veto);
                    }
                }
            }
        }
    }
    
    if veto_count == 0 {
        println!("✅ No active veto signals for this PR");
    } else {
        println!("🚫 Found {} active veto signal(s) with total strength: {}%", veto_count, total_strength);
        println!("");
        
        for (i, veto) in active_vetoes.iter().enumerate() {
            println!("  {}. Node: {}", i + 1, veto["node_name"]);
            println!("     Reason: {}", veto["reason"]);
            println!("     Strength: {}%", veto["strength"]);
            println!("     Time: {}", veto["timestamp"]);
            println!("");
        }
        
        // Check if veto threshold is met (30% for hash rate, 40% for economic activity)
        if total_strength >= 30 {
            println!("⚠️  VETO THRESHOLD MET! This PR is blocked by economic node veto.");
        } else {
            println!("ℹ️  Veto threshold not yet met (30% required)");
        }
    }
    
    Ok(())
}

fn list_vetoes(
    repo_filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Listing active veto signals...");
    
    let veto_dir = "veto-signals";
    if !fs::metadata(veto_dir).is_ok() {
        println!("❌ No veto signals found");
        return Ok(());
    }
    
    let mut veto_count = 0;
    
    for entry in fs::read_dir(veto_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(veto) = serde_json::from_str::<serde_json::Value>(&content) {
                    if veto["active"] == true {
                        if let Some(repo) = repo_filter {
                            if veto["repository"] != repo {
                                continue;
                            }
                        }
                        
                        veto_count += 1;
                        println!("  {}. {} - {}/{}#{} ({}%)", 
                            veto_count,
                            veto["node_name"],
                            veto["repository"],
                            veto["pr_number"],
                            veto["strength"]
                        );
                        println!("     Reason: {}", veto["reason"]);
                        println!("     Time: {}", veto["timestamp"]);
                        println!("");
                    }
                }
            }
        }
    }
    
    if veto_count == 0 {
        println!("✅ No active veto signals found");
    } else {
        println!("📊 Total active vetoes: {}", veto_count);
    }
    
    Ok(())
}

fn withdraw_veto(
    node: &str,
    key: &str,
    repo: &str,
    pr: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("↩️ Withdrawing veto signal for {}/{}#{}", repo, pr);
    
    // Load private key
    let _private_key = fs::read_to_string(key)?;
    
    // Find the veto signal to withdraw
    let veto_file = format!("veto-signals/{}_{}_{}.json", repo.replace("/", "_"), pr, node);
    
    if !fs::metadata(&veto_file).is_ok() {
        return Err(format!("No veto signal found for {}/{}#{} by {}", repo, pr, node).into());
    }
    
    // Load and update veto signal
    let mut veto: serde_json::Value = serde_json::from_str(&fs::read_to_string(&veto_file)?)?;
    veto["active"] = json!(false);
    veto["withdrawn_at"] = json!(chrono::Utc::now().to_rfc3339());
    
    // Save updated veto signal
    fs::write(&veto_file, serde_json::to_string_pretty(&veto)?)?;
    
    println!("✅ Veto signal withdrawn successfully!");
    println!("📁 Updated veto file: {}", veto_file);
    println!("");
    println!("📋 Withdrawal details:");
    println!("  Node: {}", node);
    println!("  Repository: {}", repo);
    println!("  PR: #{}", pr);
    println!("  Withdrawn at: {}", veto["withdrawn_at"]);
    
    Ok(())
}

//! Economic Node Registration CLI Tool
//! 
//! This tool allows economic nodes (mining pools, exchanges, custodians) to register
//! with the governance system and submit proof-of-stake verification.

use std::env;
use std::fs;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "economic-node-register")]
#[command(about = "Register an economic node with the governance system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Register a new economic node
    Register {
        /// Node name
        #[arg(short, long)]
        name: String,
        
        /// Node type (mining_pool, exchange, custodian)
        #[arg(short, long)]
        node_type: String,
        
        /// Public key file path
        #[arg(short, long)]
        public_key: String,
        
        /// Hash rate percentage (for mining pools)
        #[arg(short, long)]
        hash_rate_percent: Option<f64>,
        
        /// Economic activity percentage (for exchanges/custodians)
        #[arg(short, long)]
        economic_activity_percent: Option<f64>,
        
        /// Proof of stake data file
        #[arg(short, long)]
        proof_file: Option<String>,
    },
    /// Generate a new keypair for economic node
    Generate {
        /// Node name
        #[arg(short, long)]
        name: String,
        
        /// Output directory
        #[arg(short, long, default_value = "./economic-keys")]
        output: String,
    },
    /// Submit proof of stake verification
    Proof {
        /// Node name
        #[arg(short, long)]
        name: String,
        
        /// Proof type (hash_rate, reserves, custody)
        #[arg(short, long)]
        proof_type: String,
        
        /// Proof data file
        #[arg(short, long)]
        data_file: String,
    },
    /// Check registration status
    Status {
        /// Node name
        #[arg(short, long)]
        name: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Register { name, node_type, public_key, hash_rate_percent, economic_activity_percent, proof_file } => {
            register_node(&name, &node_type, &public_key, hash_rate_percent, economic_activity_percent, proof_file)?;
        }
        Commands::Generate { name, output } => {
            generate_keypair(&name, &output)?;
        }
        Commands::Proof { name, proof_type, data_file } => {
            submit_proof(&name, &proof_type, &data_file)?;
        }
        Commands::Status { name } => {
            check_status(&name)?;
        }
    }
    
    Ok(())
}

fn register_node(
    name: &str,
    node_type: &str,
    public_key: &str,
    hash_rate_percent: Option<f64>,
    economic_activity_percent: Option<f64>,
    proof_file: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🏭 Registering economic node: {}", name);
    
    // Validate node type
    let valid_types = ["mining_pool", "exchange", "custodian"];
    if !valid_types.contains(&node_type) {
        return Err(format!("Invalid node type: {}. Must be one of: {:?}", node_type, valid_types).into());
    }
    
    // Load public key
    let public_key_data = fs::read_to_string(public_key)?;
    
    // Validate percentages based on node type
    match node_type {
        "mining_pool" => {
            if hash_rate_percent.is_none() {
                return Err("Mining pools must specify hash_rate_percent".into());
            }
            if let Some(percent) = hash_rate_percent {
                if percent <= 0.0 || percent > 100.0 {
                    return Err("Hash rate percentage must be between 0 and 100".into());
                }
            }
        }
        "exchange" | "custodian" => {
            if economic_activity_percent.is_none() {
                return Err("Exchanges and custodians must specify economic_activity_percent".into());
            }
            if let Some(percent) = economic_activity_percent {
                if percent <= 0.0 || percent > 100.0 {
                    return Err("Economic activity percentage must be between 0 and 100".into());
                }
            }
        }
        _ => {}
    }
    
    // Create registration payload
    let mut payload = json!({
        "name": name,
        "node_type": node_type,
        "public_key": public_key_data,
        "active": true,
        "registration_timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    if let Some(percent) = hash_rate_percent {
        payload["hash_rate_percent"] = json!(percent);
    }
    
    if let Some(percent) = economic_activity_percent {
        payload["economic_activity_percent"] = json!(percent);
    }
    
    // Load proof data if provided
    if let Some(proof_path) = proof_file {
        let proof_data = fs::read_to_string(&proof_path)?;
        payload["proof_data"] = json!(proof_data);
        println!("📄 Loaded proof data from: {}", proof_path);
    }
    
    // Save registration to file (in real implementation, this would be sent to the governance-app)
    let registration_file = format!("economic-registrations/{}.json", name);
    fs::create_dir_all("economic-registrations")?;
    fs::write(&registration_file, serde_json::to_string_pretty(&payload)?)?;
    
    println!("✅ Economic node registration created!");
    println!("📁 Registration saved to: {}", registration_file);
    println!("");
    println!("📋 Registration details:");
    println!("  Name: {}", name);
    println!("  Type: {}", node_type);
    println!("  Public Key: {}", public_key);
    if let Some(percent) = hash_rate_percent {
        println!("  Hash Rate: {}%", percent);
    }
    if let Some(percent) = economic_activity_percent {
        println!("  Economic Activity: {}%", percent);
    }
    println!("  Timestamp: {}", payload["registration_timestamp"]);
    
    println!("");
    println!("📤 To complete registration, send this file to the governance administrator:");
    println!("  scp {} governance-server:/path/to/economic-registrations/", registration_file);
    
    Ok(())
}

fn generate_keypair(
    name: &str,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔑 Generating keypair for economic node: {}", name);
    
    // Create output directory
    fs::create_dir_all(output_dir)?;
    
    // Generate Ed25519 keypair using openssl
    let private_key_path = format!("{}/{}_private.pem", output_dir, name);
    let public_key_path = format!("{}/{}_public.pem", output_dir, name);
    
    // Generate private key
    let output = std::process::Command::new("openssl")
        .args(&["genpkey", "-algorithm", "Ed25519", "-out", &private_key_path])
        .output()?;
    
    if !output.status.success() {
        return Err("Failed to generate private key".into());
    }
    
    // Extract public key
    let output = std::process::Command::new("openssl")
        .args(&["pkey", "-in", &private_key_path, "-pubout", "-out", &public_key_path])
        .output()?;
    
    if !output.status.success() {
        return Err("Failed to extract public key".into());
    }
    
    // Get public key in hex format
    let output = std::process::Command::new("openssl")
        .args(&["pkey", "-in", &private_key_path, "-pubout", "-outform", "DER"])
        .output()?;
    
    if !output.status.success() {
        return Err("Failed to get public key in DER format".into());
    }
    
    let public_key_hex = hex::encode(&output.stdout);
    
    println!("✅ Keypair generated successfully!");
    println!("📁 Private key: {}", private_key_path);
    println!("📁 Public key: {}", public_key_path);
    println!("🔑 Public key (hex): {}", public_key_hex);
    
    Ok(())
}

fn submit_proof(
    name: &str,
    proof_type: &str,
    data_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📄 Submitting proof of stake for node: {}", name);
    
    // Validate proof type
    let valid_types = ["hash_rate", "reserves", "custody"];
    if !valid_types.contains(&proof_type) {
        return Err(format!("Invalid proof type: {}. Must be one of: {:?}", proof_type, valid_types).into());
    }
    
    // Load proof data
    let proof_data = fs::read_to_string(data_file)?;
    
    // Create proof submission
    let proof_submission = json!({
        "node_name": name,
        "proof_type": proof_type,
        "proof_data": proof_data,
        "submission_timestamp": chrono::Utc::now().to_rfc3339(),
        "verified": false
    });
    
    // Save proof submission
    let proof_file = format!("economic-proofs/{}_proof_{}.json", name, proof_type);
    fs::create_dir_all("economic-proofs")?;
    fs::write(&proof_file, serde_json::to_string_pretty(&proof_submission)?)?;
    
    println!("✅ Proof submission created!");
    println!("📁 Proof saved to: {}", proof_file);
    println!("");
    println!("📋 Proof details:");
    println!("  Node: {}", name);
    println!("  Type: {}", proof_type);
    println!("  Data file: {}", data_file);
    println!("  Timestamp: {}", proof_submission["submission_timestamp"]);
    
    Ok(())
}

fn check_status(
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Checking registration status for: {}", name);
    
    // In a real implementation, this would query the governance-app database
    // For now, we'll check if registration files exist
    
    let registration_file = format!("economic-registrations/{}.json", name);
    
    if fs::metadata(&registration_file).is_ok() {
        let registration: serde_json::Value = serde_json::from_str(&fs::read_to_string(&registration_file)?)?;
        
        println!("✅ Node is registered!");
        println!("📋 Registration details:");
        println!("  Name: {}", registration["name"]);
        println!("  Type: {}", registration["node_type"]);
        println!("  Active: {}", registration["active"]);
        println!("  Registered: {}", registration["registration_timestamp"]);
        
        if registration["hash_rate_percent"].is_number() {
            println!("  Hash Rate: {}%", registration["hash_rate_percent"]);
        }
        
        if registration["economic_activity_percent"].is_number() {
            println!("  Economic Activity: {}%", registration["economic_activity_percent"]);
        }
    } else {
        println!("❌ Node is not registered");
        println!("💡 Use 'register' command to register this node");
    }
    
    Ok(())
}

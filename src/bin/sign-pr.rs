//! CLI tool for offline PR signing
//!
//! This tool allows maintainers to sign PRs offline using their private keys.
//! It generates the proper signature that can be posted as a comment on GitHub.

use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
use std::str::FromStr;

use blvm_commons::crypto::signatures::SignatureManager;

#[derive(Parser)]
#[command(name = "sign-pr")]
#[command(about = "Sign a pull request for governance approval")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sign a pull request
    Sign {
        /// Private key file path
        #[arg(short, long)]
        key: String,

        /// Repository name (e.g., "btcdecoded/governance")
        #[arg(short, long)]
        repo: String,

        /// Pull request number
        #[arg(short, long)]
        pr: u64,

        /// Optional message to sign (defaults to "PR #X in Y")
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Generate a new keypair
    Generate {
        /// Output directory for keys
        #[arg(short, long, default_value = "./keys")]
        output: String,

        /// Username for the key
        #[arg(short, long)]
        username: String,
    },
    /// Verify a signature
    Verify {
        /// Public key file path
        #[arg(short, long)]
        public_key: String,

        /// Message that was signed
        #[arg(short, long)]
        message: String,

        /// Signature to verify
        #[arg(short, long)]
        signature: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Sign {
            key,
            repo,
            pr,
            message,
        } => {
            sign_pr(&key, &repo, pr, message)?;
        }
        Commands::Generate { output, username } => {
            generate_keypair(&output, &username)?;
        }
        Commands::Verify {
            public_key,
            message,
            signature,
        } => {
            verify_signature(&public_key, &message, &signature)?;
        }
    }

    Ok(())
}

fn sign_pr(
    key_path: &str,
    repo: &str,
    pr: u64,
    message: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Signing PR #{pr} in {repo}");

    // Load private key
    let private_key = fs::read_to_string(key_path)?;

    // Create message to sign
    let message = message.unwrap_or_else(|| format!("PR #{pr} in {repo}"));
    println!("📝 Message to sign: {message}");

    // Initialize signature manager
    let signature_manager = SignatureManager::new();

    // Parse private key
    let secret_key = secp256k1::SecretKey::from_str(private_key.trim())?;

    // Sign the message
    let signature = signature_manager.create_signature(&message, &secret_key)?;

    println!("✅ Signature generated successfully!");
    println!();
    println!("📋 Copy this command to post on GitHub:");
    println!("/governance-sign {signature}");
    println!();
    println!("🔍 Signature details:");
    println!("  Repository: {repo}");
    println!("  PR Number: {pr}");
    println!("  Message: {message}");
    println!("  Signature: {signature}");

    Ok(())
}

fn generate_keypair(output_dir: &str, username: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔑 Generating keypair for {username}");

    // Create output directory
    fs::create_dir_all(output_dir)?;

    // Generate keypair using openssl
    let private_key_path = Path::new(output_dir).join(format!("{username}_private.pem"));
    let public_key_path = Path::new(output_dir).join(format!("{username}_public.pem"));

    // Generate private key
    let output = std::process::Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "Ed25519",
            "-out",
            private_key_path.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        return Err("Failed to generate private key".into());
    }

    // Extract public key
    let output = std::process::Command::new("openssl")
        .args([
            "pkey",
            "-in",
            private_key_path.to_str().unwrap(),
            "-pubout",
            "-out",
            public_key_path.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        return Err("Failed to extract public key".into());
    }

    // Get public key in hex format
    let output = std::process::Command::new("openssl")
        .args([
            "pkey",
            "-in",
            private_key_path.to_str().unwrap(),
            "-pubout",
            "-outform",
            "DER",
        ])
        .output()?;

    if !output.status.success() {
        return Err("Failed to get public key in DER format".into());
    }

    let public_key_hex = hex::encode(&output.stdout);

    println!("✅ Keypair generated successfully!");
    println!("📁 Private key: {}", private_key_path.display());
    println!("📁 Public key: {}", public_key_path.display());
    println!("🔑 Public key (hex): {public_key_hex}");
    println!();
    println!("💾 To add this maintainer to the database:");
    println!(
        "INSERT INTO maintainers (github_username, public_key, layer, active, last_updated) VALUES"
    );
    println!("('{username}', '{public_key_hex}', 1, true, CURRENT_TIMESTAMP);");

    Ok(())
}

fn verify_signature(
    public_key_path: &str,
    message: &str,
    signature: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Verifying signature...");

    // Load public key
    let public_key = fs::read_to_string(public_key_path)?;

    // Initialize signature manager
    let signature_manager = SignatureManager::new();

    // Verify signature
    let is_valid =
        signature_manager.verify_governance_signature(message, signature, &public_key)?;

    if is_valid {
        println!("✅ Signature is VALID");
        println!("📝 Message: {message}");
        println!("🔑 Public key: {public_key_path}");
        println!("✍️  Signature: {signature}");
    } else {
        println!("❌ Signature is INVALID");
        println!("📝 Message: {message}");
        println!("🔑 Public key: {public_key_path}");
        println!("✍️  Signature: {signature}");
    }

    Ok(())
}

//! Governance Fork Migration Tool
//! 
//! This tool helps migrate between different governance rulesets and manage fork transitions.

use std::env;
use std::fs;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "fork-migrate")]
#[command(about = "Migrate between governance rulesets and manage fork transitions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available rulesets
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },
    /// Show current ruleset
    Current,
    /// Migrate to a different ruleset
    Migrate {
        /// Target ruleset ID
        #[arg(short, long)]
        ruleset: String,
        
        /// Force migration (skip confirmation)
        #[arg(short, long)]
        force: bool,
        
        /// Backup current configuration
        #[arg(short, long)]
        backup: bool,
    },
    /// Create a new ruleset from current configuration
    Create {
        /// Ruleset name
        #[arg(short, long)]
        name: String,
        
        /// Ruleset description
        #[arg(short, long)]
        description: Option<String>,
        
        /// Version (e.g., "1.0.0")
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Compare two rulesets
    Compare {
        /// First ruleset ID
        #[arg(short, long)]
        ruleset1: String,
        
        /// Second ruleset ID
        #[arg(short, long)]
        ruleset2: String,
    },
    /// Validate a ruleset
    Validate {
        /// Ruleset ID or file path
        #[arg(short, long)]
        ruleset: String,
    },
    /// Show migration history
    History {
        /// Number of entries to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Rollback to previous ruleset
    Rollback {
        /// Target ruleset ID
        #[arg(short, long)]
        ruleset: String,
        
        /// Force rollback (skip confirmation)
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::List { detailed } => {
            list_rulesets(detailed)?;
        }
        Commands::Current => {
            show_current_ruleset()?;
        }
        Commands::Migrate { ruleset, force, backup } => {
            migrate_to_ruleset(&ruleset, force, backup)?;
        }
        Commands::Create { name, description, version } => {
            create_ruleset(&name, description, version)?;
        }
        Commands::Compare { ruleset1, ruleset2 } => {
            compare_rulesets(&ruleset1, &ruleset2)?;
        }
        Commands::Validate { ruleset } => {
            validate_ruleset(&ruleset)?;
        }
        Commands::History { limit } => {
            show_migration_history(limit)?;
        }
        Commands::Rollback { ruleset, force } => {
            rollback_to_ruleset(&ruleset, force)?;
        }
    }
    
    Ok(())
}

fn list_rulesets(detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Available governance rulesets:");
    
    let rulesets_dir = "governance-exports";
    if !fs::metadata(rulesets_dir).is_ok() {
        println!("❌ No rulesets directory found");
        return Ok(());
    }
    
    let mut rulesets = Vec::new();
    
    for entry in fs::read_dir(rulesets_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(export) = serde_json::from_str::<serde_json::Value>(&content) {
                    rulesets.push((path, export));
                }
            }
        }
    }
    
    if rulesets.is_empty() {
        println!("✅ No rulesets found");
        return Ok(());
    }
    
    // Sort by creation date
    rulesets.sort_by(|a, b| {
        let date_a = a.1.get("created_at").and_then(|d| d.as_str()).unwrap_or("");
        let date_b = b.1.get("created_at").and_then(|d| d.as_str()).unwrap_or("");
        date_b.cmp(date_a)
    });
    
    for (i, (path, export)) in rulesets.iter().enumerate() {
        let ruleset_id = export.get("ruleset_id").and_then(|id| id.as_str()).unwrap_or("unknown");
        let version = export.get("ruleset_version").and_then(|v| v.get("major"))
            .and_then(|m| m.as_u64()).unwrap_or(0);
        let created_at = export.get("created_at").and_then(|d| d.as_str()).unwrap_or("unknown");
        
        println!("  {}. {} (v{}) - {}", i + 1, ruleset_id, version, created_at);
        
        if detailed {
            if let Some(description) = export.get("metadata").and_then(|m| m.get("description")) {
                println!("     Description: {}", description);
            }
            
            if let Some(source) = export.get("metadata").and_then(|m| m.get("source_repository")) {
                println!("     Source: {}", source);
            }
            
            if let Some(commit) = export.get("metadata").and_then(|m| m.get("commit_hash")) {
                println!("     Commit: {}", commit);
            }
        }
    }
    
    Ok(())
}

fn show_current_ruleset() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Current governance ruleset:");
    
    // Check for current ruleset indicator
    let current_file = "governance-exports/current.json";
    if fs::metadata(current_file).is_ok() {
        let content = fs::read_to_string(current_file)?;
        let current: serde_json::Value = serde_json::from_str(&content)?;
        
        let ruleset_id = current.get("ruleset_id").and_then(|id| id.as_str()).unwrap_or("unknown");
        let version = current.get("ruleset_version").and_then(|v| v.get("major"))
            .and_then(|m| m.as_u64()).unwrap_or(0);
        let created_at = current.get("created_at").and_then(|d| d.as_str()).unwrap_or("unknown");
        
        println!("  Ruleset: {}", ruleset_id);
        println!("  Version: {}", version);
        println!("  Created: {}", created_at);
        
        if let Some(description) = current.get("metadata").and_then(|m| m.get("description")) {
            println!("  Description: {}", description);
        }
    } else {
        println!("❌ No current ruleset found");
        println!("💡 Use 'migrate' command to set a current ruleset");
    }
    
    Ok(())
}

fn migrate_to_ruleset(
    ruleset: &str,
    force: bool,
    backup: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Migrating to ruleset: {}", ruleset);
    
    // Find the ruleset file
    let ruleset_file = format!("governance-exports/{}.json", ruleset);
    if !fs::metadata(&ruleset_file).is_ok() {
        return Err(format!("Ruleset not found: {}", ruleset).into());
    }
    
    // Load the target ruleset
    let content = fs::read_to_string(&ruleset_file)?;
    let target_ruleset: serde_json::Value = serde_json::from_str(&content)?;
    
    // Validate the ruleset
    validate_ruleset_content(&target_ruleset)?;
    
    if !force {
        println!("⚠️  This will change the current governance ruleset");
        println!("   Target: {}", ruleset);
        println!("   Continue? (y/N): ");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if !input.trim().to_lowercase().starts_with('y') {
            println!("❌ Migration cancelled");
            return Ok(());
        }
    }
    
    // Create backup if requested
    if backup {
        let backup_file = format!("governance-exports/backup-{}.json", 
            chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        
        if fs::metadata("governance-exports/current.json").is_ok() {
            fs::copy("governance-exports/current.json", &backup_file)?;
            println!("📦 Backup created: {}", backup_file);
        }
    }
    
    // Perform migration
    fs::copy(&ruleset_file, "governance-exports/current.json")?;
    
    // Log migration
    log_migration("migrate", ruleset, "Migration completed successfully")?;
    
    println!("✅ Migration completed successfully!");
    println!("   Current ruleset: {}", ruleset);
    
    Ok(())
}

fn create_ruleset(
    name: &str,
    description: Option<String>,
    version: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🆕 Creating new ruleset: {}", name);
    
    // Load current governance configuration
    let current_config = load_current_governance_config()?;
    
    // Create ruleset export
    let ruleset_version = version.unwrap_or_else(|| "1.0.0".to_string());
    let version_parts: Vec<&str> = ruleset_version.split('.').collect();
    
    if version_parts.len() != 3 {
        return Err("Invalid version format. Use semantic versioning (e.g., 1.0.0)".into());
    }
    
    let major = version_parts[0].parse::<u32>()?;
    let minor = version_parts[1].parse::<u32>()?;
    let patch = version_parts[2].parse::<u32>()?;
    
    let export = json!({
        "version": "1.0",
        "ruleset_id": name,
        "ruleset_version": {
            "major": major,
            "minor": minor,
            "patch": patch
        },
        "created_at": chrono::Utc::now().to_rfc3339(),
        "action_tiers": current_config.get("action_tiers"),
        "economic_nodes": current_config.get("economic_nodes"),
        "maintainers": current_config.get("maintainers"),
        "repositories": current_config.get("repositories"),
        "governance_fork": current_config.get("governance_fork"),
        "metadata": {
            "exported_by": "fork-migrate",
            "source_repository": "btcdecoded/governance",
            "commit_hash": "unknown",
            "export_tool_version": "1.0.0",
            "description": description.unwrap_or_else(|| "Custom governance ruleset".to_string())
        }
    });
    
    // Save ruleset
    let ruleset_file = format!("governance-exports/{}.json", name);
    fs::create_dir_all("governance-exports")?;
    fs::write(&ruleset_file, serde_json::to_string_pretty(&export)?)?;
    
    println!("✅ Ruleset created successfully!");
    println!("   File: {}", ruleset_file);
    println!("   ID: {}", name);
    println!("   Version: {}", ruleset_version);
    
    Ok(())
}

fn compare_rulesets(
    ruleset1: &str,
    ruleset2: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Comparing rulesets: {} vs {}", ruleset1, ruleset2);
    
    // Load both rulesets
    let file1 = format!("governance-exports/{}.json", ruleset1);
    let file2 = format!("governance-exports/{}.json", ruleset2);
    
    if !fs::metadata(&file1).is_ok() {
        return Err(format!("Ruleset not found: {}", ruleset1).into());
    }
    
    if !fs::metadata(&file2).is_ok() {
        return Err(format!("Ruleset not found: {}", ruleset2).into());
    }
    
    let content1 = fs::read_to_string(&file1)?;
    let content2 = fs::read_to_string(&file2)?;
    
    let ruleset1_data: serde_json::Value = serde_json::from_str(&content1)?;
    let ruleset2_data: serde_json::Value = serde_json::from_str(&content2)?;
    
    // Compare key components
    println!("📊 Comparison results:");
    
    // Compare action tiers
    compare_json_section("Action Tiers", &ruleset1_data["action_tiers"], &ruleset2_data["action_tiers"]);
    
    // Compare economic nodes
    compare_json_section("Economic Nodes", &ruleset1_data["economic_nodes"], &ruleset2_data["economic_nodes"]);
    
    // Compare maintainers
    compare_json_section("Maintainers", &ruleset1_data["maintainers"], &ruleset2_data["maintainers"]);
    
    // Compare repositories
    compare_json_section("Repositories", &ruleset1_data["repositories"], &ruleset2_data["repositories"]);
    
    Ok(())
}

fn compare_json_section(name: &str, section1: &serde_json::Value, section2: &serde_json::Value) {
    if section1 == section2 {
        println!("  ✅ {}: Identical", name);
    } else {
        println!("  ❌ {}: Different", name);
        
        // Show some differences
        if let (Some(obj1), Some(obj2)) = (section1.as_object(), section2.as_object()) {
            let keys1: std::collections::HashSet<_> = obj1.keys().collect();
            let keys2: std::collections::HashSet<_> = obj2.keys().collect();
            
            let only_in_1: Vec<_> = keys1.difference(&keys2).collect();
            let only_in_2: Vec<_> = keys2.difference(&keys1).collect();
            
            if !only_in_1.is_empty() {
                println!("    Only in first: {:?}", only_in_1);
            }
            if !only_in_2.is_empty() {
                println!("    Only in second: {:?}", only_in_2);
            }
        }
    }
}

fn validate_ruleset(ruleset: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Validating ruleset: {}", ruleset);
    
    let ruleset_file = format!("governance-exports/{}.json", ruleset);
    if !fs::metadata(&ruleset_file).is_ok() {
        return Err(format!("Ruleset not found: {}", ruleset).into());
    }
    
    let content = fs::read_to_string(&ruleset_file)?;
    let ruleset_data: serde_json::Value = serde_json::from_str(&content)?;
    
    validate_ruleset_content(&ruleset_data)?;
    
    println!("✅ Ruleset validation passed!");
    Ok(())
}

fn validate_ruleset_content(ruleset: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    // Check required fields
    let required_fields = ["ruleset_id", "ruleset_version", "action_tiers", "economic_nodes", "maintainers"];
    
    for field in &required_fields {
        if !ruleset.get(field).is_some() {
            return Err(format!("Missing required field: {}", field).into());
        }
    }
    
    // Validate version format
    if let Some(version) = ruleset.get("ruleset_version") {
        if let Some(major) = version.get("major") {
            if !major.is_number() {
                return Err("Invalid version format: major must be a number".into());
            }
        }
    }
    
    println!("  ✅ All required fields present");
    println!("  ✅ Version format valid");
    println!("  ✅ JSON structure valid");
    
    Ok(())
}

fn show_migration_history(limit: usize) -> Result<(), Box<dyn std::error::Error>> {
    println!("📜 Migration history (last {} entries):", limit);
    
    let history_file = "governance-exports/migration-history.jsonl";
    if !fs::metadata(history_file).is_ok() {
        println!("❌ No migration history found");
        return Ok(());
    }
    
    let content = fs::read_to_string(history_file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let start = if lines.len() > limit { lines.len() - limit } else { 0 };
    
    for line in lines.iter().skip(start) {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            let action = entry.get("action").and_then(|a| a.as_str()).unwrap_or("unknown");
            let ruleset = entry.get("ruleset").and_then(|r| r.as_str()).unwrap_or("unknown");
            let timestamp = entry.get("timestamp").and_then(|t| t.as_str()).unwrap_or("unknown");
            let message = entry.get("message").and_then(|m| m.as_str()).unwrap_or("");
            
            println!("  {} - {} {} ({})", timestamp, action, ruleset, message);
        }
    }
    
    Ok(())
}

fn rollback_to_ruleset(
    ruleset: &str,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("↩️ Rolling back to ruleset: {}", ruleset);
    
    // This is essentially the same as migrate
    migrate_to_ruleset(ruleset, force, true)?;
    
    // Log as rollback
    log_migration("rollback", ruleset, "Rollback completed successfully")?;
    
    println!("✅ Rollback completed successfully!");
    Ok(())
}

fn load_current_governance_config() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Load from governance/config directory
    let mut config = serde_json::Map::new();
    
    // Load action tiers
    if fs::metadata("governance/config/action-tiers.yml").is_ok() {
        let content = fs::read_to_string("governance/config/action-tiers.yml")?;
        let action_tiers: serde_json::Value = serde_yaml::from_str(&content)?;
        config.insert("action_tiers".to_string(), action_tiers);
    }
    
    // Load other config files similarly...
    // For now, create a minimal config
    config.insert("action_tiers".to_string(), serde_json::Value::Object(serde_json::Map::new()));
    config.insert("economic_nodes".to_string(), serde_json::Value::Object(serde_json::Map::new()));
    config.insert("maintainers".to_string(), serde_json::Value::Object(serde_json::Map::new()));
    config.insert("repositories".to_string(), serde_json::Value::Object(serde_json::Map::new()));
    
    Ok(serde_json::Value::Object(config))
}

fn log_migration(action: &str, ruleset: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let log_entry = json!({
        "action": action,
        "ruleset": ruleset,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "message": message
    });
    
    let history_file = "governance-exports/migration-history.jsonl";
    fs::create_dir_all("governance-exports")?;
    
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_file)?;
    
    use std::io::Write;
    writeln!(file, "{}", serde_json::to_string(&log_entry)?)?;
    
    Ok(())
}

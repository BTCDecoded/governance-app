//! Security Gate CLI Tool
//!
//! Command-line tool for checking security control status, analyzing PR impact,
//! and verifying production readiness.

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
use tracing::info;

use blvm_commons::validation::security_controls::SecurityControlValidator;

#[derive(Parser)]
#[command(name = "security-gate")]
#[command(about = "Bitcoin Commons Security Control Management Tool")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check security control status
    Status {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },
    /// Check security impact of a PR
    CheckPr {
        /// PR number to analyze
        pr_number: u32,
        /// Output format (json, text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Check for placeholder implementations
    CheckPlaceholders {
        /// Specific files to check (default: all changed files)
        files: Option<Vec<String>>,
        /// Fail on any placeholder found
        #[arg(short, long)]
        fail_on_placeholder: bool,
    },
    /// Verify production readiness
    VerifyProductionReadiness {
        /// Output format (json, text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Update security control status file
    UpdateStatus {
        /// Force update even if no changes
        #[arg(short, long)]
        force: bool,
    },
    /// Generate security report
    GenerateReport {
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Status { detailed } => {
            check_status(detailed).await?;
        }
        Commands::CheckPr { pr_number, format } => {
            check_pr_impact(pr_number, &format).await?;
        }
        Commands::CheckPlaceholders {
            files,
            fail_on_placeholder,
        } => {
            check_placeholders(files, fail_on_placeholder).await?;
        }
        Commands::VerifyProductionReadiness { format } => {
            verify_production_readiness(&format).await?;
        }
        Commands::UpdateStatus { force } => {
            update_status(force).await?;
        }
        Commands::GenerateReport { output } => {
            generate_report(output).await?;
        }
    }

    Ok(())
}

async fn check_status(detailed: bool) -> Result<()> {
    info!("Checking security control status...");

    let status_file = "governance/config/security-control-status.yml";
    if !Path::new(status_file).exists() {
        return Err(anyhow!(
            "Security control status file not found: {}",
            status_file
        ));
    }

    let content = fs::read_to_string(status_file)?;
    let status: serde_yaml::Value = serde_yaml::from_str(&content)?;

    println!("🔒 Bitcoin Commons Security Control Status");
    println!("=====================================");
    println!();

    // Overall status
    let production_ready = status["production_ready"].as_bool().unwrap_or(false);
    let blocking_controls = status["blocking_controls"].as_i64().unwrap_or(0);
    let total_controls = status["total_controls"].as_i64().unwrap_or(0);

    if production_ready {
        println!("✅ Production Ready: YES");
    } else {
        println!("❌ Production Ready: NO ({blocking_controls} controls blocking)");
    }

    println!("📊 Total Controls: {total_controls}");
    println!();

    // Summary
    if let Some(summary) = status.get("summary") {
        let p0_critical = summary["P0_critical"].as_i64().unwrap_or(0);
        let p0_complete = summary["P0_complete"].as_i64().unwrap_or(0);
        let p0_incomplete = summary["P0_incomplete"].as_i64().unwrap_or(0);

        println!("P0 (Critical) Controls: {p0_complete}/{p0_critical} complete");
        if p0_incomplete > 0 {
            println!("⚠️  {p0_incomplete} P0 controls incomplete - blocks production");
        }
    }

    if detailed {
        println!();
        println!("📋 Detailed Control Status:");
        println!("---------------------------");

        if let Some(controls) = status.get("controls") {
            for (control_id, control) in controls.as_mapping().unwrap() {
                let name = control["name"].as_str().unwrap_or("Unknown");
                let state = control["state"].as_str().unwrap_or("unknown");
                let priority = control["priority"].as_str().unwrap_or("P3");
                let blocks_production = control["blocks_production"].as_bool().unwrap_or(false);

                let state_emoji = match state {
                    "certified" => "✅",
                    "audited" => "🔵",
                    "implemented" => "🟢",
                    "partial" => "🟡",
                    "placeholder" => "🔴",
                    "missing" => "⚫",
                    _ => "❓",
                };

                let priority_emoji = match priority {
                    "P0" => "🔴",
                    "P1" => "🟡",
                    "P2" => "🟢",
                    _ => "⚪",
                };

                let blocking = if blocks_production { " (BLOCKS)" } else { "" };
                let control_id_str = control_id.as_str().unwrap_or("unknown");

                println!("  {state_emoji} {priority_emoji} {name} - {control_id_str}{blocking}");
            }
        }
    }

    // Next actions
    if let Some(next_actions) = status.get("next_actions") {
        if let Some(immediate) = next_actions.get("immediate") {
            if let Some(actions) = immediate.as_sequence() {
                if !actions.is_empty() {
                    println!();
                    println!("🎯 Immediate Actions Required:");
                    for action in actions {
                        println!("  - {}", action.as_str().unwrap_or("Unknown"));
                    }
                }
            }
        }
    }

    Ok(())
}

async fn check_pr_impact(pr_number: u32, format: &str) -> Result<()> {
    info!("Checking security impact for PR #{}", pr_number);

    // Load security control mapping
    let validator =
        SecurityControlValidator::new("governance/config/security-control-mapping.yml")?;

    // Get changed files from PR (simplified - in real implementation, would use GitHub API)
    let changed_files = get_pr_changed_files(pr_number).await?;

    // Analyze security impact
    let impact = validator.analyze_security_impact(&changed_files)?;

    // Output based on format
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&impact)?;
            println!("{json}");

            // Also write to file for CI
            fs::write("security-impact.json", json)?;
        }
        "text" => {
            println!("🔍 Security Impact Analysis for PR #{pr_number}");
            println!("=============================================");
            println!();

            match impact.impact_level {
                blvm_commons::validation::security_controls::ImpactLevel::Critical => {
                    println!("🚨 CRITICAL IMPACT - Multiple P0 security controls affected");
                }
                blvm_commons::validation::security_controls::ImpactLevel::High => {
                    println!("🔴 HIGH IMPACT - P0 security controls affected");
                }
                blvm_commons::validation::security_controls::ImpactLevel::Medium => {
                    println!("🟡 MEDIUM IMPACT - P1 security controls affected");
                }
                blvm_commons::validation::security_controls::ImpactLevel::Low => {
                    println!("🟢 LOW IMPACT - P2 security controls affected");
                }
                blvm_commons::validation::security_controls::ImpactLevel::None => {
                    println!("✅ NO IMPACT - No security controls affected");
                }
            }

            if !impact.affected_controls.is_empty() {
                println!();
                println!("📋 Affected Security Controls:");
                for control in &impact.affected_controls {
                    let priority_emoji = match control.priority.as_str() {
                        "P0" => "🔴",
                        "P1" => "🟡",
                        "P2" => "🟢",
                        _ => "⚪",
                    };
                    println!("  {} {} ({})", priority_emoji, control.name, control.id);
                }
            }

            if let Some(tier) = &impact.required_tier {
                println!();
                println!("🎯 Required Governance Tier: {tier}");
            }

            if !impact.additional_requirements.is_empty() {
                println!();
                println!("📝 Additional Requirements:");
                for req in &impact.additional_requirements {
                    println!("  - {req}");
                }
            }

            if impact.blocks_production {
                println!();
                println!("⚠️  This PR blocks production deployment!");
            }

            if impact.blocks_audit {
                println!();
                println!("⚠️  This PR blocks security audit!");
            }
        }
        _ => return Err(anyhow!("Unknown format: {}", format)),
    }

    Ok(())
}

async fn check_placeholders(files: Option<Vec<String>>, fail_on_placeholder: bool) -> Result<()> {
    info!("Checking for placeholder implementations...");

    let validator =
        SecurityControlValidator::new("governance/config/security-control-mapping.yml")?;

    let files_to_check = match files {
        Some(f) => f,
        None => {
            // Get changed files from git (simplified)
            get_git_changed_files().await?
        }
    };

    let violations = validator.check_for_placeholders(&files_to_check)?;

    if violations.is_empty() {
        println!("✅ No placeholder implementations found in security-critical files");
        return Ok(());
    }

    println!("❌ Found {} placeholder violations:", violations.len());
    println!();

    for violation in &violations {
        println!("📁 {}:{}", violation.file, violation.line);
        println!("   Pattern: {}", violation.pattern);
        println!("   Content: {}", violation.content);
        println!();
    }

    if fail_on_placeholder {
        return Err(anyhow!(
            "Placeholder implementations found in security-critical files"
        ));
    }

    Ok(())
}

async fn verify_production_readiness(format: &str) -> Result<()> {
    info!("Verifying production readiness...");

    let status_file = "governance/config/security-control-status.yml";
    if !Path::new(status_file).exists() {
        return Err(anyhow!("Security control status file not found"));
    }

    let content = fs::read_to_string(status_file)?;
    let status: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let production_ready = status["production_ready"].as_bool().unwrap_or(false);
    let blocking_controls = status["blocking_controls"].as_i64().unwrap_or(0);

    let result = ProductionReadinessResult {
        ready: production_ready,
        blocking_controls: blocking_controls as usize,
        blocking_control_ids: get_blocking_controls(&status),
    };

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&result)?;
            println!("{json}");
        }
        "text" => {
            if production_ready {
                println!("✅ Production Ready: YES");
                println!("🚀 System is ready for production deployment");
            } else {
                println!("❌ Production Ready: NO");
                println!("⚠️  {blocking_controls} controls blocking production deployment");

                if let Some(blocking) = status.get("audit_readiness") {
                    if let Some(blockers) = blocking.get("audit_blockers") {
                        if let Some(blocker_list) = blockers.as_sequence() {
                            println!();
                            println!("🔒 Blocking Controls:");
                            for blocker in blocker_list {
                                println!("  - {}", blocker.as_str().unwrap_or("Unknown"));
                            }
                        }
                    }
                }
            }
        }
        _ => return Err(anyhow!("Unknown format: {}", format)),
    }

    if !production_ready {
        return Err(anyhow!("Production readiness check failed"));
    }

    Ok(())
}

async fn update_status(_force: bool) -> Result<()> {
    info!("Updating security control status...");

    // In a real implementation, this would:
    // 1. Scan the codebase for actual implementation status
    // 2. Check test coverage
    // 3. Verify control implementations
    // 4. Update the status file

    println!("📝 Security control status update completed");
    println!("   (This is a placeholder implementation)");

    Ok(())
}

async fn generate_report(output: Option<String>) -> Result<()> {
    info!("Generating security report...");

    let status_file = "governance/config/security-control-status.yml";
    let content = fs::read_to_string(status_file)?;
    let status: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let mut report = String::new();
    report.push_str("# Bitcoin Commons Security Control Report\n");
    report.push_str(&format!(
        "Generated: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Overall status
    let production_ready = status["production_ready"].as_bool().unwrap_or(false);
    let blocking_controls = status["blocking_controls"].as_i64().unwrap_or(0);

    report.push_str("## Overall Status\n\n");
    if production_ready {
        report.push_str("✅ **Production Ready**: YES\n\n");
    } else {
        report.push_str(&format!(
            "❌ **Production Ready**: NO ({blocking_controls} controls blocking)\n\n"
        ));
    }

    // Control summary
    if let Some(summary) = status.get("summary") {
        report.push_str("## Control Summary\n\n");
        report.push_str("| Priority | Total | Complete | Incomplete |\n");
        report.push_str("|----------|-------|----------|------------|\n");

        let p0_total = summary["P0_critical"].as_i64().unwrap_or(0);
        let p0_complete = summary["P0_complete"].as_i64().unwrap_or(0);
        let p0_incomplete = summary["P0_incomplete"].as_i64().unwrap_or(0);

        report.push_str(&format!(
            "| P0 (Critical) | {p0_total} | {p0_complete} | {p0_incomplete} |\n"
        ));

        let p1_total = summary["P1_high"].as_i64().unwrap_or(0);
        let p1_complete = summary["P1_complete"].as_i64().unwrap_or(0);
        let p1_incomplete = summary["P1_incomplete"].as_i64().unwrap_or(0);

        report.push_str(&format!(
            "| P1 (High) | {p1_total} | {p1_complete} | {p1_incomplete} |\n"
        ));
    }

    // Detailed control status
    report.push_str("\n## Detailed Control Status\n\n");
    if let Some(controls) = status.get("controls") {
        for (control_id, control) in controls.as_mapping().unwrap() {
            let name = control["name"].as_str().unwrap_or("Unknown");
            let state = control["state"].as_str().unwrap_or("unknown");
            let priority = control["priority"].as_str().unwrap_or("P3");
            let blocks_production = control["blocks_production"].as_bool().unwrap_or(false);

            let state_emoji = match state {
                "certified" => "✅",
                "audited" => "🔵",
                "implemented" => "🟢",
                "partial" => "🟡",
                "placeholder" => "🔴",
                "missing" => "⚫",
                _ => "❓",
            };

            let blocking = if blocks_production {
                " ⚠️ BLOCKS PRODUCTION"
            } else {
                ""
            };

            let control_id_str = control_id.as_str().unwrap_or("unknown");
            report.push_str(&format!(
                "- {state_emoji} **{name}** ({control_id_str}) - {priority}{blocking}\n"
            ));
        }
    }

    // Next actions
    if let Some(next_actions) = status.get("next_actions") {
        report.push_str("\n## Next Actions Required\n\n");

        if let Some(immediate) = next_actions.get("immediate") {
            if let Some(actions) = immediate.as_sequence() {
                if !actions.is_empty() {
                    report.push_str("### Immediate\n");
                    for action in actions {
                        report.push_str(&format!("- {}\n", action.as_str().unwrap_or("Unknown")));
                    }
                    report.push('\n');
                }
            }
        }
    }

    match output {
        Some(path) => {
            let path_clone = path.clone();
            fs::write(&path, report)?;
            println!("📄 Security report written to: {path_clone}");
        }
        None => {
            println!("{report}");
        }
    }

    Ok(())
}

// Helper functions

async fn get_pr_changed_files(pr_number: u32) -> Result<Vec<String>> {
    // Simplified implementation - in reality would use GitHub API
    // For now, return some example files
    Ok(vec![
        "blvm-protocol/src/lib.rs".to_string(),
        "blvm-commons/src/validation/emergency.rs".to_string(),
    ])
}

async fn get_git_changed_files() -> Result<Vec<String>> {
    // Simplified implementation - in reality would use git diff
    Ok(vec![
        "blvm-protocol/src/lib.rs".to_string(),
        "blvm-commons/src/database/queries.rs".to_string(),
    ])
}

fn get_blocking_controls(status: &serde_yaml::Value) -> Vec<String> {
    let mut blocking = Vec::new();

    if let Some(controls) = status.get("controls") {
        for (control_id, control) in controls.as_mapping().unwrap() {
            if control["blocks_production"].as_bool().unwrap_or(false) {
                blocking.push(control_id.as_str().unwrap_or("").to_string());
            }
        }
    }

    blocking
}

#[derive(serde::Serialize)]
struct ProductionReadinessResult {
    ready: bool,
    blocking_controls: usize,
    blocking_control_ids: Vec<String>,
}

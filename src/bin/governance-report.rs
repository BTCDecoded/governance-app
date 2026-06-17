//! Governance Report Generator
//!
//! Generates monthly governance reports showing:
//! - Merge distribution by maintainer
//! - PR statistics by tier
//! - Challenge statistics
//! - Maintainer activity
//!
//! Usage:
//!   DATABASE_URL=sqlite:governance.db cargo run --bin governance-report

use blvm_commons::reporting::governance_metrics::MetricsReporter;
use chrono::{DateTime, Utc};
use clap::{Parser, ValueEnum};
use std::env;

#[derive(Parser)]
#[command(name = "governance-report")]
#[command(about = "Generate governance metrics reports")]
struct Args {
    /// Month to generate report for (YYYY-MM format, defaults to current month)
    #[arg(short, long)]
    month: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    format: OutputFormat,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
    Markdown,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Get database URL from environment
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:governance.db".to_string());

    // Connect to database
    let pool = sqlx::SqlitePool::connect(&database_url).await?;
    let reporter = MetricsReporter::new(pool);

    // Parse month or use current month
    let month = if let Some(month_str) = args.month {
        DateTime::parse_from_rfc3339(&format!("{month_str}-01T00:00:00Z"))
            .map_err(|_| "Invalid month format. Use YYYY-MM")?
            .with_timezone(&Utc)
    } else {
        Utc::now()
    };

    // Generate report
    let report = reporter.generate_monthly_report(month).await?;

    // Format output
    let output = match args.format {
        OutputFormat::Json => serde_json::to_string_pretty(&report)?,
        OutputFormat::Markdown => format_report_markdown(&report),
    };

    // Write to file or stdout
    if let Some(output_path) = args.output {
        std::fs::write(&output_path, output)?;
        println!("Report written to {output_path}");
    } else {
        println!("{output}");
    }

    Ok(())
}

fn format_report_markdown(
    report: &blvm_commons::reporting::governance_metrics::GovernanceReport,
) -> String {
    let mut md = String::new();

    md.push_str("# Governance Report\n\n");
    md.push_str(&format!(
        "**Period**: {} to {}\n\n",
        report.period_start.format("%Y-%m-%d"),
        report.period_end.format("%Y-%m-%d")
    ));

    // Merge Distribution
    md.push_str("## Merge Distribution\n\n");
    md.push_str(&format!(
        "Total merges: {}\n\n",
        report.merge_distribution.total_merges
    ));
    md.push_str("| Maintainer | Merges | Percentage |\n");
    md.push_str("|------------|--------|------------|\n");
    for maintainer in &report.merge_distribution.by_maintainer {
        md.push_str(&format!(
            "| {} | {} | {:.1}% |\n",
            maintainer.username, maintainer.count, maintainer.percentage
        ));
    }
    md.push('\n');

    // PR Statistics
    md.push_str("## PR Statistics\n\n");
    md.push_str(&format!(
        "- Total PRs: {}\n",
        report.pr_statistics.total_prs
    ));
    md.push_str(&format!("- Merged: {}\n", report.pr_statistics.merged));
    md.push_str(&format!("- Pending: {}\n", report.pr_statistics.pending));
    md.push_str(&format!(
        "- Rejected: {}\n\n",
        report.pr_statistics.rejected
    ));

    md.push_str("### By Tier\n\n");
    md.push_str("| Tier | Count |\n");
    md.push_str("|------|-------|\n");
    for tier in &report.pr_statistics.by_tier {
        md.push_str(&format!("| {} | {} |\n", tier.tier, tier.count));
    }
    md.push('\n');

    // Challenge Statistics
    md.push_str("## Challenge Statistics\n\n");
    md.push_str(&format!(
        "- Total challenges: {}\n",
        report.challenge_statistics.total_challenges
    ));
    md.push_str(&format!(
        "- Pending: {}\n",
        report.challenge_statistics.pending
    ));
    md.push_str(&format!(
        "- Resolved: {}\n",
        report.challenge_statistics.resolved
    ));
    md.push_str(&format!(
        "- Rejected: {}\n\n",
        report.challenge_statistics.rejected
    ));

    // Review Statistics
    md.push_str("## Review Statistics\n\n");
    md.push_str(&format!(
        "- Total reviews: {}\n",
        report.review_statistics.total_reviews
    ));
    md.push_str(&format!(
        "- Signatures with review: {}\n",
        report.review_statistics.signatures_with_review
    ));
    md.push_str(&format!(
        "- Signatures without review: {}\n",
        report.review_statistics.signatures_without_review
    ));
    md.push_str(&format!(
        "- PRs without reviews: {}\n",
        report.review_statistics.prs_without_reviews
    ));
    md.push_str(&format!(
        "- Reviews with comments: {:.1}%\n\n",
        report.review_statistics.average_review_comments
    ));

    md.push_str("### Reviews by Type\n\n");
    md.push_str("| Type | Count |\n");
    md.push_str("|------|-------|\n");
    for review_type in &report.review_statistics.reviews_by_type {
        md.push_str(&format!(
            "| {} | {} |\n",
            review_type.state, review_type.count
        ));
    }
    md.push('\n');

    // Maintainer Activity
    md.push_str("## Maintainer Activity\n\n");
    md.push_str("| Maintainer | PRs Merged | Signatures | Reviews | Challenges Created | Challenges Resolved |\n");
    md.push_str("|------------|------------|------------|---------|-------------------|-------------------|\n");
    for activity in &report.maintainer_activity {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            activity.username,
            activity.prs_merged,
            activity.signatures_given,
            activity.reviews_given,
            activity.challenges_created,
            activity.challenges_resolved
        ));
    }

    md
}

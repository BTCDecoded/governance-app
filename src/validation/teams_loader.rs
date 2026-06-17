//! Teams Configuration Loader
//!
//! Loads teams configuration and converts to nested multisig format

use crate::config::loader::TeamsConfig;
use crate::error::GovernanceError;
use crate::validation::nested_multisig::{Team, TeamMaintainer};
use std::path::Path;

/// Load teams configuration and convert to nested multisig format
pub fn load_teams_for_nested_multisig(config_path: &Path) -> Result<Vec<Team>, GovernanceError> {
    let teams_path = config_path.join("maintainers/teams.yml");

    if !teams_path.exists() {
        return Ok(Vec::new()); // Return empty if teams config doesn't exist
    }

    let content = std::fs::read_to_string(&teams_path)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to read teams.yml: {e}")))?;

    let teams_config: TeamsConfig = serde_yaml::from_str(&content)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse teams.yml: {e}")))?;

    // Convert TeamConfig to Team
    let teams: Vec<Team> = teams_config
        .teams
        .into_iter()
        .map(|tc| Team {
            id: tc.id,
            name: tc.name,
            maintainers: tc
                .maintainers
                .into_iter()
                .map(|tm| TeamMaintainer {
                    github: tm.github,
                    public_key: tm.public_key,
                })
                .collect(),
        })
        .collect();

    Ok(teams)
}

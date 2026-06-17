//! Dependency graph management for build orchestration

use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{info, warn};

/// Repository dependency graph for build ordering
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Map of repository name to its dependencies
    dependencies: HashMap<String, Vec<String>>,
    /// Organization name (e.g., "BTCDecoded" - the GitHub organization)
    organization: String,
}

impl DependencyGraph {
    /// Create a new dependency graph with the standard Bitcoin Commons build order
    pub fn new(organization: String) -> Self {
        let mut dependencies = HashMap::new();

        // Define build dependencies
        // Format: (repo_name, vec![dependencies])
        dependencies.insert("blvm-consensus".to_string(), vec![]);
        dependencies.insert(
            "blvm-protocol".to_string(),
            vec!["blvm-consensus".to_string()],
        );
        dependencies.insert(
            "blvm-node".to_string(),
            vec!["blvm-protocol".to_string(), "blvm-consensus".to_string()],
        );
        dependencies.insert("blvm-sdk".to_string(), vec![]); // Independent, can build in parallel with blvm-consensus
        dependencies.insert("blvm".to_string(), vec!["blvm-node".to_string()]);
        dependencies.insert("blvm-commons".to_string(), vec!["blvm-sdk".to_string()]);

        Self {
            dependencies,
            organization,
        }
    }

    /// Get all repositories in the graph
    pub fn repositories(&self) -> Vec<String> {
        self.dependencies.keys().cloned().collect()
    }

    /// Get dependencies for a repository
    pub fn get_dependencies(&self, repo: &str) -> Vec<String> {
        self.dependencies.get(repo).cloned().unwrap_or_default()
    }

    /// Get repositories that depend on this one
    pub fn get_dependents(&self, repo: &str) -> Vec<String> {
        self.dependencies
            .iter()
            .filter(|(_, deps)| deps.contains(&repo.to_string()))
            .map(|(repo_name, _)| repo_name.clone())
            .collect()
    }

    /// Get build order using topological sort
    /// Returns repositories in order they should be built
    pub fn get_build_order(&self) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize in-degree for all repos
        for repo in self.dependencies.keys() {
            in_degree.insert(repo.clone(), 0);
            graph.insert(repo.clone(), vec![]);
        }

        // Build graph and calculate in-degrees
        for (repo, deps) in &self.dependencies {
            for dep in deps {
                if !self.dependencies.contains_key(dep) {
                    warn!("Unknown dependency: {} (required by {})", dep, repo);
                    continue;
                }
                in_degree.entry(repo.clone()).and_modify(|d| *d += 1);
                graph.entry(dep.clone()).or_default().push(repo.clone());
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Add all nodes with no dependencies
        // Sort to ensure deterministic order (blvm-consensus should come first)
        let mut zero_degree_repos: Vec<String> = in_degree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(repo, _)| repo.clone())
            .collect();
        zero_degree_repos.sort();
        // Ensure blvm-consensus comes first if it has no dependencies
        if zero_degree_repos.contains(&"blvm-consensus".to_string()) {
            zero_degree_repos.retain(|r| r != "blvm-consensus");
            zero_degree_repos.insert(0, "blvm-consensus".to_string());
        }
        for repo in zero_degree_repos {
            queue.push_back(repo);
        }

        while let Some(repo) = queue.pop_front() {
            result.push(repo.clone());

            if let Some(dependents) = graph.get(&repo) {
                for dependent in dependents {
                    let degree = in_degree.get_mut(dependent).ok_or_else(|| {
                        format!("Internal error: missing in-degree for {dependent}")
                    })?;
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != self.dependencies.len() {
            return Err("Circular dependency detected in build graph".to_string());
        }

        info!("Build order: {:?}", result);
        Ok(result)
    }

    /// Get repositories that can be built in parallel
    /// Returns groups of repositories that can be built simultaneously
    pub fn get_parallel_groups(&self) -> Result<Vec<Vec<String>>, String> {
        let build_order = self.get_build_order()?;
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut completed = HashSet::new();

        for repo in build_order {
            let deps = self.get_dependencies(&repo);

            // Check if all dependencies are completed
            if deps.iter().all(|dep| completed.contains(dep)) {
                // Find or create a group for this repo
                let mut added = false;
                for group in &mut groups {
                    // Check if this repo can be added to an existing group
                    // (repos in same group must not depend on each other)
                    let can_add = group.iter().all(|r| {
                        !self.get_dependencies(&repo).contains(r)
                            && !self.get_dependencies(r).contains(&repo)
                    });

                    if can_add {
                        group.push(repo.clone());
                        added = true;
                        break;
                    }
                }

                if !added {
                    groups.push(vec![repo.clone()]);
                }

                completed.insert(repo);
            }
        }

        Ok(groups)
    }

    /// Add a custom dependency (for dynamic configuration)
    pub fn add_dependency(&mut self, repo: String, dependencies: Vec<String>) {
        self.dependencies.insert(repo, dependencies);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_order() {
        let graph = DependencyGraph::new("BTCDecoded".to_string());
        let order = graph.get_build_order().unwrap();

        // blvm-consensus should come first (no dependencies)
        assert!(
            order.iter().position(|r| r == "blvm-consensus").unwrap()
                < order.iter().position(|r| r == "blvm-protocol").unwrap()
        );

        // blvm-protocol should come before blvm-node
        assert!(
            order.iter().position(|r| r == "blvm-protocol").unwrap()
                < order.iter().position(|r| r == "blvm-node").unwrap()
        );
    }

    #[test]
    fn test_dependencies() {
        let graph = DependencyGraph::new("BTCDecoded".to_string());
        let deps = graph.get_dependencies("blvm-node");

        assert!(deps.contains(&"blvm-protocol".to_string()));
        assert!(deps.contains(&"blvm-consensus".to_string()));
    }

    #[test]
    fn test_dependents() {
        let graph = DependencyGraph::new("BTCDecoded".to_string());
        let dependents = graph.get_dependents("blvm-consensus");

        assert!(dependents.contains(&"blvm-protocol".to_string()));
    }
}

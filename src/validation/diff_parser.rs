//! Diff Parser for Consensus Modification Verification
//!
//! Parses unified diff format to detect non-import changes in consensus-critical files.
//! Used by cross-layer validation to enforce "import-only" rules.

use crate::error::GovernanceError;
use regex::Regex;
use std::collections::HashSet;
use tracing::debug;

/// Represents a file diff with additions and deletions
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub filename: String,
    pub additions: Vec<String>,
    pub deletions: Vec<String>,
}

/// Diff parser for analyzing code changes
pub struct DiffParser;

impl DiffParser {
    /// Parse unified diff format
    ///
    /// Unified diff format:
    /// ```
    /// --- a/path/to/file.rs
    /// +++ b/path/to/file.rs
    /// @@ -start,count +start,count @@
    /// -deleted line
    /// +added line
    ///  unchanged line
    /// ```
    pub fn parse_unified_diff(diff: &str) -> Result<Vec<FileDiff>, GovernanceError> {
        let mut file_diffs = Vec::new();
        let mut current_file: Option<FileDiff> = None;
        let mut in_hunk = false;
        let mut hunk_additions = Vec::new();
        let mut hunk_deletions = Vec::new();

        // Regex to match file header lines
        let file_header_re = Regex::new(r"^(?:---|\+\+\+) (.+)$").unwrap();
        // Regex to match hunk header lines
        let hunk_header_re = Regex::new(r"^@@ .+ @@").unwrap();

        for line in diff.lines() {
            // Check for file header (--- or +++)
            if let Some(caps) = file_header_re.captures(line) {
                let path = caps.get(1).unwrap().as_str();
                // Remove a/ or b/ prefix if present
                let clean_path = path.strip_prefix("a/").unwrap_or(path);
                let clean_path = clean_path.strip_prefix("b/").unwrap_or(clean_path);

                // If we have a current file, save it
                if let Some(file) = current_file.take() {
                    file_diffs.push(file);
                }

                // Start new file (only on --- line, +++ will follow)
                if line.starts_with("---") {
                    current_file = Some(FileDiff {
                        filename: clean_path.to_string(),
                        additions: Vec::new(),
                        deletions: Vec::new(),
                    });
                    in_hunk = false;
                    hunk_additions.clear();
                    hunk_deletions.clear();
                }
                continue;
            }

            // Check for hunk header
            if hunk_header_re.is_match(line) {
                // Save previous hunk's changes to current file
                if let Some(ref mut file) = current_file {
                    file.additions.append(&mut hunk_additions);
                    file.deletions.append(&mut hunk_deletions);
                }
                in_hunk = true;
                hunk_additions.clear();
                hunk_deletions.clear();
                continue;
            }

            // Process diff lines within a hunk
            if in_hunk {
                if let Some(ref mut file) = current_file {
                    if line.starts_with("+") && !line.starts_with("+++") {
                        // Added line (remove the + prefix)
                        let content = line.strip_prefix("+").unwrap_or(line).to_string();
                        hunk_additions.push(content);
                    } else if line.starts_with("-") && !line.starts_with("---") {
                        // Deleted line (remove the - prefix)
                        let content = line.strip_prefix("-").unwrap_or(line).to_string();
                        hunk_deletions.push(content);
                    }
                    // Lines starting with space are context (unchanged), ignore them
                }
            }
        }

        // Save last file
        if let Some(mut file) = current_file {
            // Save any remaining hunk changes
            file.additions.append(&mut hunk_additions);
            file.deletions.append(&mut hunk_deletions);
            file_diffs.push(file);
        }

        debug!("Parsed {} file diffs", file_diffs.len());
        Ok(file_diffs)
    }

    /// Check if file changes are import-only
    ///
    /// Returns true if all changes are:
    /// - Import statements (use declarations)
    /// - Comments
    /// - Whitespace
    ///
    /// Returns false if any changes include:
    /// - Function definitions
    /// - Type definitions
    /// - Logic changes
    /// - Macro definitions
    pub fn is_import_only_changes(file_diff: &FileDiff) -> bool {
        // Check both additions and deletions
        for line in &file_diff.additions {
            if !Self::is_import_or_comment_line(line) {
                debug!(
                    "Non-import change detected in {}: {}",
                    file_diff.filename,
                    line.trim()
                );
                return false;
            }
        }

        for line in &file_diff.deletions {
            if !Self::is_import_or_comment_line(line) {
                debug!(
                    "Non-import change detected in {}: {}",
                    file_diff.filename,
                    line.trim()
                );
                return false;
            }
        }

        true
    }

    /// Check if a line is an import statement or comment
    fn is_import_or_comment_line(line: &str) -> bool {
        let trimmed = line.trim();

        // Empty lines or whitespace
        if trimmed.is_empty() {
            return true;
        }

        // Comments (single-line and multi-line)
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
            return true;
        }

        // Import statements (use declarations)
        // Match: use ... ;
        // Match: pub use ... ;
        // Match: #[...] use ... ;
        if trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
            return true;
        }

        // Attribute macros before use statements (e.g., #[cfg(...)] use ...)
        if trimmed.starts_with("#[") && trimmed.contains("use ") {
            return true;
        }

        // Module declarations (pub mod ...) - sometimes needed for imports
        if trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ") {
            return true;
        }

        false
    }

    /// Extract import statements from diff
    pub fn extract_imports(file_diff: &FileDiff) -> Vec<String> {
        let mut imports = HashSet::new();

        for line in &file_diff.additions {
            if Self::is_import_or_comment_line(line) && line.contains("use ") {
                imports.insert(line.trim().to_string());
            }
        }

        for line in &file_diff.deletions {
            if Self::is_import_or_comment_line(line) && line.contains("use ") {
                imports.insert(line.trim().to_string());
            }
        }

        imports.into_iter().collect()
    }

    /// Check if diff contains any consensus-critical changes
    ///
    /// Consensus-critical patterns:
    /// - Function definitions (pub fn, fn)
    /// - Type definitions (struct, enum, trait, impl)
    /// - Macro definitions (macro_rules!, #[macro_export])
    /// - Logic changes (if, match, return, etc.)
    pub fn contains_consensus_logic_changes(file_diff: &FileDiff) -> bool {
        // Patterns that indicate logic changes
        let logic_patterns = vec![
            "fn ",             // Function definitions
            "pub fn ",         // Public function definitions
            "struct ",         // Struct definitions
            "enum ",           // Enum definitions
            "trait ",          // Trait definitions
            "impl ",           // Implementation blocks
            "macro_rules!",    // Macro definitions
            "#[macro_export]", // Macro exports
            "if ",             // Conditional logic
            "match ",          // Pattern matching
            "return ",         // Return statements
            "-> ",             // Function return types
            "=> ",             // Match arms, closures
        ];

        for line in &file_diff.additions {
            let trimmed = line.trim();
            for pattern in &logic_patterns {
                if trimmed.contains(pattern) && !Self::is_import_or_comment_line(line) {
                    debug!(
                        "Consensus logic change detected in {}: {}",
                        file_diff.filename, trimmed
                    );
                    return true;
                }
            }
        }

        for line in &file_diff.deletions {
            let trimmed = line.trim();
            for pattern in &logic_patterns {
                if trimmed.contains(pattern) && !Self::is_import_or_comment_line(line) {
                    debug!(
                        "Consensus logic change detected in {}: {}",
                        file_diff.filename, trimmed
                    );
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_diff() {
        let diff = r#"--- a/src/block.rs
+++ b/src/block.rs
@@ -1,3 +1,4 @@
 use crate::types::Network;
+use crate::bip_validation;
 use std::collections::HashMap;
"#;

        let file_diffs = DiffParser::parse_unified_diff(diff).unwrap();
        assert_eq!(file_diffs.len(), 1);
        assert_eq!(file_diffs[0].filename, "src/block.rs");
        assert_eq!(file_diffs[0].additions.len(), 1);
        assert!(file_diffs[0].additions[0].contains("use crate::bip_validation"));
    }

    #[test]
    fn test_is_import_only() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec!["use crate::bip_validation;".to_string()],
            deletions: vec![],
        };

        assert!(DiffParser::is_import_only_changes(&diff));
    }

    #[test]
    fn test_is_not_import_only() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec!["pub fn connect_block() {".to_string()],
            deletions: vec![],
        };

        assert!(!DiffParser::is_import_only_changes(&diff));
    }

    #[test]
    fn test_contains_consensus_logic() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec!["    pub fn validate_block() {".to_string()],
            deletions: vec![],
        };

        assert!(DiffParser::contains_consensus_logic_changes(&diff));
    }

    #[test]
    fn test_parse_multiple_files() {
        let diff = r#"--- a/src/block.rs
+++ b/src/block.rs
@@ -1,3 +1,4 @@
 use crate::types::Network;
+use crate::bip_validation;
 use std::collections::HashMap;
--- a/src/transaction.rs
+++ b/src/transaction.rs
@@ -1,2 +1,3 @@
 use crate::types::Hash;
+use crate::validation;
"#;

        let file_diffs = DiffParser::parse_unified_diff(diff).unwrap();
        assert_eq!(file_diffs.len(), 2);
        assert_eq!(file_diffs[0].filename, "src/block.rs");
        assert_eq!(file_diffs[1].filename, "src/transaction.rs");
    }

    #[test]
    fn test_parse_diff_with_context_lines() {
        let diff = r#"--- a/src/block.rs
+++ b/src/block.rs
@@ -1,5 +1,6 @@
 use crate::types::Network;
 use std::collections::HashMap;
+use crate::bip_validation;
 
 pub fn connect_block() {
     // existing code
"#;

        let file_diffs = DiffParser::parse_unified_diff(diff).unwrap();
        assert_eq!(file_diffs.len(), 1);
        assert_eq!(file_diffs[0].additions.len(), 1);
        assert!(file_diffs[0].additions[0].contains("use crate::bip_validation"));
    }

    #[test]
    fn test_is_import_only_with_comments() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec![
                "// New import for BIP validation".to_string(),
                "use crate::bip_validation;".to_string(),
            ],
            deletions: vec![],
        };

        assert!(DiffParser::is_import_only_changes(&diff));
    }

    #[test]
    fn test_is_import_only_with_whitespace() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec![
                "".to_string(), // Empty line
                "use crate::bip_validation;".to_string(),
                "    ".to_string(), // Whitespace
            ],
            deletions: vec![],
        };

        assert!(DiffParser::is_import_only_changes(&diff));
    }

    #[test]
    fn test_is_not_import_only_with_function() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec![
                "use crate::bip_validation;".to_string(),
                "pub fn new_function() {".to_string(), // This should fail
            ],
            deletions: vec![],
        };

        assert!(!DiffParser::is_import_only_changes(&diff));
    }

    #[test]
    fn test_extract_imports() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec![
                "use crate::bip_validation;".to_string(),
                "use std::collections::HashMap;".to_string(),
                "pub fn connect_block() {".to_string(), // Not an import
            ],
            deletions: vec!["use crate::old_module;".to_string()],
        };

        let imports = DiffParser::extract_imports(&diff);
        assert_eq!(imports.len(), 3); // Should extract all imports
        assert!(imports.iter().any(|i| i.contains("bip_validation")));
        assert!(imports.iter().any(|i| i.contains("HashMap")));
        assert!(imports.iter().any(|i| i.contains("old_module")));
    }

    #[test]
    fn test_parse_empty_diff() {
        let diff = "";
        let file_diffs = DiffParser::parse_unified_diff(diff).unwrap();
        assert_eq!(file_diffs.len(), 0);
    }

    #[test]
    fn test_parse_diff_with_deletions_only() {
        let diff = FileDiff {
            filename: "src/block.rs".to_string(),
            additions: vec![],
            deletions: vec!["use crate::old_module;".to_string()],
        };

        assert!(DiffParser::is_import_only_changes(&diff));
    }
}

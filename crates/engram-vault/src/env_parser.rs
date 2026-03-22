//! Parse PR diffs to extract environment variable references.
//!
//! Detects env var usage across multiple languages and patterns:
//! - `.env` file entries (KEY=value)
//! - Node.js: `process.env.VAR_NAME`
//! - Rust: `std::env::var("VAR_NAME")`
//! - Python: `os.environ["VAR_NAME"]` and `os.getenv("VAR_NAME")`

use regex::Regex;
use serde::{Deserialize, Serialize};

/// A reference to an environment variable found in a PR diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVarRef {
    /// The environment variable name (e.g. `DATABASE_URL`).
    pub name: String,
    /// The source file where the reference was found (extracted from diff header).
    pub source_file: String,
    /// The line number within the diff hunk (approximate).
    pub line: usize,
}

/// Extract all environment variable references from a unified diff string.
///
/// Scans added lines (starting with `+`) for known env var access patterns
/// across multiple languages and `.env` file formats.
pub fn extract_env_vars_from_diff(diff: &str) -> Vec<EnvVarRef> {
    let mut results: Vec<EnvVarRef> = Vec::new();

    // Regex patterns for env var references
    let dot_env_re = Regex::new(r"^([A-Z][A-Z0-9_]{1,})=").expect("invalid regex");
    let process_env_re = Regex::new(r"process\.env\.([A-Z][A-Z0-9_]{1,})").expect("invalid regex");
    let rust_env_re = Regex::new(r#"std::env::var\(\s*"([A-Z][A-Z0-9_]{1,})"\s*\)"#).expect("invalid regex");
    let python_environ_re = Regex::new(r#"os\.environ\[\s*["']([A-Z][A-Z0-9_]{1,})["']\s*\]"#).expect("invalid regex");
    let python_getenv_re = Regex::new(r#"os\.getenv\(\s*["']([A-Z][A-Z0-9_]{1,})["']\s*"#).expect("invalid regex");

    let mut current_file = String::from("<unknown>");
    let mut line_in_hunk: usize = 0;

    for raw_line in diff.lines() {
        // Track which file we're in from diff headers: +++ b/path/to/file
        if let Some(rest) = raw_line.strip_prefix("+++ b/") {
            current_file = rest.to_string();
            line_in_hunk = 0;
            continue;
        }

        // Track hunk headers for approximate line numbers: @@ -a,b +c,d @@
        if raw_line.starts_with("@@") {
            // Parse the +c part for new-file line number
            if let Some(plus_section) = raw_line.split('+').nth(1) {
                if let Some(start_str) = plus_section.split(',').next() {
                    if let Ok(start) = start_str.trim().parse::<usize>() {
                        line_in_hunk = start;
                    }
                }
            }
            continue;
        }

        // We only care about added lines
        if !raw_line.starts_with('+') || raw_line.starts_with("+++") {
            if !raw_line.starts_with('-') {
                line_in_hunk += 1;
            }
            continue;
        }

        let content = &raw_line[1..]; // Strip the leading '+'

        // Check if this is a .env file line
        let is_env_file = current_file.ends_with(".env")
            || current_file.ends_with(".env.example")
            || current_file.ends_with(".env.local")
            || current_file.ends_with(".env.production")
            || current_file.ends_with(".env.development")
            || current_file.ends_with(".env.staging")
            || current_file.ends_with(".env.template");

        if is_env_file {
            for cap in dot_env_re.captures_iter(content) {
                add_unique(&mut results, EnvVarRef {
                    name: cap[1].to_string(),
                    source_file: current_file.clone(),
                    line: line_in_hunk,
                });
            }
        }

        // process.env.XXX (JavaScript/TypeScript)
        for cap in process_env_re.captures_iter(content) {
            add_unique(&mut results, EnvVarRef {
                name: cap[1].to_string(),
                source_file: current_file.clone(),
                line: line_in_hunk,
            });
        }

        // std::env::var("XXX") (Rust)
        for cap in rust_env_re.captures_iter(content) {
            add_unique(&mut results, EnvVarRef {
                name: cap[1].to_string(),
                source_file: current_file.clone(),
                line: line_in_hunk,
            });
        }

        // os.environ["XXX"] (Python)
        for cap in python_environ_re.captures_iter(content) {
            add_unique(&mut results, EnvVarRef {
                name: cap[1].to_string(),
                source_file: current_file.clone(),
                line: line_in_hunk,
            });
        }

        // os.getenv("XXX") (Python)
        for cap in python_getenv_re.captures_iter(content) {
            add_unique(&mut results, EnvVarRef {
                name: cap[1].to_string(),
                source_file: current_file.clone(),
                line: line_in_hunk,
            });
        }

        line_in_hunk += 1;
    }

    results
}

/// Add an EnvVarRef only if the same (name, source_file) pair doesn't already exist.
fn add_unique(results: &mut Vec<EnvVarRef>, var_ref: EnvVarRef) {
    let exists = results.iter().any(|r| r.name == var_ref.name && r.source_file == var_ref.source_file);
    if !exists {
        results.push(var_ref);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dot_env() {
        let diff = r#"
diff --git a/.env.example b/.env.example
--- a/.env.example
+++ b/.env.example
@@ -1,2 +1,4 @@
 DATABASE_URL=postgres://localhost/db
+REDIS_URL=redis://localhost:6379
+API_SECRET_KEY=changeme
"#;
        let vars = extract_env_vars_from_diff(diff);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"REDIS_URL"));
        assert!(names.contains(&"API_SECRET_KEY"));
        // DATABASE_URL is context (no +), should not appear
        assert!(!names.contains(&"DATABASE_URL"));
    }

    #[test]
    fn test_extract_process_env() {
        let diff = r#"
diff --git a/src/config.ts b/src/config.ts
--- a/src/config.ts
+++ b/src/config.ts
@@ -5,6 +5,8 @@
+const dbUrl = process.env.DATABASE_URL;
+const secret = process.env.JWT_SECRET;
"#;
        let vars = extract_env_vars_from_diff(diff);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"DATABASE_URL"));
        assert!(names.contains(&"JWT_SECRET"));
    }

    #[test]
    fn test_extract_rust_env() {
        let diff = r#"
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
+let key = std::env::var("API_KEY").expect("missing");
+let port = std::env::var("PORT").unwrap_or_default();
"#;
        let vars = extract_env_vars_from_diff(diff);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"API_KEY"));
        assert!(names.contains(&"PORT"));
    }

    #[test]
    fn test_extract_python_env() {
        let diff = r#"
diff --git a/app/config.py b/app/config.py
--- a/app/config.py
+++ b/app/config.py
@@ -1,2 +1,4 @@
+db = os.environ["DATABASE_URL"]
+secret = os.getenv("SECRET_KEY")
"#;
        let vars = extract_env_vars_from_diff(diff);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert!(names.contains(&"DATABASE_URL"));
        assert!(names.contains(&"SECRET_KEY"));
    }

    #[test]
    fn test_no_duplicates_same_file() {
        let diff = r#"
diff --git a/src/app.ts b/src/app.ts
--- a/src/app.ts
+++ b/src/app.ts
@@ -1,2 +1,4 @@
+const a = process.env.API_KEY;
+const b = process.env.API_KEY;
"#;
        let vars = extract_env_vars_from_diff(diff);
        assert_eq!(vars.len(), 1);
    }
}

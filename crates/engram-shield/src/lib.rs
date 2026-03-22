pub mod agent;
pub mod audit_parser;
pub mod cve_deduplicator;
pub mod prompts;

// Re-export key types for convenience
pub use audit_parser::VulnFinding;
pub use cve_deduplicator::{generate_package_id, deduplicate, DeduplicationResult};

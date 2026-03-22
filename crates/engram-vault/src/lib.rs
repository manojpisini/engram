//! engram-vault: Environment variable tracking and secret rotation agent.
//!
//! This crate implements the ENGRAM Vault layer, responsible for:
//! - Detecting new environment variable references in merged PRs
//! - Scaffolding env var records when RFCs are approved
//! - Daily rotation policy compliance checks
//! - Three-way environment diff (dev / staging / prod)
//! - Claude-powered analysis of env config drift

pub mod agent;
pub mod env_parser;
pub mod rotation_checker;
pub mod env_differ;
pub mod prompts;

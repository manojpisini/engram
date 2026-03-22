//! engram-review — PR code review generation agent for ENGRAM.
//!
//! This crate provides:
//! - [`agent`] — Main event loop handling PrOpened, PrMerged, ReviewPatternCreated
//! - [`pr_analyzer`] — PR diff analysis against playbook rules
//! - [`pattern_extractor`] — Code pattern extraction and frequency tracking
//! - [`debt_tracker`] — Tech Debt promotion when patterns exceed thresholds
//! - [`prompts`] — Claude prompt templates for review generation

pub mod agent;
pub mod pr_analyzer;
pub mod pattern_extractor;
pub mod debt_tracker;
pub mod prompts;

//! **engram-atlas** — Module documentation and onboarding agent.
//!
//! This crate implements the Atlas layer of the ENGRAM system, responsible for:
//! - Generating and maintaining module documentation via Claude summarization.
//! - Detecting knowledge gaps (undocumented modules, stale docs, orphaned RFCs).
//! - Scaffolding role-specific onboarding tracks for new engineers.

pub mod agent;
pub mod gap_detector;
pub mod module_summarizer;
pub mod onboarding_generator;
pub mod prompts;

// Re-export key types for convenience.
pub use agent::run;
pub use gap_detector::{GapSeverity, GapType, KnowledgeGap, ModuleInfo, RfcInfo};
pub use module_summarizer::ModuleSummary;
pub use onboarding_generator::{OnboardingStep, OnboardingTrack, StepType};
pub use prompts::ModuleSummaryContext;

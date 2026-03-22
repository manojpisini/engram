use serde::{Deserialize, Serialize};

/// All event types that flow through the ENGRAM event router.
/// engram-core dispatches these to the appropriate layer agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngramEvent {
    PrOpened {
        repo: String,
        pr_number: u64,
        diff: String,
        title: String,
        description: String,
        author: String,
        branch: String,
        target_branch: String,
    },
    PrMerged {
        repo: String,
        pr_number: u64,
        diff: String,
        branch: String,
        commit_sha: String,
        title: String,
        author: String,
        rfc_references: Vec<String>,
    },
    CiBenchmarkPosted {
        project_id: String,
        raw_json: String,
        commit_sha: String,
        branch: String,
    },
    CiAuditPosted {
        project_id: String,
        raw_output: String,
        tool: AuditTool,
        commit_sha: String,
        branch: String,
    },
    RfcCreated {
        rfc_notion_page_id: String,
        rfc_id: String,
        project_id: String,
    },
    RfcApproved {
        rfc_notion_page_id: String,
        rfc_id: String,
        project_id: String,
        required_env_vars: Vec<String>,
        affected_modules: Vec<String>,
        banned_patterns: Vec<String>,
    },
    RegressionDetected {
        regression_notion_page_id: String,
        severity: Severity,
        metric_name: String,
        delta_pct: f64,
        project_id: String,
        related_pr: Option<String>,
    },
    CveDetected {
        dependency_notion_page_id: String,
        package_name: String,
        cve_ids: Vec<String>,
        severity: Severity,
        project_id: String,
    },
    SecretRotationDue {
        var_notion_page_id: String,
        var_name: String,
        days_overdue: i64,
        project_id: String,
    },
    EnvVarMissingInProd {
        var_notion_page_id: String,
        var_name: String,
        project_id: String,
    },
    NewEngineerOnboards {
        engineer_name: String,
        role: Role,
        project_id: String,
        /// The GitHub repo (owner/name) this onboarding is for.
        repo: String,
    },
    ReviewPatternCreated {
        pattern_notion_page_id: String,
        pattern_name: String,
        frequency: u32,
        project_id: String,
    },
    WeeklyDigestTrigger {
        project_id: String,
    },
    DailyAuditTrigger {
        project_id: String,
    },
    WeeklyRfcStalenessTrigger {
        project_id: String,
    },
    DailyRotationCheckTrigger {
        project_id: String,
    },
    WeeklyKnowledgeGapTrigger {
        project_id: String,
    },
    ReleaseCreated {
        project_id: String,
        version: String,
        milestone: String,
    },
    /// Fired after initial Notion setup completes — triggers all intelligence
    /// layers to generate their baseline data and an onboarding document.
    SetupComplete {
        project_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditTool {
    CargoAudit,
    NpmAudit,
    PipAudit,
    OsvScanner,
}

impl std::fmt::Display for AuditTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditTool::CargoAudit => write!(f, "cargo-audit"),
            AuditTool::NpmAudit => write!(f, "npm-audit"),
            AuditTool::PipAudit => write!(f, "pip-audit"),
            AuditTool::OsvScanner => write!(f, "osv-scanner"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "Critical"),
            Severity::High => write!(f, "High"),
            Severity::Medium => write!(f, "Medium"),
            Severity::Low => write!(f, "Low"),
            Severity::Info => write!(f, "Info"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Backend,
    Frontend,
    DevOps,
    FullStack,
    OssContributor,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Backend => write!(f, "Backend"),
            Role::Frontend => write!(f, "Frontend"),
            Role::DevOps => write!(f, "DevOps"),
            Role::FullStack => write!(f, "Full-Stack"),
            Role::OssContributor => write!(f, "OSS Contributor"),
        }
    }
}

/// Enum for all source layers in ENGRAM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceLayer {
    Decisions,
    Pulse,
    Shield,
    Atlas,
    Vault,
    Review,
}

impl std::fmt::Display for SourceLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceLayer::Decisions => write!(f, "Decisions"),
            SourceLayer::Pulse => write!(f, "Pulse"),
            SourceLayer::Shield => write!(f, "Shield"),
            SourceLayer::Atlas => write!(f, "Atlas"),
            SourceLayer::Vault => write!(f, "Vault"),
            SourceLayer::Review => write!(f, "Review"),
        }
    }
}

/// Event types for the Timeline/Events database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineEventType {
    RfcCreated,
    RfcApproved,
    RegressionDetected,
    CveFound,
    PrMerged,
    SecretRotated,
    NewEngineer,
    ModuleUpdated,
    DebtCreated,
    HealthReport,
    ConfigMismatch,
    RotationDue,
}

impl std::fmt::Display for TimelineEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TimelineEventType::RfcCreated => "RFC Created",
            TimelineEventType::RfcApproved => "RFC Approved",
            TimelineEventType::RegressionDetected => "Regression Detected",
            TimelineEventType::CveFound => "CVE Found",
            TimelineEventType::PrMerged => "PR Merged",
            TimelineEventType::SecretRotated => "Secret Rotated",
            TimelineEventType::NewEngineer => "New Engineer",
            TimelineEventType::ModuleUpdated => "Module Updated",
            TimelineEventType::DebtCreated => "Debt Created",
            TimelineEventType::HealthReport => "Health Report",
            TimelineEventType::ConfigMismatch => "Config Mismatch",
            TimelineEventType::RotationDue => "Rotation Due",
        };
        write!(f, "{s}")
    }
}

/// RFC lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RfcStatus {
    Draft,
    UnderReview,
    Approved,
    Implementing,
    Implemented,
    Deprecated,
}

impl std::fmt::Display for RfcStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RfcStatus::Draft => "Draft",
            RfcStatus::UnderReview => "Under Review",
            RfcStatus::Approved => "Approved",
            RfcStatus::Implementing => "Implementing",
            RfcStatus::Implemented => "Implemented",
            RfcStatus::Deprecated => "Deprecated",
        };
        write!(f, "{s}")
    }
}

/// Benchmark status thresholds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkStatus {
    Normal,
    Warning,
    Regression,
    Critical,
}

impl BenchmarkStatus {
    pub fn from_delta(delta_pct: f64, warning: f64, critical: f64, production: f64) -> Self {
        let abs = delta_pct.abs();
        if abs > production {
            BenchmarkStatus::Critical
        } else if abs > critical {
            BenchmarkStatus::Regression
        } else if abs > warning {
            BenchmarkStatus::Warning
        } else {
            BenchmarkStatus::Normal
        }
    }
}

impl std::fmt::Display for BenchmarkStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchmarkStatus::Normal => write!(f, "Normal"),
            BenchmarkStatus::Warning => write!(f, "Warning"),
            BenchmarkStatus::Regression => write!(f, "Regression"),
            BenchmarkStatus::Critical => write!(f, "Critical"),
        }
    }
}

/// Metric types for benchmarks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Latency,
    Throughput,
    Memory,
    Cpu,
    BinarySize,
    StartupTime,
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetricType::Latency => write!(f, "Latency"),
            MetricType::Throughput => write!(f, "Throughput"),
            MetricType::Memory => write!(f, "Memory"),
            MetricType::Cpu => write!(f, "CPU"),
            MetricType::BinarySize => write!(f, "Binary Size"),
            MetricType::StartupTime => write!(f, "Startup Time"),
        }
    }
}

/// Triage status for dependencies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriageStatus {
    Unreviewed,
    AcceptedRisk,
    FixScheduled,
    Fixed,
    WontFix,
}

impl std::fmt::Display for TriageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriageStatus::Unreviewed => write!(f, "New"),
            TriageStatus::AcceptedRisk => write!(f, "Accepted Risk"),
            TriageStatus::FixScheduled => write!(f, "Fix Scheduled"),
            TriageStatus::Fixed => write!(f, "Fixed"),
            TriageStatus::WontFix => write!(f, "Won't Fix"),
        }
    }
}

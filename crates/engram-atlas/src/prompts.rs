/// Claude prompt templates for the Atlas agent (module documentation & onboarding).

/// Generates a prompt that asks Claude to summarize a code module.
///
/// The expected JSON response schema:
/// ```json
/// {
///   "what_it_does": "...",
///   "main_abstractions": ["..."],
///   "entry_points": ["..."],
///   "common_gotchas": ["..."],
///   "complexity_score": 1-10,
///   "complexity_reasoning": "..."
/// }
/// ```
pub fn module_summarization_prompt(
    module_name: &str,
    module_path: &str,
    key_files: &[String],
    diff_context: Option<&str>,
) -> String {
    let files_list = if key_files.is_empty() {
        "No key files provided.".to_string()
    } else {
        key_files
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{}. {}", i + 1, f))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let diff_section = match diff_context {
        Some(diff) => format!(
            r#"
## Recent Changes (PR diff)
```
{diff}
```
"#
        ),
        None => String::new(),
    };

    format!(
        r#"You are a documentation assistant for the ENGRAM engineering system.

Analyze the following module and produce a structured summary.

## Module: {module_name}
- Path: {module_path}

## Key Files
{files_list}
{diff_section}
Produce a JSON object with exactly these fields:

- "what_it_does": A 2-3 sentence plain-English summary of the module's purpose. Avoid jargon; a new engineer should understand it.
- "main_abstractions": An array of strings naming the key structs, traits, or concepts in this module (max 8 items).
- "entry_points": An array of strings listing the primary public functions, endpoints, or commands a developer would call first (max 6 items).
- "common_gotchas": An array of strings describing non-obvious pitfalls, ordering constraints, or footguns (max 5 items).
- "complexity_score": An integer 1-10 rating the cognitive complexity (1 = trivial wrapper, 10 = requires deep domain knowledge).
- "complexity_reasoning": A single sentence justifying the complexity score.

Respond ONLY with the JSON object, no surrounding text."#
    )
}

/// Generates a prompt that asks Claude to create an onboarding track for a given role.
///
/// The expected JSON response schema:
/// ```json
/// {
///   "track_name": "...",
///   "estimated_hours": 40,
///   "steps": [
///     {
///       "title": "...",
///       "week_day": "Week 1 / Day 1",
///       "step_type": "setup|reading|hands-on|review",
///       "description": "...",
///       "estimated_time": "2h",
///       "related_module": "module-name or null"
///     }
///   ]
/// }
/// ```
pub fn onboarding_track_prompt(
    role: &str,
    project_id: &str,
    module_summaries: &[ModuleSummaryContext],
    env_vars: &[String],
    recent_rfc_titles: &[String],
) -> String {
    let modules_section = if module_summaries.is_empty() {
        "No modules documented yet.".to_string()
    } else {
        module_summaries
            .iter()
            .map(|m| {
                format!(
                    "- **{}** (complexity: {}): {}",
                    m.name, m.complexity_score, m.what_it_does
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let env_section = if env_vars.is_empty() {
        "No environment variables registered.".to_string()
    } else {
        env_vars
            .iter()
            .map(|v| format!("- {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let rfc_section = if recent_rfc_titles.is_empty() {
        "No recent RFCs.".to_string()
    } else {
        recent_rfc_titles
            .iter()
            .map(|r| format!("- {r}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You are an onboarding assistant. You are generating onboarding documentation for a new maintainer of a specific GitHub repository.

Generate a structured onboarding track for a new **{role}** engineer joining the **{project_id}** repository.

## Available Modules
{modules_section}

## Environment Variables to Set Up on Day 1
{env_section}

## Recent RFCs (for context reading)
{rfc_section}

Create an onboarding track as a JSON object with these fields:

- "track_name": A descriptive name like "{role} Onboarding - {{project}}"
- "estimated_hours": Total estimated hours for the entire track (integer).
- "steps": An array of step objects, each with:
  - "title": Short step title.
  - "week_day": When to do it, e.g. "Week 1 / Day 1", "Week 1 / Day 2", "Week 2 / Day 1".
  - "step_type": One of "setup", "reading", "hands-on", "review".
  - "description": 2-4 sentence description of what the engineer should do. For the Day 1 setup step, include ALL environment variables listed above.
  - "estimated_time": E.g. "2h", "30m", "4h".
  - "related_module": The module name this step relates to, or null if general.

Guidelines:
- Week 1 should focus on environment setup, codebase orientation, and reading key RFCs.
- Week 2+ should ramp into hands-on tasks starting with lower-complexity modules.
- Order modules by ascending complexity score when building the hands-on steps.
- Include at least one "review" step where the engineer presents what they learned.
- The Day 1 setup step MUST list every environment variable.

Respond ONLY with the JSON object, no surrounding text."#
    )
}

/// Minimal context about a module summary, used when building onboarding prompts.
pub struct ModuleSummaryContext {
    pub name: String,
    pub what_it_does: String,
    pub complexity_score: u8,
}

<p align="center">
  <img src="images/engram_banner.png" alt="ENGRAM Banner" width="100%">
</p>

<h3 align="center">Engineering Intelligence, etched in Notion.</h3>

<p align="center">
  <a href="#quick-start">Quick Start</a> &middot;
  <a href="#how-it-works">How It Works</a> &middot;
  <a href="#intelligence-layers">Intelligence Layers</a> &middot;
  <a href="#dashboard">Dashboard</a> &middot;
  <a href="#deployment">Deployment</a> &middot;
  <a href="SECURITY.md">Security</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Built_with-Rust-orange?style=flat-square&logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/AI-Claude_API-blueviolet?style=flat-square" alt="Claude">
  <img src="https://img.shields.io/badge/Data-Notion_API-black?style=flat-square&logo=notion" alt="Notion">
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="MIT">
</p>

---

ENGRAM is a self-organizing engineering intelligence platform. It connects your **GitHub repositories**, **Notion workspace**, and **Claude AI** into a single autonomous system that continuously analyzes your codebase and writes structured intelligence directly into Notion.

No polling. No manual data entry. GitHub webhooks push events to ENGRAM, 9 specialized AI agents interpret them using Claude, and every insight — security audits, performance regressions, architecture maps, RFC lifecycle tracking, team health reports, onboarding documents — is written as structured, queryable, relational data in your Notion workspace.

**Notion is the central nervous system.** Every metric, every decision, every piece of intelligence lives in 23 interconnected databases in your workspace.

### Key Features

- **Single binary** — dashboard, config template, and Windows icon all embedded via `rust-embed`. Just download and run.
- **9 AI agents** — each specialized for a different engineering intelligence domain, powered by Claude.
- **23 Notion databases** — auto-created with full schemas, cross-database relations, and rollup properties.
- **AI interpretations** — every data panel shows expandable AI-generated analysis (triage, drift scoring, review drafts, impact assessments).
- **JWT authentication** — secure login with argon2 password hashing and token-based sessions.
- **Setup wizard** — configure Notion, GitHub, and Claude from the browser on first launch.
- **Auto-extracting config** — `engram.toml` is generated from an embedded template on first run.
- **Demo mode** — load realistic mock data for demos and videos without touching production.

---

## Architecture

```
GitHub ──webhook──> ENGRAM Core ──broadcast──> 9 AI Agents ──write──> Notion
                         |                         |
                    axum HTTP                 Claude API
                         |
             Embedded Dashboard  <────read────  23 Databases
```

**ENGRAM Core** is a Rust daemon built on [axum](https://github.com/tokio-rs/axum). It receives GitHub webhook events, runs cron schedules, and routes everything through a tokio broadcast channel to a swarm of 9 intelligence agents. Each agent analyzes events using Claude and writes structured data to its own set of Notion databases. The embedded dashboard reads everything back from Notion via the ENGRAM API.

The dashboard is compiled into the binary at build time using [`rust-embed`](https://github.com/pyrossh/rust-embed) — no external files needed. The binary serves the dashboard directly from memory with proper MIME types and SPA routing.

---

## How It Works

### Data Flow

```
1. You push code, open PRs, merge branches on GitHub
2. GitHub sends webhook events to ENGRAM's /webhook/github endpoint
3. ENGRAM Core broadcasts each event to all 9 agents via tokio channels
4. Each agent picks up relevant events and analyzes them using Claude AI
5. Agents write structured intelligence to their respective Notion databases
6. AI analysis is fetched once per event — stored in Notion, not re-generated on each view
7. The embedded dashboard reads from Notion (via ENGRAM API) and displays everything
8. Agents cross-reference each other — Timeline correlates events,
   Health synthesizes scores, Decisions auto-generates RFCs for regressions
```

### Setup Workflow

```
Run ENGRAM (single binary)
    └──> Dashboard opens at http://localhost:3000
         └──> First-Start Setup Wizard appears
              │
              ├── 1. Notion Setup
              │      Enter integration token + parent page ID
              │      └──> Creates 23 Notion databases with schemas, relations, and seed data
              │
              ├── 2. GitHub Setup
              │      Enter Personal Access Token + repos to track
              │      Set a webhook secret (optional, recommended for production)
              │      └──> Dashboard shows the webhook URL to add to each repo
              │
              ├── 3. Claude / Anthropic API Setup
              │      Enter API key
              │      └──> Agents start analyzing data with Claude
              │
              └── 4. Server Configuration (optional)
                     Host, port, webhook secret
                     Default: 127.0.0.1:3000 (localhost only)

         After setup:
              │
              ├──> Add webhook URL to each GitHub repo's Settings → Webhooks
              │    (Server mode: https://your-domain/webhook/github)
              │    (Localhost mode: use .github/workflows/engram-notify.yml workflow instead)
              │
              ├──> GitHub starts sending webhook events
              │    └──> Agents automatically process events and write to Notion
              │
              ├──> Intelligence compounds over time
              │    More PRs, audits, benchmarks → richer Notion workspace
              │
              └──> Dashboard displays all data dynamically from Notion
```

No `.env` file needed. All configuration is done from the dashboard and persisted to `engram.toml`. The config file is auto-generated from an embedded template on first run. Environment variables are supported as an optional override for server deployments.

---

## Intelligence Layers

ENGRAM runs **9 specialized AI agents**, each responsible for a distinct intelligence domain. Every agent listens for relevant events, analyzes them with Claude, and writes structured results to Notion. AI analysis is performed **once per event** and stored — the dashboard reads cached intelligence from Notion, not from Claude on every page load.

| # | Layer | Agent | What It Does | AI Interpretation |
|---|-------|-------|-------------|-------------------|
| 1 | **Decisions** | `engram-decisions` | RFC lifecycle management, drift scoring between code and architecture decisions, auto-RFC generation when critical regressions are detected | Decision Rationale, Drift Score, Drift Notes |
| 2 | **Pulse** | `engram-pulse` | CI benchmark ingestion, regression detection, performance baseline tracking across commits and branches | AI Impact, AI Recommendation |
| 3 | **Shield** | `engram-shield` | Security audit parsing (cargo-audit, npm-audit, pip-audit, osv-scanner), CVE deduplication, vulnerability triage and severity scoring | AI Triage per CVE |
| 4 | **Atlas** | `engram-atlas` | Module documentation, knowledge gap detection, onboarding track generation for new maintainers of each tracked repository | AI Summary, Key Files |
| 5 | **Vault** | `engram-vault` | Environment config diffing across branches, secret rotation tracking, config mismatch alerts | AI Classification, Sensitivity |
| 6 | **Review** | `engram-review` | PR analysis, review pattern extraction, tech debt tracking, playbook-driven automated code review | Quality Score, Review Draft |
| 7 | **Health** | `engram-health` | Engineering health scoring, weekly digest generation, cross-layer synthesis combining metrics from all other agents | AI Narrative, Key Risks, Key Wins |
| 8 | **Timeline** | `engram-timeline` | Event correlation across all agents, cross-agent timeline, immutable audit trail for every change | — |
| 9 | **Release** | `engram-release` | Release note generation, milestone tracking, changelog automation from merged PRs and commits | AI Readiness, Release Notes, Migration Notes |

### 23 Notion Databases

All databases are created automatically during setup with full schemas, cross-database relations, and rollup properties:

| Domain | Databases |
|--------|-----------|
| **Projects** | Projects |
| **Decisions** | RFCs, RFC Comments |
| **Performance** | Benchmarks, Regressions, Performance Baselines |
| **Security** | Dependencies, Audit Runs |
| **Knowledge** | Modules, Onboarding Tracks, Onboarding Steps, Knowledge Gaps |
| **Config** | Env Config, Config Snapshots, Secret Rotation Log |
| **Review** | PR Reviews, Review Playbook, Review Patterns, Tech Debt |
| **Health** | Health Reports, Engineering Digest |
| **Timeline** | Events |
| **Release** | Releases |

### Automatic Onboarding

When you add a GitHub repository, ENGRAM's **Atlas** agent automatically generates onboarding documentation for new maintainers of that specific repo. This includes:

- Project description and purpose
- Codebase structure and architecture overview
- Toolchain details (build system, CI, dependencies)
- Key modules and knowledge gaps
- Getting started steps tailored to the repo

Each tracked repository gets its own onboarding track in Notion — not a generic template, but documentation specific to that repository's actual codebase.

---

## Dashboard

The dashboard is a single-page application embedded directly into the ENGRAM binary. It provides real-time visibility into all 9 intelligence layers with interactive charts, tables, and AI-generated interpretations.

### Panels

| Panel | Content |
|-------|---------|
| **Home** | Overall health score, recent activity, quick stats |
| **GitHub** | PRs, issues, commits, contributors from tracked repos |
| **Health** | Engineering health scores with radar chart, trend line, key risks/wins |
| **Timeline** | Cross-agent event correlation with layer breakdown |
| **Decisions** | RFC lifecycle with drift scoring and AI rationale (expandable) |
| **Pulse** | Benchmark tracking with regression detection and AI impact (expandable) |
| **Shield** | CVE dashboard with severity/triage charts and AI triage (expandable) |
| **Atlas** | Module coverage with AI summaries and key files (expandable) |
| **Onboarding** | Auto-generated onboarding tracks, steps, and knowledge gaps |
| **Vault** | Env config tracking with AI classification and sensitivity (expandable) |
| **Review** | PR reviews with quality scores and AI review drafts (expandable) |
| **Releases** | Release tracking with AI readiness and migration notes (expandable) |
| **Docs** | Technical debt tracking |
| **Settings** | Connection status, configuration, setup wizard |
| **About** | License, author info, architecture overview |

### AI Interpretations

Every intelligence table supports **click-to-expand** detail rows showing AI-generated analysis:

- **Decisions**: Decision Rationale, Drift Score with severity tag, Drift Notes
- **Shield**: AI Triage recommendation per CVE with risk context
- **Review**: Quality Score (0-100) and AI Review Draft
- **Atlas**: Full AI Summary, Key Files as code references
- **Vault**: Sensitivity level, AI Classification of variable purpose/risk
- **Releases**: AI Readiness assessment, Release Notes, Migration Notes
- **Pulse**: AI Impact analysis, AI Recommendation for regressions
- **Health**: Key Risks and Key Wins lists below the narrative

### Demo Mode

For demos and videos, load `demo.js` externally to populate the dashboard with realistic mock data:

```bash
# Serve dashboard with demo data (development only)
python -m http.server 8080 --directory dashboard
# Then visit http://localhost:8080?demo
```

Demo data is **excluded from the production binary** via `#[exclude = "demo.js"]` in the rust-embed configuration. It never ships with releases.

---

## Quick Start

### Prerequisites

- **Notion workspace** with an [internal integration](https://www.notion.so/profile/integrations) (full access)
- **GitHub Personal Access Token** — [create one here](https://github.com/settings/tokens) (scopes: `repo`, `read:org`)
- **Anthropic API key** — [get one here](https://console.anthropic.com/settings/keys)

### Download

Download the latest release for your platform from [GitHub Releases](https://github.com/manojpisini/engram/releases):

| Platform | Archive |
|----------|---------|
| Windows x86_64 | `engram-*-x86_64-pc-windows-msvc.zip` |
| Linux x86_64 | `engram-*-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `engram-*-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `engram-*-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `engram-*-aarch64-apple-darwin.tar.gz` |

Extract and run — the dashboard is embedded in the binary and `engram.toml` is auto-generated on first launch.

### Build from Source

```bash
# Clone
git clone https://github.com/manojpisini/engram.git
cd engram

# Build (Rust 1.75+ required)
cargo build --release --bin engram

# Run — dashboard opens at http://localhost:3000
./target/release/engram
```

On first launch, the **setup wizard** walks you through connecting Notion, GitHub, and Claude. No `.env` file, no manual config editing — everything is configured from the browser.

### Connecting GitHub Webhooks

After setup, you need to tell GitHub to send events to ENGRAM. There are two modes:

#### Server Mode (public IP / domain)

Add a webhook in each GitHub repo:

1. Go to **Repo Settings → Webhooks → Add webhook**
2. **Payload URL**: `https://your-domain/webhook/github`
3. **Content type**: `application/json`
4. **Secret**: The webhook secret you set during setup (optional but recommended)
5. **Events**: Select "Pull requests" (or "Send me everything")

#### Localhost Mode (no public IP)

Copy the CI workflow into each tracked repo:

```bash
cp .github/workflows/engram-notify.yml your-repo/.github/workflows/
```

Then set these as **GitHub repository variables** (Settings → Secrets and variables → Actions → Variables):
- `ENGRAM_WEBHOOK_URL` — Your ENGRAM instance URL (e.g., `http://localhost:3000`)
- `ENGRAM_PROJECT_ID` — The Notion Projects database page ID (shown in dashboard Settings)

The workflow posts PR events to ENGRAM on every PR open/update/merge.

---

## CI Integration

Drop these GitHub Actions workflows into your tracked repos to feed additional data to ENGRAM:

### Security Audits — `.github/workflows/audit.yml`

Runs `cargo-audit` daily and on every dependency change (`Cargo.lock`, `package-lock.json`, `requirements.txt`, `go.sum`). Posts vulnerability data to ENGRAM, where the **Shield** agent triages, deduplicates, and tracks CVEs in Notion. Comments on PRs with vulnerability counts.

### Benchmarks — `.github/workflows/benchmark.yml`

Runs Criterion.rs benchmarks on every push to `main` and on PRs. Posts results to ENGRAM, where the **Pulse** agent detects regressions against stored baselines. Comments on PRs with benchmark results.

### PR Notifications — `.github/workflows/engram-notify.yml`

Posts PR open/update/merge events with diff stats (additions, deletions, files changed) and RFC references. Triggers the **Review** agent for code analysis, **Decisions** for RFC drift scoring, and **Timeline** for event correlation.

**Required repository variables:**
| Variable | Value |
|----------|-------|
| `ENGRAM_WEBHOOK_URL` | Your ENGRAM instance URL |
| `ENGRAM_PROJECT_ID` | Notion Projects database page ID |

---

## Configuration

All configuration is done from the dashboard. Settings are persisted to `engram.toml`, which is auto-generated from an embedded template on first run:

```toml
[workspace]
notion_workspace_id = ""       # Set during setup

[auth]
notion_mcp_token = ""          # Set during setup
anthropic_api_key = ""         # Set during setup
github_token = ""              # Set during setup
webhook_secret = ""            # Optional, recommended for production

[server]
host = "127.0.0.1"            # Default: localhost only. Use 0.0.0.0 for server mode
port = 3000

[github]
repos = []                     # Repos to track, e.g. ["owner/repo"]

[claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096

[schedule]
daily_audit = "0 0 2 * * * *"           # 2:00 AM daily
weekly_digest = "0 0 9 * * 1 *"         # Monday 9:00 AM
weekly_rfc_staleness = "0 0 10 * * 1 *" # Monday 10:00 AM
daily_rotation_check = "0 0 3 * * * *"  # 3:00 AM daily
weekly_knowledge_gap_scan = "0 0 11 * * 1 *" # Monday 11:00 AM

[thresholds]
warning_delta_pct = 5.0        # Benchmark regression warning threshold
critical_delta_pct = 15.0      # Critical regression threshold
auto_rfc_severities = ["Critical", "High"]  # Auto-generate RFCs for these
rfc_stale_days = 14            # Mark RFCs stale after this many days

[databases]
# Auto-populated during Notion setup — 23 database IDs
```

---

## Deployment

### Localhost (Default)

ENGRAM binds to `127.0.0.1:3000` by default. This is sufficient for individual use. Use the `.github/workflows/engram-notify.yml` workflow to send GitHub events from CI to your local instance.

### Server / Domain

To host ENGRAM on a server with a public domain:

1. Set **Server Host** to `0.0.0.0` in dashboard Settings (listen on all interfaces)
2. Set the port you want
3. Place behind a reverse proxy (nginx / Caddy) for HTTPS and authentication
4. Add GitHub webhooks pointing directly to `https://your-domain/webhook/github`
5. Set a **webhook secret** for HMAC-SHA256 signature verification

### Background Service

**Linux (systemd):**
```bash
sudo cp packaging/systemd/engram.service /etc/systemd/system/
sudo systemctl enable --now engram
```

**macOS (launchd):**
```bash
sudo ./packaging/darwin/install.sh
```

**Windows (service):**
```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\install.ps1
```

### Packages

| Platform | Format | Install |
|----------|--------|---------|
| Debian / Ubuntu | `.deb` | `sudo dpkg -i engram_0.1.0_amd64.deb` |
| RHEL / Fedora | `.rpm` | `sudo rpm -i engram-0.1.0.rpm` |
| Arch Linux | `PKGBUILD` | `makepkg -si` |
| macOS | installer | `sudo ./packaging/darwin/install.sh` |
| Windows | PowerShell | `.\packaging\windows\install.ps1` |

---

## Project Structure

```
engram/
├── crates/
│   ├── engram-core/          Main daemon: axum HTTP, webhook listener,
│   │   ├── src/main.rs       scheduler, event router, auto-config extraction
│   │   ├── src/webhook.rs    embedded dashboard serving, API routes
│   │   └── build.rs          Windows executable icon embedding
│   ├── engram-types/         Shared types: config, events, Notion schemas
│   ├── engram-decisions/     Layer 1 — RFC lifecycle, drift scoring
│   ├── engram-pulse/         Layer 2 — Benchmark tracking, regression detection
│   ├── engram-shield/        Layer 3 — Security audit, CVE triage
│   ├── engram-atlas/         Layer 4 — Module docs, onboarding, knowledge gaps
│   ├── engram-vault/         Layer 5 — Env config, secret rotation
│   ├── engram-review/        Layer 6 — PR analysis, tech debt, review patterns
│   ├── engram-health/        Layer 7 — Health scoring, weekly digest
│   ├── engram-timeline/      Layer 8 — Event correlation, audit trail
│   └── engram-release/       Layer 9 — Release notes, changelog
├── dashboard/
│   ├── index.html            Single-page dashboard (embedded in binary at build time)
│   └── demo.js               Demo mock data (excluded from binary, dev-only)
├── images/
│   ├── engram_banner.png     README banner
│   ├── engram_logo.png       Logo source
│   └── engram.ico            Windows executable icon (multi-size)
├── .github/workflows/
│   ├── release.yml           Cross-platform release builds
│   ├── audit.yml             Security audit → Shield agent
│   ├── benchmark.yml         Benchmarks → Pulse agent
│   └── engram-notify.yml     PR events → Review, Decisions, Timeline agents
├── packaging/                Service files and installers
│   ├── systemd/              Linux systemd unit
│   ├── launchd/              macOS launchd plist
│   ├── debian/               .deb package builder
│   ├── rpm/                  RPM spec
│   ├── arch/                 Arch Linux PKGBUILD
│   ├── darwin/               macOS installer
│   ├── windows/              Windows service installer
│   └── build-all.sh          Cross-platform build script
├── engram.toml.example       Config template (embedded in binary, auto-extracted on first run)
├── SECURITY.md               Security policy and architecture
└── LICENSE                   MIT
```

---

## Built With

| Technology | Role |
|-----------|------|
| **Rust** | Core daemon, all 9 intelligence agents, type system |
| **Notion API** | Central data store — 23 databases with cross-references and relations |
| **Claude API** | Intelligence analysis — code review, summarization, RFC generation, triage, onboarding docs |
| **GitHub API** | Repository metadata, PR diffs, contributor info |
| **axum** | HTTP server, webhook listener, embedded dashboard serving |
| **tokio** | Async runtime, broadcast channels for agent communication |
| **rust-embed** | Compile-time embedding of dashboard and config template into the binary |
| **Vanilla JS** | Dashboard — zero dependencies, single HTML file, Chart.js for visualizations |
| **argon2 + JWT** | Authentication — secure password hashing and token-based sessions |

---

## Author

**Manoj Pisini** — [GitHub](https://github.com/manojpisini)

## License

[MIT](LICENSE)

---

<p align="center">
  Built for the <a href="https://dev.to/challenges/notion-2026-03-04">DEV.to x Notion MCP Challenge</a>
</p>

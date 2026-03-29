# engram-core

Core daemon for the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

This is the main binary crate. It runs the axum HTTP server, receives GitHub webhooks, routes events to 9 AI agents, serves the embedded dashboard, and manages the cron scheduler.

## Install

```bash
cargo install engram-core
```

This installs the `engram` binary. Run it to start the platform:

```bash
engram
# Dashboard opens at http://localhost:3000
```

On first launch, the setup wizard walks you through connecting Notion, GitHub, and Claude AI. The dashboard is embedded in the binary — no external files needed. Config (`engram.toml`) is auto-generated on first run.

## What's inside

- **Webhook listener** — receives GitHub events at `/webhook/github` with HMAC-SHA256 verification
- **Event router** — broadcasts events to all 9 agents via tokio channels
- **Cron scheduler** — daily audits, weekly digests, RFC staleness checks
- **Dashboard API** — REST endpoints reading from 23 Notion databases
- **Embedded dashboard** — single-page app compiled into the binary via rust-embed
- **JWT authentication** — login with argon2 password hashing
- **Setup wizard** — first-run configuration for Notion, GitHub, and Claude

## Architecture

```
GitHub ──webhook──> ENGRAM Core ──broadcast──> 9 AI Agents ──write──> Notion
                         |
                    axum HTTP
                         |
             Embedded Dashboard  <────read────  23 Databases
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

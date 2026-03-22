# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in ENGRAM, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please email: **security@manojpisini.com**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and aim to release a fix within 7 days for critical issues.

## Security Architecture

ENGRAM handles sensitive data including API tokens, webhook secrets, and repository metadata. Here is how we protect it:

### Token Storage
- All tokens (Notion, GitHub, Anthropic) are configured from the dashboard setup wizard and stored in `engram.toml` on the local filesystem
- No `.env` file is required — environment variables are only supported as an optional override for server deployments
- Tokens are **never** logged, exposed via API responses, or committed to version control
- The `GET /api/config` endpoint returns boolean flags (`github_configured: true/false`) rather than actual token values

### Webhook Verification
- GitHub webhook payloads are verified using HMAC-SHA256 signatures when a webhook secret is configured
- If no webhook secret is set, unsigned payloads are accepted (suitable for localhost development)
- Configure `webhook_secret` in the dashboard Settings for production use

### Network Security
- By default, ENGRAM binds to `127.0.0.1:3000` (localhost only)
- For local-only access, configure `host = "127.0.0.1"` in Settings
- CORS is permissive by default for dashboard access; restrict in production behind a reverse proxy
- No authentication is required for the dashboard by default; deploy behind a VPN or auth proxy for multi-user environments

### Data Flow
- ENGRAM reads from GitHub API and writes to Notion API
- Claude API is called for intelligence analysis; prompts contain repository metadata and diffs
- All external API calls use HTTPS
- No data is sent to any service other than the configured Notion workspace, GitHub API, and Anthropic API

### CI/CD Integration
- CI workflows post audit and benchmark results to ENGRAM via webhook
- Use GitHub repository variables (`ENGRAM_WEBHOOK_URL`, `ENGRAM_PROJECT_ID`) rather than secrets for non-sensitive configuration
- Webhook secret should be stored as a GitHub secret if signature verification is enabled

## Dependencies

ENGRAM uses `cargo-audit` for automated dependency vulnerability scanning. The `ci/audit.yml` workflow runs daily and on every dependency change.

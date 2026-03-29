# engram-shield

**Layer 3** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

Security audit parsing, CVE deduplication, and AI-powered vulnerability triage agent. Processes output from cargo-audit, npm-audit, pip-audit, and osv-scanner.

## Features

- Multi-format security audit parsing
- CVE deduplication using HashSet tracking
- AI-powered triage and severity scoring via Claude
- Writes structured data to Notion: Dependencies, Audit Runs

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-shield = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

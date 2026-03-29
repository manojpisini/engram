# engram-vault

**Layer 5** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

Environment config diffing, secret rotation tracking, and config mismatch alert agent. Monitors environment variables across branches and enforces rotation policies.

## Features

- Environment variable tracking across branches
- Secret rotation policy enforcement
- Config mismatch detection and alerts
- AI-powered sensitivity classification
- Writes structured data to Notion: Env Config, Config Snapshots, Secret Rotation Log

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-vault = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

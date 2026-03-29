# engram-types

Shared types, configuration, events, and Notion database schemas for the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

## What's inside

- **Config** — `EngramConfig` struct with serde deserialization for `engram.toml`
- **Events** — Event types for inter-agent communication via tokio broadcast channels
- **Notion schemas** — Database names and property constants for all 23 Notion databases
- **Client traits** — `AgentContext` for Notion and Claude API access

This crate is a dependency of all other ENGRAM crates. It contains no business logic — only type definitions and shared constants.

## Usage

```toml
[dependencies]
engram-types = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

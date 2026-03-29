# engram-decisions

**Layer 1** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

RFC lifecycle management, architectural drift scoring, and auto-RFC generation agent. Listens for PR merge events and analyzes code changes against existing architecture decisions using Claude AI.

## Features

- RFC lifecycle tracking (Draft, Under Review, Approved, Superseded)
- Drift scoring between merged code and architecture decisions
- Auto-generates RFCs when critical regressions are detected
- Writes structured data to Notion: RFCs, RFC Comments

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-decisions = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

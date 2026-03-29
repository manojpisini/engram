# engram-health

**Layer 7** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

Engineering health scoring, weekly digest generation, and cross-layer synthesis agent. Combines metrics from all other agents into a unified health score with AI-generated narratives.

## Features

- Cross-layer health score synthesis (0-100)
- Per-agent scoring (Decisions, Pulse, Shield, Atlas, Vault, Review)
- AI-generated weekly narratives with key risks and wins
- Weekly digest generation with trends and recommendations
- Writes structured data to Notion: Health Reports, Engineering Digest

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-health = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

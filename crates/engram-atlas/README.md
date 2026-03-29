# engram-atlas

**Layer 4** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

Module documentation, knowledge gap detection, and onboarding track generation agent. Analyzes repository structure and generates tailored onboarding plans for new maintainers.

## Features

- Automatic module documentation from codebase analysis
- Knowledge gap detection (ownership, documentation, bus factor)
- AI-generated onboarding tracks with step-by-step plans
- Writes structured data to Notion: Modules, Onboarding Tracks, Onboarding Steps, Knowledge Gaps

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-atlas = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

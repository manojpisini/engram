# engram-timeline

**Layer 8** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

Cross-agent event correlation and immutable audit trail agent. Records and correlates events from all 9 intelligence layers into a unified timeline.

## Features

- Cross-agent event correlation
- Immutable audit trail for all engineering changes
- Milestone detection and tagging
- Source layer attribution for traceability
- Writes structured data to Notion: Events

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-timeline = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

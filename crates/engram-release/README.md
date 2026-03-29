# engram-release

**Layer 9** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

Release note generation, milestone tracking, and changelog automation agent. Generates AI-powered release readiness assessments and migration notes from merged PRs.

## Features

- Automated release note generation from merged PRs
- AI readiness assessment per release
- Migration note generation
- Release gate validation (tests, regressions, CVEs)
- Writes structured data to Notion: Releases

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-release = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

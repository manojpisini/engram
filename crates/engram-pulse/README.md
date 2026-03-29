# engram-pulse

**Layer 2** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

CI benchmark ingestion, regression detection, and performance baseline tracking agent. Processes benchmark results from Criterion.rs and detects performance regressions across commits.

## Features

- Benchmark result ingestion from CI pipelines
- Regression detection against stored baselines
- Performance trend tracking across branches
- Writes structured data to Notion: Benchmarks, Regressions, Performance Baselines

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-pulse = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

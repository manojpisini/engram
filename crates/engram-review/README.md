# engram-review

**Layer 6** of the [ENGRAM](https://github.com/manojpisini/engram) engineering intelligence platform.

PR analysis, review pattern extraction, tech debt tracking, and AI-powered automated code review agent. Analyzes pull requests and generates quality scores and review drafts.

## Features

- Automated PR code review with AI analysis
- Quality scoring (0-100) per pull request
- Review pattern extraction and playbook generation
- Tech debt identification and tracking
- Writes structured data to Notion: PR Reviews, Review Playbook, Review Patterns, Tech Debt

## Usage

This crate is used as a dependency of `engram-core`. It is not intended to be used standalone.

```toml
[dependencies]
engram-review = "1.0.0"
```

## License

[MIT](https://github.com/manojpisini/engram/blob/main/LICENSE)

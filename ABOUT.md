# About Harness-Gate

Harness-Gate is configuration-driven: **`init -> verify -> one project decision`**.
Execution and quality configuration select collectors and policy; collectors
measure and the Rust core decides. Ecosystem/language/framework identity is pack
data, with no new language branches in generic schema, compilation,
orchestration, baselines, profiles, verification, reporting or CI. Extend through
collector + capability/policy pack + certification. See the
[workflow and architecture](docs/quality-workflow.md) for reference certification
limits and future composable combinations such as Vue + Go + PostgreSQL.
`quality evaluate` and `adapter run` are advanced interfaces.

## What is Harness-Gate?

Harness-Gate is a **reusable development workflow and architecture guard CLI** built in Rust. It provides a complete quality gate and workflow management system that keeps your codebase clean, secure, and maintainable.

## Purpose

Harness-Gate solves the common problem of scattered quality checks across Shell scripts, Git hooks, and CI configurations. Instead of maintaining complex scripts in multiple places, Harness-Gate provides:

- **Unified workflow orchestration** - All quality checks in one tool
- **Configuration over code** - Extend via TOML, not Rust code
- **Git-aware scoping** - Run only relevant checks for changed files
- **Built-in security** - Secrets scanning and architecture auditing
- **Fail-fast gates** - Catch issues early in development

## Key Features

### 🛡️ Security First
- **Secret scanning** with entropy detection
- **Architecture auditing** to enforce code standards
- **Git-staged secret detection** to prevent leaks before commit

### 🎯 Smart Scoping
- Analyzes Git changes to determine affected components
- Runs only relevant checks, saving time
- Supports monorepo and multi-component projects

### ⚙️ Flexible Workflow
- **Profiles**: Different check intensity (hook, CI, full)
- **Components**: Logical groupings (frontend, backend, docs)
- **Steps**: External commands with timeout and parsing
- **Services**: Temporary service lifecycle management

### 📊 Clear Reporting
- JSON and Markdown reports
- Error context extraction from logs
- Detailed violation summaries

## How It Works

```text
Git changed files
  ↓
Scope rules (which components changed?)
  ↓
Profile selection (hook/CI/full intensity?)
  ↓
Quality gates (secrets, audit)
  ↓
Component steps (build, test, lint)
  ↓
Reports (JSON, Markdown, logs)
```

## Who Should Use It?

Harness-Gate is ideal for:

- **Teams** enforcing architecture and security standards
- **Monorepos** with multiple components needing selective testing
- **Projects** wanting consistent checks across local and CI environments
- **Developers** tired of maintaining Shell script spaghetti

## Technology

- **Language**: Rust (for performance, safety, and cross-platform support)
- **Configuration**: TOML (human-readable, strongly-typed)
- **Integration**: Git hooks, CI/CD, local development

## Project Status

- **Version**: 0.4.0
- **License**: MIT
- **Platforms**: Linux, macOS, Windows
- **Status**: Production-ready, actively maintained

## Quick Example

```bash
# Initialize in your project
harness-gate init --preset rust-api

# Run quality checks on changed files
harness-gate verify --profile hook

# Scan for secrets in staged files
harness-gate secrets --staged

# Check architecture rules
harness-gate audit
```

## Philosophy

Harness-Gate follows these principles:

1. **Fail fast** - Catch issues before they reach CI
2. **Configuration over code** - Extend without Rust knowledge
3. **Git-aware** - Only check what changed
4. **Portable** - Same checks locally and in CI
5. **Transparent** - Clear reports on what failed and why

## Community

- **GitHub**: [musutrade/Harness-Gate](https://github.com/musutrade/Harness-Gate)
- **Issues**: Report bugs and request features on GitHub Issues
- **Contributing**: See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines
- **Documentation**: Full docs at [docs.rs/harness-gate](https://docs.rs/harness-gate)

## Comparison

| Feature | Harness-Gate | Shell Scripts | CI Only |
|---------|--------------|---------------|---------|
| Local & CI consistency | ✅ | ❌ | ❌ |
| Git-aware scoping | ✅ | Manual | Manual |
| Secret scanning | ✅ Built-in | External tool | External tool |
| Architecture auditing | ✅ Built-in | N/A | N/A |
| Configuration format | TOML | Various | YAML |
| Cross-platform | ✅ | Limited | ✅ |
| Timeout handling | ✅ | Manual | Built-in |
| Service lifecycle | ✅ | Manual | Manual |

## Getting Started

See [README.md](README.md) for installation instructions and quickstart guide.

## License

MIT License - see [LICENSE](LICENSE) for details.

---

**Built with ❤️ in Rust** | Maintained by the Harness-Gate team

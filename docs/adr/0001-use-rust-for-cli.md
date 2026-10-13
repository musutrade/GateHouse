# ADR-0001: Use Rust for CLI Implementation

## Status

**Accepted** (2024)

## Context

Harness-Gate is a development workflow and architecture guard CLI tool that needs to:

- Execute external commands and manage child processes
- Parse and validate complex TOML configurations
- Perform file system operations (Git integration, file scanning)
- Run concurrent operations (parallel file scanning, multiple step execution)
- Handle signals and interrupts gracefully
- Provide reliable error handling and clear error messages
- Be distributed as a single binary with minimal dependencies

The choice of implementation language significantly impacts:
- Performance (startup time, execution speed)
- Reliability (memory safety, error handling)
- Developer experience (type safety, tooling)
- Distribution (binary size, cross-platform compatibility)
- Ecosystem (available libraries for our use cases)

Alternative languages considered:
- **Go**: Good concurrency, single binary, but less type safety
- **Python**: Rich ecosystem, but requires runtime, slower startup
- **Node.js**: Good for scripting, but requires runtime
- **C/C++**: Maximum performance, but manual memory management

## Decision

We will implement Harness-Gate as a Rust CLI application.

## Consequences

### Positive

✅ **Memory Safety**: Rust's ownership system eliminates entire classes of bugs (null pointers, data races, use-after-free) without garbage collection overhead.

✅ **Performance**: Compiled binary with zero-cost abstractions provides fast startup and execution, critical for Git hooks and CI pipelines.

✅ **Excellent CLI Ecosystem**:
- `clap` for argument parsing with auto-generated help
- `serde` for configuration serialization/deserialization
- `rayon` for data parallelism
- `anyhow` for ergonomic error handling

✅ **Single Binary Distribution**: No runtime dependencies, easy to install via cargo or download pre-built binaries.

✅ **Cross-Platform**: Excellent support for Linux, macOS, and Windows with minimal platform-specific code.

✅ **Concurrency**: Safe concurrent operations (parallel file scanning) with Rust's type system preventing data races at compile time.

✅ **Type Safety**: Strong static typing catches configuration and logic errors early, especially valuable for complex configuration validation.

### Negative

⚠️ **Learning Curve**: Rust has a steeper learning curve than scripting languages, requiring contributors to understand ownership and lifetimes.

⚠️ **Compilation Time**: Slower compilation compared to interpreted languages, though incremental compilation helps.

⚠️ **Binary Size**: Larger than C but smaller than Go; mitigated with `strip` and `lto` optimizations.

⚠️ **Ecosystem Maturity**: Some areas less mature than Python/Node.js, though CLI tooling is excellent.

### Trade-offs We Accept

- **Longer compilation** in exchange for **runtime safety and performance**
- **Steeper learning curve** in exchange for **fewer runtime bugs**
- **More verbose code** in exchange for **explicit error handling**

## References

- [Rust Language](https://www.rust-lang.org/)
- [clap - Command Line Argument Parser](https://github.com/clap-rs/clap)
- [Why Rust for CLIs?](https://rust-cli.github.io/book/index.html)

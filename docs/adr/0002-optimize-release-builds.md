# ADR-0002: Optimize Release Builds with LTO and Strip

## Status

**Accepted** (2024-08-26)

## Context

The current release builds of Harness-Gate are not optimized for binary size and performance. Analysis shows:

- Default release profile settings are used
- No Link-Time Optimization (LTO) enabled
- Debug symbols included in release binaries
- Parallel codegen units (default 16) prioritize compile speed over runtime performance

For a CLI tool that users download and run frequently (Git hooks, CI pipelines), we should prioritize:
1. **Binary size** - Smaller downloads, faster distribution
2. **Startup time** - Critical for Git hooks
3. **Runtime performance** - Faster file scanning and command execution

Current state:
- Binary size: Not yet measured (needs release build)
- LTO: Disabled
- Strip: Disabled
- Codegen units: 16 (default)

Rust provides several optimization options via `[profile.release]` in Cargo.toml:
- `lto = true` - Link-Time Optimization across all crates
- `strip = true` - Remove debug symbols
- `codegen-units = 1` - Better optimization at cost of compile time
- `opt-level = "z"` - Optimize for size (alternative to default "3")

## Decision

We will enable aggressive release optimizations in `tools/harness-gate/Cargo.toml`:

```toml
[profile.release]
strip = true          # Remove debug symbols
lto = true            # Link-time optimization
codegen-units = 1     # Better optimization, slower compile
```

We will NOT use `opt-level = "z"` because:
- Performance is more important than minimal size for our use case
- The size reduction from "3" to "z" is often marginal
- "z" can sometimes produce slower code

## Consequences

### Positive

✅ **Smaller Binary Size**: Expect 20-40% size reduction from stripping alone, additional 10-20% from LTO.

✅ **Faster Startup**: LTO can improve startup time by 5-15%, critical for Git hooks.

✅ **Better Runtime Performance**: LTO enables cross-crate inlining and dead code elimination, improving file scanning and command execution.

✅ **No Runtime Cost**: These are compile-time optimizations only.

✅ **Industry Standard**: These settings are recommended by Rust CLI best practices.

### Negative

⚠️ **Longer Release Builds**: LTO + codegen-units=1 significantly increase compile time (2-5x slower). This affects:
- Local release testing
- CI release workflow duration
- But NOT development builds (debug profile unchanged)

⚠️ **Harder to Debug Released Binaries**: Stripped binaries cannot produce meaningful stack traces. Mitigations:
- Keep debug builds for development
- Encourage users to report issues with reproducible steps
- Consider separate debug symbol packages for critical debugging

⚠️ **Disk Space During Compilation**: LTO requires more temporary disk space.

### Measured Impact (Expected)

| Metric | Before | After (Expected) | Improvement |
|--------|--------|------------------|-------------|
| Binary Size | TBD | TBD | 30-50% smaller |
| Startup Time | TBD | TBD | 5-15% faster |
| Release Build Time | TBD | TBD | 2-5x slower |

**Note**: Will measure actual impact after implementation.

### Alternatives Considered

1. **opt-level = "z"** (optimize for size)
   - Rejected: Performance more important than minimal size
   - Size difference from "3" often marginal (<10%)

2. **lto = "thin"** (faster LTO variant)
   - Rejected: Release builds are infrequent enough that full LTO is acceptable
   - May reconsider if CI time becomes a bottleneck

3. **Separate size-optimized builds**
   - Rejected: Adds complexity for minimal user benefit
   - Users who need smaller binaries can use UPX compression

### Migration Path

1. Add profile to Cargo.toml
2. Measure actual impact (binary size, build time)
3. Update CI workflow if build timeout occurs (unlikely)
4. Document expected build times in CONTRIBUTING.md

## References

- [Cargo Profile Documentation](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Rust CLI Book - Distribution](https://rust-cli.github.io/book/tutorial/packaging.html)
- [min-sized-rust](https://github.com/johnthagen/min-sized-rust)

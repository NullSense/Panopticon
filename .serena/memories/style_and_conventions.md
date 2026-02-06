# Style & Conventions

## Rust Conventions
- Edition 2021
- Standard Rust naming: snake_case for functions/variables, CamelCase for types
- Module organization: `mod.rs` pattern for directories
- Error handling via `anyhow::Result` throughout
- Logging via `tracing` macros (info!, warn!, error!)
- `//!` doc comments for module-level documentation

## Code Patterns
- `clap` derive macros for CLI argument parsing
- `serde` derive for serialization/deserialization
- `tokio` async runtime with `#[tokio::main]`
- Integration tests in `tests/` directory using `tempfile` for temporary dirs
- Test assertions with `pretty_assertions`

## Task Completion Checklist
1. Run `cargo fmt` to format code
2. Run `cargo clippy` to check for lints
3. Run `cargo test` to ensure all tests pass
4. Run `cargo build` to verify compilation

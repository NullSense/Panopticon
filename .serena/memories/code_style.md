# Code Style and Conventions

## Rust Conventions
- Edition: 2021
- Standard Rust naming: `snake_case` for functions/variables, `PascalCase` for types
- Use `Result<T>` from `anyhow` for error handling in most cases
- Use `thiserror` for defining custom error types when needed

## Module Organization
- One module per file, `mod.rs` for directory modules
- Re-exports in `mod.rs` for public API
- Integrations separated by service (linear, github, vercel, claude)

## Error Handling
- Use `anyhow::Result<T>` for functions that can fail
- Use `?` operator for error propagation
- Log errors with `tracing::warn!` or `tracing::error!` when appropriate
- Graceful degradation: show cached data if API fails

## Async Patterns
- `#[tokio::main]` for main entry point
- `async fn` for I/O-bound operations
- Use `tokio::select!` for concurrent operations

## Documentation
- Doc comments (`///`) for public functions
- Inline comments for complex logic
- No excessive documentation - code should be self-explanatory

## TUI Patterns
- App struct holds all application state
- Methods on App for state mutations
- Separate ui.rs for rendering logic
- Constants for column indices and layout

## Testing
- Tests in `tests/` directory
- Use `pretty_assertions` for better diffs

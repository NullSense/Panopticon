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

## TUI Patterns (Elm Architecture)

### Message-Based Updates
- All user actions are `Message` enum variants in `message.rs`
- `input::dispatch(app, input_state, key)` maps keys to messages
- `App::update(msg)` processes messages, returns `bool` (true = quit)
- Pure unidirectional flow: Key Event → Message → State Update → Render

### Modal State
- Single `ModalState` enum (not boolean flags)
- Variants: `None`, `Help { tab }`, `LinkMenu { show_links_popup }`, `SortMenu`, `FilterMenu`, `Description`, `Resize`

### Input Handling
- `InputState` struct manages chord sequences (gg, d1-d9, c1-c9)
- Non-blocking timeout detection in main loop
- Context-aware dispatch based on current modal/search state

### Background Operations
- `start_background_refresh()` spawns async task
- `poll_refresh()` checks for results (non-blocking)
- Progress updates via `mpsc` channel

### UI Structure
- `ui.rs` contains all rendering logic (view layer)
- `App` struct holds all application state
- Constants for column indices and layout in `app.rs`

## Testing
- Tests in `tests/` directory
- Use `pretty_assertions` for better diffs

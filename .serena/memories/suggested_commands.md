# Suggested Commands

## Development
```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run (development)
cargo run

# Run with debug logging
RUST_LOG=panopticon=debug cargo run

# Check for errors without building
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint with clippy
cargo clippy
```

## Installation
```bash
# Install to PATH
cargo install --path .

# Binary location after release build
./target/release/panopticon
```

## Application Usage
```bash
# Initialize config (first run)
panopticon --init

# Run dashboard
panopticon

# Run as daemon (not yet implemented)
panopticon --daemon
```

## Useful System Commands (macOS/Darwin)
```bash
# Git operations
git status
git diff
git add -p
git commit -m "message"
git push

# Find files
find . -name "*.rs" -type f

# Search in files
grep -r "pattern" src/

# Process management
ps aux | grep panopticon
```

# Task Completion Checklist

## Before Completing a Task

### 1. Code Quality
```bash
# Format code
cargo fmt

# Run clippy lints
cargo clippy

# Check compilation
cargo check
```

### 2. Testing
```bash
# Run all tests
cargo test
```

### 3. Build Verification
```bash
# Ensure release build works
cargo build --release
```

## Common Issues to Check
- No unused imports or variables
- No clippy warnings
- All public functions have appropriate visibility
- Error handling is in place (no unwrap() on fallible operations in production code)
- Async functions properly awaited

## Commit Guidelines
- Use conventional commit format when appropriate
- Keep commits atomic and focused
- Include relevant Linear issue ID in branch name if applicable

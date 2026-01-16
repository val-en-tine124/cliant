# Contributing to Cliant

Thank you for your interest in contributing to Cliant! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Be respectful and professional in all interactions. We aim to maintain a welcoming and inclusive community.

## Getting Started

1. **Fork the repository**: Click the "Fork" button on GitHub
2. **Clone your fork**:

   ```bash
   git clone https://github.com/your-username/cliant.git
   cd cliant
   ```

3. **Create a feature branch**:

   ```bash
   git checkout -b feature/your-feature-name
   ```

## Development Setup

### Prerequisites

- Rust 1.70+ ([Install](https://www.rust-lang.org/tools/install))
- Cargo (comes with Rust)

### Building the Project

```bash
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Code Formatting

Cliant follows Rust formatting conventions. Before committing, format your code:

```bash
cargo fmt
```

### Linting

Check for common issues:

```bash
cargo clippy
```

## Project Architecture

Cliant uses a **feature-based vertical slice architecture**. When contributing:

### Adding a New Feature

Use the Python script to generate the slice structure or create the features directories manually if that's what you want. Running the python script is not compulsory:

```bash
cd src/features
python vertical_slice.py your_feature_name
```

This creates:

- `your_feature_name/mod.rs` - Feature entrypoint
- `your_feature_name/cli.rs` - CLI argument parsing
- `your_feature_name/handler.rs` - Business logic

### File Organization

- **`features/`**: Feature-specific code with minimal cross-feature dependencies
- **`shared/`**: Code reused across multiple features
  - `network/`: HTTP client, transport protocols
  - `fs/`: Filesystem operations
  - `errors.rs`: Error type definitions
  - `progress_tracker.rs`: Download progress tracking

## Contribution Guidelines

### Before You Start

1. Check existing [issues](https://github.com/val-en-tine124/cliant/issues) to avoid duplicates
2. For major changes, open an issue first to discuss your approach
3. Ensure your contribution aligns with the project roadmap

### Code Quality

- **Write clear code**: Use meaningful variable names and comments where necessary
- **Follow conventions**: Adhere to Rust naming conventions and the project's style
- **Test your changes**: Add tests for new functionality
- **Update documentation**: If your change affects user-facing features, update README.md and relevant .md files
- **Keep scope focused**: Each PR should address a single concern

### Logging

Use the `tracing` library for logging:

```rust
use tracing::{debug, info, warn, error, instrument};

debug!("Detailed debugging information");
info!("General informational message");
warn!("Warning about potential issues");
error!("Error that occurred");
trace!("More detailed tracing of operations and execution flows e.g loops");
```

### Error Handling

- Use `anyhow::Result` for main entry points
- Use `CliantError` enum for feature-specific errors
- Always provide context with error messages

Example:

```rust
use crate::shared::errors::CliantError;
use anyhow::Context;

fn my_function() -> Result<String, CliantError> {
    let value = some_operation()
        .context("Failed to perform operation")?;
    Ok(value)
}
```

### Documentation

Add comprehensive documentation to your code:

```rust
/// Downloads data from a remote source.
///
/// This function retrieves data from the specified URL and returns
/// it as a stream of bytes.
///
/// # Arguments
///
/// * `url` - The URL to download from
///
/// # Errors
///
/// Returns an error if the network request fails
///
/// # Example
///
/// ```ignore
/// let bytes = download("https://example.com/file").await?;
/// ```
pub async fn download(url: &str) -> Result<impl Stream> {
    // implementation
}
```

### Commit Messages

Write clear, descriptive commit messages using conventional commits:

```
feat: add S3 upload support
fix: resolve progress tracker race condition
docs: update download command examples
refactor: extract retry logic into shared module
test: add tests for HTTP adapter
```

Use these prefixes:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `perf`: Performance improvements

### Resource Management

Ensure proper resource cleanup using RAII patterns:

```rust
// ✓ Correct: Resources are cleaned up automatically
let fs_writer = LocalFsBuilder::new()
    .file_name(file_name)
    .root_path(dir)
    .build()
    .await?;

// Do work with fs_writer

fs_writer.close_fs().await; // Explicit cleanup
```

## Pull Request Process

1. **Ensure tests pass**:

   ```bash
   cargo test
   ```

2. **Format code**:

   ```bash
   cargo fmt
   ```

3. **Run clippy**:

   ```bash
   cargo clippy
   ```

4. **Update CHANGELOG.md**: Document your changes under `[Unreleased]`

5. **Create your PR**:
   - Use a clear, descriptive title
   - Reference related issues (e.g., "Closes #123")
   - Provide a detailed description of changes
   - Include before/after examples if applicable

6. **Address review feedback**: Respond to code review comments promptly

## Adding Dependencies

Before adding a new dependency:

1. Ensure it's actively maintained
2. Check for alternative, lighter-weight options
3. Update `Cargo.toml` and document the addition
4. Test thoroughly for compatibility

## Reporting Issues

When reporting bugs:

1. **Provide a minimal reproduction**: Share the exact command that fails
2. **Include environment details**: OS, Rust version (`rustc --version`), etc.
3. **Share relevant logs**: Run with `-vv` or `-vvv` for verbose output:

   ```bash
   cliant -vv download <URL> -o <PATH>
   ```

4. **Expected vs. actual behavior**: Clearly describe the discrepancy

Example issue:

```
Title: Download fails with 403 Forbidden for authenticated URLs

Environment:
- OS: Ubuntu 22.04
- Rust: 1.75.0
- Cliant: 0.1.0

Steps to reproduce:
1. Run: cliant download https://api.example.com/file -U user -P pass -o ~/file
2. Observe error message

Expected: File downloads successfully
Actual: Error: 403 Forbidden

Logs (with -vv):
[paste logs here]
```

## Testing

### Writing Tests

Add tests for new functionality using the AAA (Arrange-Act-Assert) pattern:

```rust
#[tokio::test]
async fn test_download_creates_file() -> anyhow::Result<()> {
    // Arrange
    let temp_dir = TempDir::new()?;
    let output_path = temp_dir.path().join("test.bin");
    let url = Url::parse("https://example.com/file")?;

    // Act
    handle(LocalArgs {
        url,
        output: output_path.clone(),
        ..Default::default()
    }).await?;

    // Assert
    assert!(output_path.exists());
    Ok(())
}
```

Run tests with:

```bash
cargo test
cargo test save_to_local -- --nocapture  # with output
cargo test -- --test-threads=1           # single-threaded
```

See [TESTING.md](TESTING.md) for comprehensive testing guidelines.

## Documentation

- Update README.md for user-facing changes
- Add code comments for complex logic
- Update CHANGELOG.md with your changes
- Use doc comments for public APIs
- See [ARCHITECTURE.md](ARCHITECTURE.md) for system design documentation

## Performance Considerations

- Use `tokio` for async operations
- Avoid unnecessary clones (especially URLs and paths)
- Use streaming for large data transfers
- Profile code changes for regressions
- Document performance-sensitive sections with comments

## Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug cargo run -- -vv download <URL> -o <PATH>
RUST_LOG=cliant::features::save_to_local=trace cargo run
```

### Run with Debugger

```bash
# Using rust-gdb or rust-lldb
rust-gdb ./target/debug/cliant
```

## Questions?

Feel free to:

- Open a GitHub [discussion](https://github.com/val-en-tine124/cliant/discussions) for questions
- Check existing documentation in README.md, ARCHITECTURE.md, and TESTING.md
- Review existing code for patterns and conventions

## Recognition

Contributors will be recognized in:

- CHANGELOG.md (for significant contributions)
- GitHub's contributor graph

Thank you for contributing to Cliant! 🚀

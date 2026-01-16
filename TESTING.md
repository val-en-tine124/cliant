# Testing Guide for Cliant

This document describes the testing practices, strategies, and how to run tests in the Cliant project.

## Running Tests

### All Tests

```bash
cargo test
```

### Specific Test

```bash
cargo test test_name
```

### With Output

```bash
cargo test -- --nocapture
```

### Single-Threaded (useful for debugging)

```bash
cargo test -- --test-threads=1
```

### Specific Module

```bash
cargo test features::save_to_local::handler::tests
```

### With Backtrace

```bash
RUST_BACKTRACE=1 cargo test
```

## Test Organization

Tests are organized in three categories:

### 1. Unit Tests

Located in the same file as the code being tested within `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_adapter_receives_data() -> anyhow::Result<()> {
        // Arrange
        let adapter = HttpAdapter::new(HttpArgs::default())?;
        let source = Url::parse("http://speedtest.tele2.net/1MB.zip")?;
        
        // Act
        let stream_result = adapter.receive_data(source).await;
        
        // Assert
        assert!(stream_result.is_ok());
        Ok(())
    }
}
```

### 2. Integration Tests

Planned for `tests/` directory (not yet fully implemented):

```
tests/
├── download_test.rs      # Full download workflow scenarios
├── http_adapter_test.rs  # HTTP transport edge cases
└── filesystem_test.rs    # Filesystem operations
```

### 3. Documentation Tests

Planned for inline code examples in doc comments.

## Current Test Coverage

### Filesystem Module Tests

**File**: `src/shared/fs/local.rs`

```rust
#[tokio::test]
async fn test_local_fs() -> anyhow::Result<()> {
    // Tests concurrent file writes with semaphore control
}
```

### Handler Tests

**File**: `src/features/save_to_local/handler.rs`

Tests covering:

- Valid file downloads
- Invalid path handling
- Resource cleanup on errors
- Progress tracking
- Timeout configuration
- Tracing instrumentation

## Test Patterns

### Async Tests

Use `#[tokio::test]` for async test functions:

```rust
#[tokio::test]
async fn test_async_operation() -> Result<()> {
    // Async code here
    Ok(())
}
```

### Error Testing

Test both success and failure paths:

```rust
#[tokio::test]
async fn test_invalid_url_handling() -> Result<()> {
    let result = parse_url("not-a-url");
    assert!(result.is_err(), "Should fail with invalid URL");
    
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("invalid"),
        "Error should mention invalid URL"
    );
    Ok(())
}
```

### Resource Cleanup

Ensure resources are properly cleaned up:

```rust
#[tokio::test]
async fn test_file_handle_cleanup() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let fs = LocalFsBuilder::new()
        .file_name(PathBuf::from("test.txt"))
        .root_path(temp_dir.path())
        .build()
        .await?;
    
    // Test operations
    fs.close_fs().await;
    
    // Verify cleanup (no hanging file descriptors)
    Ok(())
}
```

## Writing New Tests

### Test Structure

Follow the Arrange-Act-Assert (AAA) pattern:

```rust
#[tokio::test]
async fn test_download_creates_file() -> Result<()> {
    // Arrange: Set up test data and dependencies
    let temp_dir = TempDir::new()?;
    let output_path = temp_dir.path().join("test.bin");
    let config = HttpArgs::default();
    let adapter = HttpAdapter::new(config)?;
    let url = Url::parse("https://example.com/file")?;
    
    // Act: Execute the code being tested
    let result = adapter.download_to(&url, &output_path).await;
    
    // Assert: Verify the result
    assert!(result.is_ok(), "Download should succeed");
    assert!(output_path.exists(), "File should exist");
    let metadata = std::fs::metadata(&output_path)?;
    assert!(metadata.len() > 0, "File should have content");
    
    Ok(())
}
```

### Best Practices

1. **One concern per test**: Each test should verify a single aspect
2. **Descriptive test names**: Name should describe what is tested and expected outcome
   - ✓ `test_handle_valid_output_path`
   - ✓ `test_resource_cleanup_on_invalid_domain`
   - ✗ `test1`
   - ✗ `test_stuff`

3. **Minimal setup**: Only create what's needed for the test
4. **No side effects**: Tests should be independent and re-runnable
5. **Clear failure messages**: Use assertion messages to explain expectations

   ```rust
   assert_eq!(
       actual, expected,
       "expected total_bytes to be 1024, but got {}", actual
   );
   ```

6. **Cleanup resources**: Use `TempDir` or similar for automatic cleanup

### Example: Testing Error Cases

```rust
#[tokio::test]
async fn test_http_adapter_handles_404() -> anyhow::Result<()> {
    let adapter = HttpAdapter::new(HttpArgs::default())?;
    let url = Url::parse("https://httpbin.org/status/404")?;
    
    let result = adapter.receive_data(url).await;
    
    // Should result in an error
    assert!(
        result.is_err(),
        "Should fail with 404 status"
    );
    
    Ok(())
}
```

### Example: Testing with Temporary Files

```rust
#[tokio::test]
async fn test_file_writing() -> anyhow::Result<()> {
    use tempfile::TempDir;
    use std::fs;

    // TempDir is automatically cleaned up when dropped
    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.txt");

    // Write test
    fs::write(&file_path, b"test content")?;
    
    // Verify
    assert!(file_path.exists());
    let content = fs::read(&file_path)?;
    assert_eq!(content, b"test content");

    Ok(())
} // temp_dir automatically cleaned up here
```

## Testing External Resources

Some tests require external resources:

### HTTP Tests

Tests use public HTTP endpoints:

- `http://speedtest.tele2.net/1MB.zip` - 1MB test file
- `http://speedtest.tele2.net/10MB.zip` - 10MB test file
- `https://httpbin.org/` - HTTP testing service
- `https://httpbin.org/status/404` - 404 error simulation

**To skip tests requiring external network**:

```bash
# Run only tests not requiring internet
cargo test -- --skip "speedtest\|httpbin"
```

**Network-dependent test example**:

```rust
#[tokio::test]
#[ignore]  // Skip by default, requires internet
async fn test_real_download_no_internet_required() -> anyhow::Result<()> {
    // Test implementation
    Ok(())
}
```

Run ignored tests:

```bash
cargo test -- --ignored
```

### Environment Setup

For integration tests requiring specific setup:

```rust
#[tokio::test]
async fn test_with_local_server() -> anyhow::Result<()> {
    // Start local test server (automatic with test framework)
    // Run test
    // Server stops automatically (RAII)
    Ok(())
}
```

## Mocking Strategies

### Trait-Based Mocking

Use the trait abstraction for easy mocking:

```rust
struct MockTransport;

impl DataTransport for MockTransport {
    async fn receive_data(&self, _source: Url) 
        -> Result<impl Stream<Item = Result<Bytes, CliantError>>> {
        // Return mock stream with test data
        Ok(tokio_stream::iter(vec![
            Ok(Bytes::from("test data"))
        ]))
    }
    
    async fn total_bytes(&self, _source: Url) 
        -> Result<Option<usize>> {
        Ok(Some(9)) // "test data" is 9 bytes
    }
}

#[tokio::test]
async fn test_with_mock_transport() -> Result<()> {
    let mock = MockTransport;
    let url = Url::parse("http://example.com")?;
    
    let data = mock.receive_data(url).await?;
    // Test with mock
    Ok(())
}
```

### Testing Retry Logic

```rust
#[tokio::test]
async fn test_retry_on_transient_error() -> Result<()> {
    // Create mock that fails twice then succeeds
    let attempt = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    
    // Verify retry happens and succeeds
    Ok(())
}
```

## Performance Testing

### Basic Benchmarking

```rust
#[tokio::test]
async fn test_download_performance() -> Result<()> {
    let start = std::time::Instant::now();
    
    // Perform download
    let result = download_file(/* args */).await;
    
    let elapsed = start.elapsed();
    println!("Download took {:?}", elapsed);
    
    // Assert reasonable performance
    assert!(elapsed.as_secs() < 60, "Download should complete in under 60 seconds");
    assert!(result.is_ok());
    Ok(())
}
```

## Continuous Integration

Tests should run on:

- Every commit (locally via pre-commit hooks)
- Pull requests (via GitHub Actions)
- Before releases

### GitHub Actions (Recommended Setup)

```yaml
name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --verbose
      - run: cargo test --doc
```

## Coverage

To measure test coverage (requires `tarpaulin`):

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report (HTML)
cargo tarpaulin --out Html --output-dir coverage

# View results
open coverage/index.html
```

## Common Issues

### Tokio Runtime Issues

```rust
// ❌ Wrong: No async runtime
#[test]
async fn test() { }

// ✓ Correct: Creates tokio runtime
#[tokio::test]
async fn test() { }
```

### Test Isolation

Tests can run in parallel; ensure they don't interfere:

```rust
// ❌ Wrong: Uses shared path
const TEST_FILE: &str = "/tmp/test.txt";

// ✓ Correct: Unique path per test
let test_file = format!(
    "/tmp/test_{}.txt", 
    uuid::Uuid::new_v4()
);
```

### Timeout Issues

Use timeout for external calls:

```rust
#[tokio::test]
async fn test_with_timeout() -> Result<()> {
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            // test code here
        }
    ).await?;
    Ok(())
}
```

### Async Test Cleanup

Ensure async cleanup completes:

```rust
#[tokio::test]
async fn test_with_cleanup() -> Result<()> {
    let resource = create_resource().await?;
    
    // Use resource
    
    // Cleanup must complete
    resource.cleanup().await; // Not just `drop(resource)`
    
    Ok(())
}
```

## Test Debugging

### Run Single Test with Logging

```bash
RUST_LOG=debug cargo test test_handle_valid_output_path -- --nocapture
```

### Run with Backtrace

```bash
RUST_BACKTRACE=1 cargo test test_name -- --nocapture
```

### Print Inside Tests

```rust
#[tokio::test]
async fn test_debug() -> Result<()> {
    println!("Test value: {:?}", some_value);
    dbg!(&some_value);  // Macro for printing
    Ok(())
}
```

Run with output:

```bash
cargo test -- --nocapture
```

## Future Testing Goals

- [ ] 80%+ code coverage
- [ ] Integration test suite for common download scenarios
- [ ] Performance benchmarks with criterion
- [ ] Load testing (concurrent downloads)
- [ ] Fuzz testing for URL and header parsing
- [ ] Property-based testing for retry logic (quickcheck/proptest)
- [ ] E2E tests with Docker containers
- [ ] Security scanning in tests
- [ ] Memory leak detection with valgrind

## Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing Guide](https://tokio.rs/tokio/tutorial/testing)
- [Criterion.rs - Benchmarking](https://bheisler.github.io/criterion.rs/book/)
- [Property-Based Testing](https://docs.rs/proptest/latest/proptest/)
- [Test Organization Best Practices](https://doc.rust-lang.org/book/ch11-03-test-organization.html)

---

**Last Updated**: January 2026

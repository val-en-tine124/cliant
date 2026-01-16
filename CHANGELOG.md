# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned Features

- Multiple concurrent downloads support
- Cloud storage backends (S3, GCP, Azure Blob, IPFS)
- Resume broken download capability
- Download scheduling and queue management
- Configuration file support (~/.cliant/config)
- Persistent state for resuming interrupted downloads

### Under Consideration

- Database support for download history
- Download templates for batch operations
- Web dashboard for remote management

## [0.1.0] - 2026-01-14

### Added

- **Initial Release**: First stable version of Cliant
- **HTTP/HTTPS Downloads**: Full support for downloading files via HTTP and HTTPS
- **Local File Storage**: Save downloaded files to the local filesystem with proper resource management
- **CLI Interface**: Complete command-line interface built with `clap`
  - Quiet mode for minimal output (`-q`)
  - Verbose modes for detailed logging (`-v`, `-vv`, `-vvv`)
  - Structured argument parsing with validation
- **Robust Error Handling**:
  - Exponential backoff retry strategy for transient failures
  - Configurable retry parameters (max attempts, delay)
  - Proper error context propagation with `anyhow`
  - Graceful error recovery with resource cleanup
- **Progress Tracking**:
  - Real-time download progress with `indicatif`
  - Bytes transferred and transfer speed display
  - Elapsed time visualization
  - Completion summary
- **HTTP Authentication**:
  - Basic authentication support (username/password)
  - Custom HTTP headers
  - Cookie support for session management
- **Advanced HTTP Features**:
  - Configurable request timeout
  - HTTP proxy support
  - Redirect handling with max redirect limit
  - HTTP version selection (1.0, 1.1,)
- **Logging System**:
  - Integrated `tracing` framework
  - Multiple log levels (trace, debug, info, warn, error)
  - Environment variable support (`RUST_LOG`)
  - Structured logging with context information
- **Feature-Based Architecture**:
  - Modular vertical slice design
  - Shared layer for common functionality (network, filesystem, error handling)
  - Easy extension point for new features
  - Minimal coupling between features
- **Comprehensive Documentation**:
  - Detailed README with examples
  - Architecture documentation (ARCHITECTURE.md)
  - Contributing guidelines (CONTRIBUTING.md)
  - Security policy (SECURITY.md)
  - Testing guide (TESTING.md)
- **Quality Assurance**:
  - Unit tests for download handler
  - Integration-style tests with real HTTP endpoints
  - Resource cleanup verification
  - Error handling test coverage
  - Tracing/instrumentation validation
- **Performance Optimizations**:
  - Non-blocking async I/O with `tokio`
  - Streaming downloads (no full buffering in memory)
  - Efficient chunked writing (4MB buffers)
  - Zero-copy operations with `bytes` crate
  - Progress tracking with minimal overhead
- **Security Features**:
  - HTTPS scheme enforcement recommendations
  - Secret string protection for passwords
  - Input validation for URLs and file paths
  - No credential persistence to disk

### Technical Stack

- **Runtime**: `tokio` async runtime
- **HTTP Client**: `reqwest` with middleware support
- **Retry Logic**: `reqwest-retry` with exponential backoff
- **CLI Parsing**: `clap` with derive macros
- **Logging**: `tracing` framework
- **Progress UI**: `indicatif` progress bars
- **Filesystem**: `opendal` for abstraction
- **Serialization**: `serde` with `serde_json`
- **Error Handling**: `anyhow` for error context
- **Async Streams**: `tokio-stream` for stream utilities
- **Byte Handling**: `bytes` for efficient byte management

### Known Limitations

- Single download per invocation
- No broken download resume capability (planned for v0.2.0)
- Limited to HTTP(S) transports (other protocols e.g Tor planned)
- No configuration file support (planned for v0.2.0)

### Documentation

- README.md with quick start and usage examples
- CONTRIBUTING.md for contributor guidelines
- ARCHITECTURE.md for system design documentation
- SECURITY.md for security practices and policies
- TESTING.md for testing strategies and patterns
- CHANGELOG.md (this file) for version history
- Inline code documentation with `///` comments
- Doc tests for usage examples

### Testing

- 8 comprehensive unit tests in `save_to_local/handler.rs`
- Tests cover:
  - Valid downloads to temporary directories
  - Invalid file paths (no filename, missing parent)
  - Nested directory paths
  - Resource cleanup on errors
  - Custom HTTP configuration (timeouts)
  - Progress tracking initialization
  - Tracing instrumentation
  - URL handling efficiency

---

## How to Update This Changelog

When making a release or significant changes:

1. Create a new version section at the top
2. Move changes from `[Unreleased]` to the version section
3. Add the date in `YYYY-MM-DD` format
4. Use these categories (in order):
   - **Added**: New features
   - **Changed**: Changes in existing functionality
   - **Deprecated**: Soon-to-be removed features
   - **Removed**: Removed features
   - **Fixed**: Bug fixes
   - **Security**: Security issue fixes
   - **Technical**: Technical improvements and refactoring
   - **Performance**: Performance enhancements
   - **Documentation**: Documentation updates
   - **Testing**: Test suite improvements

### Example Entry

```markdown
## [0.2.0] - 2026-03-15

### Added
- S3 upload backend with configurable credentials
- Download pause/resume capability with state persistence
- Bandwidth throttling with configurable limits

### Fixed
- Progress tracker race condition in concurrent scenarios
- Memory leak in HTTP adapter cleanup
- Incorrect timeout calculation on retry

### Changed
- Improved error messages for network timeouts
- Refactored retry logic into shared module
- Updated dependency versions

### Security
- Fixed potential credential logging in debug mode
- Added input sanitization for file paths

### Performance
- Reduced memory footprint by 15% through buffer pooling
- Improved throughput by optimizing chunk sizes

### Documentation
- Added architecture diagrams in ARCHITECTURE.md
- Updated API documentation with examples
```

## Version History

### Versioning Scheme

Cliant follows [Semantic Versioning](https://semver.org/):

- **MAJOR** (X.0.0): Breaking changes to CLI or API
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

### Release Cycle

- Major releases: When significant features or breaking changes warrant it
- Minor releases: Every 4-6 weeks with new features
- Patch releases: As needed for critical bug fixes
- Release candidates: Pre-release versions for testing (e.g., v0.2.0-rc1)

## Migration Guides

### From v0.1.0 to v0.2.0 (Planned)

- URL format remains unchanged
- CLI arguments remain backward compatible
- New optional arguments for S3 configuration
- No action required for existing scripts

---

**Note**: For the latest development changes and features being worked on, check the GitHub repository's main branch.

# Cliant

A high-performance, command-line HTTP Data Mover for embarrassingly parallel tasks, written in Rust.

## Features

- 🚀 **High Performance**: Built with Rust and `tokio` async runtime
- 📦 **Flexible Download**: Support for HTTP/HTTPS with customizable options
- 🔄 **Robust Retry Logic**: Exponential backoff retry mechanism for network resilience
- 🔐 **Security**: Basic authentication, custom headers, and cookie support
- 📊 **Progress Tracking**: Real-time progress visualization
- 🏗️ **Modular Architecture**: Feature-based vertical slice design for extensibility
- ⚡ **Non-blocking I/O**: Efficient async/await using `tokio` runtime

## Installation

### Prerequisites

- [Rust 1.70+](https://www.rust-lang.org/tools/install)
- Cargo

### Building from Source

```bash
git clone https://github.com/val-en-tine124/cliant.git
cd cliant
cargo build --release
```

The executable will be located at `target/release/cliant` (or `target/release/cliant.exe` on Windows).

## Quick Start

### Basic Download

```bash
cliant download https://example.com/file.zip -o ~/Downloads/file.zip
```

### With Authentication

```bash
cliant download https://example.com/file.zip -o ~/Downloads/file.zip -U username -P password
```

### With Proxy

```bash
cliant download https://example.com/file.zip -o ~/Downloads/file.zip -p http://proxy.example.com:8080
```

### Custom Headers

```bash
cliant download https://example.com/file.zip -o ~/Downloads/file.zip --request-headers "Authorization:Bearer token,Custom:value"
```

## Command-Line Options

### Global Options

- `-q, --quiet`: Set logging level to quiet (errors only)
- `-v, --verbose`: Increase verbosity (can be used multiple times: `-v`, `-vv`, `-vvv`)

### Download Command Options

- `<URL>`: HTTP/HTTPS URL of the file to download
- `-o, --output <PATH>`: Output file path **(required)**
- `-t, --transport <TRANSPORT>`: Transport protocol (default: `http`)
- `-U, --username <USERNAME>`: HTTP basic authentication username
- `-P, --password <PASSWORD>`: HTTP basic authentication password
- `-T, --timeout <SECONDS>`: HTTP request timeout in seconds (default: 60)
- `-r, --max-no-retries <N>`: Maximum retry attempts (default: 10)
- `-d, --retry-delay-secs <SECONDS>`: Delay between retries in seconds (default: 10)
- `--max-redirects <N>`: Maximum HTTP redirects to follow
- `-p, --proxy-url <URL>`: HTTP proxy URL
- `--request-headers <HEADERS>`: Custom HTTP headers (format: `key1:value1,key2:value2`)
- `--http-cookies <COOKIES>`: HTTP cookies from previous sessions
- `--http-version <VERSION>`: HTTP version (default: 1.1)

## Project Structure

```
cliant/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── features/               # Vertical slices (feature modules)
│   │   ├── save_to_local/      # Local file storage feature
│   │   │   ├── mod.rs
│   │   │   ├── cli.rs          # CLI argument parsing
│   │   │   └── handler.rs      # Business logic and download orchestration
│   │   └── mod.rs
│   └── shared/                 # Shared functionality across features
│       ├── network/            # HTTP client and transport layer
│       │   ├── http/           # HTTP adapter implementation
│       │   ├── factory.rs      # Transport factory pattern
│       │   └── mod.rs
│       ├── fs/                 # Filesystem operations
│       │   ├── local.rs        # Local filesystem adapter
│       │   └── mod.rs
│       ├── progress_tracker.rs # Download progress tracking
│       ├── errors.rs           # Error types and handling
│       └── mod.rs
├── Cargo.toml                  # Dependencies and project metadata
├── rustfmt.toml                # Code formatting configuration
├── README.md                   # This file
├── CONTRIBUTING.md             # Contribution guidelines
├── CHANGELOG.md                # Version history
└── LICENSE                     # MIT License
```

## Architecture

Cliant follows a **feature-based vertical slice architecture** that promotes clean separation of concerns:

- **Features Layer** (`src/features/`): Independent, self-contained features with their own CLI parsing and business logic
- **Shared Layer** (`src/shared/`): Reusable components like networking, filesystem operations, and error handling

This design makes it easy to add new features (e.g., S3, GCP, IPFS) without affecting existing code. See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed architecture documentation.

## Logging

Control logging verbosity using flags:

```bash
# Quiet mode (errors only)
cliant -q download <URL> -o <PATH>

# Verbose modes
cliant -v download <URL> -o <PATH>    # Info level
cliant -vv download <URL> -o <PATH>   # Debug level
cliant -vvv download <URL> -o <PATH>  # Trace level
```

Or set the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cliant download <URL> -o <PATH>
RUST_LOG=cliant::features::save_to_local=trace cliant download <URL> -o <PATH>
```

## Error Handling

Cliant uses a robust error handling strategy:

- **Network Errors**: Automatic retry with exponential backoff
- **Filesystem Errors**: Clear error messages with context
- **Parse Errors**: Validation of URLs and file paths
- **Timeouts**: Configurable timeout with automatic recovery

## Security Considerations

1. **Always Use HTTPS**: Recommended for all downloads

   ```bash
   cliant download https://trusted-site.com/file.zip -o ~/file.zip
   ```

2. **Handle Credentials Carefully**: Avoid exposing passwords in command-line history

   ```bash
   # Use environment variables instead
   export CLIANT_AUTH_PASSWORD="your-password"
   ```

3. **Verify Downloads**: Check checksums when available
4. **File Permissions**: Downloaded files inherit umask permissions; adjust as needed:

   ```bash
   cliant download https://example.com/key.pem -o ~/keys/key.pem
   chmod 600 ~/keys/key.pem
   ```

See [SECURITY.md](SECURITY.md) for more details.

## Development

### Code Style

Format code with `rustfmt` before committing:

```bash
cargo fmt
```

### Running Tests

```bash
cargo test
```

Run specific test:

```bash
cargo test save_to_local::handler::tests
```

### Running with Logging

```bash
RUST_LOG=debug cargo run -- download <URL> -o <PATH>
```

### Building Documentation

```bash
cargo doc --open
```

See [TESTING.md](TESTING.md) for comprehensive testing guidelines.

## Roadmap

- [ ] Multiple concurrent downloads
- [ ] Cloud storage backends (S3, GCP, Azure Blob, IPFS)
- [ ] Graphical User Interface (GUI)
- [ ] Checksum verification (MD5, SHA256, SHA512)
- [ ] Pause/resume downloads
- [ ] Download scheduling and queue management
- [ ] Bandwidth throttling
- [ ] Configuration file support (~/.cliant/config)
- [ ] Persistent state for resuming interrupted downloads

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:

- Getting started with development
- Code quality standards
- Commit message conventions
- Pull request process
- Reporting issues

## License

This project is licensed under the MIT License - see [LICENSE](LICENSE) file for details.

## Support

For issues, questions, or suggestions:

- **Bug Reports**: Open an [issue](https://github.com/val-en-tine124/cliant/issues)
- **Feature Requests**: Open an [issue](https://github.com/val-en-tine124/cliant/issues) with `[FEATURE]` prefix
- **Security Issues**: Email security concerns to **<valentinechibueze400@gmail.com>** (do not open public issues)

## Author

**Abba Valentine**

- GitHub: [@val-en-tine124](https://github.com/val-en-tine124)

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and release notes.

## Performance

Cliant is optimized for high-performance downloads:

- **Streaming**: No buffering of entire file in memory
- **Chunked Writes**: 4MB buffers reduce syscalls
- **Async I/O**: Non-blocking operations with `tokio`
- **Retry Strategy**: Exponential backoff prevents thundering herd
- **Progress Tracking**: Minimal overhead with atomic operations

---

**Note**: This project is actively maintained. For the latest updates, features, and bug fixes, please check the [GitHub repository](https://github.com/val-en-tine124/cliant).

# Cliant Architecture

This document provides a detailed overview of the Cliant project's architecture, design patterns, and component interactions.

## Overview

Cliant is built using a **feature-based vertical slice architecture**, which promotes:

- **Separation of Concerns**: Each feature is self-contained with its own logic
- **Code Reusability**: Shared utilities are centralized in the `shared/` layer
- **Extensibility**: New features can be added without modifying existing code
- **Testability**: Independent features are easier to test in isolation

## High-Level Architecture

```
┌────────────────────────────────────────────┐
│      CLI Entry Point (main.rs)             │
│  - Argument parsing with clap              │
│  - Logging setup with tracing              │
│  - Feature routing and dispatch            │
└──────────────┬─────────────────────────────┘
               │
         ┌─────▼──────────────┐
         │   Features Layer   │
         │  (Vertical Slices) │
         ├────────────────────┤
         │  save_to_local:    │
         │  - cli.rs (args)   │
         │  - handler.rs      │
         │    (orchestration) │
         │  - mod.rs          │
         │    (entrypoint)    │
         └─────┬──────────────┘
               │
         ┌─────▼─────────────────────────────┐
         │     Shared Layer                  │
         ├───────────────────────────────────┤
         │ • Network (HTTP/HTTPS)            │
         │ • Filesystem Operations           │
         │ • Error Handling                  │
         │ • Progress Tracking               │
         └───────────────────────────────────┘
```

## Layer Breakdown

### 1. CLI Layer (`src/main.rs`)

**Responsibility**: Command-line interface and application bootstrap

**Components**:

- `Cliant` struct: Main CLI argument parser built with `clap`
- `Commands` enum: Subcommand routing (Download, etc.)
- `setup_tracing()`: Logging configuration with verbosity levels

**Flow**:

```
User Input
    ↓
Parse Arguments (clap parser)
    ↓
Setup Logging (tracing subscriber)
    ↓
Route to Feature Handler
    ↓
Execute Feature Logic
    ↓
Return Result to CLI
```

### 2. Features Layer (`src/features/`)

**Responsibility**: Feature-specific logic with three standard files per feature

#### Feature Structure

Each feature follows the vertical slice pattern:

```
feature_name/
├── mod.rs          # Entrypoint - re-exports public APIs
├── cli.rs          # CLI argument definitions specific to feature
└── handler.rs      # Business logic orchestration
```

#### Example: `save_to_local/`

**`cli.rs`** - Argument Parsing

```rust
pub struct LocalArgs {
    pub url: Url,                       // Download source URL
    pub output: PathBuf,                // Destination file path
    pub http_args: HttpArgs,            // Shared HTTP configuration
    pub transport: TransportType,       // Protocol selection
}
```

**`handler.rs`** - Business Logic

The handler orchestrates the entire download process:

```
1. Parse and validate arguments (URL scheme, file path)
2. Extract file name and parent directory
3. Initialize transport layer (HTTP client with middleware)
4. Create local filesystem writer (OpenDAL-based)
5. Retrieve remote file size (for progress tracking)
6. Stream data from source in chunks
7. Write chunks to filesystem and update progress
8. Ensure resource cleanup (RAII pattern)
9. Display completion information
```

**`mod.rs`** - Module Re-exports

```rust
pub mod handler;
pub mod cli;
```

### 3. Shared Layer (`src/shared/`)

**Responsibility**: Common functionality across features

#### Components

##### Network Module (`shared/network/`)

**Purpose**: Data transport abstraction

```
DataTransport Trait (define contract)
    ├── receive_data()   // Stream bytes from source
    └── total_bytes()    // Get content length
        ↓
    Implementations:
    ├── HttpAdapter      // HTTP/HTTPS support
    └── [Future: TorAdapter, etc.]
```

**HTTP Adapter Features**:

- Retry logic with exponential backoff using `reqwest-retry`
- Authentication (basic auth, custom headers, cookies)
- Proxy support
- Configurable timeouts
- Middleware-based architecture for cross-cutting concerns

**Design Pattern**: Strategy Pattern

- `DataTransport` trait allows multiple transport implementations
- Easy to add new transports (Tor, IPFS, etc.) without modifying existing code
- Decouples transport logic from download orchestration

```rust
pub trait DataTransport: Send + Sync {
    async fn receive_data(&self, source: Url) 
        -> Result<impl Stream<Item = Result<Bytes, CliantError>>>;
    async fn total_bytes(&self, source: Url) 
        -> Result<Option<usize>>;
}
```

##### Filesystem Module (`shared/fs/`)

**Purpose**: Filesystem abstraction with OpenDAL backend

```
FsOps Trait
    ├── append_bytes()   // Write data to file
    └── close_fs()       // Flush and close
        ↓
    Implementations:
    └── LocalFs          // Uses opendal for I/O
```

**Key Features**:

- Async write operations with `tokio`
- In-memory buffering (4MB chunks) for efficiency
- Thread-safe operations with `Arc<RwLock<>>`
- Proper resource cleanup via `close_fs()`
- Automatic cleanup with RAII patterns

**LocalFsBuilder Pattern**:

```rust
let fs = LocalFsBuilder::new()
    .file_name(PathBuf::from("file.zip"))
    .root_path(PathBuf::from("/downloads"))
    .build()
    .await?;

// Use fs...

fs.close_fs().await; // Explicit cleanup
```

##### Progress Tracker (`shared/progress_tracker.rs`)

**Purpose**: Download progress abstraction with pluggable implementations

```
ProgressTracker Trait
    ├── start()          // Initialize progress display
    ├── update()         // Update with bytes written
    └── finish()         // Mark download complete
        ↓
    Implementations:
    ├── CliProgressTracker   // Terminal-based progress bars
    └── [Future: GuiProgressTracker, WebProgressTracker, etc.]
```

**CliProgressTracker Implementation**:

- Uses `indicatif` for visual progress bars
- Displays bytes transferred and speed
- Shows elapsed time
- Thread-safe with `Arc<RwLock<ProgressBar>>`
- Lock minimization for better performance

**Extensible Design**: New progress implementations can be added by implementing the trait.

##### Error Handling (`shared/errors.rs`)

**CliantError Enum**:

```rust
pub enum CliantError {
    ReqwestClient(reqwest::Error),              // HTTP client errors
    ReqwestMiddleware(reqwest_middleware::Error), // Middleware errors
    Io(std::io::Error),                         // Filesystem errors
    Fatal(String),                              // Unrecoverable errors
    ParseError(String),                         // Parsing errors
    Error(anyhow::Error),                       // Generic errors
}
```

**Error Handling Strategy**:

- Feature handlers return `Result<T, CliantError>`
- Main entry point uses `anyhow::Result`
- Error context added at each layer with `.context()`
- Sensitive information (passwords) never logged
- Resource cleanup occurs even on error paths

## Data Flow

### Download Operation

```
User: cliant download https://example.com/file.zip -o ~/file.zip

Step 1: CLI Parsing
   - clap parses arguments into LocalArgs
   - URL validation (http/https scheme check)
   - Path validation (must be a file, not directory)

Step 2: Handler Initialization
   - Transport factory creates HttpAdapter
   - LocalFsBuilder prepares file handle
   - CliProgressTracker initialized with total size

Step 3: Data Streaming
   - HTTP adapter makes HEAD request → total_bytes
   - Progress tracker displays initialization
   - HTTP adapter streams chunks from source
   
   For each chunk:
     a. Receive bytes from stream
     b. LocalFs.append_bytes() writes to file
     c. ProgressTracker.update() updates progress display
     d. Error handling: retries or fails gracefully

Step 4: Completion
   - LocalFs.close_fs() flushes buffers and closes file
   - ProgressTracker.finish() displays summary
   - Handler returns Result to CLI
   - Exit with appropriate status code
```

### Error Handling Flow

```
Error Occurs During Streaming
    ↓
Is error retryable? (network timeout, transient error)
    ├─ Yes → exponential backoff retry logic
    │         (up to max_retries, default 10)
    │         Retry Delay: 10s * 2^attempt
    │
    └─ No  → Convert to CliantError
              ↓
           Propagate up stack with context
              ↓
           Log error at appropriate level
              ↓
           Ensure resource cleanup (close_fs)
              ↓
           Return error to CLI
              ↓
           Display user-friendly message
              ↓
           Exit with error status
```

## Key Design Patterns

### 1. Trait-Based Abstraction

**Purpose**: Decouple implementations from interfaces

```rust
pub trait DataTransport: Send + Sync {
    async fn receive_data(&self, source: Url) 
        -> Result<impl Stream<Item = Result<Bytes, CliantError>>>;
    async fn total_bytes(&self, source: Url) 
        -> Result<Option<usize>>;
}
```

**Benefits**:

- Easy to add new transports
- Easy to mock for testing
- Clear contract between layers
- Single Responsibility Principle

### 2. Builder Pattern

**Purpose**: Construct complex objects with optional parameters

```rust
pub struct LocalFsBuilder {
    root_path: Option<PathBuf>,
    file_name: Option<PathBuf>,
}

impl LocalFsBuilder {
    pub fn new() -> Self {
        Self {
            root_path: None,
            file_name: None,
        }
    }

    pub fn root_path(mut self, value: PathBuf) -> Self {
        self.root_path = Some(value);
        self
    }

    pub fn file_name(mut self, value: PathBuf) -> Self {
        self.file_name = Some(value);
        self
    }

    pub async fn build(self) -> Result<LocalFs, CliantError> {
        // Validation and initialization
        Ok(LocalFs { /* ... */ })
    }
}
```

**Benefits**:

- Fluent API for easy construction
- Validation in single place (build method)
- Clear intent with method names
- Supports optional parameters elegantly

### 3. Factory Pattern

**Purpose**: Centralize transport creation

```rust
pub fn handle_http(http_args: HttpArgs, transport_type: &TransportType) 
    -> Result<impl DataTransport> {
    match transport_type {
        TransportType::Http => HttpAdapter::new(http_args)
    }
}
```

**Benefits**:

- Single place for transport initialization
- Easy to add new transport types
- Encapsulates complex construction logic

### 4. Middleware Pattern

**Purpose**: Add cross-cutting concerns to HTTP client

```
HTTP Request Flow:
User Request (for data)
    ↓
TracingMiddleware (logs HTTP operations)
    ↓
RetryTransientMiddleware (retries on transient errors)
    ↓
TimeoutMiddleware (enforces timeout limits)
    ↓
reqwest::Client (actual HTTP request execution)
```

**Benefits**:

- Separate concerns (logging, retry, timeout)
- Reusable across different transports
- Maintainable and testable

### 5. RAII (Resource Acquisition Is Initialization)

**Purpose**: Automatic resource cleanup

```rust
// Resources are cleaned up automatically when fs_writer goes out of scope
match stream_result {
    Ok(mut stream) => {
        // Process stream
        fs_writer.close_fs().await; // Explicit cleanup
    }
    Err(err) => {
        // Even on error, cleanup occurs
        fs_writer.close_fs().await;
        return Err(err);
    }
}
```

**Benefits**:

- No leaked resources
- Exception safety
- Clear ownership semantics

## Concurrency Model

Cliant uses **async/await with `tokio`**:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // All I/O operations are non-blocking
    // Efficient resource usage with green threads
}
```

### Concurrency Characteristics

- **Single Download**: Sequential streaming (one download at a time in current version)
- **Async Operations**: Each I/O operation is async and yields to the runtime
- **No Blocking**: All operations use `tokio::fs`, `reqwest`, etc. for non-blocking I/O
- **Future Enhancement**: Multiple concurrent downloads using semaphores

### Concurrency Safety

- Thread-safe types: `Arc`, `RwLock`
- No raw pointers or unsafe code in safe interfaces
- Futures are `Send + Sync` by default

## Testing Strategy

### Unit Tests

Located in respective modules with `#[cfg(test)]` attribute:

```rust
#[tokio::test]
async fn test_http_adapter_receives_data() -> anyhow::Result<()> {
    // Arrange
    let adapter = HttpAdapter::new(HttpArgs::default())?;
    let url = Url::parse("http://speedtest.tele2.net/1MB.zip")?;
    
    // Act
    let stream = adapter.receive_data(url).await?;
    
    // Assert
    // Verify functionality
    
    Ok(())
}
```

### Test Coverage Areas

- HTTP adapter (network operations with real endpoints)
- Filesystem operations (writing, permissions, cleanup)
- Progress tracking (concurrent updates, state)
- Error handling and retry logic
- Path validation and extraction
- Resource cleanup on errors
- Tracing instrumentation

### Integration-Style Tests

Tests use real HTTP servers for validation:

- `http://speedtest.tele2.net/1MB.zip` - 1MB test file
- `https://httpbin.org/` - HTTP testing service

## Extension Points

### Adding a New Feature

1. Use the vertical slice generator:

   ```bash
   cd src/features
   python vertical_slice.py save_to_s3
   ```

2. Implement `cli.rs`:

   ```rust
   #[derive(Clone, Parser)]
   pub struct S3Args {
       pub bucket: String,
       pub key: String,
       // ... S3-specific args
   }
   ```

3. Implement `handler.rs`:

   ```rust
   pub async fn handle(args: S3Args) -> Result<()> {
       // S3-specific orchestration
   }
   ```

4. Register in `main.rs`:

   ```rust
   #[derive(Subcommand, Clone)]
   enum Commands {
       Download(LocalArgs),
       SaveToS3(S3Args),
   }
   ```

### Adding a New Transport

1. Implement `DataTransport` trait:

   ```rust
   pub struct TorAdapter { /* ... */ }
   
   impl DataTransport for TorAdapter {
       async fn receive_data(&self, source: Url) 
           -> Result<impl Stream> { }
       async fn total_bytes(&self, source: Url) 
           -> Result<Option<usize>> { }
   }
   ```

2. Update `shared/network/factory.rs`:

   ```rust
   pub fn handle_tor(tor_args: TorArgs) 
       -> Result<impl DataTransport> {
       TorAdapter::new(tor_args)
   }
   ```

### Adding Progress UI

1. Implement `ProgressTracker` trait:

   ```rust
   pub struct GuiProgressTracker { /* ... */ }
   
   impl ProgressTracker for GuiProgressTracker {
       async fn start(&self) { }
       async fn update(&self, bytes_written: usize) { }
       async fn finish(&self) { }
   }
   ```

2. Pass to handler instead of `CliProgressTracker`

## Dependency Management

### Key Dependencies

| Crate | Purpose | Version |
    |-------|---------|---------|
    | `tokio` | Async runtime | 1.x |
    | `reqwest` | HTTP client | 0.11.x |
    | `reqwest-middleware` | Middleware for reqwest | 0.2.x |
    | `reqwest-retry` | Retry middleware | 0.2.x |
    | `clap` | CLI argument parsing | 4.x |
    | `tracing` | Logging framework | 0.1.x |
    | `indicatif` | Progress bars | 0.17.x |
    | `opendal` | Filesystem abstraction | 0.40.x |
    | `bytes` | Efficient byte handling | 1.x |
    | `tokio-stream` | Stream utilities | 0.1.x |
    | `serde` | Serialization framework | 1.x |
    | `anyhow` | Error handling | 1.x |

### Adding Dependencies

1. Evaluate necessity and maintenance status
2. Consider lighter alternatives
3. Check for security vulnerabilities
4. Update `Cargo.toml`
5. Add documentation for why dependency is needed
6. Test thoroughly for compatibility

## Performance Considerations

1. **Streaming**: No buffering entire file in memory
   - Uses chunked reading with 4MB buffers
   - Minimizes memory footprint regardless of file size

2. **Non-blocking I/O**: Async operations throughout
   - All I/O uses `tokio` runtime
   - CPU yields to other tasks while waiting for I/O

3. **Retry Strategy**: Exponential backoff
   - Prevents thundering herd problem
   - Reduces server load on transient failures
   - Configurable delays and max attempts

4. **Progress Tracking**: Minimal overhead
   - Atomic updates to progress bar
   - Lock contention minimized
   - No blocking on progress updates

5. **Resource Management**: RAII patterns
   - No resource leaks
   - Automatic cleanup on error
   - Explicit close for optimization

## Security Considerations

1. **HTTPS by Default**: Scheme validation in CLI parser
2. **Authentication**: Basic auth with secret string protection
3. **Error Messages**: No exposure of sensitive data in logs
4. **Input Validation**: URLs, file paths, and headers validated
5. **Dependency Updates**: Regular security audits
6. **No Credential Persistence**: Credentials not saved to disk

## Future Architecture Improvements

1. **Plugin System**: Load features at runtime
2. **Configuration Files**: ~/.cliant/config
3. **Persistent State**: Resume downloads across sessions
4. **Metrics Collection**: Prometheus integration
5. **Middleware Composition**: More granular middleware pipeline
6. **Connection Pooling**: Reuse connections for multiple downloads
7. **Rate Limiting**: Per-destination bandwidth limits
8. **Request Pipelining**: Multiple concurrent requests to same host

---

For implementation details and code examples, see [ARCHITECTURE.md](ARCHITECTURE.md).
For testing strategies, see [TESTING.md](TESTING.md).
For contribution guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

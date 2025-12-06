# Cliant-rs

![Rust](https://github.com/val-en-tine124/cliant-rs/actions/workflows/rust.yml/badge.svg)

A state-of-the-art HTTP client for embarrassingly parallel tasks.

## Overview

`cliant-rs` is a powerful command-line HTTP client designed for efficient and parallel downloading of files. It leverages multi-threading to split large files into smaller parts and download them concurrently, significantly speeding up the download process. The client provides robust error handling, including retry mechanisms for network issues, and offers a customizable experience through various command-line arguments.

## Features

* **Parallel Downloads**: Splits files into multiple parts and downloads them simultaneously.
* **Configurable Concurrency**: Control the number of concurrent download parts.
* **Customizable HTTP Client**: Configure timeouts, redirects, proxies, and custom headers.
* **Authentication Support**: Basic authentication with username and password.
* **Progress Tracking**: Visual progress bars for ongoing downloads.
* **Robust Error Handling**: Retry mechanisms for transient network errors.
* **Flexible File Naming**: Automatically determines filenames from headers or generates random ones.
* **Verbose Logging**: Optional detailed logging for debugging.

## Installation

To build and run `cliant-rs`, you need to have [Rust](https://www.rust-lang.org/tools/install) and Cargo installed on your system.

1. **Clone the repository:**

    ```bash
    git clone https://github.com/Abba-Valentine/cliant-rs.git
    cd cliant-rs
    ```

2. **Build the project:**

    ```bash
    cargo build --release
    ```

    The executable will be found in `target/release/cliant-rs` (or `target/release/cliant-rs.exe` on Windows).

## Usage

```bash
cliant-rs [OPTIONS] --url <URL>
```

### Command-Line Arguments

* `-u`, `--url <URL>` (Required):
    The URL of the file to download.

* `--username <USERNAME>`:
    The username for authentication.

* `-p`, `--password <PASSWORD>`:
    The password for authentication.

* `--max-redirects <MAX_REDIRECTS>`:
    The maximum number of redirects to follow. Defaults to `5`.

* `-t`, `--timeout <TIMEOUT>`:
    The timeout in seconds for the request. Defaults to `60` seconds.

* `--proxy-url <PROXY_URL>`:
    The URL of the proxy to use (e.g., `http://localhost:8080`).

* `-H`, `--request-headers <HEADERS>`:
    Custom HTTP request headers (e.g., `"Authorization: Bearer token"`).

* `-c`, `--http-cookies <COOKIES>`:
    HTTP cookies to include in the request (e.g., `"sessionid=abc; csrftoken=xyz"`).

* `--http-version <VERSION>`:
    The HTTP version to use (`1.1` or `2`).

* `-M`, `--max-concurrent-part <MAX_CONCURRENT_PART>`:
    The maximum number of concurrent parts to download. Defaults to `10`.

* `-v`, `--verbose`:
    Enable verbose logging to stdout.

### Environment Variables

* `CLIANT_ROOT`:
    Specifies the root directory where downloaded files will be saved. If not set, defaults to the current working directory.

### Example

To download a file with 5 concurrent parts and verbose logging:

```bash
cliant-rs -u https://example.com/large_file.zip -M 5 -v
```

To download a file to a specific directory:

```bash
CLIANT_ROOT=/path/to/downloads cliant-rs -u https://example.com/another_file.tar.gz
# On Windows:
set CLIANT_ROOT=C:\Users\YourUser\Downloads && cliant-rs -u https://example.com/another_file.tar.gz
```

## Contributing

Contributions are welcome! Please feel free to open issues or submit pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

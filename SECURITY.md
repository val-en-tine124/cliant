# Security Policy

This document outlines the security practices and policies for the Cliant project.

## Reporting Security Vulnerabilities

If you discover a security vulnerability in Cliant, please **do not** open a public GitHub issue.

Instead, please email security concerns to: **<valentinechibueze400@gmail.com>**

Please include:

- Description of the vulnerability
- Steps to reproduce (if applicable)
- Potential impact assessment
- Suggested fix (if available)

We will respond to security reports within 48 hours and work with you to validate and fix the issue.

## Security Practices

### Input Validation

All user inputs are validated:

- **URLs**: Must contain valid `http://` or `https://` scheme
- **File Paths**: Must be valid file paths (not directories)
- **HTTP Headers**: Validated against RFC specifications
- **Query Parameters**: Type-checked and range-validated
- **Timeouts**: Must be positive integers

### Authentication

- **Basic Auth**: Uses `secrecy` crate to keep passwords in memory with protection
- **Headers/Cookies**: User-provided but warned about insecure transmission
- **No Credential Storage**: Credentials are not persisted to disk by default
- **Secret Handling**: Sensitive strings are not logged or displayed

### HTTPS

- HTTPS is the recommended protocol
- HTTP is allowed but users should be aware of security implications
- TLS version is handled by `reqwest` (modern TLS 1.2+)
- Certificate validation is enabled by default

### Error Handling

- Sensitive information (passwords, tokens) are **never** logged
- Error messages are informative but don't expose system paths unnecessarily
- Stack traces in debug builds only (not in release builds)
- Error context includes enough information for debugging without exposure

### Dependency Security

- Regular updates for critical dependencies
- Use of well-maintained, reputable crates from crates.io
- Minimal dependencies to reduce attack surface
- Dependencies are checked for known vulnerabilities via `cargo audit`
- License compliance verified for all dependencies

### Retry and Timeout Logic

- Configurable timeouts prevent hanging on unresponsive servers
- Max retry limits prevent infinite retry loops (default: 10 retries)
- Retry delay starts at 10 seconds and doubles each attempt

### Proxy Support

- HTTP proxy support implemented for flexibility
- HTTPS proxy connections use TLS encryption
- Proxy URL validation ensures valid format
- Consider security implications of proxy usage

## Data Protection

### In Transit

- HTTPS recommended for all downloads (default for security-conscious users)
- TLS encryption with modern cipher suites (handled by `reqwest`)
- HTTP traffic is unencrypted (user's responsibility to use HTTPS)
- Proxy traffic security depends on proxy configuration

### At Rest

- Files are written to filesystem with standard OS permissions
- No encryption of downloaded files (user's responsibility if needed)
- Temporary buffers (4MB) are in RAM, cleared after write
- No temporary files left behind (all I/O direct to destination)

### Memory Security

- Passwords stored in memory using `secrecy` crate
- Sensitive data cleared when no longer needed
- No unnecessary copies of sensitive data
- Memory-safe operations via Rust's type system

### Logging

- Verbose logs may contain URLs (but not credentials)
- Set appropriate log levels to control information disclosure
- Use `RUST_LOG` environment variable to filter operations
- Debug builds may include more information than release builds
- Credentials are explicitly excluded from all log outputs

## Safe Practices for Users

### 1. Always Use HTTPS

```bash
# Good - secure HTTPS connection
cliant download https://trusted-site.com/file.zip -o ~/file.zip

# Avoid - unencrypted HTTP connection
cliant download http://untrusted-site.com/file.zip -o ~/file.zip
```

### 2. Handle Credentials Carefully

```bash
# Avoid - password visible in process list and shell history
cliant download https://api.example.com/file -U user -P password

# Better - use environment variables or credential managers
export CLIANT_HTTP_USERNAME="user"
export CLIANT_HTTP_PASSWORD="your-password"
cliant download https://api.example.com/file
```

### 3. Verify Downloaded Files

- Check file checksums when provided by the source
- Verify cryptographic signatures for critical files
- Scan for malware if downloading executables
- Use antivirus software for suspicious files

### 4. Proxy Usage

- Only use trusted proxies
- Avoid proxies for sensitive authentication data
- Be aware proxy operators can see traffic metadata (even with HTTPS)
- Consider VPN as alternative to untrusted proxies

### 5. File Permissions

Downloaded files inherit umask permissions; consider adjusting:

```bash
cliant download https://example.com/key.pem -o ~/keys/key.pem
chmod 600 ~/keys/key.pem  # Owner read/write only
```

### 6. Network Security

- Use VPN on untrusted networks (public WiFi)
- Verify domain names carefully (phishing prevention)
- Use firewall rules if available
- Monitor network traffic with tools like `tcpdump` or Wireshark

## Vulnerability Handling

### Disclosure Timeline

1. **Report Received**: Immediate acknowledgment
2. **Validation** (24-48 hours): Verify vulnerability and impact
3. **Fix Development** (1-2 weeks): Create and test patch
4. **Coordination** (optional): Coordinate disclosure with affected parties
5. **Release**: Patch released in new version
6. **Disclosure**: Public disclosure after fix is available

### Severity Levels

- **Critical**: Remote code execution, auth bypass, data corruption
  - Action: Emergency patch release
  - Timeline: 24-48 hours

- **High**: Privilege escalation, information disclosure, DoS
  - Action: Priority patch release
  - Timeline: 1 week

- **Medium**: Partial bypass, non-critical vulnerability
  - Action: Standard patch release
  - Timeline: 2-4 weeks

- **Low**: Minor issues, edge cases, theoretical vulnerabilities
  - Action: Include in next release
  - Timeline: Standard release cycle

## Version Support

- Only the latest stable version receives security updates
- Major releases are maintained for critical vulnerabilities (6 months)
- Older versions are encouraged to upgrade
- Long-term support (LTS) versions planned for future

## Security Roadmap

- [ ] Support for credential manager integration (macOS Keychain, Windows Credential Manager)
- [ ] Certificate pinning for critical connections
- [ ] Checksum verification (MD5, SHA256, SHA512)
- [ ] File integrity checking
- [ ] Audit logging option
- [ ] FIPS mode compliance (future consideration)
- [ ] Signed releases with GPG
- [ ] Security scanning in CI/CD pipeline

## Compliance

This project aims to follow:

- OWASP Top 10 prevention practices
- Rust security best practices and RFC 3156 (PGP)
- CWE/SANS top 25 mitigations where applicable
- NIST Cybersecurity Framework recommendations

## Security Testing

### Regular Audits

- Dependency scanning via `cargo audit`
- Code review for security issues
- Manual penetration testing (planned)
- Fuzzing tests for input handling (planned)

### Testing Practices

- Input validation tests
- Boundary condition testing
- Error handling verification
- Resource cleanup on errors
- Timeout and retry logic validation

## Incident Response

### If a Vulnerability is Discovered

1. **Immediate Actions**:
   - Acknowledge receipt of report
   - Validate vulnerability claims
   - Assess impact and severity

2. **Mitigation**:
   - Create secure branch for patch
   - Develop and test fix
   - Ensure no new vulnerabilities introduced

3. **Communication**:
   - Keep reporter informed of progress
   - Coordinate disclosure timing
   - Prepare user advisory

4. **Release**:
   - Release patched version
   - Update security advisories
   - Notify users of available patch

## Third-Party Security Services

### Recommended Tools

- **Cargo Audit**: Check for known vulnerabilities in dependencies

  ```bash
  cargo audit
  ```

- **Clippy**: Lint for common mistakes

  ```bash
  cargo clippy
  ```

- **OWASP Dependency-Check**: Identify vulnerable dependencies
- **Snyk**: Continuous vulnerability scanning
- **GitHub Security**: Automated vulnerability scanning

### Best Practices

- Enable security scanning in CI/CD
- Keep dependencies up to date
- Monitor security advisories
- Subscribe to Rust security mailing list

## Questions?

For security-related questions (non-vulnerability), you can:

- Open a GitHub [discussion](https://github.com/val-en-tine124/cliant/discussions)
- Email the maintainers (non-sensitive questions only)
- Check the documentation and existing issues first

---

**Last Updated**: January 2026
**Version**: 1.0

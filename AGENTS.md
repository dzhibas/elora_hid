# AGENTS.md - Agentic Coding Guidelines for elora_hid

## Project Overview

This is a Rust application that pushes stock prices (TSLA, VWRL.AS, NVDA) to an Elora split keyboard via USB HID. The project uses Tokio for async runtime, Reqwest for HTTP, and hidapi for USB communication.

**Tech Stack:**
- Language: Rust (Edition 2021)
- Async Runtime: Tokio
- HTTP Client: Reqwest
- USB/HID: hidapi
- Logging: env_logger + log

---

## Build Commands

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Run the application
cargo run

# Build with verbose output (CI style)
cargo build --verbose
```

---

## Lint Commands

```bash
# Format code
cargo fmt

# Check formatting (CI)
cargo fmt --check

# Run Clippy linter
cargo clippy

# Run Clippy with all warnings as errors
cargo clippy -- -D warnings
```

No custom rustfmt.toml or clippy.toml - uses default Rust conventions.

---

## Test Commands

```bash
# Run all tests
cargo test

# Run all tests with verbose output
cargo test --verbose

# Run a single test by name
cargo test testing_fetch_of_stock
cargo test testing_conversion_to_buffer

# Run tests with partial name match
cargo test fetch_of_stock
cargo test conversion

# Show stdout/stderr output during tests
cargo test -- --nocapture

# Run single test with output visible
cargo test testing_fetch_of_stock -- --nocapture

# List all available tests
cargo test -- --list
```

### Current Tests

| Test Name | Type | Location |
|-----------|------|----------|
| `testing_fetch_of_stock` | Async (#[tokio::test]) | src/main.rs:143 |
| `testing_conversion_to_buffer` | Sync (#[test]) | src/main.rs:166 |

---

## Code Style Guidelines

### Import Organization

Imports are organized in this order:
1. Standard library imports (grouped with curly braces)
2. External crates (one per line, alphabetical)

```rust
use std::{collections::BTreeMap, error::Error, time::Duration};

use hidapi::{DeviceInfo, HidApi};
use regex::Regex;
use reqwest::Client;
```

### Naming Conventions

| Type | Convention | Examples |
|------|------------|----------|
| Constants | SCREAMING_SNAKE_CASE | `VENDOR_ID`, `PRODUCT_ID`, `REFRESH_RATE_SECS` |
| Functions | snake_case | `fetch_stock_tickers()`, `convert_to_buffer()` |
| Type aliases | PascalCase | `StockTickerType`, `AppError` |
| Variables | snake_case | `stocks`, `regex_str`, `chrome_user_agent` |

### Type Aliases

Use type aliases for complex types and custom errors:
```rust
type StockTickerType = BTreeMap<&'static str, f64>;
type AppError = Box<dyn Error>;
```

### Error Handling

1. Functions return `Result<T, AppError>` where `AppError = Box<dyn Error>`
2. Use `?` operator for error propagation
3. Use `.into()` to convert strings to errors

```rust
// Propagate errors with ?
let req = client.get(url).send().await?;

// Return string errors
if device.is_none() {
    return Err("Device disconnected".into());
}

// Check errors explicitly when you don't want to propagate
if res.is_err() {
    log::error!("Error occurred while sending data to keyboard");
}
```

### Documentation

Use `///` doc comments for public items:
```rust
/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8d1d;

/// Converts StockTickerType into string which is sent through usb to keyboard
fn convert_to_buffer(stocks: StockTickerType) -> Vec<u8> {
```

Use `//` inline comments for clarifications:
```rust
// we use max 4 chars for ticker so it fits
let st_string = format!("{:.4}: {:.0}$", ticker, v);
```

### Async Code

- Mark async functions with `async fn`
- Use `#[tokio::main]` for main entry point
- Use `#[tokio::test]` for async tests

### Logging

Use the `log` crate macros:
```rust
log::info!("Fetching stock tickers from remote");
log::debug!("Fetching complete");
log::error!("Error occurred while sending data to keyboard");
```

---

## Testing Conventions

1. Tests are inline in the same file (not in separate test modules)
2. Test function naming: `testing_<what_is_tested>`
3. Use `assert_eq!` for assertions
4. Use `dbg!` for debug output during test development

```rust
#[tokio::test]
async fn testing_fetch_of_stock() -> Result<(), AppError> {
    let st = fetch_stock_tickers().await?;
    assert_eq!(st.contains_key("TSLA"), true);
    dbg!(&st);
    Ok(())
}

#[test]
fn testing_conversion_to_buffer() {
    let stocks: StockTickerType = BTreeMap::from([("TSLA", 500.0)]);
    let buf = convert_to_buffer(stocks);
    assert_eq!(String::from_utf8(buf).unwrap(), "TSLA: 500$");
}
```

---

## Project Structure

```
elora_hid/
├── src/
│   └── main.rs          # All application code (single-file app)
├── .github/
│   └── workflows/
│       └── rust.yml     # CI pipeline
├── Cargo.toml           # Project manifest
├── Cargo.lock           # Dependency lock file
└── README.md            # User documentation
```

---

## CI/CD

GitHub Actions runs on push/PR to `main`:
1. Installs libudev-dev
2. Runs `cargo build --verbose`
3. Runs `cargo test --verbose`

---

## Hardware Context

- Target device: splitkb.com Elora keyboard
- Vendor ID: `0x8d1d`
- Product ID: `0x9d9d`
- Uses custom QMK firmware with HID support

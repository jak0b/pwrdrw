# Repository Guidelines

## Structure

`src/main.rs` contains the CLI entry point, `src/metrics.rs` implements the Prometheus endpoint, and `src/power_reading.rs` handles AppleSMC access. `Cargo.toml` and `Cargo.lock` define the Rust application. `scripts/` contains the LaunchAgent installer and uninstaller.

## Build and Run

Run `cargo build --release` to build `target/release/power-usage`. Run `./target/release/power-usage` on a Mac with the `AppleSMC` service to view power readings. Run `cargo clean` to remove build artifacts.

## Style and Testing

Use standard Rust formatting; run `cargo fmt --check` and `cargo clippy -- -D warnings`. No automated test suite is configured. After changes, run `cargo build --release` and check that `./target/release/power-usage` prints both output lines with plausible values. Run it outside the sandbox for this manual check: sandboxed access to AppleSMC can fail with `Unable to open AppleSMC` even when the service is available. When changing metrics, also run the server outside the sandbox and check the `/metrics` response.

## Commits and Pull Requests

The initial commit uses a `chore:` prefix. Use short, descriptive commit subjects. Pull requests should describe the change and include build and manual test results; include sample output for changes to displayed readings.

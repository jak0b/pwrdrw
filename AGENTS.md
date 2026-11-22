# Repository Guidelines

## Structure

`main.c` contains the complete macOS command-line program. There are no tests or assets yet.

## Build and Run

Run `make` to compile `main.c` with IOKit and CoreFoundation into `power-usage`. Run `./power-usage` on a Mac with the `AppleSMC` service to view power readings. Run `make clean` to remove the binary.

## Style and Testing

Use two-space indentation, same-line opening braces, and `snake_case` names, matching `main.c`. No formatter or automated test suite is configured. After changes, run `make` and check that `./power-usage` prints both output lines with plausible values. Run `./power-usage` outside the sandbox for this manual check: sandboxed access to AppleSMC can fail with `Unable to open AppleSMC` even when the service is available. When changing metrics, also run the server outside the sandbox and check the `/metrics` response.

## Commits and Pull Requests

The initial commit uses a `chore:` prefix. Use short, descriptive commit subjects. Pull requests should describe the change and include build and manual test results; include sample output for changes to displayed readings.

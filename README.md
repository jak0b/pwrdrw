# power-usage-macos

Read DC input voltage, current, power, and total system power from the macOS AppleSMC service.

## Build

Build the Rust application:

```sh
cargo build --release
```

## Readings

Run `./target/release/power-usage` to print one reading.

Use `./target/release/power-usage -l` to print readings repeatedly. The repeat interval defaults to one second.

- `-r milliseconds` changes the repeat interval.
- `-c` clears the screen between readings.

## Prometheus metrics

Run the exporter on port 9100:

```sh
./target/release/power-usage -p 9100
```

Metrics are available at <http://localhost:9100/metrics>. The server listens on all network interfaces and reads AppleSMC on each scrape. When Prometheus runs on the same Mac, use `localhost:9100` as the scrape target.

The endpoint exports four gauges:

- `macos_power_input_voltage_volts`
- `macos_power_input_current_amperes`
- `macos_power_input_watts`
- `macos_system_power_watts`

DC input power is calculated as voltage multiplied by current. The endpoint returns HTTP 503 if a reading fails.

## Install as a background service

Build and install a per-user LaunchAgent that starts at login and restarts if it exits:

```sh
cargo build --release
./scripts/install.sh target/release/power-usage
```

The service listens on all IPv4 and IPv6 network interfaces on port 9100. Choose another port with `PORT=9200 ./scripts/install.sh target/release/power-usage`. Because the unauthenticated metrics endpoint is reachable from the network, restrict access with your firewall if needed.

To stop the service for the current login session:

```sh
launchctl bootout "gui/$(id -u)/com.jak0b.power-usage-macos.agent"
```

The installed binary is in `~/Library/Application Support/power-usage-macos/`, and the LaunchAgent is in `~/Library/LaunchAgents/`.

Remove the service, binary, and its log with:

```sh
./scripts/uninstall.sh
```

## License

This project is licensed under the [MIT License](LICENSE).

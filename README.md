# pwrdrw

Read DC input voltage, current, power, and total system power from the macOS AppleSMC service.

## Build

Build the Rust application:

```sh
cargo build --release
```

## Readings

Run `pwrdrw` to print one reading, including the current power source (`AC Power` or `Battery`), DC input voltage/current/power, total system power, and battery charge percentage when the SMC exposes it.

Use `pwrdrw -l` to print readings repeatedly. The repeat interval defaults to one second.

- `-r milliseconds` changes the repeat interval.
- `-c` clears the screen between readings.

## Prometheus metrics

Run the exporter on the default port, 9100:

```sh
pwrdrw metrics
```

By default, the server listens on all IPv4 and IPv6 network interfaces and polls AppleSMC once per second in the background. Use `-p` to choose another port, `-l` to change the polling interval in milliseconds, and specify a comma-separated list of IP addresses with `-h` to restrict which interfaces it binds to:

```sh
pwrdrw metrics -p 9100 -l 500 -h 127.0.0.1,192.168.1.10
```

Metrics are available at `http://<bound-address>:9100/metrics`. The app prints the actual bound addresses when it starts. When Prometheus runs on the same Mac, use `localhost:9100` as the scrape target if the server is listening on the loopback interface.

The endpoint exports these gauges:

- `macos_power_input_voltage_volts`
- `macos_power_input_current_amperes`
- `macos_power_input_watts`
- `macos_system_power_watts`
- `macos_power_source_connected`
- `macos_battery_charge_percentage` (when the SMC percentage key is available)

DC input power is calculated as voltage multiplied by current. `macos_power_source_connected` is `1` when an external power source is connected and `0` when the Mac is using battery power. The endpoint returns HTTP 503 if a reading fails.

## Install as a background service

Build and install a per-user LaunchAgent that starts at login and restarts if it exits:

```sh
cargo build --release
./scripts/install.sh target/release/pwrdrw
```

The service listens on all IPv4 and IPv6 network interfaces on port 9100. Choose another port with `PORT=9200 ./scripts/install.sh target/release/pwrdrw`. Because the unauthenticated metrics endpoint is reachable from the network, restrict access with your firewall if needed.

To stop the service for the current login session:

```sh
launchctl bootout "gui/$(id -u)/com.jak0b.pwrdrw.agent"
```

The installed binary is in `~/Library/Application Support/pwrdrw/`, and the LaunchAgent is in `~/Library/LaunchAgents/`.

Remove the service, binary, and its log with:

```sh
./scripts/uninstall.sh
```

## License

This project is licensed under the [MIT License](LICENSE).

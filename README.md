# pwrdrw

Read DC input voltage, current, and power, along with total system power, on
macOS and Linux systems with Apple Silicon power sensors (including Asahi Linux
and its forks).

## Build

Build the Rust application:

```sh
cargo build --release
```

## Readings

Run `pwrdrw` to print a single reading. The output includes the current power
source (`AC Power` or `Battery`), DC input voltage, current, and power, total
system power, and battery charge percentage when available.

On macOS, readings come from AppleSMC. On Linux, `pwrdrw` reads the `Total
System Power`, `dc in`/`input voltage`, and `dc in`/`input current` labels under
`/sys/class/hwmon`. It also reads
`/sys/class/power_supply/macsmc-battery/capacity` and
`/sys/class/power_supply/macsmc-ac/online`.

Linux power sensors must be exposed by the kernel. Missing required hwmon labels
or AC status will produce a read error. Battery capacity is optional.

Use `pwrdrw -l` to print readings repeatedly. The repeat interval defaults to
one second.

- `-r <milliseconds>` changes the repeat interval.
- `-c` clears the screen between readings.

## How it works

On macOS, `pwrdrw` opens the `AppleSMC` service via the IOKit framework and reads
four SMC keys: `VD0R` (input voltage), `ID0R` (input current), `PSTR` (total
system power), and `BUIC` (battery percentage, optional). Whether the system is
on AC power comes from the IOPS power-sources API rather than the SMC.

On Linux, it walks `/sys/class/hwmon`, matches each sensor by its `*_label`
name, and reads the corresponding `*_input` value, converting millivolts,
milliamps, and microwatts into volts, amperes, and watts.

The `pwrdrw metrics` server is built on `tokio` and `axum`. It spawns a
background task that polls the sensors on the configured interval — on a
blocking thread, guarded by a lock so the async runtime is never blocked — and
records each reading into the `metrics` gauge registry, which the `/metrics`
handler renders on every scrape.

## Prometheus metrics

Run the exporter on the default port, 9100:

```sh
pwrdrw metrics
```

By default, the server listens on all IPv4 and IPv6 network interfaces and
polls the platform's power sensors once per second in the background.

- `-p <port>` selects a different port.
- `-l <milliseconds>` changes the polling interval.
- `-h <addresses>` restricts the server to a comma-separated list of IP
  addresses.

```sh
pwrdrw metrics -p 9100 -l 500 -h 127.0.0.1,192.168.1.10
```

Metrics are available at `http://<bound-address>:9100/metrics`. The app prints
the actual bound addresses when it starts. When Prometheus runs on the same
machine, use `localhost:9100` as the scrape target if the server is listening on
the loopback interface.

The endpoint exports the same gauges on macOS and Linux. Each gauge is prefixed
with `pwrdrw_` and has a `system` label (`darwin` or `linux`) identifying the
operating system:

- `pwrdrw_power_input_voltage_volts`
- `pwrdrw_power_input_current_amperes`
- `pwrdrw_power_input_watts`
- `pwrdrw_system_power_watts`
- `pwrdrw_power_source_connected`
- `pwrdrw_battery_charge_percentage` (when available)

For example, a macOS reading is exported as
`pwrdrw_system_power_watts{system="darwin"}`. DC input power is calculated as
voltage multiplied by current. The `pwrdrw_power_source_connected` gauge is `1`
when an external power source is connected and `0` when the system is using
battery power. The endpoint returns HTTP 503 if a reading fails.

## Install as a background service

On macOS, build and install a per-user LaunchAgent. It starts at login and
restarts if it exits:

```sh
cargo build --release
./scripts/install-darwin.sh target/release/pwrdrw
```

The service listens on all IPv4 and IPv6 network interfaces on port 9100. To
choose another port, set `PORT` when installing:

```sh
PORT=9200 ./scripts/install-darwin.sh target/release/pwrdrw
```

The unauthenticated metrics endpoint is reachable from the network. Restrict
access with your firewall if needed.

To stop the service for the current login session:

```sh
launchctl bootout "gui/$(id -u)/com.jak0b.pwrdrw.agent"
```

The installed binary is in `~/Library/Application Support/pwrdrw/`, and the
LaunchAgent is in `~/Library/LaunchAgents/`.

Remove the service, binary, and its log with:

```sh
./scripts/uninstall-darwin.sh
```

On Linux with systemd, install a per-user service. It starts at login and
restarts if it exits:

```sh
cargo build --release
./scripts/install-systemd.sh target/release/pwrdrw
```

Set `PORT=9200` during installation to select another port. The service listens
on all IPv4 and IPv6 network interfaces, so restrict access with your firewall
if needed. View its logs with `journalctl --user -u pwrdrw.service`.

The installed binary is in `~/.local/share/pwrdrw/`, and the unit is in
`~/.config/systemd/user/`.

Stop the service for the current login session with
`systemctl --user stop pwrdrw.service`, or remove the service and installed
binary with:

```sh
./scripts/uninstall-systemd.sh
```

## License

This project is licensed under the [MIT License](LICENSE).

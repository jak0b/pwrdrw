# power-usage-macos

Read DC input voltage, current, power, and total system power from the macOS AppleSMC service.

## Build

Clone the repository with its CivetWeb submodule:

```sh
git clone --recurse-submodules https://github.com/jak0b/power-usage-macos.git
```

For an existing clone, initialize the submodule and build:

```sh
make submodule
make
```

## Readings

Run `./power-usage` to print one reading.

Use `./power-usage -l` to print readings repeatedly. The repeat interval defaults to one second.

- `-r milliseconds` changes the repeat interval.
- `-c` clears the screen between readings.

## Prometheus metrics

Run the exporter on port 9100:

```sh
./power-usage -p 9100
```

Metrics are available at <http://localhost:9100/metrics>. The server listens on all network interfaces and reads AppleSMC on each scrape. When Prometheus runs on the same Mac, use `localhost:9100` as the scrape target.

The endpoint exports four gauges:

- `macos_power_input_voltage_volts`
- `macos_power_input_current_amperes`
- `macos_power_input_watts`
- `macos_system_power_watts`

DC input power is calculated as voltage multiplied by current. The endpoint returns HTTP 503 if a reading fails.

The HTTP server is [CivetWeb v1.16](https://github.com/civetweb/civetweb/releases/tag/v1.16), pinned as a Git submodule in `vendor/civetweb/` under its MIT license. No separate HTTP package is needed.

## License

This project is licensed under the [MIT License](LICENSE).

# power-usage-macos

Read DC input voltage, current, power, and total system power from the macOS AppleSMC service.

Clone with `git clone --recurse-submodules`, or run `make submodule` in an existing clone, then build with `make`. Run `./power-usage` for one reading, or `./power-usage -l` to print readings repeatedly. Use `-r milliseconds` to change the repeat interval and `-c` to clear the screen between repeated readings.

Run `./power-usage -p 9100` to serve Prometheus metrics at `http://localhost:9100/metrics`. The server listens on all network interfaces, and reads AppleSMC on each scrape. Configure Prometheus with a scrape target of `localhost:9100` when it runs on the same Mac.

The endpoint exports four gauges: `power_usage_dc_voltage_volts`, `power_usage_dc_current_amperes`, `power_usage_dc_power_watts`, and `power_usage_system_total_power_watts`. DC power is calculated as voltage multiplied by current. The endpoint returns HTTP 503 if a reading fails.

The HTTP server is [CivetWeb v1.16](https://github.com/civetweb/civetweb/releases/tag/v1.16), pinned as a Git submodule at `vendor/civetweb/` under its MIT license. No separate HTTP package is needed.

## License

This project is licensed under the [MIT License](LICENSE).

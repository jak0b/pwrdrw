#!/bin/sh
set -eu

service="pwrdrw.service"
port="${PORT:-9100}"
binary="${1:-target/release/pwrdrw}"

case "$port" in
  ''|*[!0-9]*)
    echo "Invalid PORT: expected a number from 1 to 65535" >&2
    exit 1
    ;;
esac
if [ "$port" -lt 1 ] || [ "$port" -gt 65535 ]; then
  echo "Invalid PORT: expected a number from 1 to 65535" >&2
  exit 1
fi
if [ "$(id -u)" -eq 0 ]; then
  echo "Run scripts/install-systemd.sh as the target user, not with sudo" >&2
  exit 1
fi
if [ ! -x "$binary" ]; then
  echo "Binary not found or not executable: $binary (build with cargo build --release first)" >&2
  exit 1
fi
if ! command -v systemctl >/dev/null 2>&1; then
  echo "systemctl not found; this installer requires systemd" >&2
  exit 1
fi

app_dir="$HOME/.local/share/pwrdrw"
unit_dir="$HOME/.config/systemd/user"
installed_binary="$app_dir/pwrdrw"
unit_file="$unit_dir/$service"

mkdir -p "$app_dir" "$unit_dir"
install -m 755 "$binary" "$installed_binary"

cat > "$unit_file" <<UNIT
[Unit]
Description=pwrdrw Prometheus power metrics exporter

[Service]
Type=simple
ExecStart=%h/.local/share/pwrdrw/pwrdrw metrics -p $port
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
UNIT
chmod 644 "$unit_file"

systemctl --user daemon-reload
systemctl --user enable --now "$service"

printf 'Installed and started %s on port %s.\n' "$installed_binary" "$port"
printf 'Metrics endpoint: http://<this-linux-host-ip>:%s/metrics\n' "$port"
printf 'Logs: journalctl --user -u %s\n' "$service"

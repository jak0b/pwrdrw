#!/bin/sh
set -eu

label="com.jak0b.pwrdrw.agent"
legacy_label="com.jak0b.power-usage-macos.agent"
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
  echo "Run scripts/install-darwin.sh as the target user, not with sudo" >&2
  exit 1
fi
if [ ! -x "$binary" ]; then
  echo "Binary not found or not executable: $binary (build with cargo build --release first)" >&2
  exit 1
fi

app_dir="$HOME/Library/Application Support/pwrdrw"
legacy_app_dir="$HOME/Library/Application Support/power-usage-macos"
agent_dir="$HOME/Library/LaunchAgents"
log_dir="$HOME/Library/Logs/pwrdrw"
installed_binary="$app_dir/pwrdrw"
plist="$agent_dir/$label.plist"
legacy_plist="$agent_dir/$legacy_label.plist"
uid="$(id -u)"

mkdir -p "$app_dir" "$agent_dir" "$log_dir"
install -m 755 "$binary" "$installed_binary"
launchctl bootout "gui/$uid/$legacy_label" >/dev/null 2>&1 || true
rm -f "$legacy_plist" "$legacy_app_dir/power-usage"

xml_escape() {
  printf '%s' "$1" | sed \
    -e 's/&/\&amp;/g' \
    -e 's/</\&lt;/g' \
    -e 's/>/\&gt;/g' \
    -e 's/"/\&quot;/g' \
    -e "s/'/\&apos;/g"
}

escaped_binary="$(xml_escape "$installed_binary")"
escaped_log="$(xml_escape "$log_dir")"
cat > "$plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>$escaped_binary</string>
    <string>metrics</string>
    <string>-p</string>
    <string>$port</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$escaped_log/pwrdrw.log</string>
  <key>StandardErrorPath</key>
  <string>$escaped_log/pwrdrw.log</string>
</dict>
</plist>
PLIST
chmod 644 "$plist"

launchctl bootout "gui/$uid/$label" >/dev/null 2>&1 || true
launchctl enable "gui/$uid/$label"
launchctl bootstrap "gui/$uid" "$plist"

printf 'Installed and started %s on port %s.\n' "$installed_binary" "$port"
printf 'Metrics endpoint: http://<this-mac-ip>:%s/metrics\n' "$port"
printf 'Logs: %s/pwrdrw.log\n' "$log_dir"

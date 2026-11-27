#!/bin/sh
set -eu

label="com.jak0b.pwrdrw.agent"
legacy_label="com.jak0b.power-usage-macos.agent"
app_dir="$HOME/Library/Application Support/pwrdrw"
legacy_app_dir="$HOME/Library/Application Support/power-usage-macos"
agent_dir="$HOME/Library/LaunchAgents"
log_dir="$HOME/Library/Logs/pwrdrw"
legacy_log_dir="$HOME/Library/Logs/power-usage-macos"
uid="$(id -u)"

if [ "$uid" -eq 0 ]; then
  echo "Run scripts/uninstall-darwin.sh as the user who installed the LaunchAgent, not with sudo" >&2
  exit 1
fi

launchctl bootout "gui/$uid/$label" >/dev/null 2>&1 || true
launchctl bootout "gui/$uid/$legacy_label" >/dev/null 2>&1 || true
rm -f "$agent_dir/$label.plist"
rm -f "$agent_dir/$legacy_label.plist"
rm -f "$app_dir/pwrdrw"
rm -f "$log_dir/pwrdrw.log"
rm -f "$legacy_app_dir/power-usage"
rm -f "$legacy_log_dir/power-usage.log"
rmdir "$app_dir" "$log_dir" "$legacy_app_dir" "$legacy_log_dir" 2>/dev/null || true

echo "Uninstalled $label."

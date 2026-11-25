#!/bin/sh
set -eu

label="com.jak0b.power-usage-macos.agent"
app_dir="$HOME/Library/Application Support/power-usage-macos"
agent_dir="$HOME/Library/LaunchAgents"
log_dir="$HOME/Library/Logs/power-usage-macos"
uid="$(id -u)"

if [ "$uid" -eq 0 ]; then
  echo "Run scripts/uninstall.sh as the user who installed the LaunchAgent, not with sudo" >&2
  exit 1
fi

launchctl bootout "gui/$uid/$label" >/dev/null 2>&1 || true
rm -f "$agent_dir/$label.plist"
rm -f "$app_dir/power-usage"
rm -f "$log_dir/power-usage.log"
rmdir "$app_dir" "$log_dir" 2>/dev/null || true

echo "Uninstalled $label."

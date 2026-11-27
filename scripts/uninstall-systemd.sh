#!/bin/sh
set -eu

service="pwrdrw.service"
app_dir="$HOME/.local/share/pwrdrw"
unit_dir="$HOME/.config/systemd/user"

if [ "$(id -u)" -eq 0 ]; then
  echo "Run scripts/uninstall-systemd.sh as the user who installed the service, not with sudo" >&2
  exit 1
fi
if ! command -v systemctl >/dev/null 2>&1; then
  echo "systemctl not found; this uninstaller requires systemd" >&2
  exit 1
fi

systemctl --user disable --now "$service" >/dev/null 2>&1 || true
rm -f "$unit_dir/$service" "$app_dir/pwrdrw"
systemctl --user daemon-reload
rmdir "$app_dir" 2>/dev/null || true

echo "Uninstalled $service."

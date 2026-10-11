#!/usr/bin/env bash
# Remove everything install.sh put into the system.
set -euo pipefail

sudo systemctl disable titan-kbd-light.service 2>/dev/null || true
sudo rm -f /etc/systemd/system/titan-kbd-light.service \
    /etc/udev/rules.d/99-titan-kbd-light.rules \
    /etc/default/titan-kbd-light \
    /usr/local/bin/titan-kbd-light
sudo systemctl daemon-reload
sudo udevadm control --reload

echo "Uninstalled. The backlight stays as it is until the next reboot."

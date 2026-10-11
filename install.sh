#!/usr/bin/env bash
# Build titan-kbd-light and install it with its systemd service and udev rule.
# Run as your normal user: it builds with cargo and asks for sudo to install.
set -euo pipefail
cd "$(dirname "$0")"

if [[ $EUID -eq 0 ]]; then
    echo "Run ./install.sh as your normal user, not with sudo (cargo builds as you)." >&2
    exit 1
fi

cargo build --release

sudo install -Dm755 target/release/titan-kbd-light /usr/local/bin/titan-kbd-light
sudo install -Dm644 dist/linux/titan-kbd-light.service /etc/systemd/system/titan-kbd-light.service
sudo install -Dm644 dist/linux/99-titan-kbd-light.rules /etc/udev/rules.d/99-titan-kbd-light.rules
# Keep the user's color if the config already exists.
if [[ ! -e /etc/default/titan-kbd-light ]]; then
    sudo install -Dm644 dist/linux/titan-kbd-light.default /etc/default/titan-kbd-light
fi

sudo systemctl daemon-reload
sudo udevadm control --reload
sudo systemctl enable titan-kbd-light.service
sudo systemctl start titan-kbd-light.service

echo "Installed. Color is set in /etc/default/titan-kbd-light"

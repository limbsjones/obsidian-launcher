#!/bin/bash
set -e

echo "=== Obsidian Launcher Hotkey Daemon Setup ==="

BIN_DIR="$HOME/.cargo/bin"
SERVICE_FILE="obsidian-hotkey-daemon.service"
UDEV_RULE="/etc/udev/rules.d/99-obsidian-launcher.rules"

echo ""
echo "1. Building daemon..."
cargo build --release --bin obsidian-hotkey-daemon

echo "2. Installing binary to $BIN_DIR..."
cp target/release/obsidian-hotkey-daemon "$BIN_DIR/"

echo "3. Setting up udev rule for keyboard access..."
echo 'KERNEL=="event*", SUBSYSTEM=="input", GROUP="input", MODE="0660"' | sudo tee "$UDEV_RULE" > /dev/null
sudo udevadm control --reload-rules
sudo udevadm trigger

echo "4. Adding user to input group..."
sudo usermod -aG input "$USER"

echo "5. Installing systemd user service..."
mkdir -p "$HOME/.config/systemd/user/"
sed "s|%h|$HOME|g" "$SERVICE_FILE" > "$HOME/.config/systemd/user/obsidian-hotkey-daemon.service"

echo "6. Enabling and starting service..."
systemctl --user daemon-reload
systemctl --user enable --now obsidian-hotkey-daemon.service

echo ""
echo "=== Setup complete! ==="
echo ""
echo "The daemon is now running in the background."
echo "It will start automatically on login."
echo ""
echo "Check status: systemctl --user status obsidian-hotkey-daemon"
echo "View logs:    journalctl --user -u obsidian-hotkey-daemon -f"
echo ""
echo "Make sure your hotkey is configured in:"
echo "  ~/.config/obsidian-launcher/config.toml"

#!/bin/bash

sudo systemctl stop forcefield &> /dev/null

if command -v apt-get >/dev/null 2>&1; then
    echo "Detected Debian/Ubuntu (apt)"
    sudo apt-get update
    sudo apt-get install -y wireguard curl
elif command -v pacman >/dev/null 2>&1; then
    echo "Detected Arch Linux (pacman)"
    sudo pacman -Sy --needed --noconfirm wireguard-tools curl
elif command -v dnf >/dev/null 2>&1; then
    echo "Detected Fedora/RHEL (dnf)"
    sudo dnf install -y wireguard-tools curl
elif command -v yum >/dev/null 2>&1; then
    echo "Detected older RHEL/CentOS (yum)"
    sudo yum install -y wireguard-tools curl
else
    echo "ERROR: Could not detect a supported package manager."
    echo "Supported: apt, pacman, dnf, yum"
    exit 1
fi


SERVICE_NAME="forcefield.service"
sudo mkdir /opt/forcefield/ &> /dev/null
x=$(realpath {})
chmod +x $x
sudo mv $x /opt/forcefield/{}
PROGRAM="/opt/forcefield/{}"
UNIT_FILE="/etc/systemd/system/forcefield.service"

sudo tee "$UNIT_FILE" > /dev/null <<EOF
[Unit]
Description=Forcefield
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
ExecStart=$PROGRAM
User=root
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable forcefield
sudo systemctl start forcefield

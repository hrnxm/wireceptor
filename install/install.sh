#!/bin/bash
set -e

echo "Installing..."

sudo rm -rf /opt/wireceptor/
sudo rm -rf /usr/share/applications/wireceptor.desktop
rm -rf ~/.config/autostart/wireceptor.desktop

sudo cp -r wireceptor /opt/wireceptor/
sudo cp wireceptor.desktop /usr/share/applications/
mkdir -p ~/.config/autostart
cp wireceptor.desktop ~/.config/autostart/

echo "Wireceptor installed."

gtk-launch wireceptor
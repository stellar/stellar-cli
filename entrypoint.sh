#!/bin/bash
set -e

# Start D-Bus session bus
export DBUS_SESSION_BUS_ADDRESS="unix:path=/tmp/dbus-session"
dbus-daemon --session --address="$DBUS_SESSION_BUS_ADDRESS" --fork

# Unlock gnome-keyring with an empty password for non-interactive use
eval "$(echo '' | gnome-keyring-daemon --unlock --components=secrets)"
export GNOME_KEYRING_CONTROL
export SSH_AUTH_SOCK

cd /source
exec "$@"

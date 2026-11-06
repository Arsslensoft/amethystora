#!/bin/bash
# Runs in the container that plays this computer: a session bus of its own, a stand-in for the notification
# server, and amethystora-connect, as the user unit would start it (apart from the options a test needs).
set -u
export HOME=/tmp/ours/home XDG_CONFIG_HOME=/tmp/ours/config XDG_STATE_HOME=/tmp/ours/state XDG_RUNTIME_DIR=/tmp/ours/run
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR" /tmp/ours/downloads
chmod 700 "$XDG_RUNTIME_DIR"
# The image's schema, compiled here; the settings are a key file, which the program and gsettings share
mkdir -p /tmp/schemas && cp /schemas-src/org.amethystora.Connect.gschema.xml /tmp/schemas/ && glib-compile-schemas /tmp/schemas
export GSETTINGS_SCHEMA_DIR=/tmp/schemas GSETTINGS_BACKEND=keyfile
# Fedora's KDE Connect in the other container is a desktop: another computer, which the program talks to only while
# the switch `computers` is on (plans/beyond/spec-r4-later.md, part A); run.sh turns it off and on again once
gsettings set org.amethystora.Connect computers true
# The checks send files from that computer and look at them landing: Send to nearby's card (R7-27) has a run of its own in
# tests/computers.rs
gsettings set org.amethystora.Connect ask-first never
ADDRESS=$(dbus-daemon --session --fork --print-address)
echo "$ADDRESS" > /tmp/bus-address
export DBUS_SESSION_BUS_ADDRESS="$ADDRESS"
python3 /interop/fake-notifications.py /tmp/notifications.jsonl > /tmp/fake-notifications.log 2>&1 &
sleep 1
start_daemon() {
    # gdbus stands in for the app, python (fake-module.py) for the module: the bus lets only those call the service
    /target/debug/amethystora-connect --verbose --home-interfaces eth0 --download-dir /tmp/ours/downloads --pictures-dir "$HOME/Pictures" --app-command /interop/fake-app.sh --no-launcher --show-ongoing --app-exe "$(readlink -f /usr/bin/gdbus)" --module-exe "$(readlink -f /usr/bin/python3)" >> /tmp/ours.log 2>&1 &
    echo $! > /tmp/ours.pid
}
start_daemon
echo ready > /tmp/ready
# `restart` asks for the daemon to be started again (a test of reconnecting a paired phone)
while true; do
    if [ -e /tmp/restart ]; then
        rm -f /tmp/restart
        old=$(cat /tmp/ours.pid)
        kill "$old" 2>/dev/null
        # Until it has gone (the loop does not wait: the notification stand-in never ends)
        while kill -0 "$old" 2>/dev/null; do sleep 0.2; done
        start_daemon
    fi
    sleep 0.5
done

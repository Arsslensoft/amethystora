#!/usr/bin/bash
# A machine with no account yet, as an installer leaves one that did not make it: GNOME's first-run setup
# starts on its own account, and none of Amethystora's per-account setup runs in it (the Manual's welcome page
# once opened behind it).
# vmlab-paths: @sf/usr/libexec/amethystora-layout-offer @sf/usr/share/amethystora/user-setup.hooks.d/* @sf/etc/xdg/autostart/* @ui/*hello*
source "$(dirname "$0")/lib.sh"

if getent passwd "${TEST_USER}" >/dev/null; then
    skip "an account exists already, so there is no first run to look at"
    finish
fi
# By its path: pgrep -x sees only the first 15 characters of a process's name. Its account is numbered from
# GNOME 51 (gnome-initial-setup-2), as GDM's greeter accounts are
setup_user() {
    local pid
    pid="$(pgrep -o -f /usr/libexec/gnome-initial-setup)" &&
        ps -o user:64= -p "${pid}" | grep -xE 'gnome-initial-setup(-[0-9]+)?'
}
if wait_for 180 setup_user; then
    ok "GNOME's first-run setup started"
    SETUP_USER="$(setup_user)"
    sleep 60
    check "no Amethystora setup ran for the first-run account" \
        sh -c '! pgrep -u "$1" -f "amethystora-manual|amethystora-layout-offer|amethystora-control welcome"' sh "${SETUP_USER}"
else
    not_ok "GNOME's first-run setup did not start" "$(loginctl list-sessions --no-legend)"
fi
finish

#!/usr/bin/bash
# An account for the suites after this one, logged in on the screen as a person would be: an administrator
# (wheel), signed in by GDM, past GNOME's first-run setup. Its first login runs the per-account setup.
# vmlab-paths: @sf/usr/share/amethystora/user-setup.hooks.d/* @ui/*hints* @ui/*hello*
source "$(dirname "$0")/lib.sh"

if ! getent passwd "${TEST_USER}" >/dev/null; then
    useradd -m -G wheel -c "Test account" "${TEST_USER}"
    echo "${TEST_USER}:${TEST_USER}" | chpasswd
    # The lab's account is new, but the suites after this one need neither Pick a look over their windows nor a tip
    # under the pointer: the first-look hook is marked as run, as for an account that existed before it, and every tip
    # is recorded as shown, in the tips' own state file (modules/hints/state.js), so none is offered while the setting
    # stays the image's, and the greeting as shown, in its own mark (28-first-start.sh has an account of its own for all three)
    install -d -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.local" "/var/home/${TEST_USER}/.local/share" \
        "/var/home/${TEST_USER}/.local/share/amethystora" "/var/home/${TEST_USER}/.local/state"
    install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.local/state/amethystora"
    echo '{"version": {"user": {"first-look": "1"}}}' >"/var/home/${TEST_USER}/.local/share/amethystora/setup_versioning.json"
    chown "${TEST_USER}:" "/var/home/${TEST_USER}/.local/share/amethystora/setup_versioning.json"
    echo '{"version": 1}' >"/var/home/${TEST_USER}/.local/state/amethystora/hello.json"
    chown "${TEST_USER}:" "/var/home/${TEST_USER}/.local/state/amethystora/hello.json"
    seen="$(date +%s)"
    ids="$(sed -n "s/^export const KNOWN = \[\(.*\)\];\$/\1/p" \
        /usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/hints.js | tr -d "' ")"
    hints="$(jq -nc --argjson t "${seen}" --arg ids "${ids}" '{version: 1, since: $t, last: $t,
        shown: ($ids | split(",") | map(select(. != "") | {(.): $t}) | add // {})}')"
    for file in hints.json hints.json.bak; do
        printf '%s' "${hints}" >"/var/home/${TEST_USER}/.local/state/amethystora/${file}"
        chown "${TEST_USER}:" "/var/home/${TEST_USER}/.local/state/amethystora/${file}"
    done
fi
printf '[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=%s\n' "${TEST_USER}" >/etc/gdm/custom.conf
install -d -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.config"
install -o "${TEST_USER}" -g "${TEST_USER}" /dev/null "/var/home/${TEST_USER}/.config/gnome-initial-setup-done"
# The lab renders in software, where GNOME turns animations off, and code behind them would never run (a reveal that threw
# froze real machines' screens, and no suite saw it): the account's shell has them forced on, as a real GPU has them. A
# drop-in of the account's own, never in the image. A suite that wants them off turns the setting off, as a person would
start="$(sed -n 's/^ExecStart=//p' /usr/lib/systemd/user/org.gnome.Shell@.service | head -n 1)"
[[ -n "${start}" ]] || not_ok "GNOME Shell's user unit has no ExecStart to force animations with"
dropin="/var/home/${TEST_USER}/.config/systemd/user/org.gnome.Shell@user.service.d"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "/var/home/${TEST_USER}/.config/systemd" \
    "/var/home/${TEST_USER}/.config/systemd/user" "${dropin}"
printf '[Service]\nExecStart=\nExecStart=%s --force-animations\n' "${start}" >"${dropin}/vmlab-animations.conf"
chown "${TEST_USER}:" "${dropin}/vmlab-animations.conf"
systemctl restart gdm

if wait_for 240 has_session; then
    ok "${TEST_USER} is signed in"
else
    not_ok "${TEST_USER} did not get a session" "$(loginctl list-sessions --no-legend)" "$(journalctl -b -u gdm --no-pager -o cat | tail -n 10)"
    finish
fi
check "the session is Wayland" sh -c "loginctl show-session \$(loginctl show-user ${TEST_USER} -p Display --value) -p Type --value | grep -qx wayland"
check "GNOME Shell runs in it" wait_for 120 pgrep -u "${TEST_USER}" -x gnome-shell
# GNOME's welcome dialog of a first login, left with the keyboard alone (keys.py): Just Perfection once took the
# keyboard from it, and without a mouse there was no way past it
shows_welcome() { user_bus python3 "$(dirname "$0")/atspi.py" gnome-shell has "Welcome to"; }
# By its own button: the overview behind it shows the Manual's window, whose caption is "Welcome to Amethystora" too
welcome_gone() { ! user_bus python3 "$(dirname "$0")/atspi.py" gnome-shell has "Take Tour"; }
if wait_for 120 shows_welcome; then
    # Pressed once the dialog has had time to take the keyboard at the end of its opening, and once more when the
    # first came before it had (as in one round): a keyboard taken from it fails both
    sleep 2
    python3 "$(dirname "$0")/keys.py" esc
    wait_for 10 welcome_gone || python3 "$(dirname "$0")/keys.py" esc
    check "Escape closes GNOME's welcome dialog" wait_for 20 welcome_gone
else
    skip "GNOME's welcome dialog did not show"
fi
# The per-account setup, then the session settled
setup_state() { user_systemctl show amethystora-user-setup.service -p ActiveState -p Result --value | paste -sd ' '; }
setup_done() { [[ "$(setup_state)" == inactive* ]]; }
if wait_for 300 setup_done; then
    state="$(setup_state)"
    if [[ "${state}" == "inactive success" ]]; then
        ok "the per-account setup ran"
    else
        not_ok "the per-account setup ended as ${state}" "$(journalctl -b _SYSTEMD_USER_UNIT=amethystora-user-setup.service --no-pager -o cat | tail -n 15)"
    fi
else
    not_ok "the per-account setup did not finish within 5 minutes: $(setup_state)"
fi
check "the Manual's welcome page opened once" wait_for 120 pgrep -u "${TEST_USER}" -f "amethystora-manual welcome"
failed="$(user_systemctl --failed --no-legend --plain | awk '{print $1}')"
if [[ -z "${failed}" ]]; then ok "no user unit failed"; else not_ok "user units failed: $(paste -sd ' ' <<<"${failed}")"; fi
# The suites drive the session through Eval and atspi, which GNOME does not count as use: after five idle minutes it
# would lock the screen in the middle of one (which turns the extensions off). A suite that wants the lock locks it.
user_bus gsettings set org.gnome.desktop.session idle-delay 0
finish

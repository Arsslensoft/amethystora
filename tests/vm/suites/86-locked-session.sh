#!/usr/bin/bash
# The locked session (plans/done/PLAN-locked-session.md), on the machine as a person would use it.
#
#   1-4. ame security usb locked, in the running session: the screen locked, a USB stick and a plain keyboard
#        plugged in (run.mjs, usb_next): the stick blocked, the keyboard let through, each with GNOME's
#        notification; unlocked, the stick still blocked until it is plugged in again; the login screen of another
#        account counting as unlocked; then `on`, which leaves no rule of GNOME's and blocks a new stick while
#        unlocked; then off.
#   5.   ame security private-folder create in a terminal, files in it, a thumbnail of a picture, a recent file,
#        programs holding a file and a folder in it, Files on it and on another folder; the screen locked: closed at
#        once, gocryptfs gone, the programs named, the thumbnail and the recent file gone, nothing writable left in
#        ~/Private, Files' window on it closed and the other kept.
#   6.   Opened from the app grid's launcher, the wrong password then the right one in its dialog, shown in
#        Files; Close Now; a watcher stopped or killed closes it; a suspend without the screen locking closes
#        it before the machine sleeps.
#   7-8. A new password with the old one and with the recovery key, the old one's file gone; a backup holds the
#        encrypted copy, not the open folder; logging out closes it; removing refuses a folder with a file in
#        it, then takes an empty one away.
# vmlab-paths: @sf/usr/libexec/amethystora-private-folder* @sf/usr/lib/systemd/user/amethystora-private-folder* @sf/usr/share/polkit-1/rules.d/*usb* @sf/usr/share/glib-2.0/schemas/zz1*
source "$(dirname "$0")/lib.sh"

STATE=/var/lib/vmlab-tests/86-locked-session
mkdir -p "${STATE}"
HERE="$(dirname "$0")"
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}" "$(loginctl list-sessions --no-legend)"
    finish
fi
UID_TEST="$(id -u "${TEST_USER}")"
HOME_TEST="$(getent passwd "${TEST_USER}" | cut -d: -f6)"
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"
HELPER=/usr/libexec/amethystora-private-folder
PRIVATE="${HOME_TEST}/Private"
CIPHER="${HOME_TEST}/.local/share/amethystora/private"
PASSWORD_ONE=lockedsession1
PASSWORD_TWO=lockedsession2
PASSWORD_THREE=lockedsession3
# Where sudo asks for the account's password, at its own prompt (a suite before may have let the account off)
SUDO=('\[sudo\] password for' "${TEST_USER}"$'\r')
runuser -u "${TEST_USER}" -- sudo -n true 2>/dev/null && SUDO=()
# atspi.py where the account can run it: the suites' own folder is root's
install -D -m 0644 "${HERE}/atspi.py" /run/vmlab-86/atspi.py
install -m 0644 "${HERE}/keys.py" /run/vmlab-86/keys.py

# A command on a terminal of its own in the account's session, each EXPECT answered with SEND (term.py)
session_terminal() {
    local steps=()
    while [[ $# -gt 0 && "$1" != -- ]]; do steps+=("$1"); shift; done
    shift
    in_terminal "${steps[@]}" -- runuser -u "${TEST_USER}" -- env XDG_RUNTIME_DIR="/run/user/${UID_TEST}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${UID_TEST}/bus" \
        systemd-run --user --pty --wait --quiet --collect -- /usr/bin/bash -lc "$*"
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
record() { runuser -u "${TEST_USER}" -- env HOME="${HOME_TEST}" /usr/libexec/amethystora-security-status --json 2>/dev/null |
    jq -c --arg id "$1" '[.. | objects | select(.id? == $id)][0] | {state, text}'; }
locked() { [[ "$(loginctl show-session "${SESSION}" -p LockedHint --value)" == yes ]]; }
unlocked() { ! locked; }
policy() { usbguard get-parameter InsertedDevicePolicy 2>/dev/null; }
policy_is() { [[ "$(policy)" == "$1" ]]; }
# The sysfs folder of the USB device whose product name is $1 (QEMU's: "QEMU USB HARDDRIVE", "QEMU USB Keyboard"),
# and its serial when two share a name
usb_device() {
    local dev
    for dev in /sys/bus/usb/devices/*; do
        [[ -f "${dev}/product" && "$(cat "${dev}/product")" == "$1" ]] || continue
        [[ -z "${2:-}" || "$(cat "${dev}/serial" 2>/dev/null)" == "$2" ]] || continue
        echo "${dev}"
        return 0
    done
    return 1
}
authorized() { local dev; dev="$(usb_device "$@")" && cat "${dev}/authorized"; }
is_authorized() { [[ "$(authorized "$1" "${2:-}")" == "$3" ]]; }
# What GNOME's notifications said to the account since the monitor started (notify_watch)
notify_watch() {
    pkill -u "${TEST_USER}" -x dbus-monitor 2>/dev/null
    : >/var/tmp/vmlab-86-notifications
    chmod 0666 /var/tmp/vmlab-86-notifications
    # Nothing of it on the suite's own output, which the lab reads until every holder of it has ended
    user_bus sh -c "exec dbus-monitor --session 'type=method_call,interface=org.freedesktop.Notifications,member=Notify' \
        >/var/tmp/vmlab-86-notifications 2>&1" >/dev/null 2>&1 &
    sleep 2
}
notified() { grep -qF "$1" /var/tmp/vmlab-86-notifications; }
mounted() { findmnt -rn -M "${PRIVATE}" >/dev/null 2>&1; }
gocryptfs_running() { pgrep -u "${TEST_USER}" -x gocryptfs >/dev/null; }
closed() { ! mounted && ! gocryptfs_running; }
watcher() { user_systemctl is-active amethystora-private-folder-watch.service 2>/dev/null; }
watcher_gone() { [[ "$(watcher)" != active ]]; }
# Every tab Files has open, one a line (nothing while it does not run)
files_tabs() {
    user_bus timeout 5 busctl --user --auto-start=no --json=short call org.freedesktop.FileManager1 /org/freedesktop/FileManager1 \
        org.freedesktop.DBus.Properties Get ss org.freedesktop.FileManager1 OpenWindowsWithLocations 2>/dev/null | jq -r '.data[0].data[][]'
}
files_shows() { files_tabs | grep -qF "$1"; }
files_left() { ! files_shows "file://${PRIVATE}"; }

# --- 1. Lock-screen USB protection, turned on in the running session -------------------------------------------
if [[ "${SUITE_PHASE}" == 1 ]]; then
    same "USBGuard's D-Bus service is masked as the image ships" "$(systemctl is-enabled usbguard-dbus.service 2>&1)" masked
    same "the report has USB protection off" "$(record usb-protection | jq -r .state)" off
    out="$(session_terminal 300 'Block new USB devices while the screen is locked\?' y "${SUDO[@]}" -- ame security usb locked 2>&1)"
    if [[ $? -eq 0 ]]; then ok "ame security usb locked"; else not_ok "ame security usb locked" "$(tail -n 15 <<<"${out}")"; fi
    same "USBGuard's D-Bus service is enabled" "$(systemctl is-enabled usbguard-dbus.service 2>&1)" enabled
    check "USBGuard and its D-Bus service run" systemctl is-active --quiet usbguard.service usbguard-dbus.service
    same "the rules allow every device, and nothing else" "$(cat /etc/usbguard/rules.conf)" 'allow id *:* label "amethystora-usb-locked"'
    same "the report says it blocks while the screen is locked" "$(record usb-protection)" \
        '{"state":"on","text":"Blocks new USB devices while the screen is locked; a plain keyboard or mouse still works, so that you can unlock"}'
    # GNOME's plugin in the session that was running already: it saw the name appear, and Settings may not change it
    available() {
        user_bus gdbus call --session --dest org.gnome.SettingsDaemon.UsbProtection --object-path /org/gnome/SettingsDaemon/UsbProtection \
            --method org.freedesktop.DBus.Properties.Get org.gnome.SettingsDaemon.UsbProtection Available 2>/dev/null | grep -q true
    }
    check "GNOME's USB protection is available in the session that was running" wait_for 60 available
    same "Settings may not switch it" "$(user_bus gsettings writable org.gnome.desktop.privacy usb-protection) $(user_bus gsettings writable org.gnome.desktop.privacy usb-protection-level)" "false false"
    same "new devices are let in while the screen is unlocked" "$(policy)" apply-policy
    notify_watch
    loginctl lock-session "${SESSION}"
    check "the screen locks" wait_for 60 locked
    if wait_for 60 policy_is block; then
        ok "the running session's plugin blocks new devices once the screen locks, with no new login"
    else
        not_ok "the policy stayed $(policy) with the screen locked" \
            "plugin: $(pgrep -u "${TEST_USER}" -af gsd-usb-protection || echo none running)" \
            "screen saver: $(user_bus gdbus call --session --dest org.gnome.ScreenSaver --object-path /org/gnome/ScreenSaver --method org.gnome.ScreenSaver.GetActive 2>&1)" \
            "$(journalctl -b --since -5min --no-pager -o cat _COMM=gsd-usb-protect 2>&1 | tail -n 8)" \
            "$(journalctl -b --since -3min --no-pager -o cat | grep -iE 'usb.?protection|usbguard|polkit' | grep -v AVC | tail -n 8)"
    fi
    usb_next '{"plug": ["stick", "keyboard"]}'
    continue_phase
fi

# --- 2. What was plugged in while locked, and after unlocking -----------------------------------------------------
if [[ "${SUITE_PHASE}" == 2 ]]; then
    check "the stick and the keyboard show up" wait_for 60 sh -c 'for p in "QEMU USB HARDDRIVE" "QEMU USB Keyboard"; do
        for d in /sys/bus/usb/devices/*; do [ "$(cat "$d/product" 2>/dev/null)" = "$p" ] && continue 2; done; exit 1; done'
    # GNOME decides once USBGuard has blocked it: the keyboard is let through a moment later
    wait_for 30 is_authorized "QEMU USB Keyboard" "" 1
    same "a USB stick plugged in while the screen is locked is blocked" "$(authorized "QEMU USB HARDDRIVE")" 0
    same "a plain keyboard plugged in while the screen is locked works" "$(authorized "QEMU USB Keyboard")" 1
    # GNOME shows one notice of its own at a time: the keyboard's or the stick's, whichever it decided first
    told() { notified "New device detected" || notified "Reconnect USB device"; }
    check "GNOME says what it did with them" wait_for 30 told
    loginctl unlock-session "${SESSION}"
    check "the screen unlocks" wait_for 60 unlocked
    check "new devices are let in again once it is unlocked" wait_for 60 policy_is apply-policy
    same "the stick stays blocked until it is plugged in again" "$(authorized "QEMU USB HARDDRIVE")" 0
    usb_next '{"unplug": ["stick"], "plug": ["stick"]}'
    continue_phase
fi

# --- 3. Plugged in again; the login screen; then `on` -------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 3 ]]; then
    check "the stick works once plugged in again while unlocked" wait_for 60 is_authorized "QEMU USB HARDDRIVE" "" 1
    # Fast user switching, from the locked screen to the login screen and back: the login screen counts as unlocked
    loginctl lock-session "${SESSION}"
    wait_for 60 locked && wait_for 60 policy_is block
    greeter() { loginctl list-sessions --no-legend | awk '$3 == "gdm" || $3 ~ /^gdm-greeter/ { print $1 }' | while read -r s; do
        [[ "$(loginctl show-session "$s" -p State --value)" == active ]] && echo "$s"; done | head -n1; }
    has_greeter() { [[ -n "$(greeter)" ]]; }
    if in_session gdbus call --system --dest org.gnome.DisplayManager --object-path /org/gnome/DisplayManager/LocalDisplayFactory \
        --method org.gnome.DisplayManager.LocalDisplayFactory.CreateTransientDisplay >/dev/null 2>&1 && wait_for 120 has_greeter; then
        GREETER="$(greeter)"
        if wait_for 60 policy_is apply-policy; then
            ok "a new device works at the login screen of another session"
        else
            not_ok "the login screen kept new devices blocked: $(policy)"
        fi
        loginctl activate "${SESSION}"
        wait_for 60 sh -c "[ \"\$(loginctl show-session ${SESSION} -p State --value)\" = active ]"
        sleep 10
        echo "# back on the locked session's screen, new devices are: $(policy)"
        # What GNOME does here is GNOME's: its plugin acts only when the screen locks or unlocks, and the login
        # screen's plugin let devices in. The manual says so (security.md).
        same "back on the locked screen, a new device is let in until it locks again (GNOME's design, in the manual)" "$(policy)" apply-policy
        [[ -n "${GREETER}" ]] && loginctl terminate-session "${GREETER}" 2>/dev/null
    else
        skip "the login screen of another session: GDM started none"
    fi
    loginctl unlock-session "${SESSION}"
    wait_for 60 unlocked
    out="$(session_terminal 300 'Everything plugged in\?' y "${SUDO[@]}" -- ame security usb on 2>&1)"
    if [[ $? -eq 0 ]]; then ok "ame security usb on, after locked"; else not_ok "ame security usb on" "$(tail -n 15 <<<"${out}")"; fi
    same "USBGuard's D-Bus service is masked again" "$(systemctl is-enabled usbguard-dbus.service 2>&1)" masked
    check "no rule of GNOME's is left" sh -c '! usbguard list-rules | grep -q GNOME_SETTINGS_DAEMON_RULE'
    same "the report says it blocks every new device" "$(record usb-protection)" \
        '{"state":"on","text":"Allows what is plugged in now, and blocks any new USB device until you allow it"}'
    usb_next '{"plug": ["stick2"]}'
    continue_phase
fi

# --- 4. A new stick under `on`, then off ------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 4 ]]; then
    check "a second stick shows up" wait_for 60 usb_device "QEMU USB HARDDRIVE" VMLAB-STICK-2
    same "a new stick is blocked while the screen is unlocked, under on" "$(authorized "QEMU USB HARDDRIVE" VMLAB-STICK-2)" 0
    same "the stick allowed when on was set up still works" "$(authorized "QEMU USB HARDDRIVE" VMLAB-STICK-1)" 1
    out="$(session_terminal 120 "${SUDO[@]}" -- ame security usb off 2>&1)"
    if [[ $? -eq 0 ]]; then ok "ame security usb off"; else not_ok "ame security usb off" "$(tail -n 15 <<<"${out}")"; fi
    same "USBGuard is off and its D-Bus service masked" \
        "$(systemctl is-enabled usbguard.service 2>&1) $(systemctl is-enabled usbguard-dbus.service 2>&1)" "disabled masked"
    same "the report has USB protection off" "$(record usb-protection | jq -r .state)" off
    usb_next '{"unplug": ["stick", "stick2", "keyboard"]}'
    continue_phase
fi

# --- 5. The private folder made, used, and closed by the lock ----------------------------------------------------
if [[ "${SUITE_PHASE}" == 5 ]]; then
    check "the test devices are gone" sh -c '! grep -lqs "QEMU USB HARDDRIVE\|QEMU USB Keyboard" /sys/bus/usb/devices/*/product'
    same "the report has no private folder" "$(record private-folder | jq -r .state)" off
    out="$(session_terminal 300 'Make the private folder\?' y 'Password:' "${PASSWORD_ONE}"$'\r' 'Repeat:' "${PASSWORD_ONE}"$'\r' \
        'Press Enter' $'\r' 'Password:' "${PASSWORD_ONE}"$'\r' -- "${HELPER}" create 2>&1)"
    rc=$?
    # The recovery key, as gocryptfs showed it once: eight groups of eight hex digits
    KEY="$(tr -d ' \n' <<<"$(sed -n '/master key is/,/If the/p' <<<"${out}")" | grep -oE '[0-9a-f]{8}(-[0-9a-f]{8}){7}' | head -n1)"
    if [[ "${rc}" -eq 0 && -n "${KEY}" ]]; then ok "ame security private-folder create, with its recovery key shown"; else
        not_ok "the private folder was not made (exit ${rc})" "$(tail -n 25 <<<"${out}")"; fi
    printf '%s\n' "${KEY}" >"${STATE}/key"
    check "it is open, as gocryptfs's mount on ~/Private" sh -c "findmnt -rn -M '${PRIVATE}' -o FSTYPE | grep -qx fuse.gocryptfs"
    same "its watcher runs" "$(watcher)" active
    check "Files' sidebar has it" grep -qx "file://${PRIVATE}" "${HOME_TEST}/.config/gtk-3.0/bookmarks"
    same "the report has the private folder" "$(record private-folder | jq -r .state)" on
    same "the helper says it is open" "$(runuser -u "${TEST_USER}" -- env HOME="${HOME_TEST}" XDG_RUNTIME_DIR="/run/user/${UID_TEST}" "${HELPER}" status --json | jq -r .state)" open
    # A picture, with the thumbnail GNOME's thumbnailer makes of it, and a letter among the recent files
    as_user "cp /usr/share/pixmaps/fedora-logo-sprite.png '${PRIVATE}/picture.png' 2>/dev/null || cp \"\$(find /usr/share/icons/hicolor -name '*.png' | head -n1)\" '${PRIVATE}/picture.png'; echo dear > '${PRIVATE}/letter.txt'; mkdir '${PRIVATE}/folder'"
    in_session "python3 - '${PRIVATE}' <<'EOF'
import sys, gi
gi.require_version('GnomeDesktop', '4.0')
gi.require_version('Gtk', '4.0')
from gi.repository import Gio, GLib, GnomeDesktop, Gtk
folder = sys.argv[1]
factory = GnomeDesktop.DesktopThumbnailFactory.new(GnomeDesktop.DesktopThumbnailSize.NORMAL)
uri = Gio.File.new_for_path(folder + '/picture.png').get_uri()
mtime = int(Gio.File.new_for_uri(uri).query_info('time::modified', 0, None).get_modification_date_time().to_unix())
pixbuf = factory.generate_thumbnail(uri, 'image/png', None)
factory.save_thumbnail(pixbuf, uri, mtime, None)
GLib.set_prgname('vmlab-86')
GLib.set_application_name('vmlab-86')
Gtk.init()
Gtk.RecentManager.get_default().add_item(Gio.File.new_for_path(folder + '/letter.txt').get_uri())
loop = GLib.MainLoop()
GLib.timeout_add(1500, loop.quit)
loop.run()
EOF" >/dev/null 2>&1
    thumbs() { grep -rlF --binary-files=text "file://${PRIVATE}/" "${HOME_TEST}/.cache/thumbnails" 2>/dev/null; }
    recent() { grep -cF "file://${PRIVATE}/" "${HOME_TEST}/.local/share/recently-used.xbel" 2>/dev/null; }
    if [[ -n "$(thumbs)" ]]; then ok "a thumbnail of the picture in it is made"; else skip "no thumbnail of the picture was made"; fi
    if [[ "$(recent)" -gt 0 ]]; then ok "the letter is among the recent files"; else skip "the letter did not reach the recent files"; fi
    # Programs holding it: one with a file open, one with its folder in it
    in_session "cd '${PRIVATE}/folder' && exec sleep 900" >/dev/null 2>&1 &
    in_session "exec tail -f '${PRIVATE}/letter.txt'" >/dev/null 2>&1 &
    wait_for 30 sh -c "pgrep -u ${TEST_USER} -x tail >/dev/null && pgrep -u ${TEST_USER} -f 'sleep 900' >/dev/null"
    TAIL="$(pgrep -u "${TEST_USER}" -x tail | head -n1)"
    # Files on the folder and on another one: what it listed must not stay on the screen
    in_session "gio open '${PRIVATE}'" >/dev/null 2>&1
    check "Files shows the folder" wait_for 30 files_shows "file://${PRIVATE}"
    as_user "mkdir -p '${HOME_TEST}/Documents'"
    in_session "gio open '${HOME_TEST}/Documents'" >/dev/null 2>&1
    check "Files shows another folder" wait_for 30 files_shows "file://${HOME_TEST}/Documents"
    notify_watch
    began="$(date +%s%N)"
    loginctl lock-session "${SESSION}"
    if wait_for 30 closed; then
        ok "the screen locked closes it: no mount, no gocryptfs"
        echo "# closed $(( ($(date +%s%N) - began) / 1000000 )) ms after the lock was asked for"
    else
        not_ok "it stayed open with the screen locked" "$(findmnt "${PRIVATE}" 2>&1)" "$(journalctl -b --since -2min _SYSTEMD_USER_UNIT=amethystora-private-folder-watch.service --no-pager -o cat | tail -n 8)"
    fi
    check "its watcher has ended" wait_for 30 watcher_gone
    check "the screen locked closes Files' window on the folder" wait_for 30 files_left
    check "and keeps its other window" files_shows "file://${HOME_TEST}/Documents"
    check "a program with a file open in it can no longer read it" sh -c "! cat /proc/${TAIL:-1}/fd/3 >/dev/null 2>&1 && ! head -c1 '${PRIVATE}/letter.txt' >/dev/null 2>&1"
    if wait_for 30 notified "These programs had files open in it"; then
        out="$(grep -A2 -F "These programs had files open in it" /var/tmp/vmlab-86-notifications)"
        if [[ "${out}" == *tail* && "${out}" == *sleep* ]]; then ok "the notification names both programs"; else not_ok "the notification does not name tail and sleep" "${out}"; fi
    else
        not_ok "no notification named the programs" "$(tail -n 20 /var/tmp/vmlab-86-notifications)"
    fi
    same "no thumbnail of it is left" "$(thumbs)" ""
    same "no recent file of it is left" "$(recent)" 0
    same "closed, ~/Private takes nothing" "$(stat -c %a "${PRIVATE}")" 500
    check "nothing can be saved in it while closed" sh -c "! runuser -u ${TEST_USER} -- touch '${PRIVATE}/plain.txt' 2>/dev/null"
    pkill -u "${TEST_USER}" -x tail; pkill -u "${TEST_USER}" -f 'sleep 900'
    loginctl unlock-session "${SESSION}"
    wait_for 60 unlocked
    continue_phase
fi

# --- 6. The launcher, Close Now, a stopped or killed watcher, a suspend ------------------------------------------
if [[ "${SUITE_PHASE}" == 6 ]]; then
    atspi() { user_bus python3 /run/vmlab-86/atspi.py "$@"; }
    dialog() { atspi zenity has "$1"; }
    # The launcher's command, in the session's environment as the app grid starts it, with nothing on its input
    user_bus systemd-run --user --collect --quiet --unit=vmlab-86-launcher -- \
        $(sed -n '/^\[Desktop Entry\]/,/^\[Desktop Action/ s/^Exec=//p' /usr/share/applications/amethystora-private-folder.desktop)
    if wait_for 60 dialog "Open the private folder"; then
        ok "Private Folder in the app grid asks for its password"
        atspi zenity type "wrongpassword" >/dev/null 2>&1 || python3 /run/vmlab-86/keys.py text:wrongpassword
        atspi zenity click "OK" >/dev/null 2>&1 || python3 /run/vmlab-86/keys.py return
        if wait_for 60 dialog "Wrong password"; then
            ok "a wrong password says so"
            atspi zenity click "OK" >/dev/null 2>&1 || python3 /run/vmlab-86/keys.py return
        else
            not_ok "a wrong password said nothing" "$(atspi zenity text 2>&1 | head -n 10)"
        fi
        if wait_for 60 dialog "Open the private folder"; then
            atspi zenity type "${PASSWORD_ONE}" >/dev/null 2>&1 || python3 /run/vmlab-86/keys.py "text:${PASSWORD_ONE}"
            atspi zenity click "OK" >/dev/null 2>&1 || python3 /run/vmlab-86/keys.py return
        fi
        if wait_for 60 mounted; then ok "the right password opens it"; else not_ok "the right password did not open it" "$(journalctl -b --since -3min --no-pager -o cat | grep -iE 'gocryptfs|private' | tail -n 8)"; fi
        check "and shows it in Files" wait_for 60 pgrep -u "${TEST_USER}" -x nautilus
    else
        not_ok "the launcher asked for no password" "$(journalctl -b --since -2min --no-pager -o cat | tail -n 10)"
    fi
    same "its watcher runs" "$(watcher)" active
    # Close Now, the launcher's action, with a program holding a file in it
    in_session "exec tail -f '${PRIVATE}/letter.txt'" >/dev/null 2>&1 &
    wait_for 30 pgrep -u "${TEST_USER}" -x tail
    notify_watch
    in_session "$(sed -n '/^\[Desktop Action close\]/,$ s/^Exec=//p' /usr/share/applications/amethystora-private-folder.desktop)" >/dev/null 2>&1
    check "Close Now closes it" wait_for 30 closed
    check "and stops its watcher" wait_for 30 watcher_gone
    check "and Files no longer shows it" wait_for 30 files_left
    if wait_for 30 notified "These programs had files open in it"; then
        out="$(grep -A2 -F "These programs had files open in it" /var/tmp/vmlab-86-notifications)"
        if [[ "${out}" == *tail* ]]; then ok "Close Now names the program"; else not_ok "Close Now's notification does not name tail" "${out}"; fi
    else
        not_ok "Close Now named no program" "$(tail -n 20 /var/tmp/vmlab-86-notifications)"
    fi
    pkill -u "${TEST_USER}" -x tail
    pkill -u "${TEST_USER}" -x nautilus
    # However the watcher ends, the folder closes
    open_in_terminal() { session_terminal 120 'Password:' "${PASSWORD_ONE}"$'\r' -- "${HELPER}" open >/dev/null 2>&1; }
    for how in TERM KILL; do
        open_in_terminal
        if mounted && [[ "$(watcher)" == active ]]; then
            in_session "gio open '${PRIVATE}'" >/dev/null 2>&1
            wait_for 30 files_shows "file://${PRIVATE}"
            user_systemctl kill -s "${how}" amethystora-private-folder-watch.service
            check "the watcher killed with SIG${how} closes it" wait_for 30 closed
            check "and Files no longer shows it" wait_for 30 files_left
        else
            not_ok "it did not open in a terminal" "$(findmnt "${PRIVATE}" 2>&1)"
        fi
    done
    # A suspend without the screen locking: closed before the machine sleeps (the kernel's own test of a suspend,
    # to its devices, which wakes it after 5 seconds: the lab's VM has no sleep a timer could end)
    if grep -qw devices /sys/power/pm_test 2>/dev/null; then
        user_bus gsettings set org.gnome.desktop.screensaver lock-enabled false
        open_in_terminal
        if mounted; then
            in_session "gio open '${PRIVATE}'" >/dev/null 2>&1
            wait_for 30 files_shows "file://${PRIVATE}"
            before="$(date +%s)"
            echo devices >/sys/power/pm_test
            systemctl suspend
            wait_for 180 sh -c "journalctl -b --since @${before} --no-pager -o cat | grep -q 'PM: suspend exit'"
            echo none >/sys/power/pm_test
            check "the suspend closes Files' window on the folder" wait_for 30 files_left
            # When the account's manager stopped gocryptfs's scope, against the kernel's suspend
            closed_at="$(journalctl -b --since "@${before}" -o short-unix --no-pager USER_UNIT=amethystora-private-folder.scope |
                grep -iE 'stopped|deactivated' | head -n1 | cut -d' ' -f1)"
            slept_at="$(journalctl -b --since "@${before}" -k -o short-unix --no-pager | grep 'PM: suspend entry' | head -n1 | cut -d' ' -f1)"
            echo "# closed at ${closed_at:-?}, suspend entry at ${slept_at:-?}"
            if closed && [[ -n "${closed_at}" && -n "${slept_at}" ]] && awk -v a="${closed_at}" -v b="${slept_at}" 'BEGIN { exit !(a <= b) }'; then
                ok "a suspend without the screen locking closes it before the machine sleeps"
            else
                not_ok "the suspend did not close it first" "closed: ${closed_at:-?} sleep: ${slept_at:-?}" "$(findmnt "${PRIVATE}" 2>&1)"
            fi
        else
            not_ok "it did not open for the suspend"
        fi
        user_bus gsettings reset org.gnome.desktop.screensaver lock-enabled
    else
        skip "a suspend: this kernel has no test of its suspend (pm_test)"
    fi
    continue_phase
fi

# --- 7. A new password, the backup, logging out ------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 7 ]]; then
    KEY="$(cat "${STATE}/key")"
    out="$(session_terminal 120 'I know the password' $'\r' 'Old password:|Password:' "${PASSWORD_ONE}"$'\r' 'Password:' "${PASSWORD_TWO}"$'\r' \
        'Repeat:' "${PASSWORD_TWO}"$'\r' -- "${HELPER}" password 2>&1)"
    if [[ $? -eq 0 && ! -e "${CIPHER}/gocryptfs.conf.bak" ]]; then ok "a new password with the old one"; else not_ok "the password did not change with the old one" "$(tail -n 12 <<<"${out}")"; fi
    out="$(session_terminal 120 'I know the password' $'\e[B\r' '[Mm]aster ?key' "${KEY}"$'\r' 'Password:' "${PASSWORD_THREE}"$'\r' \
        'Repeat:' "${PASSWORD_THREE}"$'\r' 'once more' '' 'Password:' "${PASSWORD_THREE}"$'\r' -- "${HELPER}" password 2>&1)"
    if [[ $? -eq 0 ]]; then ok "a new password with the recovery key"; else not_ok "the password did not change with the recovery key" "$(tail -n 12 <<<"${out}")"; fi
    check "the file with the old password is gone once the new one opened it" test ! -e "${CIPHER}/gocryptfs.conf.bak"
    out="$(session_terminal 120 'Password:' "${PASSWORD_THREE}"$'\r' -- "${HELPER}" open 2>&1)"
    if mounted; then ok "the new password opens it"; else not_ok "the new password did not open it" "$(tail -n 8 <<<"${out}")"; fi
    as_user "echo kept > '${PRIVATE}/kept.txt'"
    # A backup while it is open holds the encrypted copy and nothing of the open folder
    CONF=/var/tmp/vmlab-86-backup
    rm -rf "${CONF}" /var/tmp/vmlab-86-repo
    install -d -o "${TEST_USER}" "${CONF}/amethystora" /var/tmp/vmlab-86-repo
    echo vmlab-86 >"${CONF}/password"
    printf 'RESTIC_REPOSITORY=/var/tmp/vmlab-86-repo\nRESTIC_PASSWORD_FILE=%s/password\n' "${CONF}" >"${CONF}/amethystora/backup.env"
    chown -R "${TEST_USER}" "${CONF}"
    as_user "RESTIC_PASSWORD_FILE=${CONF}/password restic -r /var/tmp/vmlab-86-repo init -q" >/dev/null 2>&1
    as_user "XDG_CONFIG_HOME=${CONF} /usr/libexec/amethystora-backup" >/dev/null 2>&1
    listed="$(as_user "RESTIC_PASSWORD_FILE=${CONF}/password restic -r /var/tmp/vmlab-86-repo ls latest --tag amethystora" 2>/dev/null)"
    if grep -qx "${CIPHER}/gocryptfs.conf" <<<"${listed}" && ! grep -q "^${PRIVATE}/" <<<"${listed}"; then
        ok "a backup holds the encrypted copy, and nothing of the open folder"
    else
        not_ok "the backup is not as it should be" "$(grep -E 'Private|amethystora/private' <<<"${listed}" | head -n 8)" "$(wc -l <<<"${listed}") entries"
    fi
    rm -rf /var/tmp/vmlab-86-repo "${CONF}"
    # Logging out closes it
    loginctl terminate-session "${SESSION}"
    if wait_for 60 closed; then ok "logging out closes it"; else not_ok "it stayed open after logging out" "$(findmnt "${PRIVATE}" 2>&1)"; fi
    reboot_and_continue
fi

# --- 8. Taking it away -------------------------------------------------------------------------------------------
if [[ "${SUITE_PHASE}" == 8 ]]; then
    out="$(session_terminal 120 'Password:' "${PASSWORD_THREE}"$'\r' -- "${HELPER}" remove 2>&1)"
    if [[ $? -ne 0 && -e "${CIPHER}/gocryptfs.conf" ]] && grep -q "not empty" <<<"${out}"; then
        ok "removing refuses a folder with a file in it"
    else
        not_ok "removing a folder with a file in it did not refuse" "$(tail -n 8 <<<"${out}")"
    fi
    check "and closes it again" wait_for 30 closed
    session_terminal 120 'Password:' "${PASSWORD_THREE}"$'\r' -- "${HELPER}" open >/dev/null 2>&1
    as_user "rm '${PRIVATE}/kept.txt' '${PRIVATE}/picture.png' '${PRIVATE}/letter.txt'; rmdir '${PRIVATE}/folder'" 2>/dev/null
    # Open already, so it asks for nothing but REMOVE
    out="$(session_terminal 120 'REMOVE to go on' $'REMOVE\r' -- "${HELPER}" remove 2>&1)"
    if [[ $? -eq 0 && ! -e "${CIPHER}" && ! -e "${PRIVATE}" ]]; then ok "an empty folder is taken away"; else not_ok "the empty folder was not taken away" "$(tail -n 8 <<<"${out}")"; fi
    check "and its place in Files' sidebar" sh -c "! grep -qs 'file://${PRIVATE}' '${HOME_TEST}/.config/gtk-3.0/bookmarks'"
    check "nothing of it is left mounted or running" closed
    same "the report has no private folder again" "$(record private-folder | jq -r .state)" off
fi
finish

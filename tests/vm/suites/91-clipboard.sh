#!/usr/bin/bash
# Amethystora Clipboard (plans/PLAN-clipboard.md): the clipboard history sealed under a key in the login keyring.
# Copied to as apps copy (wl-copy, and clipcopy.py's own window), read back as the menu shows it (atspi.py) and as
# the disk holds it (opened with the keyring's key). Three phases:
#   1. At login no keyring, then one made and unlocked with the account's password, as a password login does:
#      Clipboard Indicator's plain history and a setting of it taken over; what is copied, pictures too, sealed
#      and nowhere in the clear; a password manager's copy and an excluded app's not recorded; the history
#      outlasts the screen lock and a burst of copies. Then a login with the keyring locked, as a fingerprint
#      leaves it, stood in for by locking it over D-Bus and enabling the extension again: earlier items held back,
#      a new copy in memory only, the menu's row and GNOME's prompt unlocking it and merging the two; and once
#      more, unlocked elsewhere, which the extension notices by itself. The keyring is left with no password, as
#      on a machine that signs in by itself (one with a password left the lab's GDM login behind its greeter),
#      and keep-history set to restart, as Control does.
#   2. After a restart: only the pinned item from before. keep-history set to always.
#   3. After another restart: everything kept, and nothing the extension threw in the three boots.
# vmlab-paths: @sf/usr/share/gnome-shell/extensions/amethystora-clipboard@* apps/clipboard-seal/*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-clipboard@arsslensoft.github.io
SCHEMA=org.gnome.shell.extensions.amethystora-clipboard
HELPER=/usr/libexec/amethystora-clipboard-seal
HOME_DIR="/var/home/${TEST_USER}"
STORE="${HOME_DIR}/.local/state/amethystora/clipboard"
OLD="${HOME_DIR}/.cache/clipboard-indicator@tudmotu.com"
OWN=/run/vmlab-91
TESTS=/var/lib/vmlab-tests/91

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

install -d -m 0700 "${TESTS}"
if [[ "${SUITE_PHASE}" == 1 ]]; then
    printf 'RUN=%s\nSTART=%s\n' "$(date +%s)${RANDOM}" "$(date +%s)" >"${TESTS}/markers"
fi
# shellcheck source=/dev/null
source "${TESTS}/markers"
COPIED="vmlab-copied-${RUN}"
PINNED="vmlab-pinned-${RUN}"
WINDOW="vmlab-window-${RUN}"
X11="vmlab-x11-${RUN}"
GTK_HINT="vmlab-gtk-hint-${RUN}"
SECRET="vmlab-secret-${RUN}"
KEEPASS="vmlab-keepass-${RUN}"
LOCKED="vmlab-locked-${RUN}"
WATCHED="vmlab-watched-${RUN}"
ALWAYS="vmlab-always-${RUN}"
mapfile -t PNG < <(find /usr/share/icons/hicolor/48x48/apps -name '*.png' -size +1k | sort | head -n 2)

mkdir -p "${OWN}"
install -m 0644 "$(dirname "$0")/atspi.py" "$(dirname "$0")/clipcopy.py" "${OWN}/"
chmod -R a+rX "${OWN}"
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
shell() { user_bus python3 "${OWN}/atspi.py" gnome-shell "$@"; }
# wl_copy ARGS: wl-copy in the account's session, serving the clipboard until something else takes it
wl_copy() {
    user_bus systemd-run --user --quiet --collect -- wl-copy --foreground "$@"
    sleep 2
}
copy_picture() {
    user_bus systemd-run --user --quiet --collect -- bash -c 'exec wl-copy --foreground --type image/png <"$1"' _ "$1"
    sleep 2
}
# copy_window MODE TEXT: from clipcopy.py's window, which covers the screen and copies once it has the focus: GNOME
# does not always give a new window the focus, so a click on it gives it, as a person's would
copy_window() {
    user_bus systemd-run --user --quiet --collect --unit="vmlab-clipcopy-${RANDOM}" \
        --setenv=WAYLAND_DISPLAY=wayland-0 --setenv=DISPLAY=:0 -- python3 "${OWN}/clipcopy.py" "$@"
    sleep 3
    keys click:0.5,0.5
    sleep 7
}

ext_state() { user_bus gnome-extensions info "${UUID}" 2>/dev/null | sed -n 's/^ *State: *//p'; }
ext_active() { [[ "$(ext_state)" == ACTIVE ]]; }
# The extension disabled and enabled again, as at a new login: its module stays, its menu and store are new
ext_again() {
    user_bus gnome-extensions disable "${UUID}"
    sleep 2
    user_bus gnome-extensions enable "${UUID}"
    wait_for 30 ext_active
    sleep 3
}
# The default keyring, which the extension keeps its key in, and whether it is locked
keyring_path() {
    user_bus busctl --user call org.freedesktop.secrets /org/freedesktop/secrets org.freedesktop.Secret.Service \
        ReadAlias s default 2>/dev/null | awk '{ gsub(/"/, "", $2); print $2 }'
}
keyring_says() {
    local path
    path="$(keyring_path)"
    [[ -n "${path}" && "${path}" != / ]] || return 0
    user_bus busctl --user get-property org.freedesktop.secrets "${path}" org.freedesktop.Secret.Collection Locked 2>/dev/null
}
keyring_open() { [[ "$(keyring_says)" == "b false" ]]; }
keyring_locked() { [[ "$(keyring_says)" == "b true" ]]; }
lock_keyring() {
    user_bus busctl --user call org.freedesktop.secrets /org/freedesktop/secrets org.freedesktop.Secret.Service \
        Lock ao 1 "$(keyring_path)" >/dev/null
}
# keyring unlock PASSWORD: what pam_gnome_keyring does at a password login, the default keyring unlocked with the
# password, or the one PAM names login made with it; keyring blank PASSWORD: its password taken away, as on a
# machine that signs in by itself. Through gnome-keyring's own interface for both, which asks nothing
# (gnome-keyring-daemon --unlock does not reach the daemon a session already runs)
keyring() {
    user_bus python3 - "$@" <<'EOF'
import sys
from gi.repository import Gio, GLib

bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
SERVICE, INTERNAL = "/org/freedesktop/secrets", "org.gnome.keyring.InternalUnsupportedGuiltRiddenInterface"


def call(path, interface, method, args, reply=None):
    return bus.call_sync("org.freedesktop.secrets", path, interface, method, args,
                         GLib.VariantType(reply) if reply else None, Gio.DBusCallFlags.NONE, -1, None)


def exists(path):
    try:
        call(path, "org.freedesktop.DBus.Properties", "Get",
             GLib.Variant("(ss)", ("org.freedesktop.Secret.Collection", "Locked")))
        return True
    except GLib.Error:
        return False


mode, password = sys.argv[1], sys.argv[2]
session = call(SERVICE, "org.freedesktop.Secret.Service", "OpenSession",
               GLib.Variant("(sv)", ("plain", GLib.Variant("s", ""))), "(vo)").unpack()[1]
secret = lambda value: (session, b"", value.encode(), "text/plain")
keyring = call(SERVICE, "org.freedesktop.Secret.Service", "ReadAlias", GLib.Variant("(s)", ("default",)), "(o)").unpack()[0]
if keyring == "/":
    keyring = "/org/freedesktop/secrets/collection/login"
if mode == "blank":
    call(SERVICE, INTERNAL, "ChangeWithMasterPassword",
         GLib.Variant("(o(oayays)(oayays))", (keyring, secret(password), secret(""))))
elif exists(keyring):
    call(SERVICE, INTERNAL, "UnlockWithMasterPassword", GLib.Variant("(o(oayays))", (keyring, secret(password))))
else:
    keyring = call(SERVICE, INTERNAL, "CreateWithMasterPassword",
                   GLib.Variant("(a{sv}(oayays))", ({"org.freedesktop.Secret.Collection.Label": GLib.Variant("s", "login")},
                                                    secret(password))), "(o)").unpack()[0]
call(SERVICE, "org.freedesktop.Secret.Service", "SetAlias", GLib.Variant("(so)", ("default", keyring)))
EOF
}

# The sealed history opened with the key the keyring holds (only while it is open: asking a locked one would
# bring up GNOME's prompt), as JSON
history() {
    local key
    keyring_open && [[ -f "${STORE}/history" ]] || return 1
    key="$(user_bus secret-tool lookup purpose history-key 2>/dev/null)"
    [[ -n "${key}" ]] || return 1
    { printf '%s\n' "${key}"; cat "${STORE}/history"; } | "${HELPER}" open history
}
in_history() { history | jq -e --arg t "$1" '.entries | any(.contents == $t)' >/dev/null; }
not_in_history() { history >/dev/null && ! in_history "$1"; }
pinned() { history | jq -e --arg t "$1" '.entries | any(.contents == $t and .favorite)' >/dev/null; }
pictures() { history | jq '[.entries[] | select(.mimetype | startswith("image/"))] | length'; }
pictures_at_least() { [[ "$(pictures)" -ge "$1" ]]; }
unpinned() { history | jq '[.entries[] | select(.favorite | not)] | length'; }
settled() { [[ "$(unpinned)" =~ ^[0-9]+$ && "$(unpinned)" -le 15 ]]; }
# Whether any file in the home holds TEXT as it was copied
in_the_clear() { grep -rlaF -- "$1" "${HOME_DIR}" 2>/dev/null | grep -q .; }
nowhere_in_the_clear() { ! in_the_clear "$1"; }
# Whether any file in the home is a copy of the picture FILE
picture_in_the_clear() {
    find "${HOME_DIR}" -type f -size "$(stat -c %s "$1")c" -exec cmp -s "$1" {} \; -print 2>/dev/null | grep -q .
}
# menu_shows TEXT: the history opened with its key, TEXT waited for in it, and closed again
menu_shows() {
    local found=1
    keys super+shift+v
    shell wait 15 "$1" >/dev/null && found=0
    keys esc
    # The menu fades out with animations on, and holds the keyboard meanwhile: a copy made then (wl-copy takes the
    # clipboard only once a surface of its own has the keyboard) would never happen
    sleep 1
    return "${found}"
}
menu_lacks() {
    local lacks=1
    keys super+shift+v
    sleep 3
    shell has "$1" >/dev/null || lacks=0
    keys esc
    sleep 1
    return "${lacks}"
}
prompt_up() { shell has "did not get unlocked" >/dev/null || shell has "it is locked" >/dev/null; }
session_id() { loginctl show-user "${TEST_USER}" -p Display --value; }
screen_locked() { [[ "$(loginctl show-session "$(session_id)" -p LockedHint --value 2>/dev/null)" == yes ]]; }
screen_unlocked() { ! screen_locked; }
keep() { user_bus gsettings get "${SCHEMA}" keep-history 2>/dev/null; }
said() { user_bus journalctl --user -b --no-pager -o cat 2>/dev/null | grep -F "Amethystora Clipboard" | tail -n 8; }

# A suite before this one may have left the screen locked, and GNOME Shell disables every extension on the lock
# screen; or GNOME Shell crashed in it, after which GNOME turns every extension off (this suite checks for crashes
# of its own at the end)
screen_locked && loginctl unlock-session "$(session_id)" && wait_for 30 screen_unlocked
if [[ "$(user_bus gsettings get org.gnome.shell disable-user-extensions 2>/dev/null)" == true ]]; then
    echo "# a suite before this one left every extension off (GNOME Shell crashed): turned back on"
    user_bus gsettings set org.gnome.shell disable-user-extensions false
fi
# GNOME's welcome dialog of a first login, which 10-account.sh closes, takes every key and click while it is open:
# no copy reaches the clipboard
if shell has "Welcome to" >/dev/null; then
    echo "# GNOME's welcome dialog was still open: closed with Escape"
    keys esc
    sleep 2
fi
# GNOME's overview, where a session starts, takes keys for its search: a click on its workspace leaves it
keys click:0.5,0.5

case "${SUITE_PHASE}" in
1)
    check "Clipboard Indicator is not in the image" test ! -e /usr/share/gnome-shell/extensions/clipboard-indicator@tudmotu.com
    wait_for 30 ext_active && ok "Amethystora Clipboard is on in the session" ||
        not_ok "Amethystora Clipboard is not on in the session" "$(user_bus gnome-extensions info "${UUID}" 2>&1)" "$(said)"

    # Signed in by GDM, the account has no login keyring yet: nothing is kept, and the menu says so
    if [[ -z "$(keyring_says)" ]]; then
        check "with no keyring, the menu says nothing is saved" menu_shows "Not saved"
    else
        skip "the account already had a login keyring: $(keyring_says)"
    fi

    # What Clipboard Indicator left: its plain history with a pinned item and a picture, and a setting
    install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.cache" "${OLD}"
    install -o "${TEST_USER}" -g "${TEST_USER}" -m 0644 "${PNG[0]}" "${OLD}/0123abcd"
    printf '[{"favorite":true,"mimetype":"text/plain;charset=utf-8","contents":"%s"},{"favorite":false,"mimetype":"image/png","contents":"%s/0123abcd"}]' \
        "${PINNED}" "${OLD}" >"${OLD}/registry.txt"
    chown "${TEST_USER}:" "${OLD}/registry.txt"
    user_bus dconf write /org/gnome/shell/extensions/clipboard-indicator/history-size 30
    versions="${HOME_DIR}/.local/share/amethystora/setup_versioning.json"
    if [[ -f "${versions}" ]]; then
        jq 'del(.version.user."clipboard-indicator")' "${versions}" >"${versions}.vmlab" && cat "${versions}.vmlab" >"${versions}"
        rm -f "${versions}.vmlab"
    fi
    user_bus bash /usr/share/amethystora/user-setup.hooks.d/21-clipboard-indicator.sh >/dev/null 2>&1
    if [[ "$(user_bus gsettings get "${SCHEMA}" history-size)" == 30 &&
        -z "$(user_bus dconf dump /org/gnome/shell/extensions/clipboard-indicator/)" ]]; then
        ok "Clipboard Indicator's setting is carried over and its own taken away (21-clipboard-indicator.sh)"
    else
        not_ok "Clipboard Indicator's setting was not carried over" "history-size: $(user_bus gsettings get "${SCHEMA}" history-size)" \
            "left: $(user_bus dconf dump /org/gnome/shell/extensions/clipboard-indicator/)"
    fi
    user_bus gsettings reset "${SCHEMA}" history-size
    # The extension started again over what was left, as at the login after the update: a suite before this one in
    # the same session may have unlocked the keyring, and the history kept from before is only merged once, when it
    # first opens
    ext_again

    # A keyring now, made and unlocked with the password: the menu, opened, asks for it again
    out="$(keyring unlock "${TEST_USER}" 2>&1)"
    wait_for 20 keyring_open && ok "the login keyring is unlocked with the account's password" ||
        not_ok "the login keyring was not unlocked" "${out}" "default: $(keyring_path) $(keyring_says)"
    check "once there is a keyring, the menu shows Clipboard Indicator's pinned item" menu_shows "${PINNED}"
    wait_for 30 in_history "${PINNED}" && ok "the history is sealed under a key the keyring keeps" ||
        not_ok "the history was not sealed" "$(ls -la "${STORE}" 2>&1)" "$(said)"
    check "Clipboard Indicator's item stays pinned" pinned "${PINNED}"
    check "and its picture is kept, sealed" wait_for 20 pictures_at_least 1
    check "its plain history and pictures are gone" wait_for 30 test ! -e "${OLD}"

    wl_copy -- "${COPIED}"
    check "a text copied is in the sealed history" wait_for 20 in_history "${COPIED}"
    copy_picture "${PNG[1]}"
    check "a picture copied is in it too" wait_for 20 pictures_at_least 2
    check "the store is the account's alone (0700), its files 0600" sh -c \
        "[[ \$(stat -c %a '${STORE}') == 700 ]] && ! find '${STORE}' -type f ! -perm 0600 | grep -q ."
    for text in "${COPIED}" "${PINNED}"; do
        in_the_clear "${text}" && not_ok "${text} is in the clear in: $(grep -rlaF -- "${text}" "${HOME_DIR}" | head -n 3)" ||
            ok "${text%-*} is nowhere in the home in the clear"
    done
    for picture in "${PNG[@]}"; do
        picture_in_the_clear "${picture}" && not_ok "the picture ${picture##*/} is in the home as it was copied" ||
            ok "the picture ${picture##*/} is nowhere in the home as it was copied"
    done

    # A window's copy is recorded; a password manager's marked secret, and KeePassXC's own, are not. The Manual's
    # welcome, open since the first login, closed first: GNOME gave it the focus back from the new window
    pkill -u "${TEST_USER}" -f '^/usr/lib/amethystora-manual/' && sleep 2
    copy_window text "${WINDOW}"
    if wait_for 20 in_history "${WINDOW}"; then
        ok "a window's copy is recorded (clipcopy.py works here)"
        # The password manager's hint goes through X11 (clipcopy.py says why): an X11 window's copy first
        copy_window x11 "${X11}"
        if wait_for 20 in_history "${X11}"; then
            ok "an X11 window's copy is recorded (through Xwayland)"
            copy_window secret "${SECRET}"
            sleep 3
            check "a copy marked x-kde-passwordManagerHint: secret is not recorded" not_in_history "${SECRET}"
            check "nor written anywhere" nowhere_in_the_clear "${SECRET}"
        else
            not_ok "an X11 window's copy was not recorded, so the password manager's hint cannot be checked" \
                "$(user_bus journalctl --user -b --since "-60s" --no-pager -o cat 2>/dev/null | grep -v MESA | tail -n 10)"
        fi
        # A GTK app that offers the hint sends it empty, which counts as secret
        copy_window gtk-hint "${GTK_HINT}"
        sleep 3
        check "a copy whose x-kde-passwordManagerHint comes empty (a GTK app's) is not recorded" not_in_history "${GTK_HINT}"
        copy_window keepass "${KEEPASS}"
        sleep 3
        check "a copy made in KeePassXC's window is not recorded (excluded-apps)" not_in_history "${KEEPASS}"
    else
        not_ok "clipcopy.py's window did not get its copy recorded, so the password manager checks cannot run" \
            "$(user_bus journalctl --user -b --since "-60s" --no-pager -o cat 2>/dev/null | tail -n 10)"
    fi

    # The screen lock disables the extension and unlocking enables it: the history is read back, sealed
    wl_copy -- "${COPIED}"
    loginctl lock-session "$(session_id)"
    if wait_for 30 screen_locked; then
        sleep 3
        keys esc
        sleep 2
        keys "text:${TEST_USER}" return
        if wait_for 30 screen_unlocked; then
            keys click:0.5,0.5
            check "after the screen lock the menu still shows what was copied before it" menu_shows "${COPIED}"
        else
            not_ok "the screen did not unlock with the password"
            loginctl unlock-session "$(session_id)"
        fi
    else
        not_ok "the screen did not lock"
    fi

    # A hundred copies in a burst: the shell lives, and the history settles at its size
    shell_pid="$(pgrep -u "${TEST_USER}" -x gnome-shell)"
    for i in $(seq 1 100); do
        user_bus systemd-run --user --quiet --collect -- wl-copy --foreground -- "vmlab-burst-${i}-${RUN}"
        sleep 0.2
    done
    check "GNOME Shell outlives a hundred copies in a burst" [ "$(pgrep -u "${TEST_USER}" -x gnome-shell)" == "${shell_pid}" ]
    wait_for 60 settled && ok "the sealed history settles at 15 items not pinned" ||
        not_ok "the sealed history holds $(unpinned) items not pinned a minute after the burst"
    wl_copy -- "${COPIED}"

    # A login with the keyring locked, as after a fingerprint: earlier items held back, a new copy in memory only
    lock_keyring
    ext_again
    check "with the keyring locked, the menu offers to unlock the earlier items" menu_shows "Unlock to see earlier items"
    check "and does not show them" menu_lacks "${PINNED}"
    written="$(stat -c %Y "${STORE}/history")"
    wl_copy -- "${LOCKED}"
    check "a copy made while locked is in the menu" menu_shows "${LOCKED}"
    check "and on no disk: the sealed history is untouched" [ "$(stat -c %Y "${STORE}/history")" == "${written}" ]
    check "the copy made while locked is nowhere in the clear" nowhere_in_the_clear "${LOCKED}"
    # The row clicked where it is, with the pointer as a person's (its label has no action, and atspi.py's own
    # click, through the screen's top left corner, closed the menu first), and GNOME's own prompt for the keyring
    # answered with the password
    keys super+shift+v
    sleep 2
    clicked="$(shell where "Unlock to see earlier items" 2>&1)" && keys "click:${clicked}"
    # GNOME's own dialog, its words gnome-keyring's ("The login keyring did not get unlocked when you logged into
    # your computer", or for a keyring of another name "... but it is locked"): not its Cancel button, which a
    # "Copied to clipboard" notification has too
    if wait_for 15 prompt_up; then
        ok "the row brings up GNOME's prompt for the keyring"
        # Typed once the dialog has taken the keyboard, at the end of its opening, as a person would
        sleep 2
        keys "text:${TEST_USER}" return
        if ! wait_for 10 keyring_open && prompt_up; then
            echo "# the prompt was still open 10 s after the password: typed again"
            keys "text:${TEST_USER}" return
        fi
        if wait_for 30 keyring_open; then
            ok "the row's prompt unlocks the keyring"
        else
            not_ok "the row's prompt did not unlock the keyring" "$(shell text 2>&1 | head -n 30)" \
                "$(user_bus journalctl --user -b --since "-60s" --no-pager -o cat 2>/dev/null | grep -v MESA | tail -n 10)"
            keys esc
            keyring unlock "${TEST_USER}" >/dev/null 2>&1
        fi
    else
        not_ok "the row brought up no prompt" "click: ${clicked}" "$(shell text 2>&1 | head -n 30)" "$(said)"
        keys esc
        keyring unlock "${TEST_USER}" >/dev/null 2>&1
    fi
    wait_for 30 in_history "${LOCKED}" && ok "the history is written again, with the copy made while locked" ||
        not_ok "the copy made while locked was not written" "$(said)"
    check "and what was kept before" pinned "${PINNED}"
    check "the menu shows the two together" menu_shows "${PINNED}"

    # Locked again, then unlocked elsewhere: the extension notices by itself, with no menu opened
    lock_keyring
    ext_again
    wl_copy -- "${WATCHED}"
    keyring unlock "${TEST_USER}" >/dev/null 2>&1
    wait_for 30 in_history "${WATCHED}" && ok "unlocked elsewhere, the extension writes what was copied meanwhile by itself" ||
        not_ok "the extension did not notice the keyring unlocked" "$(said)"

    # A machine that signs in by itself has a keyring with no password; and how long it is kept, as Control sets it.
    # oo7 (Fedora 45) refuses a keyring with no password, so there it keeps the account's, which each restart below
    # unlocks it with, as a password login does
    install -d -m 0700 /var/lib/vmlab-tests
    out="$(keyring blank "${TEST_USER}" 2>&1)"
    if [[ -z "${out}" ]]; then
        ok "the keyring has no password now, for the restarts"
        : >/var/lib/vmlab-tests/91-keyring
    elif grep -q "Password too short" <<<"${out}"; then
        ok "oo7 keeps the keyring's password, which each restart unlocks it with"
        printf '%s' "${TEST_USER}" >/var/lib/vmlab-tests/91-keyring
    else
        not_ok "the keyring kept its password" "${out}"
    fi
    user_bus bash -lc 'ame desktop clipboard restart' >/dev/null
    check "ame desktop clipboard restart sets keep-history" [ "$(keep)" == "'restart'" ]
    reboot_and_continue
    ;;
2)
    check "Amethystora Clipboard is on after the restart" wait_for 30 ext_active
    # GDM's automatic login leaves even a keyring with no password locked: unlocked here, as a password login
    # does, and the extension, watching it, opens the history by itself
    keyring unlock "$(cat /var/lib/vmlab-tests/91-keyring 2>/dev/null)" >/dev/null 2>&1
    check "the keyring opens, with no password or the account's" wait_for 30 keyring_open
    check "keep-history restart kept the pinned item" wait_for 30 pinned "${PINNED}"
    check "and dropped what was not pinned before the restart" wait_for 30 not_in_history "${COPIED}"
    check "the menu shows the pinned item" menu_shows "${PINNED}"
    user_bus bash -lc 'ame desktop clipboard always' >/dev/null
    check "ame desktop clipboard always sets keep-history" [ "$(keep)" == "'always'" ]
    wl_copy -- "${ALWAYS}"
    check "a copy is sealed at once" wait_for 20 in_history "${ALWAYS}"
    reboot_and_continue
    ;;
3)
    check "Amethystora Clipboard is on after the second restart" wait_for 30 ext_active
    keyring unlock "$(cat /var/lib/vmlab-tests/91-keyring 2>/dev/null)" >/dev/null 2>&1
    check "the keyring opens" wait_for 30 keyring_open
    check "under always, what was copied before the restart is kept" wait_for 30 in_history "${ALWAYS}"
    check "and the pinned item" pinned "${PINNED}"
    check "the menu shows it" menu_shows "${ALWAYS}"

    uid="$(id -u "${TEST_USER}")"
    errors="$(journalctl _UID="${uid}" --since "@${START}" --no-pager -o cat 2>/dev/null |
        grep -E "JS ERROR|had error|Error:" | grep -iF -e "${UUID}" -e "Amethystora Clipboard" | sort -u | head -n 20)"
    [[ -z "${errors}" ]] && ok "the extension threw nothing in three boots" || not_ok "the extension threw" "${errors}"
    crashes="$(coredumpctl list --since "@${START}" --no-legend --no-pager 2>/dev/null | grep -w gnome-shell)"
    [[ -z "${crashes}" ]] && ok "GNOME Shell did not crash" || not_ok "GNOME Shell crashed" "${crashes}"

    user_bus gsettings reset "${SCHEMA}" keep-history
    ;;
esac
finish

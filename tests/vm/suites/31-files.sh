#!/usr/bin/bash
# Files' scripts menu (plans/beyond/spec-phone-files-first.md, section 9, suite 31, the Files part): Folder colour and
# Back in time as user-setup.hooks.d/24-files-menu.sh writes them, run as Files runs a script (the folder shown as
# the current folder, the selection as arguments), on names Files allows and a shell would not, the coloured folder in
# an open Files window, the icon packs' coloured folders, and a script of the person's own that the hook leaves alone.
# vmlab-paths: @ui/modules/pill/moving.js @sf/usr/libexec/amethystora-folder-colour @sf/usr/libexec/amethystora-back-in-time @sf/usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh @sf/usr/share/icons/*/places/* @sf/usr/libexec/amethystora-open-anything @sf/usr/share/applications/zz-amethystora-open-anything.desktop build_files/shared/open-anything-types.py @ui/modules/tools/opener.js @sf/usr/libexec/amethystora-flatpak-ask @ui/modules/pill/stick* @sf/usr/libexec/amethystora-pdf-sign
source "$(dirname "$0")/lib.sh"

HOME_DIR="/var/home/${TEST_USER}"
SCRIPTS="${HOME_DIR}/.local/share/nautilus/scripts"
WORK=/run/vmlab-31
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        sed -E -e "s/^\(true, '(.*)'\)\$/\1/" -e 's/^"(.*)"$/\1/'
}
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -m 0755 "${WORK}"
TEST="${HOME_DIR}/vmlab-31"
rm -rf "${TEST}"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${TEST}"
cleanup() {
    rm -rf "${TEST}"
    user_bus pkill -f "nautilus --new-window ${TEST}" 2>/dev/null
}
trap cleanup EXIT

# --- 24: the entries the account's first login wrote -------------------------------------------------------------------
# The lab's account logs in for the first time on this image, so user-setup has run the hook for it (in English)
FOLDER="${SCRIPTS}/Folder colour"
entries=0
for name in Amethyst Sky Jade Amber Coral Slate "No colour"; do
    [[ -x "${FOLDER}/${name}" && "$(sed -n 2p "${FOLDER}/${name}")" == "# amethystora-files-menu 1."* ]] && entries=$((entries + 1))
done
same "24 Folder colour holds its seven entries, each executable and marked" "${entries}" 7
check "24 Back in time is there, executable and marked" \
    bash -c "[[ -x '${SCRIPTS}/Back in time' && \"\$(sed -n 2p '${SCRIPTS}/Back in time')\" == '# amethystora-files-menu 1.'* ]]"
same "24 files-menu.json lists the eleven entries (Earlier versions, Copy without hidden data and What is using this? among them) and the folder" \
    "$(jq -r '.version, (.paths | length)' "${HOME_DIR}/.local/state/amethystora/files-menu.json" 2>&1 | paste -sd ' ')" "1 12"
same "24 an entry passes Files' arguments after --" "$(sed -n 3p "${FOLDER}/Jade")" \
    'exec /usr/libexec/amethystora-folder-colour jade -- "$@"'

# --- 25-28: the colours, run as Files runs a script ------------------------------------------------------------------------
# A script run as the account, in its session (the metadata store is gvfs's, on its bus), from a folder, with
# arguments that reach it whole: the names are written into a file and read back as an array, never through a shell
cat >"${WORK}/as-files.sh" <<'EOF'
#!/usr/bin/bash
# as-files.sh FOLDER SCRIPT NAMES-FILE: SCRIPT run from FOLDER with the NUL-separated names of NAMES-FILE as arguments
cd "$1" || exit 1
mapfile -d '' -t names <"$3"
"$2" "${names[@]}"
EOF
chmod 0755 "${WORK}/as-files.sh"
run_as_files() { # SCRIPT NAME...: the names relative to ${TEST}
    local list="${WORK}/names"
    printf '%s\0' "${@:2}" >"${list}"
    chmod 0644 "${list}"
    user_bus "${WORK}/as-files.sh" "${TEST}" "$1" "${list}"
}
icon_of() { # The folder's colour as gio reads it: the last line of that name, which is the attribute's
    user_bus gio info -n -a metadata::custom-icon-name -- "$1" 2>/dev/null | sed -n 's/^  metadata::custom-icon-name: //p' | tail -n 1
}
picture_of() { user_bus gio info -n -a metadata::custom-icon -- "$1" 2>/dev/null | sed -n 's/^  metadata::custom-icon: //p' | tail -n 1; }

runuser -u "${TEST_USER}" -- mkdir "${TEST}/Projects"
before="$(stat -c %.9Y "${TEST}/Projects")"
run_as_files "${FOLDER}/Jade" Projects
same "25 Jade on Projects" "$(icon_of "${TEST}/Projects")" folder-jade
same "25 no custom-icon" "$(picture_of "${TEST}/Projects")" ""
same "25 its modification time is the same" "$(stat -c %.9Y "${TEST}/Projects")" "${before}"

ODD=$'-x\ny\xff'
runuser -u "${TEST_USER}" -- mkdir -- "${TEST}/${ODD}"
runuser -u "${TEST_USER}" -- touch -- "${TEST}/${ODD}/inside"
listing_before="$(ls -lab --time-style=+%s.%N "${TEST}" "${TEST}/${ODD}" | grep -v ' \.$\| \.\.$')"
run_as_files "${FOLDER}/Jade" "${ODD}"
same "26 a folder named with a dash, a newline and a byte that is not UTF-8 is coloured" "$(icon_of "${TEST}/${ODD}")" folder-jade
same "26 ...and nothing else in the folder changes" \
    "$(ls -lab --time-style=+%s.%N "${TEST}" "${TEST}/${ODD}" | grep -v ' \.$\| \.\.$')" "${listing_before}"

runuser -u "${TEST_USER}" -- mkdir "${TEST}/Mixed"
runuser -u "${TEST_USER}" -- touch "${TEST}/one.txt" "${TEST}/two.txt"
MONITOR="${WORK}/notify.log"
# GNOME's notification service owns org.freedesktop.Notifications and passes each Notify on to the shell with
# x-shell-sender added; dbus-broker addresses both by unique name, so a destination match keeps both: the second counts
user_bus timeout 15 dbus-monitor --session "type='method_call',interface='org.freedesktop.Notifications',member='Notify'" >"${MONITOR}" 2>&1 &
sleep 2
run_as_files "${FOLDER}/Coral" Mixed one.txt two.txt
sleep 2
same "27 the folder is coloured" "$(icon_of "${TEST}/Mixed")" folder-coral
same "27 the files are not" "$(icon_of "${TEST}/one.txt")$(icon_of "${TEST}/two.txt")" ""
said="$(awk '/^method call/ { n += body && shell; body = shell = 0 } /2 items were left as they were/ { body = 1 }
    /string "x-shell-sender"/ { shell = 1 } END { print n + (body && shell) }' "${MONITOR}")"
same "27 one notification says 2 items were left as they were" "${said}" 1
if grep -q 'one.txt\|two.txt' "${MONITOR}"; then not_ok "27 the notification names a file"; else ok "27 ...and names no file"; fi

run_as_files "${FOLDER}/No colour" Projects
same "28 No colour takes Jade away" "$(icon_of "${TEST}/Projects")" ""
runuser -u "${TEST_USER}" -- mkdir "${TEST}/Pictured"
user_bus gio set -n -t string -- "${TEST}/Pictured" metadata::custom-icon "file:///usr/share/icons/hicolor/scalable/apps/amethystora-notes.svg"
run_as_files "${FOLDER}/No colour" Pictured
same "28 ...and leaves a picture the person set" "$(picture_of "${TEST}/Pictured")" \
    "file:///usr/share/icons/hicolor/scalable/apps/amethystora-notes.svg"
user_bus gio set -n -t string -- "${TEST}/Projects" metadata::custom-icon-name my-own-icon
run_as_files "${FOLDER}/No colour" Projects
same "28 ...and an icon name that is not a colour of ours" "$(icon_of "${TEST}/Projects")" my-own-icon
user_bus gio set -n -t unset -- "${TEST}/Projects" metadata::custom-icon-name

# --- 29: an open Files window shows the colour at once ----------------------------------------------------------------------
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
    wait_for 240 has_session
    sleep 10
fi
# A session that locked meanwhile (GNOME's idle lock, an earlier suite) shows the lock screen where the window is
loginctl unlock-sessions
unlocked() { [[ "$(ev "Main.screenShield?.locked ?? false")" == false ]]; }
wait_for 10 unlocked || not_ok "29 the session stays locked"
VIEW="${TEST}/view"
runuser -u "${TEST_USER}" -- mkdir -p "${VIEW}/Projects"
# Plain to start with: an earlier run left the colour it gave (the folder and its metadata stay in the home)
user_bus gio set -t unset -- "${VIEW}/Projects" metadata::custom-icon-name 2>/dev/null || true
user_bus systemd-run --user --collect --unit="vmlab-files31-$$" nautilus --new-window "${VIEW}" >/dev/null
files_window() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus' && a.meta_window.get_title().includes('view'))")" == true ]]; }
if wait_for 20 files_window; then
    # A session GNOME just started opens on the overview, where the window's place on the screen shows something else
    ev "Main.overview.hide(); 'hidden'" >/dev/null
    sleep 3
    ev "global.get_window_actors().find(a => a.meta_window.get_wm_class() === 'org.gnome.Nautilus' && a.meta_window.get_title().includes('view')).meta_window.activate(global.get_current_time()); 'up'" >/dev/null
    sleep 2
    # The window's own picture, as the screen shows it, before and after
    shot() { # FILE
        local rect
        rect="$(ev "(() => { const w = global.get_window_actors().find(a => a.meta_window.get_title().includes('view')).meta_window.get_frame_rect(); return [w.x, w.y, w.width, w.height].join(' '); })()")"
        # shellcheck disable=SC2086
        user_bus gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
            --method org.gnome.Shell.Screenshot.ScreenshotArea ${rect} false "$1" >/dev/null 2>&1
    }
    pixels() { user_bus python3 -c 'import sys, hashlib, gi; gi.require_version("GdkPixbuf", "2.0"); from gi.repository import GdkPixbuf; p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1]); print(hashlib.sha256(p.get_pixels()).hexdigest())' "$1"; }
    shot /tmp/vmlab-31-a.png
    sleep 1
    shot /tmp/vmlab-31-b.png
    if [[ "$(pixels /tmp/vmlab-31-a.png)" != "$(pixels /tmp/vmlab-31-b.png)" ]]; then
        skip "29 the Files window changes by itself between two pictures, so a colour cannot be told from it"
    else
        printf '%s\0' Projects >"${WORK}/names"
        user_bus "${WORK}/as-files.sh" "${VIEW}" "${FOLDER}/Amber" "${WORK}/names"
        changed() { shot /tmp/vmlab-31-c.png && [[ "$(pixels /tmp/vmlab-31-c.png)" != "$(pixels /tmp/vmlab-31-a.png)" ]]; }
        deadline=$((SECONDS + 2))
        until changed || ((SECONDS >= deadline)); do sleep 0.3; done
        if changed; then ok "29 the open Files window shows the colour within 2 s"; else not_ok "29 the open Files window still shows the plain folder"; fi
    fi
    user_bus systemctl --user stop "vmlab-files31-$$" 2>/dev/null
else
    not_ok "29 no Files window opened"
fi

# --- 30: every pack draws the six, and GTK finds the pack's own -------------------------------------------------------------
cat >"${WORK}/folders.py" <<'EOF'
import sys
import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk
Gtk.init_check()
bad = []
for pack in ('glass', 'glass-light', 'neon', 'neon-light', 'duotone', 'line', 'clay', 'muted'):
    theme = Gtk.IconTheme()
    theme.set_search_path(['/usr/share/icons'])
    theme.set_theme_name('amethystora-' + pack)
    for colour in ('amethyst', 'sky', 'jade', 'amber', 'coral', 'slate'):
        icon = theme.lookup_icon('folder-' + colour, None, 48, 1, Gtk.TextDirection.NONE, 0)
        path = (icon.get_file().get_path() or '') if icon and icon.get_file() else ''
        if path != f'/usr/share/icons/amethystora-{pack}/places/scalable/folder-{colour}.svg':
            bad.append(f'{pack}: folder-{colour} is {path or "nothing"}')
print('\n'.join(bad))
sys.exit(1 if bad else 0)
EOF
chmod 0644 "${WORK}/folders.py"
out="$(user_bus python3 "${WORK}/folders.py" 2>&1)"
if [[ $? == 0 ]]; then ok "30 GTK finds each pack's own six coloured folders"; else not_ok "30 a pack lacks a coloured folder" "${out}"; fi

# --- 31: Back in time --------------------------------------------------------------------------------------------------------
backups_window() { [[ "$(ev "global.get_window_actors().some(a => a.meta_window.get_wm_class() === 'org.amethystora.Backups' && a.meta_window.get_title().length > 0)")" == true ]]; }
# No Backups left by an earlier suite, which would be handed this folder and leave the started one to exit
BACKUPS='^/usr/lib/amethystora-backups/amethystora-backups( |$)'
pkill -u "${TEST_USER}" -f "${BACKUPS}" 2>/dev/null
sleep 1
run_as_files "${SCRIPTS}/Back in time" Pictured &
BACK=$!
if wait_for 30 backups_window; then
    ok "31 Back in time opens Backups"
    # The one Backups running, and what it was started with, one argument a line
    pids="$(pgrep -u "${TEST_USER}" -f "${BACKUPS}")"
    started="$(tr '\0' '\n' <"/proc/${pids%%[!0-9]*}/cmdline" 2>/dev/null)"
    if [[ "$(wc -l <<<"${pids}")" == 1 && "$(tail -n 1 <<<"${started}")" == "${TEST}/Pictured" ]]; then
        ok "31 ...on that folder"
    else
        not_ok "31 Backups was not given the folder" "${pids}" "${started}"
    fi
    ev "global.get_window_actors().filter(a => a.meta_window.get_wm_class() === 'org.amethystora.Backups').forEach(a => a.meta_window.delete(global.get_current_time())); 'closed'" >/dev/null
else
    not_ok "31 no Backups window" "$(pgrep -u "${TEST_USER}" -af amethystora-backups)"
fi
kill "${BACK}" 2>/dev/null
user_bus timeout 10 dbus-monitor --session "type='method_call',interface='org.freedesktop.Notifications',member='Notify'" >"${MONITOR}" 2>&1 &
sleep 2
printf '%s\0' /tmp >"${WORK}/names"
user_bus "${WORK}/as-files.sh" /tmp "${SCRIPTS}/Back in time" "${WORK}/names"
sleep 2
check "31 outside the home folder it says so" grep -q 'Back in time covers your home folder.' "${MONITOR}"
sleep 1
if backups_window; then not_ok "31 ...but Backups opened"; else ok "31 ...and opens nothing"; fi

# --- 32: a script of the person's own is left alone ----------------------------------------------------------------------------
FRESH="${WORK}/fresh"
rm -rf "${FRESH}"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${FRESH}" "${FRESH}/.local/share/nautilus/scripts"
printf '#!/bin/sh\necho mine\n' >"${FRESH}/.local/share/nautilus/scripts/Back in time"
chown "${TEST_USER}:" "${FRESH}/.local/share/nautilus/scripts/Back in time"
user_bus env HOME="${FRESH}" XDG_DATA_HOME="${FRESH}/.local/share" XDG_STATE_HOME="${FRESH}/.local/state" LANG=C.UTF-8 \
    bash /usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh >/dev/null 2>&1
same "32 the person's own Back in time is as it was" "$(cat "${FRESH}/.local/share/nautilus/scripts/Back in time")" "$(printf '#!/bin/sh\necho mine')"
if jq -e '.paths | map(endswith("/Back in time")) | any' "${FRESH}/.local/state/amethystora/files-menu.json" >/dev/null 2>&1; then
    not_ok "32 the hook lists it as its own"
else
    ok "32 ...and the hook did not write that entry, but wrote the colours"
fi
same "32 the colours are written beside it" "$(ls "${FRESH}/.local/share/nautilus/scripts/Folder colour" 2>/dev/null | wc -l)" 7
rm -rf "${FRESH}"
finish

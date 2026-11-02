#!/usr/bin/bash
# Every other app in the app grid opens in the test account's session and stays open, as it does from the grid:
# each launcher started as GNOME Shell starts it (activated on the session bus, or with gio when it is not), given
# a few seconds, then closed. None may crash, none may be refused what it
# needs (a Flatpak whose `flatpak-spawn --host` the image's global override denies says so on its output, as
# Mission Center's gatherer did), and opening them may make the security watcher say nothing: with the image's
# own settings, using the image's own apps is not news. Then a restart, after which a boot and a login make it
# say nothing either: loading its own audit rules, Fedora's sssd and tuned, and a program of the login once did.
# Amethystora's own apps are 20-apps.sh's.
# vmlab-paths: @sf/usr/share/flatpak/* @sf/etc/flatpak/* @sf/usr/libexec/amethystora-security-watch @sf/usr/share/applications/zz-amethystora-open-anything.desktop build_files/shared/open-anything-types.py @sf/usr/libexec/amethystora-uninstall
source "$(dirname "$0")/lib.sh"

EVENTS=/var/lib/amethystora/security/events.json
BOOTED=/var/lib/vmlab-tests/25-every-app.booted
if [[ -e "${BOOTED}" ]]; then
    rm -f "${BOOTED}"
    BOOT="$(date -d "$(uptime -s)" +%s)"
    if ! wait_for 300 has_session; then
        not_ok "no session for ${TEST_USER} after the restart"
        finish
    fi
    # What the login starts, the phone link among it if it is on, given time to start
    sleep 90
    systemctl start amethystora-security-watch.service
    said="$(jq -r --argjson boot "${BOOT}" '.[] | select(.time >= $boot and .level != "quiet") | "\(.level) \(.key): \(.text)"' \
        "${EVENTS}" 2>/dev/null)"
    if [[ -z "${said}" ]]; then
        ok "a restart and a login made the security watcher say nothing"
    else
        not_ok "a restart and a login made the security watcher say something" "${said}"
    fi
    finish
fi

if ! has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
# The default Flatpaks install on the first boots with a network, as in 40-setup.sh
preinstalled() { [[ "$(systemctl show flatpak-preinstall.service -p SubState -p Result --value | paste -sd ' ')" == "exited success" ]]; }
if ! wait_for 600 preinstalled; then
    if ((SUITE_PHASE < 4)); then
        echo "# the default Flatpaks were still installing after 10 minutes: on with them after a restart"
        reboot_and_continue
    fi
    not_ok "flatpak-preinstall.service did not finish" "$(systemctl status flatpak-preinstall.service --no-pager 2>&1 | tail -n 8)"
    finish
fi

UID_TESTER="$(id -u "${TEST_USER}")"
HOME_DIR="$(getent passwd "${TEST_USER}" | cut -d: -f6)"
# What the watcher had to say before any app opened is not the apps'
systemctl start amethystora-security-watch.service
START="$(date +%s)"
SINCE="$(date '+%F %T')"

# The launchers the app grid shows, a line each: file, the Flatpak it runs or "-", whether it is started over
# D-Bus, its Exec line
launchers() {
    local dir file
    for dir in /usr/share/applications /var/lib/flatpak/exports/share/applications \
        "${HOME_DIR}/.local/share/flatpak/exports/share/applications" "${HOME_DIR}/.local/share/applications"; do
        for file in "${dir}"/*.desktop; do
            [[ -f "${file}" && "${file##*/}" != amethystora-* ]] || continue
            awk -v file="${file}" -F= '
                /^\[/ { group = $0; next }
                group != "[Desktop Entry]" { next }
                $1 == "Type" { type = $2 } $1 == "NoDisplay" { hidden = hidden || $2 == "true" }
                $1 == "Hidden" { hidden = hidden || $2 == "true" } $1 == "OnlyShowIn" { only = $2 }
                $1 == "NotShowIn" { notin = $2 } $1 == "X-Flatpak" { flatpak = $2 }
                $1 == "DBusActivatable" { dbus = $2 == "true" }
                $1 == "Exec" && exec == "" { exec = substr($0, 6) }
                END {
                    if (type != "Application" || hidden || (only != "" && only !~ /GNOME/) || notin ~ /GNOME/) exit
                    print file "\t" (flatpak == "" ? "-" : flatpak) "\t" (dbus ? "dbus" : "-") "\t" exec
                }' "${file}"
        done
    done
}

# The program a launcher that is not a Flatpak runs, by its name: the first word of its Exec line that is not
# env or a variable, and a web page opened with xdg-open is the browser's
program_of() {
    local word
    for word in $1; do
        [[ "${word}" == env || "${word}" == *=* ]] && continue
        word="${word##*/}"
        [[ "${word}" == xdg-open ]] && word=firefox
        echo "${word}"
        return
    done
}
# Its processes, oldest first: by the command line, since a Python program's name is its interpreter's
pids_of() { pgrep -u "${TEST_USER}" -f "(^|/)${1}( |$)" 2>/dev/null; }
open() {
    if [[ "$1" != - ]]; then
        user_bus flatpak ps --columns=application 2>/dev/null | grep -qxF -- "$1"
    else
        [[ -n "$(pids_of "$2")" ]]
    fi
}
closed() { ! open "$1" "$2"; }
close() {
    if [[ "$1" != - ]]; then
        user_bus flatpak kill "$1" >/dev/null 2>&1
    else
        # The main process, as quitting does: the oldest of them
        pkill -TERM -u "${TEST_USER}" -o -f "(^|/)${2}( |$)" 2>/dev/null
    fi
    wait_for 15 closed "$1" "$2" || { [[ "$1" != - ]] || pkill -KILL -u "${TEST_USER}" -f "(^|/)${2}( |$)" 2>/dev/null; }
}

count=0
while IFS=$'\t' read -r file flatpak dbus exec; do
    name="$(sed -n 's/^Name=//p' "${file}" | head -n1)"
    program="-"
    [[ "${flatpak}" != - ]] || program="$(program_of "${exec}")"
    count=$((count + 1))
    from="$(date '+%F %T')"
    if [[ "${dbus}" == dbus ]]; then
        # As GNOME Shell starts it: activated on the session bus, waiting for the answer (gio launch sends the
        # call and exits, and the call was lost now and then)
        id="${file##*/}"
        id="${id%.desktop}"
        object="/${id//./\/}"
        started="$(user_bus gdbus call --session --timeout 90 --dest "${id}" --object-path "${object//-/_}" \
            --method org.freedesktop.Application.Activate '{}' 2>&1)"
    else
        started="$(user_bus systemd-run --user --unit="test-grid-${count}" --collect --quiet -p KillMode=process -- \
            gio launch "${file}" 2>&1)"
    fi
    # Open within 40 seconds, and still open 10 seconds later: a window drawn in software is slow to come
    if wait_for 40 open "${flatpak}" "${program}" && sleep 10 && open "${flatpak}" "${program}"; then
        ok "${name} opens and stays open"
    else
        not_ok "${name} (${file##*/}) did not stay open" "${started}" \
            "$(journalctl -b _UID="${UID_TESTER}" --since "${from}" --no-pager -o cat | grep -v '^$' | tail -n 15)"
    fi
    # Nothing it needs was taken from it: a display, and the Flatpak service when its own manifest asks for it
    # (Smile tries `flatpak-spawn --host` without, and gets on without it)
    needs='cannot open display|Failed to open display'
    if [[ "${flatpak}" != - ]] && flatpak info --show-metadata "${flatpak}" 2>/dev/null | grep -qxF org.freedesktop.Flatpak=talk; then
        needs+='|only works when the Flatpak is allowed'
    fi
    refused="$(journalctl -b _UID="${UID_TESTER}" --since "${from}" --no-pager -o cat --grep "${needs}" 2>/dev/null | sort -u)"
    [[ -z "${refused}" ]] || not_ok "${name} was refused what it needs" "${refused}"
    # Mission Center reads the machine through its gatherer, run outside its sandbox
    if [[ "${flatpak}" == io.missioncenter.MissionCenter ]]; then
        if wait_for 20 pgrep -u "${TEST_USER}" -f /files/bin/missioncenter-magpie; then
            ok "Mission Center's gatherer runs outside its sandbox"
        else
            not_ok "Mission Center's gatherer does not run" "$(journalctl -b _UID="${UID_TESTER}" --since "${from}" --no-pager -o cat | tail -n 8)"
        fi
    fi
    close "${flatpak}" "${program}"
done < <(launchers | sort)
echo "# ${count} launchers opened in $(( $(date +%s) - START )) s"
[[ "${count}" -gt 20 ]] || not_ok "the app grid has only ${count} launchers besides Amethystora's own"

dumps="$(crashes_since "${SINCE}" COREDUMP_UID="${UID_TESTER}")"
if [[ -z "${dumps}" ]]; then ok "no app crashed"; else not_ok "an app crashed" "${dumps}"; fi

# What the watcher says after the apps opened, beyond what it only records
systemctl start amethystora-security-watch.service
said="$(jq -r --argjson start "${START}" '.[] | select(.time >= $start and .level != "quiet") | "\(.level) \(.key): \(.text)"' \
    "${EVENTS}" 2>/dev/null)"
if [[ -z "${said}" ]]; then
    ok "opening the apps made the security watcher say nothing"
else
    not_ok "opening the apps made the security watcher say something" "${said}"
fi
mkdir -p "${BOOTED%/*}"
touch "${BOOTED}"
reboot_and_continue

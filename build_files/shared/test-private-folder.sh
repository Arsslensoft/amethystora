#!/usr/bin/bash
# The private folder (/usr/libexec/amethystora-private-folder, or the copy named first) against stand-ins for
# gocryptfs, systemd-run, systemctl, zenity and findmnt, for 20-tests.sh, since a build has no FUSE: opening
# refuses a mount point with plain files in it, starts the watcher before it mounts and stops it when the
# mount fails, and the password reaches no argument and no file; closing takes away the thumbnails and the
# recent files of what was in the folder and nothing else, writes the rest of the list back as it was, names the
# programs that had a file open in it, and closes the Files tabs that show it (a stand-in for Files over busctl,
# whose tab row need not be the order it publishes) and no other; removing refuses a folder with something in it. It
# runs as nobody, since the helper refuses root.

set -euo pipefail

HELPER="${1:-/usr/libexec/amethystora-private-folder}"
T="$(mktemp -d)"
trap 'rm -rf "${T}"' EXIT
fail() { echo "test-private-folder: $*" >&2; exit 1; }
mkdir -p "${T}/bin" "${T}/home" "${T}/state"
HOME_DIR="${T}/home"
PLAIN="${HOME_DIR}/Private"
CIPHER="${HOME_DIR}/.local/share/amethystora/private"
LOG="${T}/state/log"
: >"${LOG}"
# The password the dialog gives, kept outside the home
printf 'wrong one\n' >"${T}/answer"

# Every stand-in logs how it was called. gocryptfs opens the folder for one password only; the mount and the
# two units are files in ${T}/state.
cat >"${T}/bin/stub" <<'EOF'
#!/usr/bin/bash
echo "${0##*/} $*" >>"${LOG}"
case "${0##*/}" in
    systemctl)
        [[ "$1" == --user ]] && shift
        unit="${@: -1}"
        case "$1" in
            start) touch "${STATE}/${unit}" ;;
            stop) rm -f "${STATE}/${unit}"; [[ "${unit}" == *.scope ]] && rm -f "${STATE}/mounted" ;;
            is-active) [[ -e "${STATE}/${unit}" ]]; exit ;;
        esac
        ;;
    systemd-run)
        while [[ "$1" != -- ]]; do shift; done
        shift
        touch "${STATE}/amethystora-private-folder.scope"
        "$@" || { rc=$?; rm -f "${STATE}/amethystora-private-folder.scope"; exit "${rc}"; }
        ;;
    gocryptfs)
        read -r password
        [[ "${password}" == right-password ]] || exit 12
        touch "${STATE}/mounted"
        ;;
    zenity)
        [[ "$1" == --password ]] && cat "${ANSWER}"
        ;;
    findmnt) [[ -e "${STATE}/mounted" ]]; exit ;;
    fusermount3) rm -f "${STATE}/mounted" ;;
    busctl)
        # Files: a window's tabs in the order they were made, its row of tabs, the tab in front and the one last closed
        [[ "$*" == *--auto-start=no* ]] || { echo "busctl may start Files: $*" >>"${STATE}/started"; exit 1; }
        [[ -e "${STATE}/files.json" ]] || exit 1
        while [[ "$1" == --* ]]; do shift; done
        case "$3" in
            /org/freedesktop/FileManager1)
                jq -c '{type: "v", data: [{type: "a{sas}", data: map_values(.tabs)}]}' "${STATE}/files.json" ;;
            *)
                case "$7" in
                    go-to-tab) change='.[$w].front = .[$w].row[$n]' ;;
                    close-current-view) change='.[$w].closed = .[$w].front | .[$w].tabs -= [.[$w].front] | .[$w].row -= [.[$w].front] |
                        if .[$w].tabs == [] then del(.[$w]) else .[$w].front = .[$w].row[0] end' ;;
                    restore-tab) change='.[$w].tabs += [.[$w].closed] | .[$w].row += [.[$w].closed]' ;;
                esac
                jq -c --arg w "$3" --argjson n "${10:-0}" "${change}" "${STATE}/files.json" >"${STATE}/files.new"
                mv "${STATE}/files.new" "${STATE}/files.json" ;;
        esac
        ;;
esac
exit 0
EOF
chmod +x "${T}/bin/stub"
for name in systemctl systemd-run gocryptfs zenity findmnt fusermount3 notify-send gio busctl; do
    ln -s stub "${T}/bin/${name}"
done
chown -R 65534:65534 "${T}/home" "${T}/state"
chmod 0755 "${T}"
run() {
    setpriv --reuid=65534 --regid=65534 --clear-groups env HOME="${HOME_DIR}" PATH="${T}/bin:${PATH}" \
        LOG="${LOG}" STATE="${T}/state" ANSWER="${T}/answer" "$@"
}
# On a terminal of its own, for what asks questions
on_terminal() {
    run python3 -I -c 'import os, pty, sys; sys.exit(os.waitstatus_to_exitcode(pty.spawn(sys.argv[1:])))' "$@" </dev/null
}
as_nobody() { run bash -c "$1"; }

[[ "$(run "${HELPER}" status --json | jq -r .state)" == none ]] || fail "no folder is not none"
as_nobody "mkdir -p '${CIPHER}' '${PLAIN}' && touch '${CIPHER}/gocryptfs.conf' && chmod 0500 '${PLAIN}'"
[[ "$(run "${HELPER}" status --json | jq -r .state)" == closed ]] || fail "a folder not mounted is not closed"

# Plain files in the mount point while it is closed are not encrypted, and opening says so and stops there
as_nobody "chmod 0700 '${PLAIN}' && touch '${PLAIN}/loose.txt' && chmod 0500 '${PLAIN}'"
out="$(run "${HELPER}" open --show 2>&1 </dev/null)" && fail "opened over a file left in the mount point"
grep -q "loose.txt" "${LOG}" || [[ "${out}" == *loose.txt* ]] || fail "the file left in the mount point was not named"
grep -qE "^(gocryptfs|systemctl --user start)" "${LOG}" && fail "it went on to open over a file left in the mount point"
as_nobody "chmod 0700 '${PLAIN}' && rm '${PLAIN}/loose.txt' && chmod 0500 '${PLAIN}'"

# The wrong password, five times: the watcher started before the first try and stopped after the last
: >"${LOG}"
run "${HELPER}" open --show </dev/null >/dev/null 2>&1 && fail "opened with the wrong password"
grep -nE "^(systemctl --user start|systemd-run|systemctl --user stop amethystora-private-folder-watch)" "${LOG}" >"${T}/order"
head -n1 "${T}/order" | grep -q "start amethystora-private-folder-watch.service" || fail "the watcher did not start first"
[[ "$(grep -c "^gocryptfs" "${LOG}")" == 5 ]] || fail "the wrong password was not asked again, four times"
tail -n1 "${T}/order" | grep -q "stop amethystora-private-folder-watch.service" || fail "the watcher stayed after a failed open"
[[ "$(stat -c %a "${PLAIN}")" == 500 ]] || fail "the mount point stayed writable after a failed open"

# The right one opens it and shows it, and is nowhere but where it came from
printf 'right-password\n' >"${T}/answer"
: >"${LOG}"
run "${HELPER}" open --show </dev/null || fail "did not open with the right password"
[[ "$(run "${HELPER}" status --json | jq -r .state)" == open ]] || fail "an open folder is not open"
grep -q "^gio open ${PLAIN}$" "${LOG}" || fail "the open folder was not shown in Files"
grep -q "^gocryptfs -q ${CIPHER} ${PLAIN}$" "${LOG}" || fail "gocryptfs was not given the folder"
grep -rqF right-password "${LOG}" "${HOME_DIR}" && fail "the password reached an argument or a file"

# What would show what was in it: thumbnails (the host's and a Flatpak app's, and a failed one) and recent files
uri="file://${PLAIN}"
thumb() {
    as_nobody "mkdir -p '$(dirname "$1")' && printf '\x89PNG\r\n\x1a\n....tEXtThumb::URI\0%s\0....' '$2' >'$1'"
}
thumb "${HOME_DIR}/.cache/thumbnails/large/private.png" "${uri}/holiday.jpg"
thumb "${HOME_DIR}/.cache/thumbnails/fail/gnome-thumbnail-factory/failed.png" "${uri}/broken.pdf"
thumb "${HOME_DIR}/.var/app/org.gnome.Loupe/cache/thumbnails/normal/app.png" "${uri}/sub/scan.png"
thumb "${HOME_DIR}/.cache/thumbnails/large/other.png" "file://${HOME_DIR}/Documents/report.png"
thumb "${HOME_DIR}/.cache/thumbnails/large/near.png" "file://${HOME_DIR}/Private-notes/a.png"
as_nobody "mkdir -p '${HOME_DIR}/.local/share' && cat >'${HOME_DIR}/.local/share/recently-used.xbel'" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0"
      xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
      xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info"
>
  <bookmark href="${uri}/letter.odt" added="2026-10-21T10:00:00.000000Z" modified="2026-10-21T10:00:00.000000Z" visited="2026-10-21T10:00:00.000000Z">
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="application/vnd.oasis.opendocument.text"/>
        <bookmark:applications>
          <bookmark:application name="LibreOffice" exec="&apos;soffice %u&apos;" modified="2026-10-21T10:00:00.000000Z" count="1"/>
        </bookmark:applications>
      </metadata>
    </info>
  </bookmark>
  <bookmark href="file://${HOME_DIR}/Documents/plan.txt" added="2026-10-21T11:00:00.000000Z" modified="2026-10-21T11:00:00.000000Z" visited="2026-10-21T11:00:00.000000Z">
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="text/plain"/>
        <bookmark:applications>
          <bookmark:application name="Text Editor" exec="&apos;gnome-text-editor %u&apos;" modified="2026-10-21T11:00:00.000000Z" count="2"/>
        </bookmark:applications>
      </metadata>
    </info>
  </bookmark>
</xbel>
EOF

# Files: a window on the folder, one on a folder in it, a tab in it beside another tab, the same with the tab moved by
# hand, a search, and a window elsewhere
files_window() { jq -c --arg w "/org/gnome/Nautilus/window/$1" --argjson tabs "$2" --argjson row "$3" '.[$w] = {tabs: $tabs, row: $row, front: $row[0]}'; }
docs="file://${HOME_DIR}/Documents"
echo '{}' | files_window 1 "[\"${uri}\"]" "[\"${uri}\"]" | files_window 2 "[\"${uri}/sub\"]" "[\"${uri}/sub\"]" |
    files_window 3 "[\"${uri}\",\"${docs}\"]" "[\"${uri}\",\"${docs}\"]" |
    files_window 4 "[\"${uri}/a\",\"${docs}/b\",\"${uri}/c\"]" "[\"${docs}/b\",\"${uri}/a\",\"${uri}/c\"]" |
    files_window 5 '["x-nautilus-search://0/"]' '["x-nautilus-search://0/"]' |
    files_window 6 "[\"${docs}\",\"file://${HOME_DIR}/Private-notes\"]" "[\"${docs}\",\"file://${HOME_DIR}/Private-notes\"]" >"${T}/state/files.json"
chown 65534:65534 "${T}/state/files.json"

# A program with its working folder in it, which closing names
run bash -c "cd '${PLAIN}' && exec sleep 60" &
holder=$!
for _ in $(seq 50); do
    [[ "$(readlink "/proc/${holder}/cwd" 2>/dev/null)" == "${PLAIN}" ]] && break
    sleep 0.1
done
: >"${LOG}"
run "${HELPER}" close </dev/null || fail "did not close"
kill "${holder}" 2>/dev/null || true
grep -q "^systemctl --user stop amethystora-private-folder-watch.service$" "${LOG}" || fail "closing by hand left the watcher"
grep -q "^systemctl --user stop amethystora-private-folder.scope$" "${LOG}" || fail "gocryptfs's scope was not stopped"
[[ "$(run "${HELPER}" status --json | jq -r .state)" == closed ]] || fail "a closed folder is not closed"
[[ "$(stat -c %a "${PLAIN}")" == 500 ]] || fail "the closed mount point takes files"
grep -q "^notify-send .*sleep" "${LOG}" || fail "the program that had its working folder in it was not named"
left="$(jq -cS 'map_values(.tabs)' "${T}/state/files.json")"
[[ "${left}" == "$(jq -ncS --arg docs "${docs}" --arg notes "file://${HOME_DIR}/Private-notes" '{"/org/gnome/Nautilus/window/3": [$docs], "/org/gnome/Nautilus/window/4": [$docs + "/b"], "/org/gnome/Nautilus/window/6": [$docs, $notes]}')" ]] ||
    fail "the Files tabs left are not what they should be: ${left}"
[[ ! -e "${T}/state/started" ]] || fail "$(cat "${T}/state/started")"
# A closing with nothing opened leaves a search alone
echo '{"/org/gnome/Nautilus/window/5":{"tabs":["x-nautilus-search://0/"],"row":["x-nautilus-search://0/"],"front":"x-nautilus-search://0/"}}' >"${T}/state/files.json"
chown 65534:65534 "${T}/state/files.json"
run "${HELPER}" close </dev/null || fail "did not close again"
[[ "$(jq -r 'keys | length' "${T}/state/files.json")" == 1 ]] || fail "a closing with nothing opened took a search away"
rm "${T}/state/files.json"
for gone in .cache/thumbnails/large/private.png .cache/thumbnails/fail/gnome-thumbnail-factory/failed.png \
    .var/app/org.gnome.Loupe/cache/thumbnails/normal/app.png; do
    [[ ! -e "${HOME_DIR}/${gone}" ]] || fail "a thumbnail of the folder stayed: ${gone}"
done
for kept in .cache/thumbnails/large/other.png .cache/thumbnails/large/near.png; do
    [[ -e "${HOME_DIR}/${kept}" ]] || fail "a thumbnail of something else went: ${kept}"
done
XBEL="${HOME_DIR}/.local/share/recently-used.xbel"
grep -qF "${uri}/" "${XBEL}" && fail "a recent file in the folder stayed"
grep -qF "href=\"file://${HOME_DIR}/Documents/plan.txt\"" "${XBEL}" || fail "a recent file elsewhere went"
grep -q '<bookmark:application name="Text Editor"' "${XBEL}" || fail "the recent files were not written back as they were"
grep -q '<mime:mime-type type="text/plain"' "${XBEL}" || fail "the recent files lost their mime: prefix"

# As the watcher's ExecStopPost (systemd sets SERVICE_RESULT), closing does not stop the watcher it runs in
: >"${LOG}"
run env SERVICE_RESULT=success "${HELPER}" close </dev/null
grep -q "stop amethystora-private-folder-watch" "${LOG}" && fail "the watcher's own ExecStopPost stops the watcher"

# Nothing in it is ever deleted: removing an open folder with a file in it refuses
run "${HELPER}" open </dev/null >/dev/null || fail "did not open again"
as_nobody "touch '${PLAIN}/keep.txt'"
on_terminal "${HELPER}" remove >/dev/null 2>&1 && fail "removed a folder with a file in it"
[[ -e "${CIPHER}/gocryptfs.conf" && -e "${PLAIN}/keep.txt" ]] || fail "removing a folder with a file in it deleted something"
echo "test-private-folder: opening, closing and removing as the private folder should"

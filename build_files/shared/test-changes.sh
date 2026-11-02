#!/usr/bin/bash
# What changed (R7-32): /usr/libexec/amethystora-changes against stand-ins for what it reads, for 20-tests.sh. The list is
# worked out from what exists and nothing is kept; Undo adds back and never deletes or overwrites; an id it did not list is
# refused; a security line only opens Security; and the feature the report is silent on is never named.
#   bash test-changes.sh [HELPER]

set -uo pipefail

HELPER="${1:-/usr/libexec/amethystora-changes}"
ROOT="$(realpath "$(mktemp -d)")"
trap 'rm -rf "${ROOT}"' EXIT
export AMETHYSTORA_CHANGES_ROOT="${ROOT}"
export HOME="${ROOT}/var/home/me"
export XDG_CONFIG_HOME="${HOME}/.config" XDG_DATA_HOME="${HOME}/.local/share" XDG_STATE_HOME="${HOME}/.local/state"
export XDG_RUNTIME_DIR="${ROOT}/run"
export LOG="${ROOT}/log"
TRASH="${XDG_DATA_HOME}/Trash"
SNAP="${ROOT}/var/home/.snapshots"
NOW="$(date +%s)"
mkdir -p "${HOME}/Documents" "${TRASH}/info" "${TRASH}/files" "${XDG_CONFIG_HOME}/dconf" "${XDG_CONFIG_HOME}/amethystora/current" \
    "${XDG_STATE_HOME}/amethystora/agent-tools" "${XDG_RUNTIME_DIR}" "${ROOT}/bin" "${ROOT}/usr/libexec" "${ROOT}/usr/bin" \
    "${ROOT}/var/lib/amethystora/security" "${SNAP}/2026-10-01_1000/me/.config/dconf" "${SNAP}/2026-10-01_1000/me/.config/amethystora/current" \
    "${SNAP}/2026-09-28_1000/me/.config/dconf" "${SNAP}/2026-09-28_1000/me/.config/amethystora/current"
: >"${LOG}"
failed=0
fail() {
    echo "test-changes: $*" >&2
    failed=1
}

# dconf reads the file its folder holds, and records what it is told to write
cat >"${ROOT}/bin/dconf" <<'EOF'
#!/usr/bin/bash
case "$1" in
    dump) [[ -n "${DCONF_PROFILE:-}" && -f "${DCONF_PROFILE}" ]] && cat "${XDG_CONFIG_HOME}/dconf/dump" 2>/dev/null ;;
    write)
        echo "dconf $*" >>"${LOG}"
        # What was written is what the next dump says
        sed -i "s|^${2##*/}=.*|${2##*/}=$3|" "${XDG_CONFIG_HOME}/dconf/dump"
        ;;
    reset) echo "dconf $*" >>"${LOG}" ;;
esac
EOF
cat >"${ROOT}/bin/flatpak" <<'EOF'
#!/usr/bin/bash
case "$1" in
    history) printf 'Oct 30 09:00:00\tuninstall\tapp/org.inkscape.Inkscape/x86_64/stable\nOct 30 09:10:00\tdeploy install\tapp/org.gnome.Maps/x86_64/stable\nOct 30 09:20:00\tuninstall\truntime/org.gnome.Platform/x86_64/49\n' ;;
    list) echo org.gnome.Maps ;;
    install) echo "flatpak $*" >>"${LOG}" ;;
esac
EOF
cat >"${ROOT}/usr/libexec/amethystora-restore" <<EOF
#!/usr/bin/bash
echo "restore \$*" >>"\${LOG}"
case "\$1" in
    points) echo '{"version":1,"points":[{"source":"local","id":"2026-10-01_1000","time":$((NOW - 3600))},{"source":"local","id":"2026-09-28_1000","time":$((NOW - 4 * 86400))}],"backup":"none"}' ;;
    status) echo '{"version":1,"snapshots":{"on":true}}' ;;
    ls)
        if [[ "\$3" == Documents ]]; then
            echo '{"version":1,"entries":[{"name":"Taxes","type":"dir","now":"missing"},{"name":"Kept","type":"file","now":"same"},{"name":".hidden","type":"file","now":"missing"},{"name":"In Trash","type":"file","now":"missing"}]}'
        else
            echo '{"version":1,"entries":[]}'
        fi ;;
    put-back) echo '{"version":1,"added":1,"same":0,"kept":0,"replaced":0,"skipped":0,"missing":[]}' ;;
esac
EOF
# R7-41's helper: the apps' folders in the trash, and what brings an app back with its folder
cat >"${ROOT}/usr/libexec/amethystora-uninstall" <<EOF
#!/usr/bin/bash
case "\$1" in
    removed) echo '[{"id":"org.example.Trashed","removed":$((NOW - 60))},{"id":"org.gnome.Maps","removed":$((NOW - 70))},{"id":"not an id","removed":1}]' ;;
    undo)
        echo "uninstall \$*" >>"\${LOG}"
        [[ "\$2" == org.example.Trashed ]] && echo data-stays-in-trash
        ;;
esac
EOF
cat >"${ROOT}/usr/libexec/amethystora-deployments" <<EOF
#!/usr/bin/bash
echo '{"deployments":[{"booted":true,"version":"44.20261101","timestamp":$((NOW - 7200))},{"version":"44.20261025","timestamp":$((NOW - 800000))}]}'
EOF
cat >"${ROOT}/usr/libexec/amethystora-reset-settings" <<'EOF'
#!/usr/bin/bash
[[ "$1 $2" == "saved account" ]] && echo "${XDG_STATE_HOME}/amethystora/reset-settings/$(date '+%Y-%m-%dT%H-%M-%S')"
exit 0
EOF
cat >"${ROOT}/usr/bin/amethystora-agent" <<'EOF'
#!/usr/bin/bash
if [[ "$1" == status ]]; then
    echo "Agentic features: ${AGENT_STATE:-on} (Stand-in)"
    exit 0
fi
if [[ "$1 $2" == "tools status" ]]; then
    echo "Changes by the agent: ${TOOLS_STATE:-on}"
    [[ "${TOOLS_STATE:-on}" == on ]]
    exit
fi
echo "agent $*" >>"${LOG}"
EOF
cat >"${ROOT}/usr/bin/amethystora-theme" <<'EOF'
#!/usr/bin/bash
echo "theme $*" >>"${LOG}"
echo "$2" >"${XDG_CONFIG_HOME}/amethystora/current/theme.name"
EOF
chmod +x "${ROOT}"/bin/* "${ROOT}"/usr/libexec/* "${ROOT}"/usr/bin/*
export PATH="${ROOT}/bin:${PATH}"

# A file in the trash, by the specification
trash() {
    printf 'canary\n' >"${TRASH}/files/$1"
    printf '[Trash Info]\nPath=%s\nDeletionDate=%s\n' "$2" "$(date '+%Y-%m-%dT%H:%M:%S')" >"${TRASH}/info/$1.trashinfo"
}
trash 'report.odt' "${HOME// /%20}/Documents/My%20Report.odt"
trash 'second.odt' "${HOME}/Documents/Second.odt"
trash 'In Trash' "${HOME}/Documents/In%20Trash"
trash 'org.example.Trashed' "${HOME}/.var/app/org.example.Trashed"
printf '[Trash Info]\nPath=%s\n' "${HOME}/Documents/Nameless" >"${TRASH}/info/.trashinfo"
printf 'mine\n' >"${HOME}/Documents/Second.odt"

# A setting that changed since the snapshot; another that is no setting of the table
printf '[org/gnome/shell/extensions/amethystora-ui/dock]\nedge='"'"'left'"'"'\n\n[org/example]\nsecret='"'"'now'"'"'\n' >"${XDG_CONFIG_HOME}/dconf/dump"
printf '[org/gnome/shell/extensions/amethystora-ui/dock]\nedge='"'"'left'"'"'\n\n[org/example]\nsecret='"'"'then'"'"'\n' >"${SNAP}/2026-10-01_1000/me/.config/dconf/dump"
printf '[org/gnome/shell/extensions/amethystora-ui/dock]\nedge='"'"'bottom'"'"'\n\n[org/example]\nsecret='"'"'then'"'"'\n' >"${SNAP}/2026-09-28_1000/me/.config/dconf/dump"
echo 'Nord' >"${XDG_CONFIG_HOME}/amethystora/current/theme.name"
echo 'Nord' >"${SNAP}/2026-10-01_1000/me/.config/amethystora/current/theme.name"
echo 'Tokyo Night' >"${SNAP}/2026-09-28_1000/me/.config/amethystora/current/theme.name"
# The records the agent's tools write (apps/agent-tools/src/act.rs): an undoing is a record of its own, of the same tool, with
# the id of the change in `of` and a way back of its own
{
    echo "{\"id\":\"ab12cd34\",\"time\":$((NOW - 100)),\"tool\":\"change\",\"entry\":\"layout\",\"value\":\"tiling\",\"before\":\"classic\",\"result\":\"ok\",\"undo\":{\"tool\":\"change\",\"entry\":\"layout\",\"value\":\"classic\"}}"
    echo "{\"id\":\"ef567890\",\"time\":$((NOW - 90)),\"tool\":\"change\",\"entry\":\"style\",\"value\":\"neon\",\"before\":\"floating\",\"result\":\"ok\",\"undo\":{\"tool\":\"change\",\"entry\":\"style\",\"value\":\"floating\"}}"
    echo "{\"id\":\"11111111\",\"time\":$((NOW - 80)),\"tool\":\"change\",\"entry\":\"style\",\"value\":\"floating\",\"before\":\"neon\",\"of\":\"ef567890\",\"result\":\"ok\",\"undo\":{\"tool\":\"change\",\"entry\":\"style\",\"value\":\"neon\"}}"
    echo "{\"id\":\"22222222\",\"time\":$((NOW - 70)),\"tool\":\"software\",\"action\":\"install\",\"source\":\"flathub\",\"name\":\"org.example.Agent\",\"result\":\"ok\",\"undo\":{\"tool\":\"software\",\"action\":\"remove\",\"source\":\"flathub\",\"id\":\"org.example.Agent\"}}"
    echo "{\"id\":\"33333333\",\"time\":$((NOW - 60)),\"tool\":\"software\",\"action\":\"remove\",\"source\":\"flathub\",\"name\":\"org.example.Gone\",\"result\":\"ok\",\"undo\":{\"tool\":\"software\",\"action\":\"install\",\"source\":\"flathub\",\"id\":\"org.example.Gone\"}}"
} >"${XDG_STATE_HOME}/amethystora/agent-tools/actions.jsonl"

# Trash entries somebody wrote by hand: where a file is put back is the info file's word, and it is only trusted inside the home,
# away from the places that start programs
OUTSIDE="${ROOT}/outside"
mkdir -p "${OUTSIDE}" "${HOME}/.config/autostart"
ln -s "${OUTSIDE}" "${HOME}/linkout"
trash 'escape.txt' "../../../outside/escape.txt"
trash 'absolute.txt' "${OUTSIDE}/absolute.txt"
trash 'autostart.desktop' "${HOME}/.config/autostart/evil.desktop"
trash 'through-link.txt' "${HOME}/linkout/through-link.txt"
trash 'dotdot.txt' "${HOME}/Documents/../../../../outside/dotdot.txt"
trash 'bashrc' "${HOME}/.bashrc"
# A name that is not text, as the file manager writes it: percent-encoded bytes
printf 'canary\n' >"${TRASH}/files/$(printf 'caf\xe9.txt')"
printf '[Trash Info]\nPath=%s\nDeletionDate=%s\n' "${HOME}/Documents/caf%E9.txt" "$(date '+%Y-%m-%dT%H:%M:%S')" >"${TRASH}/info/$(printf 'caf\xe9.txt').trashinfo"
# An info file whose file is gone
printf '[Trash Info]\nPath=%s\nDeletionDate=%s\n' "${HOME}/Documents/Vanished.txt" "$(date '+%Y-%m-%dT%H:%M:%S')" >"${TRASH}/info/vanished.txt.trashinfo"
cat >"${ROOT}/var/lib/amethystora/security/events.json" <<EOF
[{"id":"1-0","time":$((NOW - 50)),"level":"quiet","key":"settings","text":"lockdown was turned off, of what the image enforces"},
 {"id":"1-1","time":$((NOW - 40)),"level":"quiet","key":"settings","text":"the duress finger changed"},
 {"id":"1-2","time":$((NOW - 30)),"level":"critical","key":"processes","text":"not a setting"}]
EOF

before="$(cd "${ROOT}" && find . -type f -not -path ./log -exec md5sum {} + | sort)"
LIST="$("${HELPER}" list --json)"
after="$(cd "${ROOT}" && find . -type f -not -path ./log -exec md5sum {} + | sort)"
[[ "${before}" == "${after}" ]] || fail "listing changed a file: $(diff <(echo "${before}") <(echo "${after}"))"
[[ -z "$(ls -A "${XDG_RUNTIME_DIR}")" ]] || fail "listing left something in the runtime folder"
find "${HOME}" "${XDG_STATE_HOME}" -name '*changes*' | grep -q . && fail "it keeps a record of its own"

has() { jq -e "$1" <<<"${LIST}" >/dev/null || fail "$2: ${LIST}"; }
has '.snapshots == true' "the snapshots are on"
has '.items | any(.id == "trash:report.odt.trashinfo" and .kind == "file" and (.what | contains("My Report.odt")) and .undo.terminal == false)' "a file in the trash"
has '.items | any(.id == "gone:2026-10-01_1000:Documents/Taxes" and .undo != null)' "a file gone from a usual folder"
has '.items | all(.id != "gone:2026-10-01_1000:Documents/.hidden" and .id != "gone:2026-10-01_1000:Documents/Kept" and .id != "gone:2026-10-01_1000:Documents/In Trash")' "what is hidden, still there or in the trash is not gone"
has '.items | any(.id == "setting:2026-09-28_1000:/org/gnome/shell/extensions/amethystora-ui/dock/edge" and .what == "The dock moved")' "a setting that changed"
has '.items | all(.id | contains("/org/example") | not)' "a setting outside the table"
has '.items | any(.id == "theme:2026-09-28_1000:Tokyo Night")' "the theme"
has '.items | any(.id == "app:org.inkscape.Inkscape" and .undo.terminal == true and (.what == "org.inkscape.Inkscape was removed" or .what == "Inkscape was removed"))' "an app removed, from Flatpak's history, which does not say who did it"
has '.items | any(.id == "app:org.example.Trashed" and (.what | startswith("You removed")))' "an app removed, from the account's own trash, was the account's"
has '.items | any(.id == "app:org.example.Trashed" and .undo.terminal == true)' "an app removed with its folder, from the trash"
has '.items | all(.id != "trash:org.example.Trashed.trashinfo")' "an app's folder in the trash is the app's line"
has '.items | all(.id != "app:org.gnome.Maps")' "an app that is installed again is not removed"
has '.items | any(.id == "app-new:org.gnome.Maps" and .undo == null and (.what == "Maps was installed"))' "an app installed has no Undo: Undo only adds back"
has '.items | all(.id | contains("runtime/") | not)' "a runtime is not an app"
has '.items | any(.kind == "update" and .undo.terminal == true)' "the system's update"
has '.items | any(.id == "reset:account" and .undo.terminal == true)' "a reset of the settings"
has '.items | any(.id == "agent:ab12cd34")' "the agent's change"
has '.items | all(.id != "agent:ef567890")' "an agent's change already undone, in the shape the tools write it"
has '.items | all(.id != "agent:11111111")' "the undoing itself, whose Undo would do the change again"
has '.items | all(.id != "agent:22222222")' "an app the agent installed: its way back removes an app, which Undo never does"
has '.items | any(.id == "agent:33333333")' "an app the agent removed can be brought back"
has '.items | all(.id != "trash:vanished.txt.trashinfo")' "an info file whose file is gone has nothing to put back"
has '.items | any(.id == "trash:caf%E9.txt.trashinfo" and (.what | contains("caf")))' "a name that is not text is listed, by an id of ASCII"
has '.items | any(.id == "trash:escape.txt.trashinfo" and .undo == null)' "a Path= out of the home by ../ has no Undo"
has '.items | any(.id == "trash:absolute.txt.trashinfo" and .undo == null)' "a Path= outside the home has no Undo"
has '.items | any(.id == "trash:through-link.txt.trashinfo" and .undo == null)' "a Path= through a link out of the home has no Undo"
has '.items | any(.id == "trash:dotdot.txt.trashinfo" and .undo == null)' "a Path= that climbs out by .. has no Undo"
has '.items | any(.id == "trash:autostart.desktop.trashinfo" and .undo == null)' "a Path= in the autostart folder has no Undo"
has '.items | any(.id == "trash:bashrc.trashinfo" and .undo == null)' "a Path= that is a shell's start file has no Undo"
has '.items | any(.kind == "security" and .undo == null and .security.page == "settings")' "a security line opens Security"
has '[.items[] | tostring | test("duress"; "i")] | any | not' "the report's silent feature is never named"
has '.items | all(.kind != "security" or (.detail | contains("not a setting") | not))' "only the watcher's settings lines"
has '.items | all(.id != "trash:.trashinfo")' "an info file with no name names nothing"
AGENT_STATE=off "${HELPER}" list --json | jq -e '.items | all(.kind != "agent")' >/dev/null || fail "the agent's Undo while the features are off"
TOOLS_STATE=off "${HELPER}" list --json | jq -e '.items | all(.kind != "agent")' >/dev/null || fail "the agent's Undo at level 1, where the tools refuse any change"
"${HELPER}" list --json | python3 -c 'import sys; sys.stdin.buffer.read().decode("utf-8")' || fail "the list is not text"
has '[.items[].time] | . == (sort | reverse)' "the newest first"

# --- Undo adds back, and refuses what was not listed -------------------------------------------------------------------------
undo() { "${HELPER}" undo "$1" --json; }
refused() { [[ "$(undo "$1" | jq -r .ok)" == false ]] || fail "$2"; }
refused "trash:nonsense.trashinfo" "an id it did not list was taken"
refused "gone:2026-10-01_1000:../../etc/passwd" "a path it did not list was taken"
refused "security:1-0" "a security line was undone"
refused "app-new:org.gnome.Maps" "an app installed was removed"
refused "app:org.not.Listed" "an app it did not list was brought back"
refused "agent:11111111" "the record of an undoing was undone"
refused "agent:ef567890" "a change undone already was undone again"
refused "agent:22222222" "an app the agent installed was removed"
for planted in escape.txt absolute.txt autostart.desktop through-link.txt dotdot.txt bashrc; do
    refused "trash:${planted}.trashinfo" "a planted Path= was put back: ${planted}"
done
[[ -z "$(find "${OUTSIDE}" -mindepth 1 -print -quit)" && ! -e "${HOME}/.config/autostart/evil.desktop" && ! -e "${HOME}/.bashrc" ]] || fail "a planted Path= wrote outside the places a file is put back"
[[ -e "${TRASH}/files/escape.txt" ]] || fail "a refused file left the trash"
# A parent that is a file, or a folder that cannot be made, is an answer and not a crash
touch "${HOME}/Documents/blocked"
trash 'blocked.txt' "${HOME}/Documents/blocked/under.txt"
[[ "$(undo "trash:blocked.txt.trashinfo" | jq -r .ok)" == false ]] || fail "a parent that is a file did not give an answer"
[[ -e "${TRASH}/files/blocked.txt" ]] || fail "a file that could not be put back left the trash"

undo "trash:report.odt.trashinfo" | jq -e '.ok == true' >/dev/null || fail "the file did not come back"
[[ "$(cat "${HOME}/Documents/My Report.odt" 2>/dev/null)" == canary ]] || fail "the file is not where it was"
[[ ! -e "${TRASH}/files/report.odt" && ! -e "${TRASH}/info/report.odt.trashinfo" ]] || fail "the trash still holds it"
undo "trash:second.odt.trashinfo" | jq -e '.ok == true' >/dev/null || fail "the second file did not come back"
[[ "$(cat "${HOME}/Documents/Second.odt")" == mine ]] || fail "Undo wrote over what has the name now"
[[ "$(cat "${HOME}/Documents/Second (restored).odt" 2>/dev/null)" == canary ]] || fail "it is not beside what has the name now"

undo "gone:2026-10-01_1000:Documents/Taxes" | jq -e '.ok == true' >/dev/null || fail "a gone file was not asked back"
grep -qxF "restore put-back 2026-10-01_1000 Documents/Taxes --keep-both --json" "${LOG}" || fail "a gone file does not keep both: $(cat "${LOG}")"
undo "trash:caf%E9.txt.trashinfo" | jq -e '.ok == true' >/dev/null || fail "a name that is not text did not come back"
[[ "$(cat "${HOME}/Documents/$(printf 'caf\xe9.txt')" 2>/dev/null)" == canary ]] || fail "it is not back under the bytes it had"
undo "setting:2026-09-28_1000:/org/gnome/shell/extensions/amethystora-ui/dock/edge" | jq -e '.ok == true' >/dev/null || fail "a setting did not come back"
grep -qxF "dconf write /org/gnome/shell/extensions/amethystora-ui/dock/edge 'bottom'" "${LOG}" || fail "a setting's old value: $(cat "${LOG}")"
# Undone, it is gone from the list, and the point that holds the value it undid does not offer that value as a new Undo
"${HELPER}" list --json | jq -e '.items | all(.id | contains("dock/edge") | not)' >/dev/null || fail "an Undo of a setting stayed on the list, offering to do it again"
refused "setting:2026-09-28_1000:/org/gnome/shell/extensions/amethystora-ui/dock/edge" "an Undo of a setting was pressed twice"
refused "setting:2026-10-01_1000:/org/gnome/shell/extensions/amethystora-ui/dock/edge" "the setting was changed back to what it undid"
undo "theme:2026-09-28_1000:Tokyo Night" | jq -e '.ok == true' >/dev/null || fail "the theme did not come back"
grep -qxF "theme set Tokyo Night" "${LOG}" || fail "the theme's name: $(cat "${LOG}")"
"${HELPER}" list --json | jq -e '.items | all(.kind != "setting" or (.id | startswith("theme:") | not))' >/dev/null || fail "an Undo of the theme stayed on the list"
undo "agent:ab12cd34" | jq -e '.ok == true' >/dev/null || fail "the agent's change was not undone"
grep -qxF "agent undo ab12cd34" "${LOG}" || fail "the agent's own undo: $(cat "${LOG}")"
undo "app:org.example.Trashed" | jq -e '.ok == true and (.words | contains("stays in the trash"))' >/dev/null || fail "undo's word for a folder that stays in the trash"
grep -qxF "uninstall undo org.example.Trashed" "${LOG}" || fail "an app does not come back through the uninstall's own undo: $(cat "${LOG}")"
grep -q "flatpak install" "${LOG}" && fail "a plain install brings an app back empty: $(cat "${LOG}")"

# --- With nothing there, it says nothing, and nothing in it deletes -----------------------------------------------------------
rm -rf "${SNAP}" "${TRASH}" "${XDG_STATE_HOME}/amethystora"
cat >"${ROOT}/usr/libexec/amethystora-restore" <<'EOF'
#!/usr/bin/bash
case "$1" in
    points) echo '{"version":1,"points":[],"backup":"none"}' ;;
    status) echo '{"version":1,"snapshots":{"on":false}}' ;;
esac
EOF
"${HELPER}" list --json | jq -e '.snapshots == false and (.items | all(.kind != "file" and (.id | startswith("setting:") | not)))' >/dev/null || fail "with no snapshots or trash"
if grep -nE 'rmtree|shutil|os\.remove|"rm"|forget|prune|subvolume|--delete' "${HELPER}"; then
    fail "something in it deletes"
fi

if [[ "${failed}" != 0 ]]; then
    exit 1
fi
echo "test-changes: the list and every Undo as they should be"

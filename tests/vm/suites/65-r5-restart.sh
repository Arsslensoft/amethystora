#!/usr/bin/bash
# vmlab-vm: r5-restart
# Rounds 5 and 6, group restart (plans/ui-ideas/spec-restart.md, its acceptance criteria), in the test account's
# session, asked of GNOME Shell itself through Eval, which a lab-only extension of the test account opens (never in the
# image):
#   - the three keys and their defaults;
#   - Pick up where you left off: a resume.json from another version, read by the shell that GDM's restart starts,
#     offered in the pill and deleted; Reopen opening Files on its saved workspace; one from the running version
#     dropped; Save over D-Bus keeping app IDs and places only, 0600; the key off exporting nothing; Not now; a lock
#     while an update is staged keeping the apps, and the unlock taking them away again;
#   - Restart while you're away: the time refused out of 02:00 to 05:00, set, tonight's timer made and taken away, a
#     night with the screen unlocked or the computer asleep closing nothing and saying why;
#   - Saving data: the units' conditions, a run a person starts always running, and on a connection made metered the
#     pill's Saving data, the stamp of a wait, a week's wait running anyway;
#   - a new network's zone: the first run recording the saved connections silently, a new one said once with its
#     zone's words, never twice;
#   - ten disables and enables leaving nothing and the D-Bus object back, Control's entries and the setup file.
# A staged update is stood in for by /run/ostree/staged-deployment, made only when there is none, and taken away again.
# vmlab-paths: @ui/extension.js @ui/*workspaces* @ui/*pill* @ui/*privacy* @sf/usr/libexec/amethystora-soft-reboot @sf/usr/libexec/amethystora-night-restart @sf/usr/libexec/amethystora-metered @sf/usr/lib/systemd/*uupd* @sf/usr/lib/systemd/user/amethystora-backup.service @sf/usr/lib/systemd/user/amethystora-pkg-upgrade.service @sf/*just/desktop.just @sf/*just/update.just @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup apps/update/*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@iarsslen.github.io
HERE="$(dirname "$0")"
HOME_DIR="/var/home/${TEST_USER}"
WORKSPACES=org.gnome.shell.extensions.amethystora-ui.workspaces
PILL=org.gnome.shell.extensions.amethystora-ui.pill
PRIVACY=org.gnome.shell.extensions.amethystora-ui.privacy
STATE="${HOME_DIR}/.local/state/amethystora"
RESUME="${STATE}/resume.json"
STAGED=/run/ostree/staged-deployment
FILES=org.gnome.Nautilus.desktop
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c 'import json, sys
from gi.repository import GLib
v = GLib.Variant.parse(None, sys.stdin.read().strip(), None, None)
try:
    x = json.loads(v[1])
except ValueError:
    x = v[1]
print(x if isinstance(x, str) else json.dumps(x))'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
like() { [[ "$1" == $2 ]]; }
is() { [[ "$(ev "$1")" == "$2" ]]; }
tester() { user_bus bash -lc "$*" </dev/null; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
W="${UI}.get('workspaces')"
P="${UI}.get('pill')"
Z="${UI}.get('privacy').zone"
save_gone() { like "$(save)" '*rror*'; }
same_save() { [[ "$(save)" == "(true,)" ]]; }
state_is() { is "Main.extensionManager.lookup('${UUID}').state" "$1"; }
save() { user_bus gdbus call --session --timeout 5 --dest org.gnome.Shell --object-path /org/amethystora/UI/Resume \
    --method org.amethystora.UI.Resume.Save 2>&1; }
files_on() { ev "JSON.stringify(global.display.list_all_windows().filter(w => (w.get_wm_class() ?? '').toLowerCase().includes('nautilus')).map(w => w.get_workspace()?.index() ?? -1))"; }
armed_now() { like "$(tester /usr/libexec/amethystora-night-restart status --json)" '*"armed":1*'; }
# The titles and bodies of the notifications in the list, as JSON
notice_has() { like "$(ev "JSON.stringify(Main.messageTray.getSources().flatMap(s => s.notifications.map(n => [n.title, n.body])))")" "$1"; }
files_on_is() { [[ "$(files_on)" == "$1" ]]; }
close_files() { ev "global.display.list_all_windows().filter(w => (w.get_wm_class() ?? '').toLowerCase().includes('nautilus')).forEach(w => w.delete(global.get_current_time())); 'x'" >/dev/null; wait_for 20 is "$(printf '%s' "global.display.list_all_windows().filter(w => (w.get_wm_class() ?? '').toLowerCase().includes('nautilus')).length")" 0; }
# A resume.json as an earlier version would have left it: Files on the second workspace, in a place of its own
plant_resume() {
    local from="$1"
    install -d -m 0700 -o "${TEST_USER}" -g "${TEST_USER}" "${STATE}"
    cat >"${RESUME}" <<EOF
{"version": 1, "saved": $(date +%s), "from": "${from}", "kind": "restart", "active": 0,
 "workspaces": [{"index": 1, "layout": "classic", "windows": [{"app": "${FILES}", "rank": 0,
   "place": {"m": "Virtual-1", "x": 120, "y": 90, "w": 900, "h": 640, "max": false}}]}]}
EOF
    chown "${TEST_USER}:" "${RESUME}"
    chmod 0600 "${RESUME}"
}
since="$(date '+%Y-%m-%d %H:%M:%S')"

if ! has_session; then
    bash "${HERE}/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
[[ ${enabled} == *vmlab-eval@lab* ]] || user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
gs reset "${WORKSPACES}" resume
gs reset "${PILL}" saving-data
gs reset "${PRIVACY}" zone-notice
rm -f "${STATE}/networks-seen.json" "${STATE}/networks-seen.json.bak"
# What an update's restart would have left, for the shell GDM's restart starts
plant_resume "vmlab-old:0000000"
loginctl unlock-sessions
systemctl restart gdm
ready() { is "typeof ${W}?.resume?.save" function; }
if ! wait_for 240 has_session || ! wait_for 120 ready; then
    not_ok "picking up after a restart did not start in the session"
    finish
fi
wait_for 30 is "Main.layoutManager._startingUp" false
ev "Main.overview.hide(); 'hidden'" >/dev/null
made_staged=""
METERED_CONN=""
cleanup() {
    [[ -n "${made_staged}" ]] && rm -f "${STAGED}"
    if [[ -n "${METERED_CONN}" ]]; then
        nmcli connection modify "${METERED_CONN}" connection.metered unknown >/dev/null 2>&1
        nmcli device reapply "${METERED_DEV}" >/dev/null 2>&1
    fi
    rm -f /var/lib/amethystora/metered/system
    tester "/usr/libexec/amethystora-night-restart set off; rm -f ~/.local/state/amethystora/night-restart.json ~/.local/state/amethystora/metered/*" >/dev/null 2>&1
    rm -f "${RESUME}"
    gs reset "${WORKSPACES}" resume
    gs reset "${PILL}" saving-data
    gs reset "${PRIVACY}" zone-notice
}
trap cleanup EXIT
stage() {
    if [[ ! -e "${STAGED}" && -d /run/ostree ]]; then
        : >"${STAGED}"
        made_staged=1
    fi
}
unstage() {
    [[ -n "${made_staged}" ]] && rm -f "${STAGED}"
    made_staged=""
}

# --- 1: the keys and their defaults ---------------------------------------------------------------------------------
same "1 resume is on" "$(gs get "${WORKSPACES}" resume)" true
same "1 saving-data is on" "$(gs get "${PILL}" saving-data)" true
same "1 zone-notice is on" "$(gs get "${PRIVACY}" zone-notice)" true

# --- 2, 3: a file from another version, read at the shell's start ---------------------------------------------------------
check "3 the file from another version is offered" is "JSON.stringify(${W}.resume.offer?.apps)" "[{\"app\":\"${FILES}\",\"workspace\":1}]"
check "3 the pill offers it" wait_for 20 is "${P}.state().resume" true
check "3 in the pill's words" is "${P}.resume.text" "Reopen your apps"
check "2 the file is gone once read" test ! -e "${RESUME}"
card="$(ev "(() => { const c = ${P}.resume.card({close() {}}); const t = []; const v = a => { if (a.text) t.push(a.text); a.get_children?.().forEach(v); }; v(c); c.destroy(); return JSON.stringify(t); })()")"
check "3 the card says welcome back" like "${card}" '*"Welcome back"*'
check "3 the card counts the apps" like "${card}" '*"The update is in. 1 app was open before the restart."*'
check "3 the card says nothing was kept of them" like "${card}" '*no file, page or tab was kept.*'
close_files
ev "global.workspace_manager.get_workspace_by_index(0).activate(global.get_current_time()); 'x'" >/dev/null
ev "${P}.resume.reopen(); 'x'" >/dev/null
check "3 Reopen opens Files on its own workspace" wait_for 60 files_on_is "[1]"
check "3 the opening ends on the workspace that was in front" wait_for 40 is "global.workspace_manager.get_active_workspace_index()" 0
check "3 the pill lets it go" wait_for 10 is "${P}.state().resume" false
check "3 and it is offered once" is "${W}.resume.offer" null

# --- 5: Save over D-Bus ----------------------------------------------------------------------------------------------------
same "5 Save keeps the open apps" "$(save)" "(true,)"
check "5 in a file of the account's own, 0600" test "$(stat -c '%U %a' "${RESUME}" 2>/dev/null)" == "${TEST_USER} 600"
same "5 Files on its workspace" "$(jq -c '[.kind, (.workspaces[] | [.index, .windows[0].app])]' "${RESUME}" 2>/dev/null)" \
    "[\"restart\",[1,\"${FILES}\"]]"
title="$(ev "global.display.list_all_windows().find(w => (w.get_wm_class() ?? '').toLowerCase().includes('nautilus'))?.get_title() ?? ''")"
if [[ -n "${title}" ]] && grep -qF "${title}" "${RESUME}"; then not_ok "5 no window's title is kept" "${title}"; else ok "5 no window's title is kept"; fi
# --- 4: the same version's file is dropped
check "4 a file from the running version is not offered" is "${W}.resume.readForTest()" false
check "4 ...and is gone" test ! -e "${RESUME}"

# --- 6: the key off ------------------------------------------------------------------------------------------------------
gs set "${WORKSPACES}" resume false
check "6 off, the Save object is gone" wait_for 10 save_gone
check "6 off, nothing is written" test ! -e "${RESUME}"
gs reset "${WORKSPACES}" resume
check "6 on again, Save is back" wait_for 10 same_save
rm -f "${RESUME}"

# --- 7: Not now --------------------------------------------------------------------------------------------------------------
plant_resume "vmlab-old:0000000"
check "7 read again, it is offered" is "${W}.resume.readForTest()" true
check "7 the pill shows it" is "${P}.resumeForTest()" true
ev "${P}.resume.notNow(); 'x'" >/dev/null
check "7 Not now takes it from the pill" is "${P}.state().resume" false
check "7 ...and forgets it" is "${W}.resume.offer" null

# --- 2b: a lock while an update is staged ------------------------------------------------------------------------------
SESSION="$(loginctl show-user "${TEST_USER}" -p Display --value)"
stage
if [[ -n "${made_staged}" ]]; then
    rm -f "${RESUME}"
    loginctl lock-session "${SESSION}"
    check "2b a lock keeps the open apps while an update waits" wait_for 30 test -s "${RESUME}"
    same "2b ...as the lock's" "$(jq -r .kind "${RESUME}" 2>/dev/null)" locked
    sleep 2
    loginctl unlock-session "${SESSION}"
    wait_for 30 ready
    check "2b the unlock takes them away: no restart came" wait_for 20 test ! -e "${RESUME}"
    unstage
    loginctl lock-session "${SESSION}"
    sleep 3
    check "2b with nothing staged, a lock keeps nothing" test ! -e "${RESUME}"
    loginctl unlock-session "${SESSION}"
    wait_for 30 ready
else
    skip "2b a lock while an update is staged: this machine has one staged already"
fi
close_files

# --- 8: Restart while you're away ------------------------------------------------------------------------------------------
if tester "ame update night-restart 06:00" >/dev/null 2>&1; then not_ok "8 06:00 is refused"; else ok "8 06:00 is refused"; fi
tester "ame update night-restart 03:00" >/dev/null 2>&1
same "8 03:00 is chosen" "$(tester /usr/libexec/amethystora-night-restart get)" 03:00
tester "ame update tonight" >/dev/null 2>&1
check "8 tonight's timer is made" wait_for 10 armed_now
check "8 as a timer of the account" like "$(user_bus systemctl --user list-timers --all --no-legend)" '*amethystora-night-restart.timer*'
tester "ame update tonight off" >/dev/null 2>&1
same "8 Not tonight takes it away" "$(tester /usr/libexec/amethystora-night-restart status --json | jq -c .armed)" null
tester "/usr/libexec/amethystora-night-restart run $(date +%s)"
same "8 with nothing staged, a night does nothing" "$(tester /usr/libexec/amethystora-night-restart status --json | jq -c .last)" null
stage
if [[ -n "${made_staged}" ]]; then
    tester "/usr/libexec/amethystora-night-restart run $(date +%s)"
    same "8 the screen unlocked: nothing closes, and why is said" \
        "$(tester /usr/libexec/amethystora-night-restart status --json | jq -c '[.last.result, .last.why]')" '["waited","unlocked"]'
    check "8 the notification says so" wait_for 20 notice_has '*"Not restarted last night"*'
    tester "/usr/libexec/amethystora-night-restart run $(($(date +%s) - 7200))"
    same "8 two hours late: the computer was asleep" "$(tester /usr/libexec/amethystora-night-restart status --json | jq -r .last.why)" late
    user_bus systemctl --user stop 'amethystora-night-restart-notice-*' >/dev/null 2>&1
    unstage
else
    skip "8 a night with an update staged: this machine has one staged already"
fi
tester "ame update night-restart off" >/dev/null 2>&1
same "8 off" "$(tester /usr/libexec/amethystora-night-restart get)" ""

# --- 9: Saving data --------------------------------------------------------------------------------------------------------
check "9 the automatic update waits on a metered connection" like "$(systemctl cat uupd.service 2>/dev/null)" '*ExecCondition=/usr/libexec/amethystora-metered wait system*'
check "9 Update now does not" bash -c "! systemctl cat uupd-manual.service | grep -q amethystora-metered"
check "9 nor does a run a person starts" env -u TRIGGER_UNIT /usr/libexec/amethystora-metered wait system
check "9 the pill shows Saving data when metered" is "${P}.saving.forTest(true)" true
card="$(ev "(() => { const c = ${P}.saving.card({close() {}}); const t = []; const v = a => { if (a.text) t.push(a.text); a.get_children?.().forEach(v); }; v(c); c.destroy(); return JSON.stringify(t); })()")"
check "9 its card lists the system update" like "${card}" '*"The system update and app updates"*'
check "9 ...and Update anyway" like "${card}" '*"Update anyway"*'
check "9 and leaves the pill when not" is "${P}.saving.forTest(false)" false
ev "${P}.saving.forTest(null); 'x'" >/dev/null
METERED_DEV="$(nmcli -g DEVICE,STATE device status 2>/dev/null | awk -F: '$2 == "connected" { print $1; exit }')"
conn="$(nmcli -g GENERAL.CONNECTION device show "${METERED_DEV}" 2>/dev/null)"
if [[ -n "${conn}" ]] && nmcli connection modify "${conn}" connection.metered yes >/dev/null 2>&1; then
    METERED_CONN="${conn}"
    nmcli device reapply "${METERED_DEV}" >/dev/null 2>&1
    if wait_for 20 /usr/libexec/amethystora-metered metered; then
        ok "9 NetworkManager calls the connection metered"
        check "9 the pill says Saving data by itself" wait_for 20 is "${P}.state().saving" true
        rm -f /var/lib/amethystora/metered/system
        if TRIGGER_UNIT=uupd.timer /usr/libexec/amethystora-metered wait system >/dev/null; then not_ok "9 the timer's run waits"; else ok "9 the timer's run waits"; fi
        check "9 ...from when it first waited" test -s /var/lib/amethystora/metered/system
        echo $(($(date +%s) - 8 * 86400)) >/var/lib/amethystora/metered/system
        check "9 after a week it runs anyway" env TRIGGER_UNIT=uupd.timer /usr/libexec/amethystora-metered wait system
        /usr/libexec/amethystora-metered done system
        check "9 a run that worked takes the stamp away" test ! -e /var/lib/amethystora/metered/system
    else
        skip "9 NetworkManager did not take the metered flag on a reapply"
    fi
    nmcli connection modify "${conn}" connection.metered unknown >/dev/null 2>&1
    nmcli device reapply "${METERED_DEV}" >/dev/null 2>&1
    METERED_CONN=""
    check "9 unmetered again, the pill lets it go" wait_for 30 is "${P}.state().saving" false
else
    skip "9 no connection to make metered"
fi

# --- 10: a new network's zone --------------------------------------------------------------------------------------------
check "10 the first run records the saved connections" wait_for 30 test -s "${STATE}/networks-seen.json"
uuid="$(nmcli -g UUID connection show --active 2>/dev/null | head -n1)"
if [[ -n "${uuid}" ]]; then
    check "10 ...the connection in use among them, without a word" jq -e --arg u "${uuid}" '.seen | index($u)' "${STATE}/networks-seen.json"
fi
check "10 a new network is said" is "${Z}.consider('00000000-0000-4000-8000-00000000cafe', 'Café Lumière', 'amethystora')" true
check "10 by its name" wait_for 10 notice_has '*"Café Lumière, a new network","In the default zone. Phone pairing is closed here*'
check "10 with Open Security" like "$(ev "JSON.stringify(Main.messageTray.getSources().flatMap(s => s.notifications.filter(n => n.title.endsWith('a new network')).flatMap(n => n.actions.map(a => a.label))))")" '*"Open Security"*'
check "10 never twice" is "${Z}.consider('00000000-0000-4000-8000-00000000cafe', 'Café Lumière', 'amethystora')" false
check "10 kept by its id, never its name" bash -c "! grep -q Lumi '${STATE}/networks-seen.json'"
check "10 the home zone's words" like "$(ev "${Z}.consider('00000000-0000-4000-8000-0000000000aa', 'Home', 'amethystora-home') ? Main.messageTray.getSources().flatMap(s => s.notifications).at(-1).body : ''")" 'In the home zone.*'

# --- 12: ten disables and enables ----------------------------------------------------------------------------------------
census='(() => { const m = {}; const v = a => { const k = a.style_class || a.constructor.name; m[k] = (m[k] ?? 0) + 1; a.get_children().forEach(v); }; v(Main.uiGroup); m.statusArea = Object.keys(Main.panel.statusArea).length; return m; })()'
off() { ev "Main.extensionManager.disableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 2 && sleep 1; }
on() { ev "Main.extensionManager.enableExtension('${UUID}'); 'x'" >/dev/null; wait_for 20 state_is 1; }
off
check "12 off, no Save object" save_gone
ev "globalThis.vmlabCensus = ${census}; 'x'" >/dev/null
cycles=0
for _ in 1 2 3 4 5 6 7 8 9 10; do
    on || break
    off || break
    cycles=$((cycles + 1))
done
same "12 ten cycles" "${cycles}" 10
same "12 leave nothing" "$(ev "(() => { const a = ${census}, b = globalThis.vmlabCensus;
    return JSON.stringify(Object.keys({...a, ...b}).filter(k => (a[k] ?? 0) !== (b[k] ?? 0)).map(k => k + ' ' + ((a[k] ?? 0) - (b[k] ?? 0))).slice(0, 12)); })()")" "[]"
on
wait_for 60 ready
save_back() { local s; s="$(save)"; [[ "${s}" == "(true,)" || "${s}" == "(false,)" ]]; } # false: no window to keep, the object is there
check "12 on again, Save is back" wait_for 10 save_back
rm -f "${RESUME}"

# --- 13: Control and the setup file --------------------------------------------------------------------------------------
control="$(in_session "/usr/libexec/amethystora-control-list --json" 2>/dev/null)"
same "13 Control's entries" \
    "$(jq -r '[.items[] | select(.id | IN("resume", "saving-data", "zone-notice", "night-restart")) | .id] | sort | join(",")' <<<"${control}")" \
    "night-restart,resume,saving-data,zone-notice"
gs set "${WORKSPACES}" resume false
gs set "${PRIVACY}" zone-notice false
in_session "ame setup save" >/dev/null 2>&1
same "13 the setup file carries them" \
    "$(jq -c '[.settings.workspaces.resume, .settings.privacy["zone-notice"]]' "${HOME_DIR}/.config/amethystora/setup.json")" '[false,false]'
gs reset "${WORKSPACES}" resume
gs reset "${PRIVACY}" zone-notice

# --- and nothing threw ---------------------------------------------------------------------------------------------------
errors="$(journalctl -b "_UID=$(id -u "${TEST_USER}")" _COMM=gnome-shell --since "${since}" --no-pager -o cat 2>/dev/null |
    grep -E "JS ERROR|Amethystora UI: (picking up|the open apps|the networks|the zone notice|NetworkManager|the saved sets)" | head -n 20)"
if [[ -z ${errors} ]]; then ok "nothing threw"; else not_ok "something threw" "${errors}"; fi
finish
